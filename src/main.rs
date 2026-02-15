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
#[allow(dead_code)]
mod user;

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use config::ServerConfig;
use error::ChatError;
use filter::FilterAction;
use server::Server;

fn main() -> Result<(), ChatError> {
    let config = ServerConfig::builder()
        .addr("127.0.0.1")
        .port(8080)
        .max_users(100)
        .motd("Welcome to the Rust chat server!")
        .build();

    let mut server = Server::new(config);

    // Register a filter — the closure is Send because it only captures a u64.
    let mut count = 0u64;
    server.filters.add(move |_username: &str, _body: &str| {
        count += 1;
        println!("  [filter] message #{count} processed");
        FilterAction::Allow
    });

    let addr = server.bind_addr();

    // Wrap server in Arc<Mutex> for thread-safe shared access.
    let server = Arc::new(Mutex::new(server));

    let listener = TcpListener::bind(&addr)?;
    println!("Chat server listening on {addr} (multi-threaded)");

    for stream in listener.incoming() {
        let stream = stream?;
        let server = Arc::clone(&server);

        // Spawn a thread per client — each thread gets its own Arc handle.
        std::thread::spawn(move || {
            if let Err(e) = server::handle_client(server, stream) {
                println!("Client error: {e}");
            }
        });
    }

    Ok(())
}
