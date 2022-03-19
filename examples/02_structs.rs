//! This example demonstrates how using subsystem structs enables
//! custom parameters to be passed to the subsystem.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{Error, SubsystemHandle, Toplevel};

struct Subsystem1 {
    arg: u32,
}

impl Subsystem1 {
    async fn run(self, subsys: SubsystemHandle) -> Result<()> {
        log::info!("Subsystem1 started. Extra argument: {}", self.arg);
        subsys.on_shutdown_requested().await;
        log::info!("Shutting down Subsystem1 ...");
        sleep(Duration::from_millis(500)).await;
        log::info!("Subsystem1 stopped.");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    let subsys = Subsystem1 { arg: 42 };

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", |a| subsys.run(a))
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}
