use std::sync::Once;

pub mod event;

static INIT: Once = Once::new();

/// Setup function that is only run once, even if called multiple times.
pub fn setup() {
    INIT.call_once(|| {
        // Init logging
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    });
}
