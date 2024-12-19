use crate::connection::ConnectionPool;
use log::{error, info};
use redis::{self, ErrorKind, RedisError, RedisResult, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct Proxy {
    connection_pool: Arc<ConnectionPool>,
}

impl Proxy {
    pub fn new(connection_pool: Arc<ConnectionPool>) -> Self {
        Proxy { connection_pool }
    }

    pub async fn handle_connection(
        &self,
        mut client_stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut buffer = vec![0; 4096];
        let backend_connection = self.connection_pool.get_connection().await?;

        loop {
            // Read from client
            match client_stream.read(&mut buffer).await {
                Ok(0) => {
                    info!("Client disconnected");
                    break;
                }
                Ok(n) => {
                    // Forward the exact bytes to backend
                    let mut backend = backend_connection.lock().await;
                    backend.write_all(&buffer[..n]).await?;
                    backend.flush().await?;

                    // Read response from backend
                    let mut response = Vec::new();
                    let mut temp_buf = [0u8; 4096];
                    loop {
                        match backend.read(&mut temp_buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                response.extend_from_slice(&temp_buf[..n]);
                                // Try to detect end of Redis response
                                if n < 4096 {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Error reading from backend: {}", e);
                                break;
                            }
                        }
                    }

                    // Forward response to client
                    if !response.is_empty() {
                        client_stream.write_all(&response).await?;
                        client_stream.flush().await?;
                    }
                }
                Err(e) => {
                    error!("Error reading from client: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}
