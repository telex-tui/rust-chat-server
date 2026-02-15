use std::fmt;

/// A unique identifier for a connected user.
///
/// Wrapping `u64` in a newtype prevents accidentally passing a raw
/// integer where a user ID is expected — the compiler catches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl UserId {
    pub fn next(counter: &mut u64) -> Self {
        let id = *counter;
        *counter += 1;
        Self(id)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "user#{}", self.0)
    }
}

/// A unique identifier for a chat room (used from Stage 2 onward).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(u64);

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "room#{}", self.0)
    }
}
