mod error;
#[allow(dead_code)]
mod message;
#[allow(dead_code)]
mod protocol;
mod room;
mod server;
mod types;
mod user;

use std::net::TcpListener;

use error::ChatError;
use server::Server;

fn main() -> Result<(), ChatError> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Chat server listening on 127.0.0.1:8080");

    let mut server = Server::new();

    for stream in listener.incoming() {
        let stream = stream?;

        if let Err(e) = server.handle_client(stream) {
            println!("Client error: {e}");
        }
    }

    Ok(())
}
