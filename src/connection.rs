use crate::proxy::Proxy;
use log::{error, info};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::{watch, Mutex, Semaphore};

/// Manages a pool of backend Redis connections
pub struct ConnectionPool {
    backend_addr: String,
    pool: Arc<Mutex<VecDeque<TcpStream>>>,
    semaphore: Arc<Semaphore>,
    active_connections: Arc<AtomicUsize>,
}

impl ConnectionPool {
    pub fn new(backend_addr: String, max_connections: usize) -> Self {
        ConnectionPool {
            backend_addr,
            pool: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_connections)),
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the current number of active connections
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get a shared backend connection or create a new one if necessary
    pub async fn get_connection(
        &self,
    ) -> Result<Arc<Mutex<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await.unwrap();
        let mut pool_guard = self.pool.lock().await;

        self.active_connections.fetch_add(1, Ordering::SeqCst);
        info!("Active connections: {}", self.active_connections());

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
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
        info!(
            "Connection returned. Active connections: {}",
            self.active_connections()
        );
    }
}

/// Manages client connections and integrates with the Proxy
pub struct ConnectionManager {
    listen_addr: String,
    pool: Arc<ConnectionPool>,
    shutdown_rx: watch::Receiver<()>,
}

impl ConnectionManager {
    pub fn new(
        listen_addr: String,
        backend_addr: String,
        max_connections: usize,
        shutdown_rx: watch::Receiver<()>,
    ) -> Self {
        let pool = Arc::new(ConnectionPool::new(backend_addr, max_connections));
        ConnectionManager {
            listen_addr,
            pool,
            shutdown_rx,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        info!("Listening on {}", self.listen_addr);

        let proxy = Proxy::new(self.pool.clone(), self.shutdown_rx.clone());
        let mut shutdown_rx = self.shutdown_rx.clone(); // Clone it here

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((socket, addr)) => {
                            info!("New client connected from {}", addr);
                            let proxy_clone = proxy.clone();
                            tokio::spawn(async move {
                                if let Err(e) = proxy_clone.handle_connection(socket).await {
                                    error!("Error handling connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error accepting connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("Shutdown signal received, stopping connection manager");
                    break;
                }
            }
        }

        // Wait for active connections to finish
        while self.pool.active_connections() > 0 {
            info!(
                "Waiting for {} active connections to close...",
                self.pool.active_connections()
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        info!("All connections closed, shutdown complete");
        Ok(())
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        info!(
            "Connection pool shutting down. Final active connections: {}",
            self.active_connections()
        );
    }
}
