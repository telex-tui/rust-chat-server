use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use crate::error::ChatError;
use crate::message::Message;
use crate::room::Room;
use crate::types::{RoomId, UserId};
use crate::user::User;

/// Index-based design: users and rooms live in Vecs, referenced by their
/// newtype IDs. This avoids self-referential lifetimes and plays well
/// with Rust's borrow checker — you can read `self.users` and write
/// `self.rooms` simultaneously because they're separate fields (split borrows).
pub struct Server {
    pub users: Vec<Option<User>>,
    pub rooms: Vec<Room>,
    next_user_id: u64,
}

impl Server {
    pub fn new() -> Self {
        let mut server = Self {
            users: Vec::new(),
            rooms: Vec::new(),
            next_user_id: 0,
        };

        // Create a default "lobby" room.
        server.create_room("lobby".to_string());
        server
    }

    pub fn create_room(&mut self, name: String) -> RoomId {
        let id = RoomId::new(self.rooms.len() as u64);
        self.rooms.push(Room::new(id, name));
        id
    }

    pub fn add_user(&mut self, username: String, stream: TcpStream) -> UserId {
        let id = UserId::new(self.next_user_id);
        self.next_user_id += 1;

        let user = User::new(id, username, stream);

        // Index-based: the user's slot in the Vec matches their ID.
        if id.index() < self.users.len() {
            self.users[id.index()] = Some(user);
        } else {
            self.users.push(Some(user));
        }

        id
    }

    /// Remove a user from the server. Setting their slot to None drops
    /// the User value, which runs User's Drop impl — logging the
    /// disconnect and closing the TCP stream automatically.
    pub fn remove_user(&mut self, user_id: UserId) {
        // Remove from all rooms first.
        for room in &self.rooms {
            room.remove_member(user_id);
        }

        // Drop the user — RAII cleanup happens here.
        if let Some(slot) = self.users.get_mut(user_id.index()) {
            *slot = None;
        }
    }

    pub fn join_room(&mut self, user_id: UserId, room_id: RoomId) -> Result<(), ChatError> {
        let room = self
            .rooms
            .get(room_id.index())
            .ok_or_else(|| ChatError::UnknownRoom(room_id.to_string()))?;

        room.add_member(user_id);

        // Announce to the room — split borrow: we read room data above
        // (cloning what we need), then access self.users below.
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

    /// Announce that a user is leaving, then remove them from the room.
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

    /// Broadcast a message to all members of a room except the sender.
    pub fn broadcast(
        &mut self,
        room_id: RoomId,
        sender_id: UserId,
        msg: &Message,
    ) -> Result<(), ChatError> {
        let room = self
            .rooms
            .get(room_id.index())
            .ok_or_else(|| ChatError::UnknownRoom(room_id.to_string()))?;

        // Read the member list (borrows RefCell briefly), then release.
        let members = room.member_ids();
        let text = msg.to_string();

        // Now mutably access users — split borrow: rooms is not borrowed
        // here, only users.
        for &member_id in &members {
            if member_id != sender_id {
                if let Some(Some(member)) = self.users.get_mut(member_id.index()) {
                    member.send(&text);
                }
            }
        }

        Ok(())
    }

    /// Handle a single client connection. Returns when the client disconnects.
    pub fn handle_client(&mut self, stream: TcpStream) -> Result<(), ChatError> {
        let peer = stream.peer_addr()?;
        let mut write_stream = stream.try_clone()?;
        let reader = BufReader::new(stream.try_clone()?);

        // Ask for username.
        writeln!(write_stream, "Enter your username:")?;
        let mut lines = reader.lines();

        let username = match lines.next() {
            Some(Ok(name)) if !name.trim().is_empty() => name.trim().to_string(),
            _ => return Ok(()),
        };

        // Register the user.
        let user_id = self.add_user(username.clone(), stream);
        println!("[{user_id}] {username} connected from {peer}");

        writeln!(write_stream, "Welcome, {username}! You're in #lobby.")?;
        writeln!(write_stream, "Format: just type a message (broadcasts to #lobby)")?;

        // Join lobby — room 0.
        let lobby = RoomId::new(0);
        self.join_room(user_id, lobby)?;

        // Read messages until disconnect.
        let result = self.client_loop(user_id, &username, lobby, lines);

        // Cleanup — announce departure, then remove.
        // remove_user drops the User, triggering RAII cleanup.
        self.leave_room(user_id, lobby);
        self.remove_user(user_id);

        result
    }

    fn client_loop(
        &mut self,
        user_id: UserId,
        username: &str,
        room_id: RoomId,
        lines: impl Iterator<Item = Result<String, std::io::Error>>,
    ) -> Result<(), ChatError> {
        for line in lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let msg = Message {
                username: username.to_string(),
                body: line,
            };

            println!("[{user_id}] {msg}");

            // Echo to sender.
            if let Some(Some(user)) = self.users.get_mut(user_id.index()) {
                user.send(&msg.to_string());
            }

            // Broadcast to room.
            self.broadcast(room_id, user_id, &msg)?;
        }

        Ok(())
    }
}
