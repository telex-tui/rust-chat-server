use std::borrow::Cow;
use std::fmt;

use crate::error::ChatError;

/// A chat message with a username and body.
///
/// Uses Cow<str> so it can borrow from an input buffer (zero-copy)
/// or own its data when needed.
#[derive(Debug, Clone)]
pub struct Message<'a> {
    pub username: Cow<'a, str>,
    pub body: Cow<'a, str>,
}

impl<'a> Message<'a> {
    pub fn new(username: Cow<'a, str>, body: Cow<'a, str>) -> Self {
        Self { username, body }
    }

    /// Parse from the old "username:body" format (backwards compat).
    pub fn parse(line: &'a str) -> Result<Self, ChatError> {
        let (username, body) = line
            .split_once(':')
            .ok_or_else(|| ChatError::Parse("missing ':' delimiter".into()))?;

        let username = username.trim();
        if username.is_empty() {
            return Err(ChatError::Parse("empty username".into()));
        }

        Ok(Message {
            username: Cow::Borrowed(username),
            body: Cow::Borrowed(body),
        })
    }

    /// Convert to an owned Message with 'static lifetime.
    /// The 'static + Clone escape hatch for crossing scope boundaries.
    pub fn into_owned(self) -> Message<'static> {
        Message {
            username: Cow::Owned(self.username.into_owned()),
            body: Cow::Owned(self.body.into_owned()),
        }
    }
}

impl fmt::Display for Message<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}> {}", self.username, self.body)
    }
}
