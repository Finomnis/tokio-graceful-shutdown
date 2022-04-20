mod api;
mod errors;
pub mod internal;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;
