use anyhow::Result;
use async_trait::async_trait;
use env_logger::{Builder, Env};
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle, Toplevel};

struct MySubsystem {}
struct StopSubsystem {}

#[async_trait]
impl AsyncSubsystem for StopSubsystem {
    async fn run(&mut self, subsys: SubsystemHandle) -> Result<()> {
        tokio::select!{
            _ = sleep(Duration::from_millis(3000)) => {
                log::info!("Stopping system ...");
                subsys.request_shutdown();
            },
            _ = subsys.on_shutdown_requested() => {
                log::info!("System already shutting down.");
            }
        };

        log::info!("StopSubsystem ended.");
        Ok(())
    }
}

#[async_trait]
impl AsyncSubsystem for MySubsystem {
    async fn run(&mut self, mut subsys: SubsystemHandle) -> Result<()> {
        subsys.start("StopSubsystem", StopSubsystem{});
        log::info!("MySubsystem started.");
        subsys.on_shutdown_requested().await;
        log::info!("Shutting down MySubsystem ...");
        sleep(Duration::from_millis(500)).await;
        log::info!("MySubsystem stopped.");
        Ok(())
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("MySubsystem", MySubsystem {})
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
