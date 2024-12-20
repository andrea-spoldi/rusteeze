mod connection;
mod proxy;

use clap::Parser;
use connection::ConnectionManager;
use log::info;
use serde::Deserialize;
use serde_yaml;
use std::fs;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config: Option<String>,

    #[arg(short, long)]
    listen: Option<String>,

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

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    // Set up shutdown signal handler
    let shutdown_handler = tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Shutdown signal received, initiating graceful shutdown...");
                let _ = shutdown_tx.send(());
            }
            Err(err) => {
                eprintln!("Error setting up ctrl-c handler: {}", err);
            }
        }
    });

    // Parse command-line arguments
    let cli = Cli::parse();

    // Load configuration
    let (listen_addr, backend_addr) = if let Some(config_path) = cli.config {
        let config_content =
            fs::read_to_string(config_path).expect("Failed to read configuration file");
        let config: Config =
            serde_yaml::from_str(&config_content).expect("Invalid configuration file format");
        (config.listen, config.backend)
    } else {
        let listen = cli.listen.expect("Missing --listen parameter");
        let backend = cli.backend.expect("Missing --backend parameter");
        (listen, backend)
    };

    info!("Listening on: {}", listen_addr);
    info!("Forwarding to backend: {}", backend_addr);

    // Initialize the connection manager with shutdown receiver
    let manager = ConnectionManager::new(listen_addr, backend_addr, 10, shutdown_rx.clone());

    // Run the connection manager
    let manager_handle = tokio::spawn(async move {
        if let Err(e) = manager.run().await {
            eprintln!("Error running connection manager: {}", e);
        }
    });

    // Wait for either the manager to finish or shutdown signal
    tokio::select! {
        _ = manager_handle => {
            info!("Manager finished normally");
        }
        _ = shutdown_handler => {
            info!("Shutdown handler finished");
        }
    }

    info!("Rusteze shutdown complete");
}
