# Building a Chat Server in Rust

Companion repository for the "Building a Chat Server in Rust" blog series — a peer series to "Rust Patterns That Matter."

## Series Posts & Branches

Each blog post corresponds to a branch. Branches form a chain — each builds on the previous one. Check out any branch to see the code at that stage, or diff between branches to see what each post adds.

| Branch | Blog Post | Description |
|--------|-----------|-------------|
| `01-hello-tcp` | #1: Hello, TCP | Echo server, newtypes, error handling |
| `02-rooms-users` | #2: Rooms and Users | Room/user state, broadcasting |
| `03-parsing` | #3: Parsing and Performance | Wire protocol, zero-copy parsing |
| `04-commands` | #4: Commands and Plugins | Command system, plugins, builder, typestate |
| `05-threaded` | #5: Going Multi-threaded | Multi-threaded with Arc/Mutex and channels |
| `06-async` | #6: Going Async | Async with tokio |

`main` is the final state (post 6).

## Quick Start

```bash
# See the code at any stage
git checkout 01-hello-tcp

# See what a post adds
git diff 01-hello-tcp..02-rooms-users

# Run the server (at any branch)
cargo run
```

## Companion Series

This code accompanies two peer blog series:
- **"Building a Chat Server in Rust"** (6 posts) — project-focused, builds this server
- **"Rust Patterns That Matter"** (22 posts) — pattern-focused, each post isolates one concept

A reader can enter from either series and cross over at any time.
