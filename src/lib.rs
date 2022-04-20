mod errors;
pub mod internal;
//mod api;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;
