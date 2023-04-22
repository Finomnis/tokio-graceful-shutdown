//! This example shows how to use this library with std::error::Error instead of miette::Error

use std::error::Error;
use std::fmt;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

#[derive(Debug, Clone)]
struct MyError;

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MyError")
    }
}

impl Error for MyError {}

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(_subsys: SubsystemHandle) -> Result<(), MyError> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(MyError {})
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}
