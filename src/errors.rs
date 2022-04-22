use thiserror::Error;

/// This enum contains all the possible errors that could be returned
/// by [`handle_shutdown_requests()`](crate::Toplevel::handle_shutdown_requests).
#[derive(Error, Debug)]
pub enum GracefulShutdownError {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemFailed,
    /// The shutdown did not finish within the given timeout.
    #[error("shutdown timed out")]
    ShutdownTimeout,
}

/// This enum contains all the possible errors that a partial shutdown
/// could cause.
#[derive(Debug, Error)]
pub enum PartialShutdownError {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemFailed,
    /// The given nested subsystem does not seem to be a child of
    /// the parent subsystem.
    #[error("unable to find nested subsystem in given subsystem")]
    SubsystemNotFound,
    /// A partial shutdown can not be performed because the entire program
    /// is already shutting down.
    #[error("unable to perform partial shutdown, the program is already shutting down")]
    AlreadyShuttingDown,
}
