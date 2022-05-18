use crate::ErrTypeTraits;

use super::SubsystemDescriptor;

impl<ErrType: ErrTypeTraits> Drop for SubsystemDescriptor<ErrType> {
    fn drop(&mut self) {
        self.data.cancellation_token.cancel();
    }
}
