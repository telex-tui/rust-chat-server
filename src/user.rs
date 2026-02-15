use std::io::Write;
use std::net::TcpStream;

use crate::types::UserId;

/// A connected user. Holds the TCP stream for sending messages.
pub struct User {
    pub id: UserId,
    pub username: String,
    pub stream: TcpStream,
}

impl User {
    pub fn new(id: UserId, username: String, stream: TcpStream) -> Self {
        Self {
            id,
            username,
            stream,
        }
    }

    pub fn send(&mut self, text: &str) {
        // Best-effort: if a write fails, the client is gone.
        let _ = writeln!(self.stream, "{text}");
    }
}

/// RAII: when a User is dropped, the TCP stream is closed automatically
/// (TcpStream implements Drop to close the socket). We add logging so
/// cleanup is visible. No manual close() calls needed — if the User
/// value goes away for any reason, resources are freed.
impl Drop for User {
    fn drop(&mut self) {
        println!("[{}] {} dropped — connection closed", self.id, self.username);
    }
}
