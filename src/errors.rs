//! All the errors that can be caused by this crate.

use miette::Diagnostic;
use thiserror::Error;

use crate::ErrTypeTraits;

/// This enum contains all the possible errors that could be returned
/// by [`handle_shutdown_requests()`](crate::Toplevel::handle_shutdown_requests).
#[derive(Error, Debug, Diagnostic)]
pub enum GracefulShutdownError<ErrType: ErrTypeTraits = crate::BoxedError> {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Vec<SubsystemError<ErrType>>),
    /// The shutdown did not finish within the given timeout.
    #[error("shutdown timed out")]
    ShutdownTimeout(#[related] Vec<SubsystemError<ErrType>>),
}

impl<ErrType: ErrTypeTraits> GracefulShutdownError<ErrType> {
    /// Converts the error into a list of subsystem errors that occurred.
    pub fn into_subsystem_errors(self) -> Vec<SubsystemError<ErrType>> {
        match self {
            GracefulShutdownError::SubsystemsFailed(rel) => rel,
            GracefulShutdownError::ShutdownTimeout(rel) => rel,
        }
    }
    /// Queries the list of subsystem errors that occurred.
    pub fn get_subsystem_errors(&self) -> &Vec<SubsystemError<ErrType>> {
        match self {
            GracefulShutdownError::SubsystemsFailed(rel) => rel,
            GracefulShutdownError::ShutdownTimeout(rel) => rel,
        }
    }
}

/// This enum contains all the possible errors that a partial shutdown
/// could cause.
#[derive(Debug, Error, Diagnostic)]
pub enum PartialShutdownError<ErrType: ErrTypeTraits = crate::BoxedError> {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemsFailed(#[related] Vec<SubsystemError<ErrType>>),
    /// The given nested subsystem does not seem to be a child of
    /// the parent subsystem.
    #[error("unable to find nested subsystem in given subsystem")]
    SubsystemNotFound,
    /// A partial shutdown can not be performed because the entire program
    /// is already shutting down.
    #[error("unable to perform partial shutdown, the program is already shutting down")]
    AlreadyShuttingDown,
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
    #[error("Error in subsystem '{0}'")]
    Failed(String, #[source] SubsystemFailure<ErrType>),
    /// The subsystem was cancelled. Should only happen if the shutdown timeout is exceeded.
    #[error("Subsystem '{0}' was aborted")]
    Cancelled(String),
    /// The subsystem panicked.
    #[error("Subsystem '{0}' panicked")]
    Panicked(String),
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
            SubsystemError::Cancelled(name) => name,
            SubsystemError::Panicked(name) => name,
        }
    }
}

/// The error that happens when a task gets cancelled through
/// [`cancel_on_shutdown()`](crate::FutureExt::cancel_on_shutdown).
#[derive(Error, Debug, Diagnostic)]
#[error("A shutdown request caused this task to be cancelled")]
pub struct CancelledByShutdown;

#[cfg(test)]
mod tests {
    use crate::BoxedError;

    use super::*;

    fn examine_report(report: miette::Report) {
        println!("{}", report);
        println!("{:?}", report);
        // Convert to std::error::Error
        let boxed_error: BoxedError = report.into();
        println!("{}", boxed_error);
        println!("{:?}", boxed_error);
    }

    #[test]
    fn errors_can_be_converted_to_diagnostic() {
        examine_report(GracefulShutdownError::ShutdownTimeout::<BoxedError>(vec![]).into());
        examine_report(GracefulShutdownError::SubsystemsFailed::<BoxedError>(vec![]).into());
        examine_report(PartialShutdownError::AlreadyShuttingDown::<BoxedError>.into());
        examine_report(PartialShutdownError::SubsystemNotFound::<BoxedError>.into());
        examine_report(PartialShutdownError::SubsystemsFailed::<BoxedError>(vec![]).into());
        examine_report(SubsystemError::Cancelled::<BoxedError>("".into()).into());
        examine_report(SubsystemError::Panicked::<BoxedError>("".into()).into());
        examine_report(
            SubsystemError::Failed::<BoxedError>("".into(), SubsystemFailure("".into())).into(),
        );
        examine_report(CancelledByShutdown.into());
    }

    #[test]
    fn extract_related_from_graceful_shutdown_error() {
        let related = || {
            vec![
                SubsystemError::Cancelled("a".into()),
                SubsystemError::Panicked("b".into()),
            ]
        };

        let matches_related = |data: &Vec<SubsystemError<BoxedError>>| {
            let mut iter = data.iter();

            let elem = iter.next().unwrap();
            assert_eq!(elem.name(), "a");
            assert!(matches!(elem, SubsystemError::Cancelled(_)));

            let elem = iter.next().unwrap();
            assert_eq!(elem.name(), "b");
            assert!(matches!(elem, SubsystemError::Panicked(_)));

            assert!(iter.next().is_none());
        };

        matches_related(GracefulShutdownError::ShutdownTimeout(related()).get_subsystem_errors());
        matches_related(GracefulShutdownError::SubsystemsFailed(related()).get_subsystem_errors());
        matches_related(&GracefulShutdownError::ShutdownTimeout(related()).into_subsystem_errors());
        matches_related(
            &GracefulShutdownError::SubsystemsFailed(related()).into_subsystem_errors(),
        );
    }

    #[test]
    fn extract_contained_error_from_convert_subsystem_failure() {
        let msg = "MyFailure".to_string();
        let failure = SubsystemFailure(msg.clone());

        assert_eq!(&msg, failure.get_error());
        assert_eq!(msg, *failure);
        assert_eq!(msg, failure.into_error());
    }
}
