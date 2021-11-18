use anyhow::Result;
use async_trait::async_trait;
use std::panic;
use std::time::Duration;

use crate::{
    shutdown_token::ShutdownToken, signal_handling::wait_for_signal, AsyncSubsystem,
    SubsystemHandle,
};

pub struct Toplevel {
    toplevel_subsys: SubsystemHandle,
}

struct DummySubsystem {}

#[async_trait(?Send)]
impl AsyncSubsystem for DummySubsystem {
    async fn run(&mut self, _: &mut SubsystemHandle) -> Result<()> {
        Ok(())
    }
}

impl Toplevel {
    pub fn new() -> Self {
        let shutdown_token = ShutdownToken::new();

        // Register panic handler to trigger shutdown token
        let panic_shutdown_token = shutdown_token.clone();
        panic::set_hook(Box::new(move |panic_info| {
            log::error!("ERROR: {}", panic_info);
            panic_shutdown_token.shutdown();
        }));

        Self {
            toplevel_subsys: SubsystemHandle::new(Box::new(DummySubsystem {}), shutdown_token),
        }
    }

    pub fn start<S: AsyncSubsystem + 'static>(
        &mut self,
        name: &'static str,
        subsystem: S,
    ) -> &mut Self {
        self.toplevel_subsys.start(name, subsystem);

        self
    }

    pub fn catch_signals(&mut self) -> &mut Self {
        let shutdown_token = self.toplevel_subsys.shutdown_token();
        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.shutdown();
        });
        self
    }

    pub async fn wait_for_shutdown(&mut self, shutdown_timeout: Duration) -> Result<()> {
        self.toplevel_subsys.on_shutdown_request().await;

        Ok(())
    }
}
