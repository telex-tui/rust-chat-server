use std::fmt;
use std::str::FromStr;

use crate::error::ChatError;

/// A chat message with a username and body.
///
/// Wire format: `username:message body here\n`
/// Display format: `<username> message body here`
#[derive(Debug, Clone)]
pub struct Message {
    pub username: String,
    pub body: String,
}

impl FromStr for Message {
    type Err = ChatError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let (username, body) = line
            .split_once(':')
            .ok_or_else(|| ChatError::Parse("missing ':' delimiter".into()))?;

        let username = username.trim();
        if username.is_empty() {
            return Err(ChatError::Parse("empty username".into()));
        }

        Ok(Message {
            username: username.to_string(),
            body: body.to_string(),
        })
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}> {}", self.username, self.body)
    }
}

/// Convert a Message into a String for sending over the wire.
/// Uses the Display implementation — demonstrating From/Into.
impl From<Message> for String {
    fn from(msg: Message) -> Self {
        msg.to_string()
    }
}
