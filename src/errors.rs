use thiserror::Error;

/// This enum contains all the possible errors that could be returned
/// by [`handle_shutdown_requests()`](Toplevel::handle_shutdown_requests).
#[derive(Error, Debug)]
pub enum GracefulShutdownError {
    /// At least one subsystem caused an error.
    #[error("at least one subsystem returned an error")]
    SubsystemFailed,
    /// The shutdown did not finish within the given timeout.
    #[error("shutdown timed out")]
    ShutdownTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_correct_error_messages() {
        assert_eq!(
            format!("{}", GracefulShutdownError::SubsystemFailed),
            "at least one subsystem returned an error",
        );
        assert_eq!(
            format!("{}", GracefulShutdownError::ShutdownTimeout),
            "shutdown timed out",
        );
    }
}
