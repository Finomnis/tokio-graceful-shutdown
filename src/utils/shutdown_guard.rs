use crate::ShutdownToken;

/// Triggers the ShutdownToken when dropped
pub struct ShutdownGuard(ShutdownToken);

impl ShutdownGuard {
    pub fn new(token: ShutdownToken) -> Self {
        Self(token)
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.0.shutdown()
    }
}
