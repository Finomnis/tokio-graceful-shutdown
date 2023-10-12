use std::sync::atomic::Ordering;

use crate::{errors::SubsystemJoinError, ErrTypeTraits, ErrorAction};

use super::NestedSubsystem;

impl<ErrType: ErrTypeTraits> NestedSubsystem<ErrType> {
    pub async fn join(&self) -> Result<(), SubsystemJoinError<ErrType>> {
        self.joiner.join().await;

        let errors = self.errors.lock().unwrap().finish();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(SubsystemJoinError::SubsystemsFailed(errors))
        }
    }

    pub fn initiate_shutdown(&self) {
        self.cancellation_token.cancel()
    }

    pub fn change_failure_action(&self, action: ErrorAction) {
        self.error_actions
            .on_failure
            .store(action, Ordering::Relaxed);
    }

    pub fn change_panic_action(&self, action: ErrorAction) {
        self.error_actions.on_panic.store(action, Ordering::Relaxed);
    }
}
