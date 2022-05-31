use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::errors::GracefulShutdownError;
use crate::exit_state::prettify_exit_states;
use crate::shutdown_token::create_shutdown_token;
use crate::signal_handling::wait_for_signal;
use crate::utils::wait_forever;
use crate::utils::ShutdownGuard;
use crate::ErrTypeTraits;
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
/// use miette::Result;
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
#[must_use = "This toplevel must be consumed by calling `handle_shutdown_requests` on it."]
pub struct Toplevel<ErrType: ErrTypeTraits = crate::BoxedError> {
    subsys_data: Arc<SubsystemData<ErrType>>,
    subsys_handle: SubsystemHandle<ErrType>,
    shutdown_guard: Option<Arc<ShutdownGuard>>,
}

impl<ErrType: ErrTypeTraits> Toplevel<ErrType> {
    /// Creates a new Toplevel object.
    ///
    /// The Toplevel object is the base for everything else in this crate.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        // On the top-level, the global and local shutdown token are identical
        let global_shutdown_token = create_shutdown_token();
        let group_shutdown_token = global_shutdown_token.clone();
        let local_shutdown_token = group_shutdown_token.clone();
        let cancellation_token = CancellationToken::new();
        let shutdown_guard = Arc::new(ShutdownGuard::new(group_shutdown_token.clone()));

        let subsys_data = Arc::new(SubsystemData::new(
            "",
            global_shutdown_token,
            group_shutdown_token,
            local_shutdown_token,
            cancellation_token,
            Arc::downgrade(&shutdown_guard),
        ));
        let subsys_handle = SubsystemHandle::new(subsys_data.clone());
        Self {
            subsys_data,
            subsys_handle,
            shutdown_guard: Some(shutdown_guard),
        }
    }

    /// Creates a new nested Toplevel object.
    ///
    /// This method is identical to `.new()`, except that the returned [Toplevel] object
    /// will receive shutdown requests from the given [SubsystemHandle] object.
    ///
    /// Any errors that happen at subsystems inside the new [Toplevel] object will cause
    /// the [Toplevel] object to initiate a shutdown, but will not propagate up to the
    /// [SubsystemHandle] object.
    ///
    /// # Arguments
    ///
    /// * `parent` - The subsystemhandle that the [Toplevel] object will receive shutdown
    ///              requests from
    pub fn nested(parent: &SubsystemHandle) -> Self {
        // Take shutdown tokesn from parent
        let global_shutdown_token = parent.global_shutdown_token().clone();
        let group_shutdown_token = parent.local_shutdown_token().child_token();
        let local_shutdown_token = group_shutdown_token.clone();
        let cancellation_token = CancellationToken::new();
        let shutdown_guard = Arc::new(ShutdownGuard::new(group_shutdown_token.clone()));

        let subsys_data = Arc::new(SubsystemData::new(
            "",
            global_shutdown_token,
            group_shutdown_token,
            local_shutdown_token,
            cancellation_token,
            Arc::downgrade(&shutdown_guard),
        ));
        let subsys_handle = SubsystemHandle::new(subsys_data.clone());
        Self {
            subsys_data,
            subsys_handle,
            shutdown_guard: Some(shutdown_guard),
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
    pub fn start<Err, Fut, Subsys>(self, name: &'static str, subsystem: Subsys) -> Self
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        Err: Into<ErrType>,
    {
        self.subsys_handle.start(name, subsystem);

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
    /// - On Unix:
    ///     - SIGINT and SIGTERM
    ///
    /// # Caveats
    ///
    /// This function internally uses [tokio::signal] with all of its caveats.
    ///
    /// Especially the caveats from [tokio::signal::unix::Signal] are important for Unix targets.
    ///
    pub fn catch_signals(self) -> Self {
        let shutdown_token = self.subsys_handle.group_shutdown_token().clone();

        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.shutdown();
        });

        self
    }

    /// Wait for all subsystems to finish.
    /// Then return and print all of their exit codes.
    async fn attempt_clean_shutdown(&self) -> Result<(), GracefulShutdownError<ErrType>> {
        let exit_states = self.subsys_data.perform_shutdown().await;

        // Prettify exit states
        let formatted_exit_states = prettify_exit_states(&exit_states);

        // Collect failed subsystems
        let failed_subsystems = exit_states
            .into_iter()
            .filter_map(|exit_state| exit_state.raw_result.err())
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

    /// Performs a clean program shutdown, once a shutdown is requested or all subsystems have
    /// finished.
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
    pub async fn handle_shutdown_requests<ReturnErrType: From<GracefulShutdownError<ErrType>>>(
        mut self,
        shutdown_timeout: Duration,
    ) -> Result<(), ReturnErrType> {
        // Remove the shutdown guard we hold ourselves, to enable auto-shutdown triggering
        // when all subsystems are finished
        self.shutdown_guard.take();

        self.subsys_handle.on_shutdown_requested().await;

        let timeout_occurred = AtomicBool::new(false);

        let cancel_on_timeout = async {
            // Wait for the timeout to happen
            tokio::time::sleep(shutdown_timeout).await;
            log::error!("Shutdown timed out. Attempting to cleanup stale subsystems ...");
            timeout_occurred.store(true, Ordering::SeqCst);
            self.subsys_data.cancel_all_subsystems();
            // Await forever, because we don't want to cancel the attempt_clean_shutdown.
            // Resolving this arm of the tokio::select would cancel the other side.
            wait_forever().await;
        };

        let result = tokio::select! {
            _ = cancel_on_timeout => unreachable!(),
            result = self.attempt_clean_shutdown() => result
        };

        // Overwrite return value with "ShutdownTimeout" if a timeout occurred
        let result = if timeout_occurred.load(Ordering::SeqCst) {
            Err(GracefulShutdownError::ShutdownTimeout(
                result.err().map_or(vec![], |e| e.into_subsystem_errors()),
            ))
        } else {
            result
        };

        result.map_err(GracefulShutdownError::into)
    }

    #[doc(hidden)]
    pub fn get_shutdown_token(&self) -> &ShutdownToken {
        self.subsys_handle.global_shutdown_token()
    }
}

impl<ErrType: ErrTypeTraits> Drop for Toplevel<ErrType> {
    fn drop(&mut self) {
        self.subsys_data.cancel_all_subsystems();
    }
}
