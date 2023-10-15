use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use bollard::Docker;

use super::Server;

pub struct DockerServer {
    docker: Docker,
    container_name: String,
    container_ip_addr: Option<SocketAddr>,
}

impl DockerServer {
    pub fn new(container_name: &str) -> Self {
        let docker = Docker::connect_with_socket_defaults().unwrap();

        DockerServer {
            docker,
            container_name: container_name.to_string(),
            container_ip_addr: None,
        }
    }

    pub async fn get_socket_addr(&self) -> SocketAddr {
        let networks = self
            .docker
            .inspect_container(&self.container_name, None)
            .await
            .unwrap()
            .network_settings
            .unwrap()
            .networks
            .unwrap();

        let ip_addr = if let Some(bridge) = networks.get("bridge") {
            let ip = bridge.ip_address.as_ref().unwrap();
            tracing::info!(ip, "using bridge ip address");
            Ipv4Addr::from_str(ip).unwrap()
        } else {
            let (name, network) = networks.into_iter().next().unwrap();
            let ip = network.ip_address.unwrap();
            tracing::info!(
                ip,
                network = name,
                "found ip address on non-default network"
            );
            Ipv4Addr::from_str(&ip).unwrap()
        };

        SocketAddr::from((ip_addr, 25565))
    }
}

#[async_trait::async_trait]
impl Server for DockerServer {
    async fn start(&mut self) -> std::io::Result<()> {
        tracing::info!(container = self.container_name, "starting docker mc server");
        self.docker
            .start_container::<&'static str>(&self.container_name, None)
            .await
            .unwrap();

        if self.container_ip_addr.is_none() {
            self.container_ip_addr = Some(self.get_socket_addr().await);
        }

        Ok(())
    }

    async fn stop(&mut self) -> std::io::Result<()> {
        tracing::info!(container = self.container_name, "stopping docker mc server");
        self.docker
            .stop_container(&self.container_name, None)
            .await
            .unwrap();

        Ok(())
    }

    fn addr(&self) -> Option<SocketAddr> {
        self.container_ip_addr
    }
}
