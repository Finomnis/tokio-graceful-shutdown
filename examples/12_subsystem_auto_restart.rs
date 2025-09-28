//! This example demonstrates how a subsystem could get implemented that auto-restarts
//! every time a panic occurs.
//!
//! This isn't really a usecase related to this library, but seems to be used regularly,
//! so I included it anyway.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{ErrorAction, SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys1(subsys: &mut SubsystemHandle) -> Result<()> {
    // This subsystem panics every two seconds.
    // It should get restarted constantly.

    tracing::info!("Subsystem1 started.");
    tokio::select! {
        _ = subsys.on_shutdown_requested() => (),
        _ = sleep(Duration::from_secs(2)) => {
            panic!("Subsystem1 panicked!");
        }
    };
    tracing::info!("Subsystem1 stopped.");

    Ok(())
}

async fn subsys1_keepalive(subsys: &mut SubsystemHandle) -> Result<()> {
    loop {
        let nested_subsys = subsys.start(
            SubsystemBuilder::new("Subsys1", subsys1)
                .on_failure(ErrorAction::CatchAndLocalShutdown)
                .on_panic(ErrorAction::CatchAndLocalShutdown),
        );

        if let Err(err) = nested_subsys.join().await {
            tracing::error!("Subsystem1 failed: {:?}", miette::Report::from(err));
        } else {
            break;
        }

        tracing::info!("Restarting subsystem1 ...");
    }

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(async |s: &mut SubsystemHandle| {
        s.start(SubsystemBuilder::new("Subsys1Keepalive", subsys1_keepalive));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
