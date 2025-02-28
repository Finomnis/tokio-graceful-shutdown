use std::sync::{Arc, Mutex};

struct Inner {
    finished_callback: Option<Box<dyn FnOnce() + Send>>,
    cancelled_callback: Option<Box<dyn FnOnce() + Send>>,
}

/// Allows registering callback functions that will get called on destruction.
///
/// This struct is the mechanism that manages lifetime of parents and children
/// in the subsystem tree. It allows for cancellation of the subsytem on drop,
/// and for automatic deregistering in the parent when the child is finished.
pub(crate) struct AliveGuard {
    inner: Arc<Mutex<Inner>>,
}
impl Clone for AliveGuard {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl AliveGuard {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                finished_callback: None,
                cancelled_callback: None,
            })),
        }
    }

    pub(crate) fn on_cancel(&self, cancelled_callback: impl FnOnce() + 'static + Send) {
        let mut inner = self.inner.lock().unwrap();
        assert!(inner.cancelled_callback.is_none());
        inner.cancelled_callback = Some(Box::new(cancelled_callback));
    }

    pub(crate) fn on_finished(&self, finished_callback: impl FnOnce() + 'static + Send) {
        let mut inner = self.inner.lock().unwrap();
        assert!(inner.finished_callback.is_none());
        inner.finished_callback = Some(Box::new(finished_callback));
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(finished_callback) = self.finished_callback.take() {
            finished_callback();
        } else {
            tracing::error!(
                "No `finished` callback was registered in AliveGuard! This should not happen, please report this at https://github.com/Finomnis/tokio-graceful-shutdown/issues."
            );
        }

        if let Some(cancelled_callback) = self.cancelled_callback.take() {
            cancelled_callback()
        }
    }
}

#[cfg(test)]
mod tests;
