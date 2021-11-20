use anyhow::Result;
use async_trait::async_trait;
use log;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle};

pub struct ImmediateSubsystem {
    shutdown_duration: Duration,
    return_value: Result<()>,
}

#[async_trait]
impl AsyncSubsystem for ImmediateSubsystem {
    async fn run(mut self, _subsys: SubsystemHandle) -> Result<()> {
        log::info!("SlowShutdownSubsystem started.");
        sleep(self.shutdown_duration).await;
        log::info!("SlowShutdownSubsystem stopped.");
        self.return_value
    }
}

impl ImmediateSubsystem {
    pub fn new() -> Self {
        Self {
            shutdown_duration: Duration::from_millis(0),
            return_value: Ok(()),
        }
    }
    pub fn return_value(mut self, value: Result<()>) -> Self {
        self.return_value = value;
        self
    }
    // pub fn shutdown_duration(mut self, value: Duration) -> Self {
    //     self.shutdown_duration = value;
    //     self
    // }
}
