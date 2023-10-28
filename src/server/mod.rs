use std::sync::Arc;
use std::net::SocketAddr;

pub mod docker;

#[derive(Debug)]
pub struct HostData {
    pub host: Arc<str>,
    pub addr: SocketAddr,
}

#[async_trait::async_trait]
pub trait Server: Sized {
    async fn start(&mut self) -> color_eyre::Result<()>;
    async fn stop(&mut self) -> color_eyre::Result<()>;

    fn addr(&self) -> Option<HostData>;
}
