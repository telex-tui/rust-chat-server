use std::borrow::Cow;

use crate::error::ChatError;

/// Wire protocol format:
///
///   TYPE:PAYLOAD\n
///
/// Types:
///   MSG:username:body     — a chat message
///   JOIN:room_name        — join a room
///   NICK:new_name         — change username
///   QUIT:                 — disconnect
///
/// Frame is the parsed representation. It borrows from the input buffer
/// when possible (zero-copy) and owns data only when transformation is
/// needed — that's what Cow gives us.
#[derive(Debug, Clone)]
pub enum Frame<'a> {
    Msg {
        username: Cow<'a, str>,
        body: Cow<'a, str>,
    },
    Join {
        room: Cow<'a, str>,
    },
    Nick {
        name: Cow<'a, str>,
    },
    Quit,
}

/// Parse a single line into a Frame.
///
/// The lifetime annotation `'a` ties the Frame to the input buffer.
/// As long as the input lives, our parsed Frame can borrow from it
/// without allocating. This is zero-copy parsing.
pub fn parse_frame<'a>(line: &'a str) -> Result<Frame<'a>, ChatError> {
    let line = line.trim();

    let (cmd, payload) = line
        .split_once(':')
        .ok_or_else(|| ChatError::Parse("missing ':' delimiter".into()))?;

    match cmd {
        "MSG" => {
            let (username, body) = payload
                .split_once(':')
                .ok_or_else(|| ChatError::Parse("MSG requires username:body".into()))?;

            let username = username.trim();
            if username.is_empty() {
                return Err(ChatError::Parse("empty username".into()));
            }

            // Cow::Borrowed — no allocation, just a reference into the input.
            Ok(Frame::Msg {
                username: Cow::Borrowed(username),
                body: Cow::Borrowed(body),
            })
        }
        "JOIN" => {
            let room = payload.trim();
            if room.is_empty() {
                return Err(ChatError::Parse("JOIN requires a room name".into()));
            }
            Ok(Frame::Join {
                room: Cow::Borrowed(room),
            })
        }
        "NICK" => {
            let name = payload.trim();
            if name.is_empty() {
                return Err(ChatError::Parse("NICK requires a name".into()));
            }
            Ok(Frame::Nick {
                name: Cow::Borrowed(name),
            })
        }
        "QUIT" => Ok(Frame::Quit),
        _ => Err(ChatError::Parse(format!("unknown command: {cmd}"))),
    }
}

impl<'a> Frame<'a> {
    /// Convert to an owned Frame with 'static lifetime.
    ///
    /// This is the 'static + Clone escape hatch: when you need to move
    /// a Frame across a boundary that requires 'static (like sending it
    /// to another thread), call .into_owned() to clone borrowed data.
    pub fn into_owned(self) -> Frame<'static> {
        match self {
            Frame::Msg { username, body } => Frame::Msg {
                username: Cow::Owned(username.into_owned()),
                body: Cow::Owned(body.into_owned()),
            },
            Frame::Join { room } => Frame::Join {
                room: Cow::Owned(room.into_owned()),
            },
            Frame::Nick { name } => Frame::Nick {
                name: Cow::Owned(name.into_owned()),
            },
            Frame::Quit => Frame::Quit,
        }
    }
}

/// Custom iterator that parses frames from a buffer of accumulated bytes.
///
/// Yields one Frame per complete line (\n-terminated) in the buffer.
/// Incomplete lines (no trailing \n) are left for the next read.
pub struct FrameIter<'a> {
    buf: &'a str,
    pos: usize,
}

impl<'a> FrameIter<'a> {
    pub fn new(buf: &'a str) -> Self {
        Self { buf, pos: 0 }
    }
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = Result<Frame<'a>, ChatError>;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.buf[self.pos..];
        let newline = remaining.find('\n')?;

        let line = &remaining[..newline];
        self.pos += newline + 1; // skip past the \n

        if line.trim().is_empty() {
            // Skip blank lines, try the next one.
            return self.next();
        }

        Some(parse_frame(line))
    }
}

/// How many bytes were consumed by the iterator.
/// The caller can drain this many bytes from the front of the buffer.
impl FrameIter<'_> {
    pub fn consumed(&self) -> usize {
        self.pos
    }
}
