//! This example demonstrates how a subsystem can initiate
//! a shutdown.

use env_logger::{Builder, Env};
use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

struct CountdownSubsystem {}
impl CountdownSubsystem {
    fn new() -> Self {
        Self {}
    }

    async fn countdown(&self) {
        for i in (1..10).rev() {
            log::info!("Shutting down in: {}", i);
            sleep(Duration::from_millis(1000)).await;
        }
    }

    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
        tokio::select! {
            _ = subsys.on_shutdown_requested() => {
                log::info!("Countdown cancelled.");
            },
            _ = self.countdown() => {
                subsys.request_shutdown();
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
        .start("Countdown", |h| CountdownSubsystem::new().run(h))
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}
