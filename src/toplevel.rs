use anyhow::Result;
use std::time::Duration;
use std::{panic, sync::Arc};

use crate::exit_state::prettify_exit_states;
use crate::signal_handling::wait_for_signal;
use crate::SubsystemHandle;
use crate::{shutdown_token::create_shutdown_token, AsyncSubsystem};

use super::subsystem::SubsystemData;

/// Acts as the base for the subsystem tree and forms the entry point for
/// any interaction with this crate.
///
/// Every project that uses this crate has to create a Toplevel object somewhere.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use async_trait::async_trait;
/// use log;
/// use tokio::time::{Duration, sleep};
/// use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle, Toplevel};
///
/// struct MySubsystem {}
///
/// #[async_trait]
/// impl AsyncSubsystem for MySubsystem {
///     async fn run(mut self, subsys: SubsystemHandle) -> Result<()> {
///         subsys.request_shutdown();
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create toplevel
///     Toplevel::new()
///         .start("MySubsystem", MySubsystem {})
///         .catch_signals()
///         .wait_for_shutdown(Duration::from_millis(1000))
///         .await
/// }
/// ```
///
pub struct Toplevel {
    subsys_data: Arc<SubsystemData>,
    subsys_handle: SubsystemHandle,
}

impl Toplevel {
    /// Creates a new Toplevel object.
    ///
    /// The Toplevel object is the base for everything else in this crate.
    ///
    /// During creation, a panic hook is registered to cause a graceful system
    /// shutdown in case a panic happens.
    /// This prevents a program hang that might happen when multithreaded asynchronous
    /// programs experience a panic on one thread.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let shutdown_token = create_shutdown_token();

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

    /// Starts a new subsystem, analogous to `SubsystemHandle::start`.
    ///
    /// Once called, the subsystem will be started immediately, similar to `tokio::spawn`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the subsystem
    /// * `subsystem` - The subsystem to be started
    ///
    pub fn start<S: AsyncSubsystem + 'static + Send>(
        self,
        name: &'static str,
        subsystem: S,
    ) -> Self {
        //self.subsys_data.start(name, subsystem);
        SubsystemHandle::new(self.subsys_data.clone()).start(name, subsystem);

        self
    }

    /// Registers signal handlers to initiate an program shutdown when certain operating system
    /// signals get received.
    ///
    /// The following signals will be handled:
    ///
    /// - On Windows:
    ///     - Ctrl+C (SIGINT)
    ///
    /// - On Linux:
    ///     - SIGINT and SIGTERM
    ///
    pub fn catch_signals(self) -> Self {
        let shutdown_token = self.subsys_handle.shutdown_token().clone();

        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.shutdown();
        });

        self
    }

    /// Waits for the program to be shut down successfully.
    ///
    /// In most cases, this will be the final method of `main()`, as it blocks until system
    /// shutdown and returns an appropriate `Result` that can be directly returned by `main()`.
    ///
    /// When a program shutdown happens, this function collects the return values of all subsystems
    /// to determine the return code of the entire program.
    ///
    /// When the shutdown takes longer than the given timeout, an error will be returned.
    ///
    /// # Arguments
    ///
    /// * `shutdown_timeout` - The maximum time that is allowed to pass after a shutdown was initiated.
    ///
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
