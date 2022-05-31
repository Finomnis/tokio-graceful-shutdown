//! This example demonstrates how a subsystem could get implemented that auto-restarts
//! every time a panic occurs.
//!
//! This isn't really a usecase related to this library, but seems to be used regularly,
//! so I included it anyway.

use env_logger::{Builder, Env};
use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    // This subsystem panics every two seconds.
    // It should get restarted constantly.

    log::info!("Subsystem1 started.");
    tokio::select! {
        _ = subsys.on_shutdown_requested() => (),
        _ = sleep(Duration::from_secs(2)) => {
            panic!("Subsystem1 panicked!");
        }
    };
    log::info!("Subsystem1 stopped.");

    Ok(())
}

async fn subsys1_keepalive(subsys: SubsystemHandle) -> Result<()> {
    loop {
        let nested_toplevel_result = Toplevel::nested(&subsys, "")
            .start("Subsys1", subsys1)
            .handle_shutdown_requests(Duration::from_millis(50))
            .await;

        match nested_toplevel_result {
            Ok(()) => break,
            Err(err) => {
                log::error!("Subsystem1 failed: {}", err);
                log::info!("Restarting subsystem1 ...");
            }
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1Keepalive", subsys1_keepalive)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}
