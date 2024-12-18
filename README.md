# Rusteze: Redis Proxy Service

Rusteze is an asynchronous Redis proxy server written in Rust. It acts as an intermediary between clients and a backend Redis server, providing support for:

- Bidirectional communication between clients and a Redis backend.
- Handling multiple client connections concurrently using Tokio's async runtime.
- Seamless proxying of Redis commands.

## Features
- **Concurrency:** Handles multiple clients simultaneously.
- **Extensibility:** Modular design for easy future enhancements (e.g., load balancing, connection pooling).
- **Performance:** Built with Rust and Tokio for high performance and reliability.

## Current Architecture

### Main Components
1. **ConnectionManager**:
   - Listens for incoming client connections.
   - Delegates each connection to a `Proxy` for handling.
2. **Proxy**:
   - Handles bi-directional communication between clients and the backend Redis server.
   - Manages Redis connection lifecycles.

### Project Structure
```
.
├── src
│   ├── main.rs          # Entry point of the service
│   ├── connection.rs    # ConnectionManager implementation
│   ├── proxy.rs         # Core proxy logic
├── Cargo.toml           # Rust dependencies and configuration
```

### Execution Flow
1. `ConnectionManager` binds to a listening address and waits for client connections.
2. Each connection is handed off to the `Proxy`, which establishes a connection to the backend Redis server.
3. The `Proxy` facilitates bidirectional data transfer between the client and the backend.

## Prerequisites

- Rust (latest stable version): [Install Rust](https://www.rust-lang.org/tools/install)
- Redis Server: Ensure a Redis server is running and reachable.

## Getting Started

### Clone the Repository
```bash
git clone <repository_url>
cd rusteze
```

### Build the Project
```bash
cargo build
```

### Run the Proxy
```bash
cargo run
```

By default, the proxy listens on `127.0.0.1:6380` and forwards requests to a backend Redis server at `127.0.0.1:6379`.

## Testing the Proxy
1. Start a Redis server:
   ```bash
   redis-server --port 6379
   ```
2. Run the proxy server:
   ```bash
   cargo run
   ```
3. Use `redis-cli` or any Redis client to connect to the proxy:
   ```bash
   redis-cli -p 6380
   ```
4. Send Redis commands and verify the responses.
   ```bash
   SET key value
   GET key
   ```

## Future Enhancements
- **Connection Pooling:** Reuse backend connections for better performance.
- **Load Balancing:** Distribute client requests across multiple Redis instances.
- **Metrics and Monitoring:** Add support for runtime statistics.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
