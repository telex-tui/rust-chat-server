use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};

use crate::command::{Command, CommandResult};
use crate::config::ServerConfig;
use crate::error::ChatError;
use crate::room::Room;
use crate::types::{RoomId, UserId};

/// A broadcast event.
#[derive(Debug, Clone)]
pub enum Event {
    Message { from: String, body: String },
    System(String),
}

/// An async message filter.
///
/// In Stage 4, filters were Box<dyn FnMut(...)>. In async, we need
/// filters that can be called from async code. But async closures
/// return anonymous Future types — you can't name them. The solution:
///
///   Pin<Box<dyn Future<Output = FilterAction> + Send>>
///
/// Pin: the future won't move in memory (required because async state
/// machines contain self-references). Box: heap-allocate to erase the
/// concrete type. Send: can be used across .await points in tokio::spawn.
pub trait AsyncFilter: Send + Sync {
    fn apply<'a>(
        &'a self,
        username: &'a str,
        body: &'a str,
    ) -> Pin<Box<dyn Future<Output = FilterAction> + Send + 'a>>;
}

#[derive(Debug)]
pub enum FilterAction {
    Allow,
    #[allow(dead_code)]
    Modify(String),
    #[allow(dead_code)]
    Block(String),
}

/// A simple counting filter — demonstrates implementing AsyncFilter.
pub struct CountingFilter {
    count: Mutex<u64>,
}

impl CountingFilter {
    pub fn new() -> Self {
        Self {
            count: Mutex::new(0),
        }
    }
}

impl AsyncFilter for CountingFilter {
    fn apply<'a>(
        &'a self,
        _username: &'a str,
        _body: &'a str,
    ) -> Pin<Box<dyn Future<Output = FilterAction> + Send + 'a>> {
        Box::pin(async move {
            let mut count = self.count.lock().await;
            *count += 1;
            println!("  [filter] message #{} processed", *count);
            FilterAction::Allow
        })
    }
}

/// Per-client handle: a broadcast sender for delivering events.
struct ClientHandle {
    username: String,
    tx: broadcast::Sender<Event>,
}

pub struct Server {
    rooms: Vec<Room>,
    clients: Vec<Option<ClientHandle>>,
    filters: Vec<Box<dyn AsyncFilter>>,
    pub config: ServerConfig,
    next_user_id: u64,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        let mut server = Self {
            rooms: Vec::new(),
            clients: Vec::new(),
            filters: Vec::new(),
            config,
            next_user_id: 0,
        };
        server.create_room("lobby".to_string());
        server
    }

    pub fn add_filter(&mut self, filter: Box<dyn AsyncFilter>) {
        self.filters.push(filter);
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

    fn register_client(&mut self, username: String) -> (UserId, broadcast::Receiver<Event>) {
        let id = UserId::new(self.next_user_id);
        self.next_user_id += 1;

        let (tx, rx) = broadcast::channel::<Event>(64);
        let handle = ClientHandle { username, tx };

        if id.index() < self.clients.len() {
            self.clients[id.index()] = Some(handle);
        } else {
            self.clients.push(Some(handle));
        }

        (id, rx)
    }

    fn unregister_client(&mut self, user_id: UserId) {
        if let Some(slot) = self.clients.get_mut(user_id.index()) {
            *slot = None;
        }
    }

    async fn join_room(&mut self, user_id: UserId, room_id: RoomId) {
        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        room.add_member(user_id).await;

        let username = self.client_name(user_id);
        let room_name = room.name.clone();
        let members = room.member_ids().await;

        let event = Event::System(format!("* {username} joined #{room_name}"));
        self.send_to_members(&members, user_id, &event);
    }

    async fn leave_room(&mut self, user_id: UserId, room_id: RoomId) {
        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        let username = self.client_name(user_id);
        let room_name = room.name.clone();
        let members = room.member_ids().await;

        let event = Event::System(format!("* {username} left #{room_name}"));
        self.send_to_members(&members, user_id, &event);

        room.remove_member(user_id).await;
    }

    async fn broadcast_message(
        &mut self,
        room_id: RoomId,
        sender_id: UserId,
        username: &str,
        body: &str,
    ) {
        // Run async filters.
        let mut final_body = body.to_string();
        for filter in &self.filters {
            match filter.apply(username, &final_body).await {
                FilterAction::Allow => {}
                FilterAction::Modify(new) => final_body = new,
                FilterAction::Block(reason) => {
                    if let Some(Some(client)) = self.clients.get(sender_id.index()) {
                        let _ = client
                            .tx
                            .send(Event::System(format!("* Message blocked: {reason}")));
                    }
                    return;
                }
            }
        }

        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        let members = room.member_ids().await;
        let event = Event::Message {
            from: username.to_string(),
            body: final_body,
        };

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

/// Handle a single client as a tokio task.
pub async fn handle_client(
    server: Arc<Mutex<Server>>,
    stream: TcpStream,
) -> Result<(), ChatError> {
    let peer = stream.peer_addr()?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    writer
        .write_all(b"Enter your username:\n")
        .await?;

    let mut username = String::new();
    reader.read_line(&mut username).await?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Ok(());
    }

    // Register and join lobby.
    let (user_id, mut rx, motd) = {
        let mut srv = server.lock().await;
        let (uid, rx) = srv.register_client(username.clone());
        let motd = srv.config.motd.clone();
        srv.join_room(uid, RoomId::new(0)).await;
        (uid, rx, motd)
    };

    println!("[{user_id}] {username} connected from {peer}");

    if let Some(motd) = motd {
        writer.write_all(format!("{motd}\n").as_bytes()).await?;
    }
    writer
        .write_all(format!("Welcome, {username}! You're in #lobby.\nType a message or /help for commands.\n").as_bytes())
        .await?;

    // Spawn a writer task — reads from the broadcast receiver.
    let mut write_clone = writer;
    let writer_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let line = match event {
                Event::Message { from, body } => format!("<{from}> {body}\n"),
                Event::System(text) => format!("{text}\n"),
            };
            if write_clone.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    // Reader loop.
    let mut current_room = RoomId::new(0);
    let mut current_name = username;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            break; // client disconnected
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('/') {
            match Command::parse(trimmed) {
                Ok(cmd) => {
                    let mut srv = server.lock().await;
                    match cmd.execute(current_room) {
                        CommandResult::JoinRoom { room } => {
                            let room_id = srv.find_or_create_room(&room);
                            srv.leave_room(user_id, current_room).await;
                            srv.join_room(user_id, room_id).await;
                            current_room = room_id;
                            // Send via channel (writer task handles output).
                            if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                                let _ = client.tx.send(Event::System(
                                    format!("* You joined #{room}"),
                                ));
                            }
                        }
                        CommandResult::ChangeNick { new_name } => {
                            let old = current_name.clone();
                            current_name = new_name.clone();
                            srv.set_client_name(user_id, new_name.clone());
                            if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                                let _ = client.tx.send(Event::System(
                                    format!("* You are now {new_name} (was {old})"),
                                ));
                            }
                        }
                        CommandResult::KickUser { .. } => {
                            if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                                let _ = client.tx.send(Event::System(
                                    "* /kick not yet implemented in async mode".to_string(),
                                ));
                            }
                        }
                        CommandResult::Quit => {
                            if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                                let _ = client.tx.send(Event::System("* Goodbye!".to_string()));
                            }
                            break;
                        }
                        CommandResult::Reply(text) => {
                            if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                                let _ = client.tx.send(Event::System(text));
                            }
                        }
                    }
                }
                Err(e) => {
                    let srv = server.lock().await;
                    if let Some(Some(client)) = srv.clients.get(user_id.index()) {
                        let _ = client.tx.send(Event::System(format!("ERROR: {e}")));
                    }
                }
            }
            continue;
        }

        // Plain text — broadcast.
        let mut srv = server.lock().await;
        srv.broadcast_message(current_room, user_id, &current_name, trimmed)
            .await;
    }

    // Cleanup.
    println!("[{user_id}] {current_name} disconnected");
    {
        let mut srv = server.lock().await;
        srv.leave_room(user_id, current_room).await;
        srv.unregister_client(user_id);
    }

    writer_task.abort();

    Ok(())
}
