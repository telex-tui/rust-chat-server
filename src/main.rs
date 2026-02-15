mod command;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod connection;
mod error;
#[allow(dead_code)]
mod filter;
#[allow(dead_code)]
mod message;
#[allow(dead_code)]
mod protocol;
mod room;
mod server;
mod types;
mod user;

use std::net::TcpListener;

use config::ServerConfig;
use error::ChatError;
use filter::FilterAction;
use server::Server;

fn main() -> Result<(), ChatError> {
    // Builder pattern: configure the server with chained methods.
    let config = ServerConfig::builder()
        .addr("127.0.0.1")
        .port(8080)
        .max_users(100)
        .motd("Welcome to the Rust chat server!")
        .build();

    let mut server = Server::new(config);

    // Register a message filter using a closure.
    // FnMut: this closure captures `count` and mutates it on each call.
    let mut count = 0u64;
    server.filters.add(move |_username: &str, _body: &str| {
        count += 1;
        println!("  [filter] message #{count} processed");
        FilterAction::Allow
    });

    let addr = server.bind_addr();
    let listener = TcpListener::bind(&addr)?;
    println!("Chat server listening on {addr}");

    for stream in listener.incoming() {
        let stream = stream?;

        if let Err(e) = server.handle_client(stream) {
            println!("Client error: {e}");
        }
    }

    Ok(())
}
