mod event;

pub use event::Event;
use std::error::Error;

/// Wrapper type to simplify lambdas
pub type BoxedError = Box<dyn Error + Sync + Send>;
pub type BoxedResult = Result<(), BoxedError>;
