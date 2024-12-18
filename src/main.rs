mod connection;
mod proxy;

use connection::ConnectionManager;
use log::info;

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting Rusteze...");

    let listen_addr = "127.0.0.1:6379".to_string();
    let backend_addr = "127.0.0.1:6380".to_string(); // Example backend Redis address

    let manager = ConnectionManager::new(listen_addr, backend_addr);
    if let Err(e) = manager.run().await {
        eprintln!("Error running connection manager: {}", e);
    }
}
