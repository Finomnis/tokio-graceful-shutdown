use miette::Diagnostic;
use thiserror::Error;

use crate::BoxedError;

/// This enum contains all the possible errors that could be returned
/// by [`handle_shutdown_requests()`](crate::Toplevel::handle_shutdown_requests).
#[derive(Error, Debug, Diagnostic)]
pub enum GracefulShutdownError {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Vec<SubsystemError>),
    /// The shutdown did not finish within the given timeout.
    #[error("shutdown timed out")]
    ShutdownTimeout(#[related] Vec<SubsystemError>),
}

/// This enum contains all the possible errors that a partial shutdown
/// could cause.
#[derive(Debug, Error, Diagnostic)]
pub enum PartialShutdownError {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Vec<SubsystemError>),
    /// The given nested subsystem does not seem to be a child of
    /// the parent subsystem.
    #[error("unable to find nested subsystem in given subsystem")]
    SubsystemNotFound,
    /// A partial shutdown can not be performed because the entire program
    /// is already shutting down.
    #[error("unable to perform partial shutdown, the program is already shutting down")]
    AlreadyShuttingDown,
}

/// This enum contains all the possible errors that a subsystem execution
/// could cause.
///
/// Every error carries the name of the subsystem as the first argument.
#[derive(Debug, Error, Diagnostic)]
pub enum SubsystemError {
    /// The subsystem returned an error value. Carries the actual error as the second argument.
    #[error("Error in subsystem '{0}'")]
    Failed(String, #[source] BoxedError),
    /// The subsystem was cancelled. Should only happen if the shutdown timeout is exceeded.
    #[error("Subsystem '{0}' was aborted")]
    Cancelled(String),
    /// The subsystem panicked.
    #[error("Subsystem '{0}' panicked")]
    Panicked(String),
}

impl SubsystemError {
    /// Retrieves the name of the subsystem that caused the error.
    ///
    /// # Returns
    ///
    /// The name of the subsystem
    pub fn name(&self) -> &str {
        match self {
            SubsystemError::Failed(name, _) => name,
            SubsystemError::Cancelled(name) => name,
            SubsystemError::Panicked(name) => name,
        }
    }
}
