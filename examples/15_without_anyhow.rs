//! This example shows how to use this library with std::error::Error instead of anyhow::Error

use env_logger::{Builder, Env};
use std::error::Error as StdError;
use std::fmt;
use std::result::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{Error, SubsystemHandle, Toplevel};

#[derive(Debug, Clone)]
struct MyError;

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MyError")
    }
}

impl StdError for MyError {}

async fn subsys1(_subsys: SubsystemHandle) -> Result<(), MyError> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(MyError {})
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await?;

    Ok(())
}
