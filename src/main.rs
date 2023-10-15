use std::net::SocketAddr;

use protocol::ping;
use tokio::{net::{TcpListener, TcpStream}, io::copy_bidirectional};

mod protocol;

fn setup_logger() {
    tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ACTIVE)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[tokio::main]
async fn main() {
    setup_logger();

    ping("localhost:25565").await;

    return;
    let listener = TcpListener::bind("127.0.0.1:2000".parse::<SocketAddr>().unwrap()).await.unwrap();

    loop {
        let (conn, _) = listener.accept().await.unwrap();

        tokio::spawn(handle_conn(conn));
    }
}

async fn handle_conn(mut conn: TcpStream) {
    let mut mc_server = TcpStream::connect("127.0.0.1:25565".parse::<SocketAddr>().unwrap()).await.unwrap();

    copy_bidirectional(&mut conn, &mut mc_server).await.unwrap();
}