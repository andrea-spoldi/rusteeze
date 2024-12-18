mod connection;
mod proxy;

use clap::Parser;
use connection::ConnectionManager;
use log::info;
use serde::Deserialize;
use serde_yaml; // Import the crate for parsing YAML files

use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to a configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Local address and port to listen on
    #[arg(short, long)]
    listen: Option<String>,

    /// Backend Redis server address and port
    #[arg(short, long)]
    backend: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    listen: String,
    backend: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();
    info!("Starting Rusteze...");

    // Parse command-line arguments
    let cli = Cli::parse();

    // Load configuration
    let (listen_addr, backend_addr) = if let Some(config_path) = cli.config {
        // Load from configuration file
        let config_content =
            fs::read_to_string(config_path).expect("Failed to read configuration file");
        let config: Config =
            serde_yaml::from_str(&config_content).expect("Invalid configuration file format");
        (config.listen, config.backend)
    } else {
        // Load from inline parameters
        let listen = cli.listen.expect("Missing --listen parameter");
        let backend = cli.backend.expect("Missing --backend parameter");
        (listen, backend)
    };

    info!("Listening on: {}", listen_addr);
    info!("Forwarding to backend: {}", backend_addr);

    // Initialize the connection manager
    let manager = ConnectionManager::new(listen_addr, backend_addr, 10);

    // Run the connection manager
    if let Err(e) = manager.run().await {
        eprintln!("Error running connection manager: {}", e);
    }
}
