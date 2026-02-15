use std::io::{BufRead, BufReader, Write};
use std::marker::PhantomData;
use std::net::TcpStream;

use crate::error::ChatError;
use crate::types::{RoomId, UserId};

/// Typestate: encode connection lifecycle as types.
///
/// Connection<Unauthenticated> → Connection<Authenticated> → Connection<InRoom>
///
/// Each state only exposes the methods that make sense. You can't
/// send a message from an unauthenticated connection — it won't compile.

/// Marker type: connection has been accepted but user hasn't identified.
pub struct Unauthenticated;

/// Marker type: user has provided a username.
pub struct Authenticated;

/// Marker type: user has joined a room and can chat.
pub struct InRoom;

/// A connection in a particular state. PhantomData<S> makes the state
/// part of the type without using any memory.
pub struct Connection<S> {
    pub stream: TcpStream,
    pub reader: BufReader<TcpStream>,
    pub user_id: Option<UserId>,
    pub username: Option<String>,
    pub room_id: Option<RoomId>,
    _state: PhantomData<S>,
}

impl Connection<Unauthenticated> {
    /// Create a new unauthenticated connection.
    pub fn new(stream: TcpStream) -> Result<Self, ChatError> {
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            stream,
            reader,
            user_id: None,
            username: None,
            room_id: None,
            _state: PhantomData,
        })
    }

    /// Authenticate: ask for a username, transition to Authenticated.
    /// This method consumes self — you can't use the Unauthenticated
    /// connection after calling it.
    pub fn authenticate(mut self) -> Result<Connection<Authenticated>, ChatError> {
        writeln!(self.stream, "Enter your username:")?;

        let mut name = String::new();
        self.reader.read_line(&mut name)?;
        let name = name.trim().to_string();

        if name.is_empty() {
            return Err(ChatError::Parse("empty username".into()));
        }

        writeln!(self.stream, "Welcome, {name}!")?;

        Ok(Connection {
            stream: self.stream,
            reader: self.reader,
            user_id: None,
            username: Some(name),
            room_id: None,
            _state: PhantomData,
        })
    }
}

impl Connection<Authenticated> {
    /// Join a room, transitioning to InRoom.
    pub fn join_room(
        mut self,
        user_id: UserId,
        room_id: RoomId,
    ) -> Result<Connection<InRoom>, ChatError> {
        writeln!(self.stream, "You're in the room. Type to chat.")?;

        Ok(Connection {
            stream: self.stream,
            reader: self.reader,
            user_id: Some(user_id),
            username: self.username,
            room_id: Some(room_id),
            _state: PhantomData,
        })
    }
}

impl Connection<InRoom> {
    /// Read the next line from the client. Only available in InRoom state.
    pub fn read_line(&mut self) -> Result<Option<String>, ChatError> {
        let mut line = String::new();
        let bytes = self.reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None); // client disconnected
        }
        Ok(Some(line))
    }

    /// Send a message to this client. Only available in InRoom state.
    pub fn send(&mut self, text: &str) {
        let _ = writeln!(self.stream, "{text}");
    }
}
