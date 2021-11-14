use anyhow::Result;
//use graceful_shutdown::{start_submodule, wait_until_shutdown};
use env_logger::{Builder, Env};
use log;

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("info")).init();

    // Register Ctrl+C and SIGTERM handlers
    //graceful_shutdown::register_signal_handlers();

    // Actual program
    log::info!("Hello, world!");
    //let dummy_task_handle = start_submodule(dummy_task::dummy_task());

    // Wait for program shutdown initiation
    //wait_until_shutdown().await;

    // Wait until all submodules have shut down
    //shutdown_with_timeout!(Duration::from_millis(1000), dummy_task_handle)

    Ok(())
}
