use crate::proxy::Proxy;
use log::{error, info};
use tokio::net::TcpListener;

pub struct ConnectionManager {
    listen_addr: String,
    backend_addr: String,
}

impl ConnectionManager {
    pub fn new(listen_addr: String, backend_addr: String) -> Self {
        ConnectionManager {
            listen_addr,
            backend_addr,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        info!("Listening on {}", self.listen_addr);

        let proxy = Proxy::new(self.backend_addr.clone());

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    info!("New client connected");
                    let proxy_clone = proxy.clone();
                    tokio::spawn(async move {
                        if let Err(e) = proxy_clone.handle_connection(socket).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => error!("Failed to accept connection: {}", e),
            }
        }
    }
}
