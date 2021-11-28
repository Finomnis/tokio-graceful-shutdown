use std::error::Error;
use std::fmt::Display;

/// This enum contains all the possible errors that a partial shutdown
/// could case.
#[derive(Debug)]
pub enum PartialShutdownError {
    /// At least one subsystem caused an error
    SubsystemFailed,
    /// The given nested subsystem does not seem to be a child of
    /// the parent subsystem.
    SubsystemNotFound,
    /// A partial shutdown can not be performed because the entire program
    /// is already shutting down.
    AlreadyShuttingDown,
}

impl Error for PartialShutdownError {}
impl Display for PartialShutdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PartialShutdownError::SubsystemFailed => "At least one subsystem returned an error",
                PartialShutdownError::SubsystemNotFound =>
                    "Cannot find nested subsystem in given subsystem!",
                PartialShutdownError::AlreadyShuttingDown =>
                    "Unable to perform partial shutdown, system is already shutting down!",
            }
        )
    }
}
