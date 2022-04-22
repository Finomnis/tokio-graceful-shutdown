use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::exit_state::prettify_exit_states;
use crate::shutdown_token::create_shutdown_token;
use crate::signal_handling::wait_for_signal;
use crate::BoxedError;
use crate::GracefulShutdownError;
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
/// use tokio::time::Duration;
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
///         .handle_shutdown_requests(Duration::from_millis(1000))
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
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        // On the top-level, the global and local shutdown token are identical
        let global_shutdown_token = create_shutdown_token();
        let local_shutdown_token = global_shutdown_token.clone();

        let subsys_data = Arc::new(SubsystemData::new(
            "",
            global_shutdown_token,
            local_shutdown_token,
        ));
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
        Err: Into<BoxedError>,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
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
        let shutdown_token = self.subsys_handle.global_shutdown_token().clone();

        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.shutdown();
        });

        self
    }

    /// Wait for all subsystems to finish.
    /// Then return and print all of their exit codes.
    async fn attempt_clean_shutdown(&self) -> Result<(), GracefulShutdownError> {
        let exit_states = self.subsys_data.perform_shutdown().await;

        // Prettify exit states
        let formatted_exit_states = prettify_exit_states(&exit_states);

        // Collect failed subsystems
        let failed_subsystems = exit_states
            .into_iter()
            .filter_map(|exit_state| match exit_state.raw_result {
                Ok(()) => None,
                Err(e) => Some(e),
            })
            .collect::<Vec<_>>();

        // Print subsystem exit states
        if failed_subsystems.is_empty() {
            log::debug!("Shutdown successful. Subsystem states:");
        } else {
            log::debug!("Some subsystems failed. Subsystem states:");
        };
        for formatted_exit_state in formatted_exit_states {
            log::debug!("    {}", formatted_exit_state);
        }

        if failed_subsystems.is_empty() {
            Ok(())
        } else {
            Err(GracefulShutdownError::SubsystemsFailed(failed_subsystems))
        }
    }

    /// Performs a clean program shutdown, once a shutdown is requested.
    ///
    /// In most cases, this will be the final method of `main()`, as it blocks until program
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
    /// # Returns
    ///
    /// An error of type [`GracefulShutdownError`] if an error occurred.
    /// An implicit `.into()` will be performed to convert it to the desired error wrapping type.
    ///
    pub async fn handle_shutdown_requests<ErrType: From<GracefulShutdownError>>(
        self,
        shutdown_timeout: Duration,
    ) -> Result<(), ErrType> {
        self.subsys_handle.on_shutdown_requested().await;

        match tokio::time::timeout(shutdown_timeout, self.attempt_clean_shutdown()).await {
            Ok(val) => val,
            Err(_) => {
                log::error!("Shutdown timed out. Attempting to cleanup stale subsystems ...");
                self.subsys_data.cancel_all_subsystems().await;
                tokio::time::timeout(shutdown_timeout, self.attempt_clean_shutdown())
                    .await
                    .or(Err(GracefulShutdownError::ShutdownTimeout))? // Happens if second shutdown times out as well
                    .or(Err(GracefulShutdownError::ShutdownTimeout)) // Happens in all other cases, as cancellation always forces an error in cancelled subsystems
            }
        }
        .map_err(GracefulShutdownError::into)
    }

    #[doc(hidden)]
    pub fn get_shutdown_token(&self) -> &ShutdownToken {
        self.subsys_handle.global_shutdown_token()
    }
}
