use thiserror::Error;

/// This enum contains all the possible errors that a partial shutdown
/// could cause.
#[derive(Debug, Error, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_correct_error_messages() {
        assert_eq!(
            format!("{}", PartialShutdownError::SubsystemFailed),
            "at least one subsystem returned an error",
        );
        assert_eq!(
            format!("{}", PartialShutdownError::SubsystemNotFound),
            "unable to find nested subsystem in given subsystem",
        );
        assert_eq!(
            format!("{}", PartialShutdownError::AlreadyShuttingDown),
            "unable to perform partial shutdown, the program is already shutting down",
        );
    }
}
