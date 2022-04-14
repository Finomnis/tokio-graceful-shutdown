//! This example demonstrates how decoupled subsystems react to errors.
//!
//! Subsys1 is a normal subsystem with the Toplevel as root and parent.
//! Subsys2 is a decoupled subsystem, it is its own root and parent.
//! Subsys3 is a normal subsystem with Subsys2 as root and parent.
//! Subsys4 is a normal subsystem with Subsys2 as root and parent.
//! Subsys5 is a normal subsystem with Subsys2 as root and Subsys3 as parent.
//!
//! If a panic/error occurs in a subsystem, a partial shutdown of the root of the subsystem
//! (= the next higher decoupled subsystem) will be initiated.
//! If the root is the Toplevel, a program shutdown will be performed.
//!
//! In this example, the decoupling point is Subsys2. This means, if an error occurs in any
//! of its children, it will only trigger a shutdown of Subsys2, 3, 4 and 5. Subsys1 will not be stopped.
//!
//! To be specific, in this example, Subsys3 will panic after 500ms.
//! This error will propagate to its root, Subsys2.
//! Subsys2 will then initiate a partial shutdown, causing Subsys2, 4 and 5 to shut down.
//! Subsys1 will still be running.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    subsys.start_decoupled("Subsys2", subsys2);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(mut subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem2 started.");
    subsys.start("Subsys3", subsys3);
    subsys.start("Subsys4", subsys4);

    subsys.on_shutdown_requested().await;
    log::info!("Subsystem2 stopped.");
    Ok(())
}

async fn subsys3(mut subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem3 started.");
    subsys.start("Subsys5", subsys5);

    sleep(Duration::from_millis(500)).await;
    panic!("Subsystem3 panicked!")
}

async fn subsys4(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem4 started.");
    subsys.on_shutdown_requested().await;
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem4 stopped.");
    Ok(())
}

async fn subsys5(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem5 started.");
    subsys.on_shutdown_requested().await;
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem5 stopped.");
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
