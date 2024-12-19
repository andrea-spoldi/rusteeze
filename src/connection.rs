use crate::proxy::Proxy;
use log::{error, info};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, Semaphore};

/// Manages a pool of backend Redis connections
pub struct ConnectionPool {
    backend_addr: String,
    pool: Arc<Mutex<VecDeque<TcpStream>>>, // Thread-safe queue of connections
    semaphore: Arc<Semaphore>,             // Limit the number of backend connections
}

impl ConnectionPool {
    pub fn new(backend_addr: String, max_connections: usize) -> Self {
        ConnectionPool {
            backend_addr,
            pool: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_connections)),
        }
    }

    /// Get a shared backend connection or create a new one if necessary
    pub async fn get_connection(
        &self,
    ) -> Result<Arc<Mutex<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await.unwrap();
        let mut pool_guard = self.pool.lock().await;

        if let Some(connection) = pool_guard.pop_front() {
            return Ok(Arc::new(Mutex::new(connection)));
        }

        let connection = TcpStream::connect(&self.backend_addr).await?;
        Ok(Arc::new(Mutex::new(connection)))
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, connection: TcpStream) {
        let mut pool_guard = self.pool.lock().await;
        pool_guard.push_back(connection);
    }
}

/// Manages client connections and integrates with the Proxy
pub struct ConnectionManager {
    listen_addr: String,
    pool: Arc<ConnectionPool>,
}

impl ConnectionManager {
    pub fn new(listen_addr: String, backend_addr: String, max_connections: usize) -> Self {
        let pool = Arc::new(ConnectionPool::new(backend_addr, max_connections));
        ConnectionManager { listen_addr, pool }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        info!("Listening on {}", self.listen_addr);

        let proxy = Proxy::new(self.pool.clone());
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
                    tokio::spawn(async move {
                        if let Err(e) = proxy_clone.handle_connection(socket).await {
                            error!("Error handling connection: {}", e);
                        }
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
