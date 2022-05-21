use crate::{errors::SubsystemJoinError, ErrTypeTraits, NestedSubsystem};

impl<ErrType: ErrTypeTraits> NestedSubsystem<ErrType> {
    /// Initiates a partial shutdown of the nested subsystem.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
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
    ///     // Spawn nested subsystem
    ///     let nested = subsys.start("nested", nested_subsystem);
    ///
    ///     // Wait for a second
    ///     sleep(Duration::from_millis(1000)).await;
    ///
    ///     // Trigger a partial shutdown of the nested subsystem
    ///     nested.request_partial_shutdown();
    ///
    ///     // Wait until the subsystem is finished shutting down
    ///     nested.join().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn request_partial_shutdown(&self) {
        self.data.local_shutdown_token.shutdown();
    }

    /// Takes ownership of the error path of the subsystem and waits for its completion.
    ///
    /// # Caveats
    ///
    /// This will redirect the error propagation to this function.
    /// Even when this function is cancelled, errors of this subsystem will no longer be
    /// received by parent subsystems or the [Toplevel](crate::Toplevel) object.
    ///
    /// # Cancellation
    ///
    /// Cancelling this function will cancel the subsystem tree.
    ///
    pub async fn join(self) -> Result<(), SubsystemJoinError<ErrType>> {
        let cancellation_guard = self.data.cancellation_token.clone().drop_guard();
        let result = self.parent_data.clone().join_subsystem(self).await;
        cancellation_guard.disarm();
        result
    }
}
