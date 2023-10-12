use std::{fmt::Debug, sync::Arc};

use tokio::sync::watch;

use crate::{errors::SubsystemError, ErrTypeTraits};

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

    // Requires `mut` access to prevent children from being spawned
    // while waiting
    pub(crate) async fn join_children(&mut self) {
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

        if let Some(stop_reason) = maybe_stop_reason {
            tracing::warn!("Unhandled stop reason: {:?}", stop_reason);
        }
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
mod tests {
    use tokio::time::{sleep, timeout, Duration};
    use tracing_test::traced_test;

    use crate::BoxedError;

    use super::*;

    #[test]
    #[traced_test]
    fn counters() {
        let (root, _) = JoinerToken::<BoxedError>::new(|_| None);
        assert_eq!(0, root.count());

        let (child1, _) = root.child_token(|_| None);
        assert_eq!(1, root.count());
        assert_eq!(0, child1.count());

        let (child2, _) = child1.child_token(|_| None);
        assert_eq!(2, root.count());
        assert_eq!(1, child1.count());
        assert_eq!(0, child2.count());

        let (child3, _) = child1.child_token(|_| None);
        assert_eq!(3, root.count());
        assert_eq!(2, child1.count());
        assert_eq!(0, child2.count());
        assert_eq!(0, child3.count());

        drop(child1);
        assert_eq!(2, root.count());
        assert_eq!(0, child2.count());
        assert_eq!(0, child3.count());

        drop(child2);
        assert_eq!(1, root.count());
        assert_eq!(0, child3.count());

        drop(child3);
        assert_eq!(0, root.count());
    }

    #[test]
    #[traced_test]
    fn counters_weak() {
        let (root, weak_root) = JoinerToken::<BoxedError>::new(|_| None);
        assert_eq!(0, weak_root.count());
        assert!(weak_root.alive());

        let (child1, weak_child1) = root.child_token(|_| None);
        assert_eq!(1, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(0, weak_child1.count());
        assert!(weak_child1.alive());

        let (child2, weak_child2) = child1.child_token(|_| None);
        assert_eq!(2, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(1, weak_child1.count());
        assert!(weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(weak_child2.alive());

        let (child3, weak_child3) = child1.child_token(|_| None);
        assert_eq!(3, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(2, weak_child1.count());
        assert!(weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(weak_child2.alive());
        assert_eq!(0, weak_child3.count());
        assert!(weak_child3.alive());

        drop(child1);
        assert_eq!(2, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(2, weak_child1.count());
        assert!(!weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(weak_child2.alive());
        assert_eq!(0, weak_child3.count());
        assert!(weak_child3.alive());

        drop(child2);
        assert_eq!(1, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(1, weak_child1.count());
        assert!(!weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(!weak_child2.alive());
        assert_eq!(0, weak_child3.count());
        assert!(weak_child3.alive());

        drop(child3);
        assert_eq!(0, weak_root.count());
        assert!(weak_root.alive());
        assert_eq!(0, weak_child1.count());
        assert!(!weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(!weak_child2.alive());
        assert_eq!(0, weak_child3.count());
        assert!(!weak_child3.alive());

        drop(root);
        assert_eq!(0, weak_root.count());
        assert!(!weak_root.alive());
        assert_eq!(0, weak_child1.count());
        assert!(!weak_child1.alive());
        assert_eq!(0, weak_child2.count());
        assert!(!weak_child2.alive());
        assert_eq!(0, weak_child3.count());
        assert!(!weak_child3.alive());
    }

    #[tokio::test]
    #[traced_test]
    async fn join() {
        let (superroot, _) = JoinerToken::<BoxedError>::new(|_| None);

        let (mut root, _) = superroot.child_token(|_| None);

        let (child1, _) = root.child_token(|_| None);
        let (child2, _) = child1.child_token(|_| None);
        let (child3, _) = child1.child_token(|_| None);

        let (set_finished, mut finished) = tokio::sync::oneshot::channel();
        tokio::join!(
            async {
                timeout(Duration::from_millis(500), root.join_children())
                    .await
                    .unwrap();
                set_finished.send(root.count()).unwrap();
            },
            async {
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(child1);
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(child2);
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(child3);
                sleep(Duration::from_millis(50)).await;
                let count = timeout(Duration::from_millis(50), finished)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(count, 0);
            }
        );
    }

    #[tokio::test]
    #[traced_test]
    async fn join_through_ref() {
        let (root, joiner) = JoinerToken::<BoxedError>::new(|_| None);

        let (child1, _) = root.child_token(|_| None);
        let (child2, _) = child1.child_token(|_| None);

        let (set_finished, mut finished) = tokio::sync::oneshot::channel();
        tokio::join!(
            async {
                timeout(Duration::from_millis(500), joiner.join())
                    .await
                    .unwrap();
                set_finished.send(()).unwrap();
            },
            async {
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(child1);
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(root);
                sleep(Duration::from_millis(50)).await;
                assert!(finished.try_recv().is_err());

                drop(child2);
                sleep(Duration::from_millis(50)).await;
                timeout(Duration::from_millis(50), finished)
                    .await
                    .unwrap()
                    .unwrap();
            }
        );
    }

    #[test]
    fn debug_print() {
        let (root, _) = JoinerToken::<BoxedError>::new(|_| None);
        assert_eq!(format!("{:?}", root), "JoinerToken(children = 0)");

        let (child1, _) = root.child_token(|_| None);
        assert_eq!(format!("{:?}", root), "JoinerToken(children = 1)");

        let (_child2, _) = child1.child_token(|_| None);
        assert_eq!(format!("{:?}", root), "JoinerToken(children = 2)");
    }

    #[test]
    fn debug_print_ref() {
        let (root, root_ref) = JoinerToken::<BoxedError>::new(|_| None);
        assert_eq!(
            format!("{:?}", root_ref),
            "JoinerTokenRef(alive = true, children = 0)"
        );

        let (child1, _) = root.child_token(|_| None);
        assert_eq!(
            format!("{:?}", root_ref),
            "JoinerTokenRef(alive = true, children = 1)"
        );

        drop(root);
        assert_eq!(
            format!("{:?}", root_ref),
            "JoinerTokenRef(alive = false, children = 1)"
        );

        drop(child1);
        assert_eq!(
            format!("{:?}", root_ref),
            "JoinerTokenRef(alive = false, children = 0)"
        );
    }
}
