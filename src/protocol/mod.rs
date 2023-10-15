use std::net::ToSocketAddrs;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio::time::Duration;

mod parsing;

pub async fn ping(addr: &str) -> bool {
	let socket_addrs = addr.to_socket_addrs().unwrap();

	let host = if let Some(e) = addr.rfind(":") {
		&addr[..e]
	} else {
		addr
	};

	let mut socket_port = None;

	for addr in socket_addrs {
		match timeout(Duration::from_secs(1), TcpStream::connect(addr)).await.ok() {
			Some(Ok(d)) => {
				println!("Succesfully connected to {addr}.");
				if socket_port.is_none() {
					socket_port = Some((d, addr.port()));
					break;
				}
			}
			Some(Err(e)) => {
				println!("Error while connecting to {addr}. {e}");
			}
			None => {
				println!("Timeout while connecting to {addr}.");
			}
		}
	} 

	let Some((mut socket, port)) = socket_port else {
		return false;
	};

	let server_list_ping = parsing::server_list_ping(host, port);
	println!("{:?}", server_list_ping);
	socket.write_all(&server_list_ping).await.unwrap();

	let status_request = parsing::status_request();
	println!("{:?}", status_request);
	socket.write_all(&status_request).await.unwrap();

	let mut resp_buffer = vec![0u8; 20000];

	loop {
		println!("Waiting for response from server.");
		let read = socket.read(&mut resp_buffer).await.unwrap();

		if read == 0 {
			panic!("Connection closed");
		}

		println!("Read {read} bytes of data: {resp_buffer:?}");
		if let Some(status) = parsing::parse_status_response(&resp_buffer[..read]) {
			println!("{:?}", status.1);
			break;
		}
	}

	return true;
}