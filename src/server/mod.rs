use tokio::net::TcpStream;

#[async_trait::async_trait]
pub trait Server: Sized {
	async fn connect() -> std::io::Result<Self>;

	fn stream(self) -> TcpStream;
}