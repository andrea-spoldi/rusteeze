use crate::connection::ConnectionPool;
use log::{error, info};
use redis_protocol::resp2::{
    decode::decode,
    encode::encode,
    types::{OwnedFrame as Frame, Resp2Frame},
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Proxy {
    connection_pool: Arc<ConnectionPool>,
    shutdown_rx: watch::Receiver<()>,
}

impl Proxy {
    pub fn new(connection_pool: Arc<ConnectionPool>, shutdown_rx: watch::Receiver<()>) -> Self {
        Proxy {
            connection_pool,
            shutdown_rx,
        }
    }

    fn monitor_command(&self, frame: &Frame) {
        if let Frame::Array(array) = frame {
            if let Some(Frame::BulkString(cmd)) = array.first() {
                let cmd_str = String::from_utf8_lossy(cmd);
                info!("Processing command: {}", cmd_str);

                match cmd_str.to_uppercase().as_str() {
                    "KEYS" => info!("KEYS command detected - this command can be expensive"),
                    "FLUSHALL" | "FLUSHDB" => info!("Destructive command detected: {}", cmd_str),
                    _ => {}
                }
            }
        }
    }

    pub async fn handle_connection(
        &self,
        mut client_stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut read_buffer = vec![0; 4096];
        let backend_connection = self.connection_pool.get_connection().await?;

        info!(
            "New connection established. Active connections: {}",
            self.connection_pool.active_connections()
        );

        let mut shutdown_rx = self.shutdown_rx.clone();

        loop {
            tokio::select! {
                read_result = client_stream.read(&mut read_buffer) => {
                    match read_result {
                        Ok(0) => {
                            info!("Client disconnected");
                            break;
                        }
                        Ok(n) => {
                            info!("Received {} bytes from client", n);

                            match decode(&read_buffer[..n]) {
                                Ok(Some((frame, _))) => {
                                    self.monitor_command(&frame);

                                    // Create buffer of exact size needed
                                    let mut encoded = vec![0; frame.encode_len(false)];
                                    if let Err(e) = encode(&mut encoded, &frame, false) {
                                        error!("Error encoding command: {}", e);
                                        continue;
                                    }

                                    let mut backend = backend_connection.lock().await;
                                    backend.write_all(&encoded).await?;
                                    backend.flush().await?;
                                    info!("Sent command to backend");

                                    let mut response_buf = vec![0; 4096];
                                    let mut response_data = Vec::new();

                                    loop {
                                        match backend.read(&mut response_buf).await {
                                            Ok(0) => break,
                                            Ok(n) => {
                                                response_data.extend_from_slice(&response_buf[..n]);

                                                match decode(&response_data) {
                                                    Ok(Some((response_frame, _))) => {
                                                        info!("Decoded response: {:?}", response_frame);

                                                        // Create buffer of exact size needed for response
                                                        let mut encoded_response = vec![0; response_frame.encode_len(false)];
                                                        if let Err(e) = encode(&mut encoded_response, &response_frame, false) {
                                                            error!("Error encoding response: {}", e);
                                                            break;
                                                        }

                                                        if let Err(e) = client_stream.write_all(&encoded_response).await {
                                                            error!("Error writing to client: {}", e);
                                                            break;
                                                        }
                                                        client_stream.flush().await?;
                                                        info!("Sent response to client");
                                                        break;
                                                    }
                                                    Ok(None) => continue,
                                                    Err(e) => {
                                                        error!("Error decoding response: {}", e);
                                                        break;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Error reading from backend: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                }
                                Ok(None) => {
                                    info!("Incomplete command received");
                                    continue;
                                }
                                Err(e) => {
                                    error!("Error decoding command: {}", e);
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error reading from client: {}", e);
                            break;
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    info!("Shutdown signal received, closing connection");
                    break;
                }
            }
        }

        info!(
            "Connection closed. Active connections: {}",
            self.connection_pool.active_connections()
        );

        Ok(())
    }
}
