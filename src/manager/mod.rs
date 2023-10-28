use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::protocol::ping;
use crate::server::{Server, HostData};

pub struct ServerManager<S: Server> {
    server: Arc<Mutex<S>>,
    turn_of_at: Instant,
}

#[derive(PartialEq, Eq)]
pub enum ProbeResult {
    TurnOff,
    KeepOn,
}

pub enum HealthCheck {
    PlayersOnline,
    Empty,
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

    #[tracing::instrument(skip(self))]
    pub async fn health_check(&mut self, host: HostData<'_>) -> Result<HealthCheck, ()> {
        let Ok(status) = ping(&host.host, host.addr).await else {
            return Err(());
        };

        tracing::info!("manager server health check completed");
        
        if status.players.online > 0 {
            tracing::info!(online = status.players.online, "players are on the server");

            Ok(HealthCheck::PlayersOnline)
        } else {
            tracing::info!("no players are on the server");

            Ok(HealthCheck::Empty)
        }
    }

    pub fn update_turn_off_at(&mut self) {
        self.turn_of_at = Instant::now()
            .checked_add(Duration::from_secs(1800))
            .unwrap();
    }

    // TODO: make this less ugly
    pub async fn get_addr<'a, 'b>(&'a mut self) -> Option<HostData<'b>> {
        self.server.lock().await.addr().map(|h| HostData { host: h.host.into_owned().into(), addr: h.addr })
    }

    pub async fn probe(&mut self) -> ProbeResult {
        if let Some(host) = self.get_addr().await {
            if let Ok(health_check) = self.health_check(host).await {
                match health_check {
                    HealthCheck::Empty => (),
                    HealthCheck::PlayersOnline => {
                        self.update_turn_off_at();
                    }
                }
            }
        }

        // There are currently zero players online, or the server is already offline. (checked above)
        if self.turn_of_at.saturating_duration_since(Instant::now()) == Duration::from_millis(0) {
            return ProbeResult::TurnOff;
        }

        ProbeResult::KeepOn
    }

    /// This is a looping task, which never exits and checks if we should shut down the server.
    pub async fn probe_task(mut self) -> ! {
        loop {
            if self.probe().await == ProbeResult::TurnOff {
                self.server.lock().await.stop().await.unwrap();
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }
}
