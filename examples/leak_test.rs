//! This example is not an actual example.
//!
//! It is just a demonstrator to show that this crate does not leak memory.
//! It gets used by the CI to perform a very crude leak check.
//!
//! Run this example with the environment variable:
//!     sudo apt install valgrind
//!     cargo build --example leak_test
//!     valgrind --leak-check=yes target/debug/examples/leak_test
//!
//! This will print allocation information, including the amount of leaked memory.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    subsys.start("Subsys2", subsys2);
    subsys.start("Subsys3", subsys3);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem2 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem2 ...");
    sleep(Duration::from_millis(400)).await;
    log::info!("Subsystem2 stopped.");
    Ok(())
}

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem3 started.");
    tokio::select! {
        _ = sleep(Duration::from_millis(200)) => {
            log::info!("Sybsystem3 initiates a shutdown ...");
            subsys.request_shutdown();
        },
        _ = subsys.on_shutdown_requested() => (),
    };
    log::info!("Subsystem3 stopped.");
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
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
