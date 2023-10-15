use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use bollard::Docker;

#[async_trait::async_trait]
pub trait Server: Sized {
	async fn start(&mut self) -> std::io::Result<()>;
	async fn stop(&mut self) -> std::io::Result<()>;

	fn addr(&self) -> SocketAddr;
}

pub struct DockerServer {
	docker: Docker,
	container_name: String,
	container_ip_addr: Option<SocketAddr>,
}

impl DockerServer {
	pub fn new(container_name: &str) -> Self {
		let docker = Docker::connect_with_socket_defaults().unwrap();

		DockerServer { docker, container_name: container_name.to_string(), container_ip_addr: None }
	}
}

#[async_trait::async_trait]
impl Server for DockerServer {
	async fn start(&mut self) -> std::io::Result<()> {
		let networks = self.docker.inspect_container(&self.container_name, None).await.unwrap().network_settings.unwrap().networks.unwrap();	

		let ip_addr = if let Some(bridge) = networks.get("bridge") {
			Ipv4Addr::from_str(bridge.ip_address.as_ref().unwrap()).unwrap()
		} else {
			Ipv4Addr::from_str(&networks.into_iter().next().unwrap().1.ip_address.unwrap()).unwrap()
		};

		self.container_ip_addr = Some(SocketAddr::from((ip_addr, 25565)));

		self.docker.start_container::<&'static str>(&self.container_name, None).await.unwrap();

		Ok(())
	}

	async fn stop(&mut self) -> std::io::Result<()> {
		self.docker.stop_container(&self.container_name, None).await.unwrap();

		Ok(())
	}

	fn addr(&self) -> SocketAddr {
		self.container_ip_addr.unwrap()
	}
}