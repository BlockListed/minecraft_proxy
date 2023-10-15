use std::net::SocketAddr;

use protocol::ping;
use tokio::{net::{TcpListener, TcpStream}, io::copy_bidirectional};

mod protocol;

#[tokio::main]
async fn main() {
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