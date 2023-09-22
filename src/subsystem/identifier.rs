use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_with_itself() {
        let identifier1 = SubsystemIdentifier::create();
        #[allow(clippy::clone_on_copy)]
        let identifier2 = identifier1.clone();
        assert_eq!(identifier1, identifier2);
    }

    #[test]
    fn does_not_equal_with_others() {
        let identifier1 = SubsystemIdentifier::create();
        let identifier2 = SubsystemIdentifier::create();
        assert_ne!(identifier1, identifier2);
    }
}
