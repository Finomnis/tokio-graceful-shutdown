use std::sync::atomic::{AtomicUsize, Ordering};

pub struct SubsystemIdentifier {
    id: usize,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

impl SubsystemIdentifier {
    pub fn create() -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
        }
    }
}
