use anyhow::Result;
use async_trait::async_trait;
use env_logger::{Builder, Env};
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle, Toplevel};

struct CountdownSubsystem {}
impl CountdownSubsystem {
    fn new() -> Self {
        Self {}
    }

    async fn countdown(&self) {
        for i in (1..10).rev() {
            log::info!("Countdown: {}", i);
            sleep(Duration::from_millis(1000)).await;
        }
    }
}

#[async_trait]
impl AsyncSubsystem for CountdownSubsystem {
    async fn run(&mut self, subsys: SubsystemHandle) -> Result<()> {
        log::info!("Starting countdown ...");

        tokio::select! {
            _ = subsys.on_shutdown_requested() => {
                log::info!("Countdown cancelled.");
            },
            _ = self.countdown() => {
                log::info!("Countdown finished.");
            }
        };

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Countdown", CountdownSubsystem::new())
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
