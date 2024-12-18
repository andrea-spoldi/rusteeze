use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct Proxy {
    backend_addr: String,
}

impl Proxy {
    pub fn new(backend_addr: String) -> Self {
        Proxy { backend_addr }
    }

    pub async fn handle_connection(
        &self,
        client_socket: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Connect to the backend Redis server
        let backend_socket = TcpStream::connect(&self.backend_addr).await?;
        info!("Connected to backend Redis at {}", self.backend_addr);

        // Split both sockets into read and write halves
        let (client_reader, client_writer) = tokio::io::split(client_socket);
        let (backend_reader, backend_writer) = tokio::io::split(backend_socket);

        // Spawn tasks for bi-directional data transfer
        let client_to_backend = tokio::spawn(Self::transfer_data(
            client_reader,
            backend_writer,
            "Client -> Backend",
        ));

        let backend_to_client = tokio::spawn(Self::transfer_data(
            backend_reader,
            client_writer,
            "Backend -> Client",
        ));

        // Wait for both tasks to complete
        let _ = tokio::try_join!(client_to_backend, backend_to_client)?;

        Ok(())
    }

    async fn transfer_data<R, W>(
        mut reader: R,
        mut writer: W,
        direction: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        R: AsyncReadExt + Unpin,
        W: AsyncWriteExt + Unpin,
    {
        let mut buffer = vec![0; 1024];
        loop {
            let n = reader.read(&mut buffer).await?;
            if n == 0 {
                info!("{}: Connection closed", direction);
                return Ok(());
            }
            writer.write_all(&buffer[..n]).await?;
            writer.flush().await?;
        }
    }
}
