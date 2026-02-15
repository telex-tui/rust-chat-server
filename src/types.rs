use std::fmt;

/// A unique identifier for a connected user.
///
/// Wrapping `u64` in a newtype prevents accidentally passing a raw
/// integer where a user ID is expected — the compiler catches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl UserId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw index for Vec-based lookup.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "user#{}", self.0)
    }
}

/// A unique identifier for a chat room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(u64);

impl RoomId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw index for Vec-based lookup.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "room#{}", self.0)
    }
}
