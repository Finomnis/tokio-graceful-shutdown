use std::sync::atomic::Ordering;

use crate::{errors::SubsystemJoinError, ErrTypeTraits, ErrorAction};

use super::NestedSubsystem;

impl<ErrType: ErrTypeTraits> NestedSubsystem<ErrType> {
    /// Wait for the subsystem to be finished.
    ///
    /// If its failure/panic action is set to [`ErrorAction::CatchAndLocalShutdown`],
    /// this function will return the list of errors caught by the subsystem.
    ///
    /// # Returns
    ///
    /// A [`SubsystemJoinError`] on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::{ErrorAction, SubsystemBuilder, SubsystemHandle};
    ///
    /// async fn nested_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem does nothing but wait for the shutdown to happen
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    ///
    /// async fn subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem waits for one second and then performs a partial shutdown
    ///
    ///     // Spawn nested subsystem.
    ///     // Make sure to catch errors, so that they are properly
    ///     // returned at `.join()`.
    ///     let nested = subsys.start(
    ///         SubsystemBuilder::new("nested", nested_subsystem)
    ///             .on_failure(ErrorAction::CatchAndLocalShutdown)
    ///             .on_panic(ErrorAction::CatchAndLocalShutdown)
    ///     );
    ///
    ///     // Wait for a second
    ///     sleep(Duration::from_millis(1000)).await;
    ///
    ///     // Perform a partial shutdown of the nested subsystem
    ///     nested.initiate_shutdown();
    ///     nested.join().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn join(&self) -> Result<(), SubsystemJoinError<ErrType>> {
        self.joiner.join().await;

        let errors = self.errors.lock().unwrap().finish();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(SubsystemJoinError::SubsystemsFailed(errors))
        }
    }

    /// Signals the subsystem and all of its children to shut down.
    pub fn initiate_shutdown(&self) {
        self.cancellation_token.cancel()
    }

    /// Changes the way this subsystem should react to failures,
    /// meaning if it or one of its children returns an `Err` value.
    ///
    /// For more information, see [ErrorAction].
    pub fn change_failure_action(&self, action: ErrorAction) {
        self.error_actions
            .on_failure
            .store(action, Ordering::Relaxed);
    }

    /// Changes the way this subsystem should react if it or one
    /// of its children panic.
    ///
    /// For more information, see [ErrorAction].
    pub fn change_panic_action(&self, action: ErrorAction) {
        self.error_actions.on_panic.store(action, Ordering::Relaxed);
    }
}
