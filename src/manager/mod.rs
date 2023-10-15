use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::protocol::ping;
use crate::server::Server;

pub struct ServerManager<S: Server> {
	server: Arc<Mutex<S>>,
	turn_of_at: Instant,
}

impl<S: Server> ServerManager<S> {
	pub fn new(server: Arc<Mutex<S>>) -> Self {
		Self {
			server,
			turn_of_at: Instant::now().checked_add(Duration::from_secs(1800)).unwrap(),
		}
	}

	/// Returns `true` if server should turn off.
	pub async fn probe(&mut self) -> bool {
		if let Some(addr) = self.server.lock().await.addr() {
			if let Some(status) = ping(addr).await {
				if status.players.online > 0 {
					self.turn_of_at = Instant::now().checked_add(Duration::from_secs(1800)).unwrap();
					return false;
				}
			}
		}

		// There are currently zero players online, or the server is already offline. (checked above)
		if self.turn_of_at.saturating_duration_since(Instant::now()) == Duration::from_millis(0) {
			return true;
		}

		return false;
	}

	/// This is a looping task, which never exits and checks if we should shut down the server.
	pub async fn probe_task(mut self) -> ! {
		loop {
			if self.probe().await {
				self.server.lock().await.stop().await.unwrap();
			}

			tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
		}
	}
}