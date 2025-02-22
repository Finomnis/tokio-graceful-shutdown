//! This example demonstrates how a subsystem can initiate
//! a shutdown.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{
    FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel, errors::CancelledByShutdown,
};

struct CountdownSubsystem {}
impl CountdownSubsystem {
    fn new() -> Self {
        Self {}
    }

    async fn countdown(&self) {
        for i in (1..10).rev() {
            tracing::info!("Shutting down in: {}", i);
            sleep(Duration::from_millis(1000)).await;
        }
    }

    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
        match self.countdown().cancel_on_shutdown(&subsys).await {
            Ok(()) => subsys.request_shutdown(),
            Err(CancelledByShutdown) => tracing::info!("Countdown cancelled."),
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("Countdown", |h| {
            CountdownSubsystem::new().run(h)
        }));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
