use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tracing::Instrument;

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
            turn_of_at: Instant::now()
                .checked_add(Duration::from_secs(1800))
                .unwrap(),
        }
    }

    /// Returns `true` if server should turn off.
    pub async fn probe(&mut self) -> bool {
        if let Some(host) = self.server.lock().await.addr() {
            let can_shutdown = async {
                if let Ok(status) = ping(&host.host, host.addr).await {
                    tracing::info!("manager server health check completed");
                    if status.players.online > 0 {
                        tracing::info!(online = status.players.online, "players are on the server");
                        self.turn_of_at = Instant::now()
                            .checked_add(Duration::from_secs(1800))
                            .unwrap();
                        return false;
                    }
                    tracing::info!("no players are on the server");
                }

                true
            }
            .instrument(tracing::info_span!("health_check", addr=%host.addr))
            .await;

            if !can_shutdown {
                return false;
            }
        }

        // There are currently zero players online, or the server is already offline. (checked above)
        if self.turn_of_at.saturating_duration_since(Instant::now()) == Duration::from_millis(0) {
            return true;
        }

        false
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
