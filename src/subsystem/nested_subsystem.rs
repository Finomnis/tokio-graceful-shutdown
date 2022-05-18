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
    ///     // Perform a partial shutdown of the nested subsystem
    ///     subsys.perform_partial_shutdown(nested).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn request_partial_shutdown(&self) {
        self.local_shutdown_token.shutdown();
    }

    /// Bla
    pub async fn join(self) -> Result<(), SubsystemJoinError<ErrType>> {
        self.parent_data.join_subsystem(self.id).await
    }
}
