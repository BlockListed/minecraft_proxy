use std::borrow::Cow;
use std::net::SocketAddr;

pub mod docker;

#[derive(Debug)]
pub struct HostData<'a> {
    pub host: Cow<'a, str>,
    pub addr: SocketAddr,
}

#[async_trait::async_trait]
pub trait Server: Sized {
    async fn start(&mut self) -> std::io::Result<()>;
    async fn stop(&mut self) -> std::io::Result<()>;

    fn addr(&self) -> Option<HostData>;
}
