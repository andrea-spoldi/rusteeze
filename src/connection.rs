use crate::proxy::Proxy;
use log::{error, info};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;

pub struct ConnectionManager {
    listen_addr: String,
    backend_addr: String,
    max_connections: usize,
}

impl ConnectionManager {
    pub fn new(listen_addr: String, backend_addr: String, max_connections: usize) -> Self {
        ConnectionManager {
            listen_addr,
            backend_addr,
            max_connections,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        info!("Listening on {}", self.listen_addr);

        let proxy = Proxy::new(self.backend_addr.clone());
        let permits = Arc::new(Semaphore::new(self.max_connections));

        let (graceful_shutdown_tx, mut graceful_shutdown_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("Received Ctrl+C, shutting down...");
            let _ = graceful_shutdown_tx.send(());
        });

        loop {
            tokio::select! {
                Ok((socket, _)) = listener.accept() => {
                    info!("New client connected");
                    let proxy_clone = proxy.clone();
                    let permits_clone = permits.clone(); // Clone the Arc
                    tokio::spawn(async move {
                        let permit = permits_clone.acquire_owned().await.unwrap(); // Use acquire_owned
                        if let Err(e) = proxy_clone.handle_connection(socket).await {
                            error!("Error handling connection: {}", e);
                        }
                        drop(permit);
                    });
                }
                _ = graceful_shutdown_rx.recv() => {
                    info!("Graceful shutdown complete");
                    break;
                }
            }
        }

        Ok(())
    }
}
