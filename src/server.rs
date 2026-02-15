use std::borrow::Cow;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use crate::command::{Command, CommandResult};
use crate::config::ServerConfig;
use crate::error::ChatError;
use crate::filter::{FilterAction, FilterRegistry};
use crate::message::Message;
use crate::protocol::{parse_frame, Frame};
use crate::room::Room;
use crate::types::{RoomId, UserId};
use crate::user::User;

pub struct Server {
    pub users: Vec<Option<User>>,
    pub rooms: Vec<Room>,
    pub filters: FilterRegistry,
    pub config: ServerConfig,
    next_user_id: u64,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        let mut server = Self {
            users: Vec::new(),
            rooms: Vec::new(),
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

    pub fn create_room(&mut self, name: String) -> RoomId {
        let id = RoomId::new(self.rooms.len() as u64);
        self.rooms.push(Room::new(id, name));
        id
    }

    pub fn find_room_by_name(&self, name: &str) -> Option<RoomId> {
        self.rooms.iter().find(|r| r.name == name).map(|r| r.id)
    }

    pub fn add_user(&mut self, username: String, stream: TcpStream) -> UserId {
        let id = UserId::new(self.next_user_id);
        self.next_user_id += 1;

        let user = User::new(id, username, stream);

        if id.index() < self.users.len() {
            self.users[id.index()] = Some(user);
        } else {
            self.users.push(Some(user));
        }

        id
    }

    pub fn remove_user(&mut self, user_id: UserId) {
        for room in &self.rooms {
            room.remove_member(user_id);
        }

        if let Some(slot) = self.users.get_mut(user_id.index()) {
            *slot = None;
        }
    }

    fn find_user_by_name(&self, name: &str) -> Option<UserId> {
        self.users
            .iter()
            .flatten()
            .find(|u| u.username == name)
            .map(|u| u.id)
    }

    pub fn join_room(&mut self, user_id: UserId, room_id: RoomId) -> Result<(), ChatError> {
        let room = self
            .rooms
            .get(room_id.index())
            .ok_or_else(|| ChatError::UnknownRoom(room_id.to_string()))?;

        room.add_member(user_id);

        let room_name = room.name.clone();
        let members = room.member_ids();

        let username = self
            .users
            .get(user_id.index())
            .and_then(|u| u.as_ref())
            .map(|u| u.username.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let announce = format!("* {username} joined #{room_name}");
        for &member_id in &members {
            if member_id != user_id {
                if let Some(Some(member)) = self.users.get_mut(member_id.index()) {
                    member.send(&announce);
                }
            }
        }

        Ok(())
    }

    fn leave_room(&mut self, user_id: UserId, room_id: RoomId) {
        let Some(room) = self.rooms.get(room_id.index()) else {
            return;
        };

        let room_name = room.name.clone();
        let members = room.member_ids();

        let username = self
            .users
            .get(user_id.index())
            .and_then(|u| u.as_ref())
            .map(|u| u.username.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let announce = format!("* {username} left #{room_name}");
        for &member_id in &members {
            if member_id != user_id {
                if let Some(Some(member)) = self.users.get_mut(member_id.index()) {
                    member.send(&announce);
                }
            }
        }

        room.remove_member(user_id);
    }

    pub fn broadcast(
        &mut self,
        room_id: RoomId,
        sender_id: UserId,
        msg: &Message<'_>,
    ) -> Result<(), ChatError> {
        let room = self
            .rooms
            .get(room_id.index())
            .ok_or_else(|| ChatError::UnknownRoom(room_id.to_string()))?;

        let members = room.member_ids();
        let text = msg.to_string();

        for &member_id in &members {
            if member_id != sender_id {
                if let Some(Some(member)) = self.users.get_mut(member_id.index()) {
                    member.send(&text);
                }
            }
        }

        Ok(())
    }

    pub fn handle_client(&mut self, stream: TcpStream) -> Result<(), ChatError> {
        let peer = stream.peer_addr()?;
        let mut write_stream = stream.try_clone()?;
        let reader = BufReader::new(stream.try_clone()?);

        writeln!(write_stream, "Enter your username:")?;
        let mut lines = reader.lines();

        let username = match lines.next() {
            Some(Ok(name)) if !name.trim().is_empty() => name.trim().to_string(),
            _ => return Ok(()),
        };

        let user_id = self.add_user(username.clone(), stream);
        println!("[{user_id}] {username} connected from {peer}");

        if let Some(ref motd) = self.config.motd {
            writeln!(write_stream, "{motd}")?;
        }

        writeln!(write_stream, "Welcome, {username}! You're in #lobby.")?;
        writeln!(write_stream, "Type a message or /help for commands.")?;

        let lobby = RoomId::new(0);
        self.join_room(user_id, lobby)?;

        let mut current_room = lobby;
        let mut current_name = username;

        let result = self.client_loop(
            user_id,
            &mut current_name,
            &mut current_room,
            &mut write_stream,
            lines,
        );

        self.leave_room(user_id, current_room);
        self.remove_user(user_id);

        result
    }

    fn client_loop(
        &mut self,
        user_id: UserId,
        current_name: &mut String,
        current_room: &mut RoomId,
        write_stream: &mut TcpStream,
        lines: impl Iterator<Item = Result<String, std::io::Error>>,
    ) -> Result<(), ChatError> {
        for line in lines {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Commands start with /
            if trimmed.starts_with('/') {
                match Command::parse(trimmed) {
                    Ok(cmd) => {
                        match cmd.execute(*current_room) {
                            CommandResult::JoinRoom { room } => {
                                let room_id = self
                                    .find_room_by_name(&room)
                                    .unwrap_or_else(|| self.create_room(room.clone()));

                                self.leave_room(user_id, *current_room);
                                self.join_room(user_id, room_id)?;
                                *current_room = room_id;
                                writeln!(write_stream, "* You joined #{room}")?;
                            }
                            CommandResult::ChangeNick { new_name } => {
                                let old_name = current_name.clone();
                                *current_name = new_name.clone();
                                if let Some(Some(user)) =
                                    self.users.get_mut(user_id.index())
                                {
                                    user.username = new_name.clone();
                                }
                                writeln!(
                                    write_stream,
                                    "* You are now {new_name} (was {old_name})"
                                )?;
                            }
                            CommandResult::KickUser { target, room_id } => {
                                if let Some(target_id) = self.find_user_by_name(&target) {
                                    if let Some(Some(target_user)) =
                                        self.users.get_mut(target_id.index())
                                    {
                                        target_user.send(&format!(
                                            "* You were kicked from the room by {current_name}"
                                        ));
                                    }
                                    self.leave_room(target_id, room_id);
                                    writeln!(write_stream, "* Kicked {target}")?;
                                } else {
                                    writeln!(write_stream, "* No user named {target}")?;
                                }
                            }
                            CommandResult::Quit => {
                                writeln!(write_stream, "* Goodbye!")?;
                                return Ok(());
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

            // Protocol frames (MSG:, JOIN:, etc.)
            if let Some(colon) = trimmed.find(':') {
                let prefix = &trimmed[..colon];
                if matches!(prefix, "MSG" | "JOIN" | "NICK" | "QUIT") {
                    match parse_frame(trimmed) {
                        Ok(Frame::Msg { username: _, body }) => {
                            self.send_chat_message(
                                user_id,
                                current_name,
                                &body,
                                *current_room,
                                write_stream,
                            )?;
                        }
                        Ok(Frame::Join { room }) => {
                            let room_name = room.into_owned();
                            let room_id = self
                                .find_room_by_name(&room_name)
                                .unwrap_or_else(|| self.create_room(room_name.clone()));
                            self.leave_room(user_id, *current_room);
                            self.join_room(user_id, room_id)?;
                            *current_room = room_id;
                            writeln!(write_stream, "* You joined #{room_name}")?;
                        }
                        Ok(Frame::Nick { name }) => {
                            let old = current_name.clone();
                            *current_name = name.into_owned();
                            if let Some(Some(user)) = self.users.get_mut(user_id.index()) {
                                user.username = current_name.clone();
                            }
                            writeln!(write_stream, "* You are now {current_name} (was {old})")?;
                        }
                        Ok(Frame::Quit) => {
                            writeln!(write_stream, "* Goodbye!")?;
                            return Ok(());
                        }
                        Err(e) => {
                            writeln!(write_stream, "ERROR: {e}")?;
                        }
                    }
                    continue;
                }
            }

            // Plain text — treat as a chat message.
            self.send_chat_message(user_id, current_name, trimmed, *current_room, write_stream)?;
        }

        Ok(())
    }

    fn send_chat_message(
        &mut self,
        user_id: UserId,
        username: &str,
        body: &str,
        room_id: RoomId,
        write_stream: &mut TcpStream,
    ) -> Result<(), ChatError> {
        // Run filters on the message.
        let final_body = match self.filters.apply(username, body) {
            FilterAction::Allow => body.to_string(),
            FilterAction::Modify(new_body) => new_body,
            FilterAction::Block(reason) => {
                writeln!(write_stream, "* Message blocked: {reason}")?;
                return Ok(());
            }
        };

        let msg = Message::new(
            Cow::Owned(username.to_string()),
            Cow::Owned(final_body),
        );

        println!("[{user_id}] {msg}");

        if let Some(Some(user)) = self.users.get_mut(user_id.index()) {
            user.send(&msg.to_string());
        }

        self.broadcast(room_id, user_id, &msg)?;
        Ok(())
    }
}
