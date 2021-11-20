use anyhow::Result;
use async_trait::async_trait;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{subsys::LambdaSubsystem, AsyncSubsystem, SubsystemHandle, Toplevel};

async fn run(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

struct Subsystem1 {}

#[async_trait]
impl AsyncSubsystem for Subsystem1 {
    async fn run(mut self, subsys: SubsystemHandle) -> Result<()> {
        log::info!("Subsystem1 started.");
        subsys.on_shutdown_requested().await;
        log::info!("Shutting down Subsystem1 ...");
        sleep(Duration::from_millis(500)).await;
        log::info!("Subsystem1 stopped.");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    let subsys = Subsystem1 {};

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", LambdaSubsystem::new(|a| subsys.run(a)))
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
