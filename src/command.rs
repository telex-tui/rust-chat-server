use crate::error::ChatError;
use crate::types::RoomId;

/// Commands are a closed set — we know every variant at compile time.
/// Enum dispatch: match on variants, no vtable, no dynamic dispatch.
/// This is faster than trait objects and exhaustive — the compiler
/// tells you if you miss a case.
#[derive(Debug)]
pub enum Command {
    Join { room: String },
    Nick { name: String },
    Kick { target: String },
    Quit,
    Help,
    List,
}

/// The result of executing a command.
pub enum CommandResult {
    JoinRoom { room: String },
    ChangeNick { new_name: String },
    #[allow(dead_code)]
    KickUser { target: String, room_id: RoomId },
    Quit,
    Reply(String),
}

impl Command {
    /// Parse a command from a "/" prefixed line.
    pub fn parse(input: &str) -> Result<Self, ChatError> {
        let input = input.trim();
        if !input.starts_with('/') {
            return Err(ChatError::Parse("commands start with /".into()));
        }

        let input = &input[1..]; // strip the /
        let (cmd, args) = input
            .split_once(' ')
            .map(|(c, a)| (c, a.trim()))
            .unwrap_or((input, ""));

        match cmd {
            "join" => {
                if args.is_empty() {
                    return Err(ChatError::Parse("/join requires a room name".into()));
                }
                Ok(Command::Join {
                    room: args.to_string(),
                })
            }
            "nick" => {
                if args.is_empty() {
                    return Err(ChatError::Parse("/nick requires a name".into()));
                }
                Ok(Command::Nick {
                    name: args.to_string(),
                })
            }
            "kick" => {
                if args.is_empty() {
                    return Err(ChatError::Parse("/kick requires a username".into()));
                }
                Ok(Command::Kick {
                    target: args.to_string(),
                })
            }
            "quit" => Ok(Command::Quit),
            "help" => Ok(Command::Help),
            "list" => Ok(Command::List),
            _ => Err(ChatError::Parse(format!("unknown command: /{cmd}"))),
        }
    }

    /// Execute the command, returning a result that the server acts on.
    /// Enum dispatch: every variant is handled in one match.
    pub fn execute(self, current_room: RoomId) -> CommandResult {
        match self {
            Command::Join { room } => CommandResult::JoinRoom { room },
            Command::Nick { name } => CommandResult::ChangeNick { new_name: name },
            Command::Kick { target } => CommandResult::KickUser {
                target,
                room_id: current_room,
            },
            Command::Quit => CommandResult::Quit,
            Command::Help => CommandResult::Reply(
                "Commands: /join <room>, /nick <name>, /kick <user>, /list, /quit, /help"
                    .to_string(),
            ),
            Command::List => CommandResult::Reply("(room listing not yet implemented)".to_string()),
        }
    }
}
