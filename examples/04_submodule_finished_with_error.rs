use anyhow::{anyhow, Result};
use env_logger::{Builder, Env};
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    register_signal_handlers, start_submodule, wait_for_submodule_shutdown,
    wait_until_shutdown_started,
};

async fn dummy_task() -> Result<()> {
    log::info!("dummy_task started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("dummy_task stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(anyhow!("dummy_task threw an error."))
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
    wait_until_shutdown_started().await;

    // Wait until all submodules have shut down
    wait_for_submodule_shutdown!(Duration::from_millis(1000), dummy_task_handle)
}
