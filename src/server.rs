use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::command::{Command, CommandResult};
use crate::config::ServerConfig;
use crate::error::ChatError;
use crate::filter::{FilterAction, FilterRegistry};
use crate::room::Room;
use crate::types::{RoomId, UserId};

/// A broadcast event sent through channels.
#[derive(Debug, Clone)]
pub enum Event {
    Message { from: String, body: String },
    System(String),
    #[allow(dead_code)]
    Quit,
}

/// Per-user state: their write stream and their channel receiver.
struct ClientHandle {
    username: String,
    tx: mpsc::Sender<Event>,
}

/// Thread-safe server state behind Arc<Mutex>.
///
/// Arc provides shared ownership across threads.
/// Mutex provides exclusive access — only one thread at a time.
pub struct Server {
    rooms: Vec<Room>,
    clients: Vec<Option<ClientHandle>>,
    pub filters: FilterRegistry,
    pub config: ServerConfig,
    next_user_id: u64,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        let mut server = Self {
            rooms: Vec::new(),
            clients: Vec::new(),
            filters: FilterRegistry::new(),
            config,
            next_user_id: 0,
        };
        server.create_room("lobby".to_string());
        server
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.config.addr, self.config.port)
    }

    fn create_room(&mut self, name: String) -> RoomId {
        let id = RoomId::new(self.rooms.len() as u64);
        self.rooms.push(Room::new(id, name));
        id
    }

    fn find_room_by_name(&self, name: &str) -> Option<RoomId> {
        self.rooms.iter().find(|r| r.name == name).map(|r| r.id)
    }

    fn find_or_create_room(&mut self, name: &str) -> RoomId {
        self.find_room_by_name(name)
            .unwrap_or_else(|| self.create_room(name.to_string()))
    }

    fn register_client(&mut self, username: String, tx: mpsc::Sender<Event>) -> UserId {
        let id = UserId::new(self.next_user_id);
        self.next_user_id += 1;

        let handle = ClientHandle {
            username,
            tx,
        };

        if id.index() < self.clients.len() {
            self.clients[id.index()] = Some(handle);
        } else {
            self.clients.push(Some(handle));
        }

        id
    }

    fn unregister_client(&mut self, user_id: UserId) {
        for room in &self.rooms {
            room.remove_member(user_id);
        }
        if let Some(slot) = self.clients.get_mut(user_id.index()) {
            *slot = None;
        }
    }

    fn join_room(&mut self, user_id: UserId, room_id: RoomId) {
        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        room.add_member(user_id);

        let username = self.client_name(user_id);
        let room_name = room.name.clone();
        let members = room.member_ids();

        let event = Event::System(format!("* {username} joined #{room_name}"));
        self.send_to_members(&members, user_id, &event);
    }

    fn leave_room(&mut self, user_id: UserId, room_id: RoomId) {
        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        let username = self.client_name(user_id);
        let room_name = room.name.clone();
        let members = room.member_ids();

        let event = Event::System(format!("* {username} left #{room_name}"));
        self.send_to_members(&members, user_id, &event);

        room.remove_member(user_id);
    }

    fn broadcast_message(
        &mut self,
        room_id: RoomId,
        sender_id: UserId,
        username: &str,
        body: &str,
    ) {
        // Run filters.
        let final_body = match self.filters.apply(username, body) {
            FilterAction::Allow => body.to_string(),
            FilterAction::Modify(new_body) => new_body,
            FilterAction::Block(reason) => {
                if let Some(Some(client)) = self.clients.get(sender_id.index()) {
                    let _ = client
                        .tx
                        .send(Event::System(format!("* Message blocked: {reason}")));
                }
                return;
            }
        };

        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        let members = room.member_ids();
        let event = Event::Message {
            from: username.to_string(),
            body: final_body,
        };

        // Send to all members including sender (echo).
        for &member_id in &members {
            if let Some(Some(client)) = self.clients.get(member_id.index()) {
                let _ = client.tx.send(event.clone());
            }
        }
    }

    fn send_to_members(&self, members: &[UserId], exclude: UserId, event: &Event) {
        for &member_id in members {
            if member_id != exclude {
                if let Some(Some(client)) = self.clients.get(member_id.index()) {
                    let _ = client.tx.send(event.clone());
                }
            }
        }
    }

    fn client_name(&self, user_id: UserId) -> String {
        self.clients
            .get(user_id.index())
            .and_then(|c| c.as_ref())
            .map(|c| c.username.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn set_client_name(&mut self, user_id: UserId, name: String) {
        if let Some(Some(client)) = self.clients.get_mut(user_id.index()) {
            client.username = name;
        }
    }
}

/// Handle a single client on its own thread.
/// The server state is behind Arc<Mutex> — lock it briefly, do work, release.
pub fn handle_client(
    server: Arc<Mutex<Server>>,
    stream: TcpStream,
) -> Result<(), ChatError> {
    let peer = stream.peer_addr()?;
    let mut write_stream = stream.try_clone()?;
    let reader = BufReader::new(stream.try_clone()?);

    writeln!(write_stream, "Enter your username:")?;
    let mut lines = reader.lines();

    let username = match lines.next() {
        Some(Ok(name)) if !name.trim().is_empty() => name.trim().to_string(),
        _ => return Ok(()),
    };

    // Create a channel for this client. The server sends Events through tx,
    // and our writer thread reads from rx.
    let (tx, rx) = mpsc::channel::<Event>();

    // Register with the server (brief lock).
    let (user_id, motd) = {
        let mut srv = server.lock().unwrap();
        let uid = srv.register_client(username.clone(), tx);
        let motd = srv.config.motd.clone();
        srv.join_room(uid, RoomId::new(0));
        (uid, motd)
    };

    println!("[{user_id}] {username} connected from {peer}");

    if let Some(motd) = motd {
        writeln!(write_stream, "{motd}")?;
    }
    writeln!(write_stream, "Welcome, {username}! You're in #lobby.")?;
    writeln!(write_stream, "Type a message or /help for commands.")?;

    // Writer thread: reads events from the channel, writes to the stream.
    let mut writer = write_stream.try_clone()?;
    let writer_handle = std::thread::spawn(move || {
        for event in rx {
            match event {
                Event::Message { from, body } => {
                    let _ = writeln!(writer, "<{from}> {body}");
                }
                Event::System(text) => {
                    let _ = writeln!(writer, "{text}");
                }
                Event::Quit => break,
            }
        }
    });

    // Reader loop: read lines, dispatch commands/messages.
    let mut current_room = RoomId::new(0);
    let mut current_name = username;

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('/') {
            match Command::parse(trimmed) {
                Ok(cmd) => {
                    let mut srv = server.lock().unwrap();
                    match cmd.execute(current_room) {
                        CommandResult::JoinRoom { room } => {
                            let room_id = srv.find_or_create_room(&room);
                            srv.leave_room(user_id, current_room);
                            srv.join_room(user_id, room_id);
                            current_room = room_id;
                            writeln!(write_stream, "* You joined #{room}")?;
                        }
                        CommandResult::ChangeNick { new_name } => {
                            let old = current_name.clone();
                            current_name = new_name.clone();
                            srv.set_client_name(user_id, new_name.clone());
                            writeln!(write_stream, "* You are now {new_name} (was {old})")?;
                        }
                        CommandResult::KickUser { target, room_id } => {
                            writeln!(write_stream, "* /kick not yet implemented in threaded mode")?;
                            let _ = (target, room_id);
                        }
                        CommandResult::Quit => {
                            writeln!(write_stream, "* Goodbye!")?;
                            break;
                        }
                        CommandResult::Reply(text) => {
                            writeln!(write_stream, "{text}")?;
                        }
                    }
                }
                Err(e) => {
                    writeln!(write_stream, "ERROR: {e}")?;
                }
            }
            continue;
        }

        // Plain text — broadcast as a message.
        {
            let mut srv = server.lock().unwrap();
            srv.broadcast_message(current_room, user_id, &current_name, trimmed);
        }
    }

    // Cleanup.
    println!("[{user_id}] {current_name} disconnected");
    {
        let mut srv = server.lock().unwrap();
        srv.leave_room(user_id, current_room);
        srv.unregister_client(user_id);
    }

    let _ = writer_handle.join();

    Ok(())
}
