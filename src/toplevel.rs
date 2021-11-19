use anyhow::Result;
use std::time::Duration;
use std::{panic, sync::Arc};

use crate::signal_handling::wait_for_signal;
use crate::SubsystemHandle;
use crate::{shutdown_token::ShutdownToken, AsyncSubsystem};

use super::subsystem::SubsystemData;

pub struct Toplevel {
    subsys_data: Arc<SubsystemData>,
    subsys_handle: SubsystemHandle,
}

// struct ToplevelSubsystem {}

// #[async_trait(?Send)]
// impl AsyncSubsystem for DummySubsystem {
//     async fn run(&mut self, _: &mut SubsystemHandle) -> Result<()> {
//         std::unreachable!("Top level subsystem should never be executed. It's just a dummy!");
//     }
// }

impl Toplevel {
    pub fn new() -> Self {
        let shutdown_token = ShutdownToken::new();

        // Register panic handler to trigger shutdown token
        let panic_shutdown_token = shutdown_token.clone();
        panic::set_hook(Box::new(move |panic_info| {
            log::error!("ERROR: {}", panic_info);
            panic_shutdown_token.shutdown();
        }));

        let subsys_data = Arc::new(SubsystemData::new("", shutdown_token));
        let subsys_handle = SubsystemHandle::new(subsys_data.clone());
        Self {
            subsys_data,
            subsys_handle,
        }
    }

    pub async fn start<S: AsyncSubsystem + 'static + Send>(
        self,
        name: &'static str,
        subsystem: S,
    ) -> Self {
        //self.subsys_data.start(name, subsystem);
        SubsystemHandle::new(self.subsys_data.clone())
            .start(name, subsystem)
            .await;

        self
    }

    pub fn catch_signals(self) -> Self {
        let shutdown_token = self.subsys_handle.shutdown_token();

        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.shutdown();
        });

        self
    }

    pub async fn wait_for_shutdown(self, shutdown_timeout: Duration) -> Result<()> {
        self.subsys_handle.on_shutdown_requested().await;

        let result = tokio::select! {
            e = self.subsys_data.perform_shutdown() => e,
            _ = tokio::time::sleep(shutdown_timeout) => Err(anyhow::anyhow!("Submodule shutdown took too long!"))
        };

        // TODO implement error forwarding from submodule errors
        match result {
            Err(e) => {
                log::error!("{:?}", e);
                Err(anyhow::anyhow!("An error occured."))
            }
            Ok(()) => {
                log::debug!("Submodules shut down successfully.");
                Ok(())
            }
        }
    }
}
