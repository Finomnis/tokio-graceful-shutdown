//! All the errors that can be caused by this crate.

use std::sync::Arc;

use miette::Diagnostic;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::ErrTypeTraits;

/// This enum contains all the possible errors that could be returned
/// by [`handle_shutdown_requests()`](crate::Toplevel::handle_shutdown_requests).
#[derive(Debug, Error, Diagnostic)]
pub enum GracefulShutdownError<ErrType: ErrTypeTraits = crate::BoxedError> {
    /// At least one subsystem caused an error.
    #[diagnostic(code(graceful_shutdown::failed))]
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Box<[SubsystemError<ErrType>]>),
    /// The shutdown did not finish within the given timeout.
    #[diagnostic(code(graceful_shutdown::timeout))]
    #[error("shutdown timed out")]
    ShutdownTimeout(#[related] Box<[SubsystemError<ErrType>]>),
}

impl<ErrType: ErrTypeTraits> GracefulShutdownError<ErrType> {
    /// Converts the error into a list of subsystem errors that occurred.
    pub fn into_subsystem_errors(self) -> Box<[SubsystemError<ErrType>]> {
        match self {
            GracefulShutdownError::SubsystemsFailed(rel) => rel,
            GracefulShutdownError::ShutdownTimeout(rel) => rel,
        }
    }
    /// Queries the list of subsystem errors that occurred.
    pub fn get_subsystem_errors(&self) -> &[SubsystemError<ErrType>] {
        match self {
            GracefulShutdownError::SubsystemsFailed(rel) => rel,
            GracefulShutdownError::ShutdownTimeout(rel) => rel,
        }
    }
}

/// This enum contains all the possible errors that joining a subsystem
/// could cause.
#[derive(Debug, Error, Diagnostic)]
pub enum SubsystemJoinError<ErrType: ErrTypeTraits = crate::BoxedError> {
    /// At least one subsystem caused an error.
    #[diagnostic(code(graceful_shutdown::subsystem_join::failed))]
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Arc<[SubsystemError<ErrType>]>),
}

/// A wrapper type that carries the errors returned by subsystems.
pub struct SubsystemFailure<ErrType>(pub(crate) ErrType);

impl<ErrType> std::ops::Deref for SubsystemFailure<ErrType> {
    type Target = ErrType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<ErrType> SubsystemFailure<ErrType> {
    /// Retrieves the containing error.
    pub fn get_error(&self) -> &ErrType {
        &self.0
    }
    /// Converts the object into the containing error.
    pub fn into_error(self) -> ErrType {
        self.0
    }
}

impl<ErrType> std::fmt::Debug for SubsystemFailure<ErrType>
where
    ErrType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
impl<ErrType> std::fmt::Display for SubsystemFailure<ErrType>
where
    ErrType: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl<ErrType> std::error::Error for SubsystemFailure<ErrType> where
    ErrType: std::fmt::Display + std::fmt::Debug
{
}

/// This enum contains all the possible errors that a subsystem execution
/// could cause.
///
/// Every error carries the name of the subsystem as the first argument.
#[derive(Debug, Error, Diagnostic)]
pub enum SubsystemError<ErrType: ErrTypeTraits = crate::BoxedError> {
    /// The subsystem returned an error value. Carries the actual error as the second argument.
    #[diagnostic(code(graceful_shutdown::subsystem::failed))]
    #[error("Error in subsystem '{0}'")]
    Failed(Arc<str>, #[source] SubsystemFailure<ErrType>),
    /// The subsystem panicked.
    #[diagnostic(code(graceful_shutdown::subsystem::panicked))]
    #[error("Subsystem '{0}' panicked")]
    Panicked(Arc<str>),
}

impl<ErrType: ErrTypeTraits> SubsystemError<ErrType> {
    /// Retrieves the name of the subsystem that caused the error.
    ///
    /// # Returns
    ///
    /// The name of the subsystem
    pub fn name(&self) -> &str {
        match self {
            SubsystemError::Failed(name, _) => name,
            SubsystemError::Panicked(name) => name,
        }
    }
}

/// The error that happens when a task gets cancelled through
/// [`cancel_on_shutdown()`](crate::FutureExt::cancel_on_shutdown).
#[derive(Error, Debug, Diagnostic)]
#[error("A shutdown request caused this task to be cancelled")]
#[diagnostic(code(graceful_shutdown::future::cancelled_by_shutdown))]
pub struct CancelledByShutdown;

// This function contains code that stems from the principle
// of defensive coding - meaning, handle potential errors
// gracefully, even if they should not happen.
// Therefore it is in this special function, so we don't
// get coverage problems.
pub(crate) fn handle_dropped_error<ErrType: ErrTypeTraits>(
    result: Result<(), mpsc::error::SendError<ErrType>>,
) {
    if let Err(mpsc::error::SendError(e)) = result {
        tracing::warn!("An error got dropped: {e:?}");
    }
}

// This function contains code that stems from the principle
// of defensive coding - meaning, handle potential errors
// gracefully, even if they should not happen.
// Therefore it is in this special function, so we don't
// get coverage problems.
pub(crate) fn handle_unhandled_stopreason<ErrType: ErrTypeTraits>(
    maybe_stop_reason: Option<SubsystemError<ErrType>>,
) {
    if let Some(stop_reason) = maybe_stop_reason {
        tracing::warn!("Unhandled stop reason: {:?}", stop_reason);
    }
}

#[cfg(test)]
mod tests;
