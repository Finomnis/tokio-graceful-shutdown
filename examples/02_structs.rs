//! This example demonstrates how using subsystem structs enables
//! custom parameters to be passed to the subsystem.
//!
//! There are two ways of using structs as subsystems, by either
//! wrapping them in a closure, or by implementing the
//! IntoSubsystem trait. Note, though, that the IntoSubsystem
//! trait requires an additional dependency, `async-trait`.

use async_trait::async_trait;
use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel};

struct Subsystem1 {
    arg: u32,
}

impl Subsystem1 {
    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
        tracing::info!("Subsystem1 started. Extra argument: {}", self.arg);
        subsys.on_shutdown_requested().await;
        tracing::info!("Shutting down Subsystem1 ...");
        sleep(Duration::from_millis(500)).await;
        tracing::info!("Subsystem1 stopped.");
        Ok(())
    }
}

struct Subsystem2 {
    arg: u32,
}

#[async_trait]
impl IntoSubsystem<miette::Report> for Subsystem2 {
    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
        tracing::info!("Subsystem2 started. Extra argument: {}", self.arg);
        subsys.on_shutdown_requested().await;
        tracing::info!("Shutting down Subsystem2 ...");
        sleep(Duration::from_millis(500)).await;
        tracing::info!("Subsystem2 stopped.");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    let subsys1 = Subsystem1 { arg: 42 };
    let subsys2 = Subsystem2 { arg: 69 };

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", |a| subsys1.run(a)));
        s.start(SubsystemBuilder::new("Subsys2", subsys2.into_subsystem()));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
