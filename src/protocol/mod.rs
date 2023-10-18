use std::net::SocketAddr;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio::time::Duration;

use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

use crate::protocol::parsing::ParseError;

mod parsing;

pub async fn ping(host: &str, addr: SocketAddr) -> Result<parsing::JsonStatusResponse, ()> {
    let port = addr.port();

    let mut socket = match timeout(Duration::from_secs(1), TcpStream::connect(addr))
        .await
        .ok()
    {
        Some(Ok(d)) => {
            tracing::info!(%addr, "successfully connected");
            d
        }
        Some(Err(e)) => {
            tracing::warn!(%addr, %e, "error while connecting to host");
            return Err(());
        }
        None => {
            tracing::warn!(%addr, "timeout while connecting to host");
            return Err(());
        }
    };

    let server_list_ping = parsing::server_list_ping(host, port);
    tracing::trace!(data=?server_list_ping, "sending server list ping status change");
    socket.write_all(&server_list_ping).await.unwrap();

    let status_request = parsing::status_request();
    tracing::trace!(data=?status_request, "sending status request");
    socket.write_all(&status_request).await.unwrap();

    let mut resp_buffer = vec![0u8; 20000];

    let mut total_read = 0;

    loop {
        tracing::info!("waiting for response from server");
        let read = socket.read(&mut resp_buffer).await.unwrap();

        if read == 0 {
            tracing::info!("connection closed");
            return Err(());
        }

        total_read += read;

        match parsing::parse_status_response(&resp_buffer[..total_read]) {
            Ok(s) => {
                tracing::info!(status=?s.1.json_response, "received status response");

                socket.shutdown().await.unwrap();

                return Ok(s.1.json_response);
            }
            Err(ParseError::Incomplete) => (),
            Err(e) => panic!("{:?}", e),
        }
    }
}

pub async fn retry_ping(host: &str, addr: SocketAddr) -> Result<parsing::JsonStatusResponse, ()> {
    let strategy = FixedInterval::from_millis(500);

    Retry::spawn(strategy, || async {
        ping(host, addr).await
    })
    .await
}
