use anyhow::Result;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{panic, sync::Arc};

use crate::exit_state::prettify_exit_states;
use crate::shutdown_token::create_shutdown_token;
use crate::signal_handling::wait_for_signal;
use crate::{ShutdownToken, SubsystemHandle};

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
/// use tokio::time::{Duration, sleep};
/// use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};
///
/// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
///     subsys.request_shutdown();
///     Ok(())
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create toplevel
///     Toplevel::new()
///         .start("MySubsystem", my_subsystem)
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

impl Drop for Toplevel {
    fn drop(&mut self) {
        // Restore panic hook to its original state
        let _ = panic::take_hook();

        // Unregister the toplevel object to make sure another one can be created in future
        if let Ok(true) =
            TOPLEVEL_EXISTS.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        {
        } else {
            log::error!("Trying to unregister Toplevel module, but there was no toplevel module registered!");
        }
    }
}

static TOPLEVEL_EXISTS: AtomicBool = AtomicBool::new(false);

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
        // Make sure only one toplevel object gets instantiated simultaneously
        let counter =
            TOPLEVEL_EXISTS.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst);
        if let Ok(false) = counter {
        } else {
            panic!("Only one Toplevel object can exist at any given time!");
        }

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

    /// Starts a new subsystem.
    ///
    /// Once called, the subsystem will be started immediately, similar to [`tokio::spawn`].
    ///
    /// # Subsystem
    ///
    /// The functionality of the subsystem is represented by the 'subsystem' argument.
    /// It has to be provided either as an asynchronous function or an asynchronous lambda.
    ///
    /// It gets provided with a [`SubsystemHandle`] object which can be used to interact with this crate.
    ///
    /// ## Returns
    ///
    /// When the subsystem returns `Ok(())` it is assumed that the subsystem was stopped intentionally and no further
    /// actions are performed.
    ///
    /// When the subsystem returns an `Err`, it is assumed that the subsystem failed and a program shutdown gets initiated.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the subsystem
    /// * `subsystem` - The subsystem to be started
    ///
    pub fn start<
        Fut: Future<Output = Result<()>> + Send,
        S: 'static + FnOnce(SubsystemHandle) -> Fut + Send,
    >(
        self,
        name: &'static str,
        subsystem: S,
    ) -> Self {
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

    /// Wait for all subsystems to finish.
    /// Then return and print all of their exit codes.
    async fn attempt_clean_shutdown(&self) -> Result<()> {
        let result = self.subsys_data.perform_shutdown().await;

        // Print subsystem exit states
        let exit_codes = match &result {
            Ok(codes) => {
                log::debug!("Shutdown successful. Subsystem states:");
                codes
            }
            Err(codes) => {
                log::debug!("Some subsystems failed. Subsystem states:");
                codes
            }
        };
        for formatted_exit_code in prettify_exit_states(exit_codes) {
            log::debug!("    {}", formatted_exit_code);
        }

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Subsytem errors occurred.")),
        }
    }

    /// Performs a clean program shutdown, once a shutdown is requested.
    ///
    /// In most cases, this will be the final method of `main()`, as it blocks until system
    /// shutdown and returns an appropriate `Result` that can be directly returned by `main()`.
    ///
    /// When a program shutdown happens, this function collects the return values of all subsystems
    /// to determine the return code of the entire program.
    ///
    /// When the shutdown takes longer than the given timeout, an error will be returned and remaining subsystems
    /// will be cancelled.
    ///
    /// # Arguments
    ///
    /// * `shutdown_timeout` - The maximum time that is allowed to pass after a shutdown was initiated.
    ///
    pub async fn wait_for_shutdown(self, shutdown_timeout: Duration) -> Result<()> {
        self.subsys_handle.on_shutdown_requested().await;

        match tokio::time::timeout(shutdown_timeout, self.attempt_clean_shutdown()).await {
            Ok(val) => val,
            Err(_) => {
                log::error!("Shutdown timed out. Attempting to cleanup stale subsystems ...");
                self.subsys_data.cancel_all_subsystems().await;
                tokio::time::timeout(shutdown_timeout, self.attempt_clean_shutdown()).await?
            }
        }
    }

    #[doc(hidden)]
    pub fn get_shutdown_token(&self) -> &ShutdownToken {
        self.subsys_handle.shutdown_token()
    }
}
