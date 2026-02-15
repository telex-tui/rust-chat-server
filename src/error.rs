use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[allow(dead_code)]
    #[error("unknown room: {0}")]
    UnknownRoom(String),

    #[allow(dead_code)]
    #[error("unknown user: {0}")]
    UnknownUser(String),
}
