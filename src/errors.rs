use miette::Diagnostic;
use thiserror::Error;

use crate::BoxedError;

#[derive(Error, Debug, Diagnostic)]
pub enum SubsystemError {
    #[error("Error in subsystem '{0}'")]
    //#[diagnostic(code(tokio_graceful_shutdown::subsystem::failed))]
    Failed(String, #[source] BoxedError),
    #[error("Subsystem '{0}' was aborted")]
    //#[diagnostic(code(tokio_graceful_shutdown::subsystem::aborted))]
    Cancelled(String),
    #[error("Subsystem '{0}' panicked")]
    //#[diagnostic(code(tokio_graceful_shutdown::subsystem::panicked))]
    Panicked(String),
}
