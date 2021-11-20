use anyhow::Result;
use async_trait::async_trait;
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle};

pub struct SlowShutdownSubsystem {
    shutdown_duration: Duration,
    return_value: Result<()>,
}

#[async_trait]
impl AsyncSubsystem for SlowShutdownSubsystem {
    async fn run(mut self, subsys: SubsystemHandle) -> Result<()> {
        log::info!("SlowShutdownSubsystem started.");
        subsys.on_shutdown_requested().await;
        log::info!("Shutting down SlowShutdownSubsystem ...");
        sleep(self.shutdown_duration).await;
        log::info!("SlowShutdownSubsystem stopped.");
        self.return_value
    }
}

impl SlowShutdownSubsystem {
    pub fn new(shutdown_duration: Duration) -> Self {
        Self {
            shutdown_duration,
            return_value: Ok(()),
        }
    }
    pub fn return_value(mut self, value: Result<()>) -> Self {
        self.return_value = value;
        self
    }
}
