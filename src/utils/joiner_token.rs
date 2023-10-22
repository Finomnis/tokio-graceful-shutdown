use std::{fmt::Debug, sync::Arc};

use tokio::sync::watch;

use crate::{
    errors::{handle_unhandled_stopreason, SubsystemError},
    ErrTypeTraits,
};

struct Inner<ErrType: ErrTypeTraits> {
    counter: watch::Sender<(bool, u32)>,
    parent: Option<Arc<Inner<ErrType>>>,
    on_error: Box<dyn Fn(SubsystemError<ErrType>) -> Option<SubsystemError<ErrType>> + Sync + Send>,
}

/// A token that keeps reference of its existance and its children.
pub(crate) struct JoinerToken<ErrType: ErrTypeTraits> {
    inner: Arc<Inner<ErrType>>,
}

/// A reference version that does not keep the content alive; purely for
/// joining the subtree.
pub(crate) struct JoinerTokenRef {
    counter: watch::Receiver<(bool, u32)>,
}

impl<ErrType: ErrTypeTraits> Debug for JoinerToken<ErrType> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JoinerToken(children = {})",
            self.inner.counter.borrow().1
        )
    }
}

impl Debug for JoinerTokenRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let counter = self.counter.borrow();
        write!(
            f,
            "JoinerTokenRef(alive = {}, children = {})",
            counter.0, counter.1
        )
    }
}

impl<ErrType: ErrTypeTraits> JoinerToken<ErrType> {
    /// Creates a new joiner token.
    ///
    /// The `on_error` callback will receive errors/panics and has to decide
    /// how to handle them. It can also not handle them and instead pass them on.
    /// If it returns `Some`, the error will get passed on to its parent.
    pub(crate) fn new(
        on_error: impl Fn(SubsystemError<ErrType>) -> Option<SubsystemError<ErrType>>
            + Sync
            + Send
            + 'static,
    ) -> (Self, JoinerTokenRef) {
        let inner = Arc::new(Inner {
            counter: watch::channel((true, 0)).0,
            parent: None,
            on_error: Box::new(on_error),
        });

        let weak_ref = JoinerTokenRef {
            counter: inner.counter.subscribe(),
        };

        (Self { inner }, weak_ref)
    }

    pub(crate) async fn join_children(&self) {
        let mut subscriber = self.inner.counter.subscribe();

        // Ignore errors; if the channel got closed, that definitely means
        // no more children exist.
        let _ = subscriber
            .wait_for(|(_alive, children)| *children == 0)
            .await;
    }

    pub(crate) fn child_token(
        &self,
        on_error: impl Fn(SubsystemError<ErrType>) -> Option<SubsystemError<ErrType>>
            + Sync
            + Send
            + 'static,
    ) -> (Self, JoinerTokenRef) {
        let mut maybe_parent = Some(&self.inner);
        while let Some(parent) = maybe_parent {
            parent
                .counter
                .send_modify(|(_alive, children)| *children += 1);
            maybe_parent = parent.parent.as_ref();
        }

        let inner = Arc::new(Inner {
            counter: watch::channel((true, 0)).0,
            parent: Some(Arc::clone(&self.inner)),
            on_error: Box::new(on_error),
        });

        let weak_ref = JoinerTokenRef {
            counter: inner.counter.subscribe(),
        };

        (Self { inner }, weak_ref)
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> u32 {
        self.inner.counter.borrow().1
    }

    pub(crate) fn raise_failure(&self, stop_reason: SubsystemError<ErrType>) {
        let mut maybe_stop_reason = Some(stop_reason);

        let mut maybe_parent = Some(&self.inner);
        while let Some(parent) = maybe_parent {
            if let Some(stop_reason) = maybe_stop_reason {
                maybe_stop_reason = (parent.on_error)(stop_reason);
            } else {
                break;
            }

            maybe_parent = parent.parent.as_ref();
        }

        handle_unhandled_stopreason(maybe_stop_reason);
    }

    pub(crate) fn downgrade(self) -> JoinerTokenRef {
        JoinerTokenRef {
            counter: self.inner.counter.subscribe(),
        }
    }
}

impl JoinerTokenRef {
    pub(crate) async fn join(&self) {
        // Ignore errors; if the channel got closed, that definitely means
        // the token and all its children got dropped.
        let _ = self
            .counter
            .clone()
            .wait_for(|&(alive, children)| !alive && children == 0)
            .await;
    }

    #[cfg(test)]
    pub(crate) fn count(&self) -> u32 {
        self.counter.borrow().1
    }

    #[cfg(test)]
    pub(crate) fn alive(&self) -> bool {
        self.counter.borrow().0
    }
}

impl<ErrType: ErrTypeTraits> Drop for JoinerToken<ErrType> {
    fn drop(&mut self) {
        self.inner
            .counter
            .send_modify(|(alive, _children)| *alive = false);

        let mut maybe_parent = self.inner.parent.as_ref();
        while let Some(parent) = maybe_parent {
            parent
                .counter
                .send_modify(|(_alive, children)| *children -= 1);
            maybe_parent = parent.parent.as_ref();
        }
    }
}

#[cfg(test)]
mod tests;
