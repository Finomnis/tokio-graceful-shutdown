//! This example demonstrates how to implement a clean shutdown
//! of a subsystem.
//!
//! The central mechanism here is tokio::select, which can cancel
//! tasks once one of them finishes. This can be used to cancel
//! tasks once the shutdown was initiated.

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
            log::info!("Countdown: {}", i);
            sleep(Duration::from_millis(1000)).await;
        }
    }

    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
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
        .start("Countdown", |h| CountdownSubsystem::new().run(h))
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}
