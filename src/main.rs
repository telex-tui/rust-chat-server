mod error;
mod message;
mod types;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use error::ChatError;
use message::Message;
use types::UserId;

fn main() -> Result<(), ChatError> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Chat server listening on 127.0.0.1:8080");

    let mut next_user_id = 0u64;

    for stream in listener.incoming() {
        let mut stream = stream?;
        let peer = stream.peer_addr()?;
        let user_id = UserId::next(&mut next_user_id);
        println!("[{user_id}] connected from {peer}");

        if let Err(e) = handle_client(&mut stream, user_id) {
            println!("[{user_id}] error: {e}");
        }

        println!("[{user_id}] disconnected");
    }

    Ok(())
}

fn handle_client(
    stream: &mut std::net::TcpStream,
    user_id: UserId,
) -> Result<(), ChatError> {
    writeln!(stream, "Welcome, {user_id}! Format: username:message")?;

    let reader = BufReader::new(stream.try_clone()?);
    for line in reader.lines() {
        let line = line?;
        match line.parse::<Message>() {
            Ok(msg) => {
                println!("[{user_id}] {msg}");
                writeln!(stream, "{msg}")?;
            }
            Err(e) => {
                writeln!(stream, "ERROR: {e}")?;
            }
        }
    }

    Ok(())
}
