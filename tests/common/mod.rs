use std::sync::Once;

pub mod event;

static INIT: Once = Once::new();

/// Setup function that is only run once, even if called multiple times.
pub fn setup() {
    INIT.call_once(|| {
        // Init logging
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}
