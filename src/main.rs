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

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use config::ServerConfig;
use error::ChatError;
use server::{CountingFilter, Server};

#[tokio::main]
async fn main() -> Result<(), ChatError> {
    let config = ServerConfig::builder()
        .addr("127.0.0.1")
        .port(8080)
        .max_users(100)
        .motd("Welcome to the Rust chat server!")
        .build();

    let mut server = Server::new(config);

    // Async filter — the trait returns Pin<Box<dyn Future + Send>>.
    server.add_filter(Box::new(CountingFilter::new()));

    let addr = server.bind_addr();
    let server = Arc::new(Mutex::new(server));

    let listener = TcpListener::bind(&addr).await?;
    println!("Chat server listening on {addr} (async)");

    loop {
        let (stream, _) = listener.accept().await?;
        let server = Arc::clone(&server);

        // tokio::spawn requires the future to be Send.
        // Our handle_client is Send because all data held across
        // .await points is Send.
        tokio::spawn(async move {
            if let Err(e) = server::handle_client(server, stream).await {
                println!("Client error: {e}");
            }
        });
    }
}
