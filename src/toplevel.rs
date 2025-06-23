use atomic::Atomic;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    BoxedError, DefaultShutdownHooks, DefaultSignalHooks, ErrTypeTraits, ErrorAction,
    NestedSubsystem, ShutdownHooks, SignalHooks, SubsystemHandle, default_on_subsystem_cancelled,
    default_on_subsystem_error,
    errors::{GracefulShutdownError, SubsystemError, handle_dropped_error},
    signal_handling::wait_for_signal,
    subsystem::{self, ErrorActions},
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
///     Toplevel::new(async |s| {
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
    /// This constructor uses the default hooks for error logging and cancellation warnings. It
    /// logs uncaught errors using `tracing::error!` and root subsystem cancellations using
    /// `tracing::warn!`.
    ///
    /// For more advanced error handling, like sending alerts to a monitoring service, see
    /// [`Self::new_with_hooks`].
    ///
    /// # Arguments
    ///
    /// * `subsystem` - The subsystem that should be spawned as the root node.
    ///                 Usually the job of this subsystem is to spawn further subsystems.
    #[allow(clippy::new_without_default)]
    #[track_caller]
    pub fn new<Fut, Subsys>(subsystem: Subsys) -> Self
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = ()> + Send,
    {
        Self::new_with_hooks(
            subsystem,
            default_on_subsystem_error,
            default_on_subsystem_cancelled,
        )
    }

    /// Creates a new Toplevel object with custom hooks for handling fatal errors and root
    /// cancellation.
    ///
    /// This is an advanced version of [`Self::new`]. It allows providing custom callbacks for two
    /// key events:
    ///
    /// 1.  An uncaught error/panic bubbling up to the top level. This is useful for immediate alerting.
    /// 2.  The cancellation of the root subsystem itself.
    ///
    /// After the error handling hook is executed, a global shutdown is initiated. The error is then
    /// collected and will be part of the final `Result` returned by [`Self::handle_shutdown_requests`].
    ///
    /// # Arguments
    ///
    /// * `subsystem` - The subsystem that should be spawned as the root node.
    ///                 Usually the job of this subsystem is to spawn further subsystems.
    /// * `on_subsystem_error` - A closure or function that will be called with a reference
    ///                          to the [`SubsystemError`] that caused the shutdown.
    ///                          This hook is executed immediately when an uncaught error
    ///                          reaches the top level.
    /// * `on_subsystem_cancelled` - A closure or function that will be called if the root subsystem
    ///                              itself is cancelled.
    #[track_caller]
    pub fn new_with_hooks<Fut, Subsys, OnSubsysErr, OnSubsysCancelled>(
        subsystem: Subsys,
        on_subsystem_error: OnSubsysErr,
        on_subsystem_cancelled: OnSubsysCancelled,
    ) -> Self
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = ()> + Send,
        OnSubsysErr: Fn(&SubsystemError<ErrType>) + Send + Sync + 'static,
        OnSubsysCancelled: FnOnce(Arc<str>) + Send + 'static,
    {
        let (error_sender, errors) = mpsc::unbounded_channel();

        let root_handle = subsystem::root_handle(move |e| {
            on_subsystem_error(&e);
            handle_dropped_error(error_sender.send(e));
        });

        let toplevel_subsys = root_handle.start_with_abs_name(
            Arc::from("/"),
            async |s| {
                subsystem(s).await;
                Result::<(), ErrType>::Ok(())
            },
            ErrorActions {
                on_failure: Atomic::new(ErrorAction::Forward),
                on_panic: Atomic::new(ErrorAction::Forward),
            },
            false,
            on_subsystem_cancelled,
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
    ///     - `CTRL_C`
    ///     - `CTRL_BREAK`
    ///     - `CTRL_CLOSE`
    ///     - `CTRL_SHUTDOWN`
    ///
    /// - On Unix:
    ///     - `SIGINT`
    ///     - `SIGTERM`
    ///
    /// This method uses default hooks that log the received signal. For more control, see
    /// [`Self::catch_signals_with_hooks`].
    ///
    /// # Caveats
    ///
    /// This function internally uses [`tokio::signal`] with all of its caveats.
    ///
    /// Especially the caveats from [`tokio::signal::unix::Signal`] are important for Unix targets.
    #[track_caller]
    pub fn catch_signals(self) -> Self {
        self.catch_signals_with_hooks(DefaultSignalHooks)
    }

    /// Registers signal handlers with custom hooks to initiate a program shutdown.
    ///
    /// This is an advanced version of [`Self::catch_signals`]. It allows you to provide a custom
    /// implementation of the [`SignalHooks`] trait to execute code when a specific OS signal is
    /// received. This is useful for applications that need to react differently to `SIGINT`
    /// (Ctrl+C) versus `SIGTERM`.
    ///
    /// See [`Self::catch_signals`] for a list of handled signals and other caveats.
    ///
    /// # Arguments
    ///
    /// * `hooks` - An object that implements the [`SignalHooks`] trait.
    #[track_caller]
    pub fn catch_signals_with_hooks(self, hooks: impl SignalHooks) -> Self {
        let shutdown_token = self.root_handle.get_cancellation_token().clone();

        crate::tokio_task::spawn(
            async move {
                wait_for_signal(hooks).await;
                shutdown_token.cancel();
            },
            "catch_signals",
        );

        self
    }

    /// Performs a clean program shutdown with custom hooks.
    ///
    /// This is an advanced version of [`Self::handle_shutdown_requests`]. It allows you to provide
    /// a custom implementation of the [`ShutdownHooks`] trait to execute code at different stages
    /// of the shutdown process.
    ///
    /// In most cases, this will be the final method of `main()`, as it blocks until program
    /// shutdown and returns an appropriate `Result` that can be directly returned by `main()`.
    ///
    /// # Arguments
    ///
    /// * `shutdown_timeout` - The maximum time that is allowed to pass after a shutdown was initiated.
    /// * `hooks` - An object that implements the [`ShutdownHooks`] trait.
    ///
    /// # Returns
    ///
    /// An error of type [`GracefulShutdownError`] if an error occurred.
    pub async fn handle_shutdown_requests_with_hooks(
        mut self,
        shutdown_timeout: Duration,
        mut hooks: impl ShutdownHooks,
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
                hooks.on_subsystems_finished().await;

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
                hooks.on_shutdown_requested().await;
            }
        );

        match tokio::time::timeout(shutdown_timeout, self.toplevel_subsys.join()).await {
            Ok(result) => {
                // An `Err` here would indicate a programming error,
                // because the toplevel subsys doesn't catch any errors;
                // it only forwards them.
                assert!(result.is_ok());

                let errors = collect_errors();
                hooks.on_shutdown_finished(&errors).await;
                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(GracefulShutdownError::SubsystemsFailed(errors))
                }
            }
            Err(_) => {
                hooks.on_shutdown_timeout().await;
                Err(GracefulShutdownError::ShutdownTimeout(collect_errors()))
            }
        }
    }

    /// Performs a clean program shutdown, once a shutdown is requested or all subsystems have
    /// finished.
    ///
    /// This function uses the default shutdown hooks which log shutdown-related events. For more
    /// control, see [`Self::handle_shutdown_requests_with_hooks`].
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
        self,
        shutdown_timeout: Duration,
    ) -> Result<(), GracefulShutdownError<ErrType>> {
        self.handle_shutdown_requests_with_hooks(shutdown_timeout, DefaultShutdownHooks)
            .await
    }

    #[doc(hidden)]
    // Only for unit tests; not intended for public use
    pub fn _get_shutdown_token(&self) -> &CancellationToken {
        self.root_handle.get_cancellation_token()
    }
}
