use std::{future::Future, sync::Arc, time::Duration};

use atomic::Atomic;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    errors::{GracefulShutdownError, SubsystemError},
    signal_handling::wait_for_signal,
    subsystem::{self, ErrorActions},
    BoxedError, ErrTypeTraits, ErrorAction, NestedSubsystem, SubsystemHandle,
};

/// Acts as the root of the subsystem tree and forms the entry point for
/// any interaction with this crate.
///
/// Every project that uses this crate has to create a [`Toplevel`] object somewhere.
///
/// # Examples
///
/// ```
/// use miette::Result;
/// use tokio::time::Duration;
/// use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
///
/// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
///     subsys.request_shutdown();
///     Ok(())
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     Toplevel::new(|s| async move {
///         s.start(SubsystemBuilder::new("MySubsystem", my_subsystem));
///     })
///     .catch_signals()
///     .handle_shutdown_requests(Duration::from_millis(1000))
///     .await
///     .map_err(Into::into)
/// }
/// ```
///
#[must_use = "This toplevel must be consumed by calling `handle_shutdown_requests` on it."]
pub struct Toplevel<ErrType: ErrTypeTraits = BoxedError> {
    root_handle: SubsystemHandle<ErrType>,
    toplevel_subsys: NestedSubsystem<ErrType>,
    errors: mpsc::UnboundedReceiver<SubsystemError<ErrType>>,
}

impl<ErrType: ErrTypeTraits> Toplevel<ErrType> {
    /// Creates a new Toplevel object.
    ///
    /// The Toplevel object is the base for everything else in this crate.
    ///
    /// # Arguments
    ///
    /// * `subsystem` - The subsystem that should be spawned as the root node.
    ///                 Usually the job of this subsystem is to spawn further subsystems.
    #[allow(clippy::new_without_default)]
    pub fn new<Fut, Subsys>(subsystem: Subsys) -> Self
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = ()> + Send,
    {
        let (error_sender, errors) = mpsc::unbounded_channel();

        let root_handle = subsystem::root_handle(move |e| {
            match &e {
                SubsystemError::Panicked(name) => {
                    tracing::error!("Uncaught panic from subsytem '{name}'.")
                }
                SubsystemError::Failed(name, e) => {
                    tracing::error!("Uncaught error from subsystem '{name}': {e}",)
                }
            };

            if let Err(mpsc::error::SendError(e)) = error_sender.send(e) {
                tracing::warn!("An error got dropped: {e:?}");
            };
        });

        let toplevel_subsys = root_handle.start_with_abs_name(
            Arc::from(""),
            move |s| async move {
                subsystem(s).await;
                Result::<(), ErrType>::Ok(())
            },
            ErrorActions {
                on_failure: Atomic::new(ErrorAction::Forward),
                on_panic: Atomic::new(ErrorAction::Forward),
            },
        );

        Self {
            root_handle,
            toplevel_subsys,
            errors,
        }
    }

    /// Registers signal handlers to initiate a program shutdown when certain operating system
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
        let shutdown_token = self.root_handle.get_cancellation_token().clone();

        tokio::spawn(async move {
            wait_for_signal().await;
            shutdown_token.cancel();
        });

        self
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
    ///
    pub async fn handle_shutdown_requests(
        mut self,
        shutdown_timeout: Duration,
    ) -> Result<(), GracefulShutdownError<ErrType>> {
        let collect_errors = move || {
            let mut errors = vec![];
            self.errors.close();
            while let Ok(e) = self.errors.try_recv() {
                errors.push(e);
            }
            drop(self.errors);
            errors.into_boxed_slice()
        };

        tokio::select!(
            _ = self.toplevel_subsys.join() => {
                tracing::info!("All subsystems finished.");

                // Not really necessary, but for good measure.
                self.root_handle.request_shutdown();

                let errors = collect_errors();
                let result = if errors.is_empty() {
                    Ok(())
                } else {
                    Err(GracefulShutdownError::SubsystemsFailed(errors))
                };
                return result;
            },
            _ = self.root_handle.on_shutdown_requested() => {
                tracing::info!("Shutting down ...");
            }
        );

        match tokio::time::timeout(shutdown_timeout, self.toplevel_subsys.join()).await {
            Ok(result) => {
                // An `Err` here would indicate a programming error,
                // because the toplevel subsys doesn't catch any errors;
                // it only forwards them.
                assert!(result.is_ok());

                let errors = collect_errors();
                if errors.is_empty() {
                    tracing::info!("Shutdown finished.");
                    Ok(())
                } else {
                    tracing::warn!("Shutdown finished with errors.");
                    Err(GracefulShutdownError::SubsystemsFailed(errors))
                }
            }
            Err(_) => {
                tracing::error!("Shutdown timed out!");
                Err(GracefulShutdownError::ShutdownTimeout(collect_errors()))
            }
        }
    }

    #[doc(hidden)]
    // Only for unit tests; not intended for public use
    pub fn _get_shutdown_token(&self) -> &CancellationToken {
        self.root_handle.get_cancellation_token()
    }
}
