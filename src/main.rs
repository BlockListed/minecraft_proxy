use std::{net::SocketAddr, sync::Arc};

use protocol::{retry_ping, ping};
use server::{Server, DockerServer};
use tokio::{net::{TcpListener, TcpStream}, io::copy_bidirectional, sync::Mutex};

mod protocol;
mod server;

fn setup_logger() {
    tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[tokio::main]
async fn main() {
    setup_logger();

    let listener = TcpListener::bind("127.0.0.1:2000".parse::<SocketAddr>().unwrap()).await.unwrap();

    let server = Arc::new(Mutex::new(DockerServer::new("mc")));

    loop {
        let (conn, _) = listener.accept().await.unwrap();

        tokio::spawn(handle_conn(conn, Arc::clone(&server)));
    }
}

async fn get_connection<S: Server>(server: &mut S) -> TcpStream {
    if let Some(addr) = server.addr() {
        if ping(addr).await.is_some() {
            return TcpStream::connect(addr).await.unwrap();
        }
    }

    server.start().await.unwrap();

    let addr = server.addr().unwrap();

    retry_ping(addr).await.unwrap();

    TcpStream::connect(addr).await.unwrap()
}

async fn handle_conn<S: Server>(mut conn: TcpStream, server: Arc<Mutex<S>>) {
    let mut mc_server = get_connection(&mut *server.lock().await).await;

    copy_bidirectional(&mut conn, &mut mc_server).await.unwrap();
}