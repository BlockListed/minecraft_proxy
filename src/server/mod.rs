use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use bollard::Docker;

pub mod docker;

#[async_trait::async_trait]
pub trait Server: Sized {
	async fn start(&mut self) -> std::io::Result<()>;
	async fn stop(&mut self) -> std::io::Result<()>;

	fn addr(&self) -> Option<SocketAddr>;
}
