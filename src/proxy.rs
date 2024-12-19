use crate::connection::ConnectionPool;
use log::{error, info};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};

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
        client_socket: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get the backend connection from the pool
        let backend_connection = self.connection_pool.get_connection().await?;

        // Split the client socket into reader and writer halves
        let (mut client_reader, mut client_writer) = tokio::io::split(client_socket);

        // Create channels for bi-directional communication
        let (client_to_backend_tx, mut client_to_backend_rx) =
            mpsc::channel::<Result<Vec<u8>, std::io::Error>>(1024);
        let (backend_to_client_tx, mut backend_to_client_rx) =
            mpsc::channel::<Result<Vec<u8>, std::io::Error>>(1024);

        // Task: Forward data from client to backend
        let client_to_backend_task = tokio::spawn({
            let backend_writer = backend_connection.clone();
            async move {
                let mut backend_writer = backend_writer.lock().await;
                loop {
                    match client_to_backend_rx.recv().await {
                        Some(Ok(buf)) => {
                            if let Err(e) = backend_writer.write_all(&buf).await {
                                error!("Error writing to backend: {}", e);
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            error!("Error reading from client: {}", e);
                            break;
                        }
                        None => break, // Sender closed
                    }
                }
                backend_writer.shutdown().await.ok();
            }
        });

        // Task: Read data from backend and send to the client via channel
        let backend_to_client_task = tokio::spawn({
            let backend_reader = backend_connection.clone();
            async move {
                let mut backend_reader = backend_reader.lock().await;
                let mut buf = vec![0u8; 1024];
                loop {
                    match backend_reader.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            if let Err(e) = backend_to_client_tx.send(Ok(buf[..n].to_vec())).await {
                                error!("Error sending to client: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error reading from backend: {}", e);
                            break;
                        }
                    }
                }
            }
        });

        // Task: Read data from client and send to the backend via channel
        let client_to_backend_send = tokio::spawn({
            let mut buf = vec![0u8; 1024];
            async move {
                loop {
                    match client_reader.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            if let Err(e) = client_to_backend_tx.send(Ok(buf[..n].to_vec())).await {
                                error!("Error sending to backend: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error reading from client: {}", e);
                            break;
                        }
                    }
                }
            }
        });

        // Task: Forward data from backend to client
        let backend_to_client_receive = tokio::spawn(async move {
            loop {
                match backend_to_client_rx.recv().await {
                    Some(Ok(buf)) => {
                        if let Err(e) = client_writer.write_all(&buf).await {
                            error!("Error writing to client: {}", e);
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        error!("Error reading from backend: {}", e);
                        break;
                    }
                    None => break, // Sender closed
                }
            }
            client_writer.shutdown().await.ok();
        });

        // Wait for all tasks to complete
        tokio::try_join!(
            client_to_backend_task,
            backend_to_client_task,
            client_to_backend_send,
            backend_to_client_receive
        )?;

        Ok(())
    }
}
