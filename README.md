# Multi-Port TCP Server with NATS Integration

This project implements a multi-port TCP server in Rust, featuring dynamic port management and NATS integration for real-time connection tracking.

## Features

- Listens on multiple ports (8081, 8082, 8083)
- Dynamic port activation/deactivation
- Connection limiting with a semaphore
- Random response delays (1-25 seconds)
- Real-time connection tracking via NATS

## Prerequisites

- Rust (edition 2021)
- NATS server running on localhost:4222

## Dependencies

- tokio (1.37.0)
- async-nats (0.35.1)
- futures (0.3.30)
- rand (0.8.5)

## Installation

1. Clone the repository
2. Run `cargo build` to compile the project

## Usage

1. Ensure a NATS server is running on localhost:4222
2. Run the server with `cargo run`

The server will start listening on ports 8081, 8082, and 8083. It will periodically activate and deactivate these ports.

## How it works

1. The server listens on three ports simultaneously.
2. A background task manages port activation/deactivation.
3. Incoming connections are handled asynchronously.
4. Each connection experiences a random delay before echoing back the received data.
5. Active connection counts are published to NATS in real-time.

## Configuration

You can modify the following constants in `src/main.rs`:

- `HOST`: The IP address to bind to (default: "127.0.0.1")
- `PORTS`: The ports to listen on (default: [8081, 8082, 8083])

## License

[MIT License](LICENSE)
