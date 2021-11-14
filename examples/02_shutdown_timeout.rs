use anyhow::Result;
use env_logger::{Builder, Env};
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    register_signal_handlers, start_submodule, wait_for_submodule_shutdown, wait_until_shutdown,
};

async fn dummy_task() -> Result<()> {
    log::info!("dummy_task started.");
    wait_until_shutdown().await;
    log::info!("Shutting down dummy_task ...");
    sleep(Duration::from_millis(1000)).await;
    log::info!("dummy_task stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Register Ctrl+C and SIGTERM handlers
    register_signal_handlers();

    // Actual program
    log::info!("Hello, world!");
    let dummy_task_handle = start_submodule(dummy_task());

    // Wait for program shutdown initiation
    wait_until_shutdown().await;

    // Wait until all submodules have shut down
    wait_for_submodule_shutdown!(Duration::from_millis(500), dummy_task_handle)
}
