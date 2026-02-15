use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Chat server listening on 127.0.0.1:8080");

    for stream in listener.incoming() {
        let mut stream = stream?;
        let peer = stream.peer_addr()?;
        println!("New connection from {peer}");

        // For now: single-threaded echo server
        // Post 1 will evolve this with newtypes, From/Into, and proper error handling
        let reader = BufReader::new(stream.try_clone()?);
        for line in reader.lines() {
            let line = line?;
            writeln!(stream, "echo: {line}")?;
        }

        println!("{peer} disconnected");
    }

    Ok(())
}
