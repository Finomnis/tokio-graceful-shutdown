//! This example demonstrates how a subsystem can initiate
//! a shutdown.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{errors::CancelledByShutdown, FutureExt, SubsystemHandle, Toplevel};

struct CountdownSubsystem {}
impl CountdownSubsystem {
    fn new() -> Self {
        Self {}
    }

    #[tracing::instrument(name = "Subsys Countdown", skip_all)]
    async fn countdown(&self) {
        for i in (1..10).rev() {
            tracing::info!("Shutting down in: {}", i);
            sleep(Duration::from_millis(1000)).await;
        }
    }

    #[tracing::instrument(name = "Subsys", skip_all)]
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
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create toplevel
    Toplevel::new()
        .start("Countdown", |h| CountdownSubsystem::new().run(h))
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}
