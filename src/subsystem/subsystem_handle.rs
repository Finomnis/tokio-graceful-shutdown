use std::{
    future::Future,
    mem::ManuallyDrop,
    sync::{atomic::Ordering, Arc, Mutex},
};

use atomic::Atomic;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    errors::{handle_dropped_error, SubsystemError},
    runner::{AliveGuard, SubsystemRunner},
    utils::{remote_drop_collection::RemotelyDroppableItems, JoinerToken},
    BoxedError, ErrTypeTraits, ErrorAction, NestedSubsystem, SubsystemBuilder,
};

use super::{error_collector::ErrorCollector, ErrorActions};

struct Inner<ErrType: ErrTypeTraits> {
    name: Arc<str>,
    cancellation_token: CancellationToken,
    toplevel_cancellation_token: CancellationToken,
    joiner_token: JoinerToken<ErrType>,
    children: RemotelyDroppableItems<SubsystemRunner>,
}

/// The handle given to each subsystem through which the subsystem can interact with this crate.
pub struct SubsystemHandle<ErrType: ErrTypeTraits = BoxedError> {
    inner: ManuallyDrop<Inner<ErrType>>,
    // When dropped, redirect Self into this channel.
    // Required as a workaround for https://stackoverflow.com/questions/77172947/async-lifetime-issues-of-pass-by-reference-parameters.
    drop_redirect: Option<oneshot::Sender<WeakSubsystemHandle<ErrType>>>,
}

pub(crate) struct WeakSubsystemHandle<ErrType: ErrTypeTraits> {
    pub(crate) joiner_token: JoinerToken<ErrType>,
    // Children are stored here to keep them alive
    _children: RemotelyDroppableItems<SubsystemRunner>,
}

impl<ErrType: ErrTypeTraits> SubsystemHandle<ErrType> {
    /// Start a nested subsystem.
    ///
    /// Once called, the subsystem will be started immediately, similar to [`tokio::spawn`].
    ///
    /// # Arguments
    ///
    /// * `builder` - The [`SubsystemBuilder`] that contains all the information
    ///               about the subsystem that should be spawned.
    ///
    /// # Returns
    ///
    /// A [`NestedSubsystem`] that can be used to control or join the subsystem.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle};
    ///
    /// async fn nested_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    ///
    /// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // start a nested subsystem
    ///     subsys.start(SubsystemBuilder::new("Nested", nested_subsystem));
    ///
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    /// ```
    #[track_caller]
    pub fn start<Err, Fut, Subsys>(
        &self,
        builder: SubsystemBuilder<ErrType, Err, Fut, Subsys>,
    ) -> NestedSubsystem<ErrType>
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        Err: Into<ErrType>,
    {
        self.start_with_abs_name(
            if self.inner.name.as_ref() == "/" {
                Arc::from(format!("/{}", builder.name))
            } else {
                Arc::from(format!("{}/{}", self.inner.name, builder.name))
            },
            builder.subsystem,
            ErrorActions {
                on_failure: Atomic::new(builder.failure_action),
                on_panic: Atomic::new(builder.panic_action),
            },
            builder.detached,
        )
    }

    #[track_caller]
    pub(crate) fn start_with_abs_name<Err, Fut, Subsys>(
        &self,
        name: Arc<str>,
        subsystem: Subsys,
        error_actions: ErrorActions,
        detached: bool,
    ) -> NestedSubsystem<ErrType>
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        Err: Into<ErrType>,
    {
        let alive_guard = AliveGuard::new();

        let (error_sender, errors) = mpsc::unbounded_channel();

        let cancellation_token = if detached {
            CancellationToken::new()
        } else {
            self.inner.cancellation_token.child_token()
        };

        let error_actions = Arc::new(error_actions);

        let (joiner_token, joiner_token_ref) = self.inner.joiner_token.child_token({
            let cancellation_token = cancellation_token.clone();
            let error_actions = Arc::clone(&error_actions);
            move |e| {
                let error_action = match &e {
                    SubsystemError::Failed(_, _) => {
                        error_actions.on_failure.load(Ordering::Relaxed)
                    }
                    SubsystemError::Panicked(_) => error_actions.on_panic.load(Ordering::Relaxed),
                };

                match error_action {
                    ErrorAction::Forward => Some(e),
                    ErrorAction::CatchAndLocalShutdown => {
                        handle_dropped_error(error_sender.send(e));
                        cancellation_token.cancel();
                        None
                    }
                }
            }
        });

        let child_handle = SubsystemHandle {
            inner: ManuallyDrop::new(Inner {
                name: Arc::clone(&name),
                cancellation_token: cancellation_token.clone(),
                toplevel_cancellation_token: self.inner.toplevel_cancellation_token.clone(),
                joiner_token,
                children: RemotelyDroppableItems::new(),
            }),
            drop_redirect: None,
        };

        let runner = SubsystemRunner::new(name, subsystem, child_handle, alive_guard.clone());

        // Shenanigans to juggle child ownership
        //
        // RACE CONDITION SAFETY:
        // If the subsystem ends before `on_finished` was able to be called, nothing bad happens.
        // alive_guard will keep the guard alive and the callback will only be called inside of
        // the guard's drop() implementation.
        let child_dropper = self.inner.children.insert(runner);
        alive_guard.on_finished(|| {
            drop(child_dropper);
        });

        NestedSubsystem {
            joiner: joiner_token_ref,
            cancellation_token,
            errors: Mutex::new(ErrorCollector::new(errors)),
            error_actions,
        }
    }

    /// Waits until all the children of this subsystem are finished.
    pub async fn wait_for_children(&self) {
        self.inner.joiner_token.join_children().await
    }

    // For internal use only - should never be used by users.
    // Required as a short-lived second reference inside of `runner`.
    pub(crate) fn delayed_clone(&mut self) -> oneshot::Receiver<WeakSubsystemHandle<ErrType>> {
        let (sender, receiver) = oneshot::channel();

        let previous = self.drop_redirect.replace(sender);
        assert!(previous.is_none());

        receiver
    }

    /// Wait for the shutdown mode to be triggered.
    ///
    /// Once the shutdown mode is entered, all existing calls to this
    /// method will be released and future calls to this method will
    /// return immediately.
    ///
    /// This is the primary method of subsystems to react to
    /// the shutdown requests. Most often, it will be used in [`tokio::select`]
    /// statements to cancel other code as soon as the shutdown is requested.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn countdown() {
    ///     for i in (1..10).rev() {
    ///         tracing::info!("Countdown: {}", i);
    ///         sleep(Duration::from_millis(1000)).await;
    ///     }
    /// }
    ///
    /// async fn countdown_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     tracing::info!("Starting countdown ...");
    ///
    ///     // This cancels the countdown as soon as shutdown
    ///     // mode was entered
    ///     tokio::select! {
    ///         _ = subsys.on_shutdown_requested() => {
    ///             tracing::info!("Countdown cancelled.");
    ///         },
    ///         _ = countdown() => {
    ///             tracing::info!("Countdown finished.");
    ///         }
    ///     };
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn on_shutdown_requested(&self) {
        self.inner.cancellation_token.cancelled().await
    }

    /// Returns whether a shutdown should be performed now.
    ///
    /// This method is provided for subsystems that need to query the shutdown
    /// request state repeatedly.
    ///
    /// This can be useful in scenarios where a subsystem depends on the graceful
    /// shutdown of its nested coroutines before it can run final cleanup steps itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn uncancellable_action(subsys: &SubsystemHandle) {
    ///     tokio::select! {
    ///         // Execute an action. A dummy `sleep` in this case.
    ///         _ = sleep(Duration::from_millis(1000)) => {
    ///             tracing::info!("Action finished.");
    ///         }
    ///         // Perform a shutdown if requested
    ///         _ = subsys.on_shutdown_requested() => {
    ///             tracing::info!("Action aborted.");
    ///         },
    ///     }
    /// }
    ///
    /// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     tracing::info!("Starting subsystem ...");
    ///
    ///     // We cannot do a `tokio::select` with `on_shutdown_requested`
    ///     // here, because a shutdown would cancel the action without giving
    ///     // it the chance to react first.
    ///     while !subsys.is_shutdown_requested() {
    ///         uncancellable_action(&subsys).await;
    ///     }
    ///
    ///     tracing::info!("Subsystem stopped.");
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn is_shutdown_requested(&self) -> bool {
        self.inner.cancellation_token.is_cancelled()
    }

    /// Triggers a shutdown of the entire subsystem tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn stop_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem wait for one second and then stops the program.
    ///     sleep(Duration::from_millis(1000)).await;
    ///
    ///     // Shut down the entire subsystem tree
    ///     subsys.request_shutdown();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn request_shutdown(&self) {
        self.inner.toplevel_cancellation_token.cancel();
    }

    /// Triggers a shutdown of the current subsystem and all
    /// of its children.
    pub fn request_local_shutdown(&self) {
        self.inner.cancellation_token.cancel();
    }

    pub(crate) fn get_cancellation_token(&self) -> &CancellationToken {
        &self.inner.cancellation_token
    }

    /// Creates a cancellation token that will get triggered once the
    /// subsystem shuts down.
    ///
    /// This is intended for more lightweight situations where
    /// creating full-blown subsystems would be too much overhead,
    /// like spawning connection handlers of a webserver.
    ///
    /// For more information, see the [hyper example](https://github.com/Finomnis/tokio-graceful-shutdown/blob/main/examples/hyper.rs).
    pub fn create_cancellation_token(&self) -> CancellationToken {
        self.inner.cancellation_token.child_token()
    }

    /// Get the name associated with this subsystem.
    ///
    /// See [`SubsystemBuilder::new()`] how to set this name.
    pub fn name(&self) -> &str {
        &self.inner.name
    }
}

impl<ErrType: ErrTypeTraits> Drop for SubsystemHandle<ErrType> {
    fn drop(&mut self) {
        // SAFETY: This is how ManuallyDrop is meant to be used.
        // `self.inner` won't ever be used again because `self` will be gone after this
        // function is finished.
        // This takes the `self.inner` object and makes it droppable again.
        //
        // This workaround is required to take ownership for the `self.drop_redirect` channel.
        let inner = unsafe { ManuallyDrop::take(&mut self.inner) };

        if let Some(redirect) = self.drop_redirect.take() {
            let redirected_self = WeakSubsystemHandle {
                joiner_token: inner.joiner_token,
                _children: inner.children,
            };

            // ignore error; an error would indicate that there is no receiver.
            // in that case, do nothing.
            let _ = redirect.send(redirected_self);
        }
    }
}

pub(crate) fn root_handle<ErrType: ErrTypeTraits>(
    on_error: impl Fn(SubsystemError<ErrType>) + Sync + Send + 'static,
) -> SubsystemHandle<ErrType> {
    let cancellation_token = CancellationToken::new();

    SubsystemHandle {
        inner: ManuallyDrop::new(Inner {
            name: Arc::from(""),
            cancellation_token: cancellation_token.clone(),
            toplevel_cancellation_token: cancellation_token.clone(),
            joiner_token: JoinerToken::new(move |e| {
                on_error(e);
                cancellation_token.cancel();
                None
            })
            .0,
            children: RemotelyDroppableItems::new(),
        }),
        drop_redirect: None,
    }
}

#[cfg(test)]
mod tests;
