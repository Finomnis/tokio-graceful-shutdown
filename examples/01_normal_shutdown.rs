//! This example demonstrates the basic usage pattern of this crate.
//!
//! It shows that a subsystem gets started, and when the program
//! gets shut down (by pressing Ctrl-C), the subsystem gets shut down
//! gracefully.
//!
//! In this case, the subsystem is an async function.
//! This crate supports async functions and async coroutines.
//!
//! If custom arguments for the subsystem coroutine are required,
//! a struct has to be used instead, as seen in other examples.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}
