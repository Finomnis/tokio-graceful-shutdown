use anyhow::Result;
use std::time::Duration;
use std::{panic, sync::Arc};

use crate::exit_state::prettify_exit_states;
use crate::signal_handling::wait_for_signal;
use crate::SubsystemHandle;
use crate::{shutdown_token::ShutdownToken, AsyncSubsystem};

use super::subsystem::SubsystemData;

pub struct Toplevel {
    subsys_data: Arc<SubsystemData>,
    subsys_handle: SubsystemHandle,
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

        let subsys_data = Arc::new(SubsystemData::new("", shutdown_token));
        let subsys_handle = SubsystemHandle::new(subsys_data.clone());
        Self {
            subsys_data,
            subsys_handle,
        }
    }

    pub fn start<S: AsyncSubsystem + 'static + Send>(
        self,
        name: &'static str,
        subsystem: S,
    ) -> Self {
        //self.subsys_data.start(name, subsystem);
        SubsystemHandle::new(self.subsys_data.clone()).start(name, subsystem);

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

        tokio::select! {
            e = self.subsys_data.perform_shutdown() => {
                // Print subsystem exit states
                let exit_codes = match &e {
                    Ok(codes) => {
                        log::debug!("Shutdown successful. Subsystem states:");
                        codes
                    },
                    Err(codes) => {
                        log::debug!("Some subsystems failed. Subsystem states:");
                        codes
                    },
                };
                for formatted_exit_code in prettify_exit_states(exit_codes) {
                    log::debug!("    {}", formatted_exit_code);
                }

                match e {
                    Ok(_) => Ok(()),
                    Err(_) => Err(anyhow::anyhow!("Subsytem errors occurred.")),
                }
            },
            _ = tokio::time::sleep(shutdown_timeout) => Err(anyhow::anyhow!("Subsystem shutdown took too long!"))
        }
    }
}
