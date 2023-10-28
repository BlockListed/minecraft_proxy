use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use bollard::Docker;

use super::{Server, HostData};

pub struct DockerServer {
    docker: Docker,
    container_name: String,
    container_ip_addr: Option<SocketAddr>,
}

impl DockerServer {
    pub async fn new(container_name: &str) -> Self {
        let docker = Docker::connect_with_socket_defaults().unwrap();

        let mut server = DockerServer {
            docker,
            container_name: container_name.to_string(),
            container_ip_addr: None,
        };

        server.test().await.expect("invalid docker server configuration");

        server
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

    async fn test(&mut self) -> Result<(), ()> {
        self.docker
            .inspect_container(&self.container_name, None)
            .await
            .map(|_| ())
            .map_err(|_| ())
    }
}

#[async_trait::async_trait]
impl Server for DockerServer {

    async fn start(&mut self) -> color_eyre::Result<()> {
        tracing::info!(container = self.container_name, "starting docker mc server");
        self.docker
            .start_container::<&'static str>(&self.container_name, None)
            .await?;

        if self.container_ip_addr.is_none() {
            self.container_ip_addr = Some(self.get_socket_addr().await);
        }

        Ok(())
    }

    async fn stop(&mut self) -> color_eyre::Result<()> {
        tracing::info!(container = self.container_name, "stopping docker mc server");
        self.docker
            .stop_container(&self.container_name, None)
            .await?;

        Ok(())
    }

    fn addr(&self) -> Option<HostData> {
        self.container_ip_addr
            .map(|addr| HostData { host: self.container_name.as_str().into(), addr })
    }
}
