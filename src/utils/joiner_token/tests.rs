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
    assert!(weak_root.recursive_alive());

    let (child1, weak_child1) = root.child_token(|_| None);
    // root
    //   \
    //   child1
    assert_eq!(1, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(0, weak_child1.count());
    assert!(weak_child1.alive());
    assert!(weak_child1.recursive_alive());

    let (child2, weak_child2) = child1.child_token(|_| None);
    // root
    //   \
    //   child1
    //     \
    //     child2
    assert_eq!(2, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(1, weak_child1.count());
    assert!(weak_child1.alive());
    assert!(weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(weak_child2.alive());
    assert!(weak_child2.recursive_alive());

    let (child3, weak_child3) = child1.child_token(|_| None);
    //    root
    //      \
    //      child1
    //      /   \
    // child2    child3
    assert_eq!(3, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(2, weak_child1.count());
    assert!(weak_child1.alive());
    assert!(weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(weak_child2.alive());
    assert!(weak_child2.recursive_alive());
    assert_eq!(0, weak_child3.count());
    assert!(weak_child3.alive());
    assert!(weak_child3.recursive_alive());

    drop(child1);
    //    root
    //      \
    //      child1 (X)
    //      /   \
    // child2    child3
    assert_eq!(2, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(2, weak_child1.count());
    assert!(!weak_child1.alive());
    assert!(weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(weak_child2.alive());
    assert!(weak_child2.recursive_alive());
    assert_eq!(0, weak_child3.count());
    assert!(weak_child3.alive());
    assert!(weak_child3.recursive_alive());

    drop(child2);
    //    root
    //      \
    //      child1 (X)
    //      /       \
    // child2 (X)    child3
    assert_eq!(1, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(1, weak_child1.count());
    assert!(!weak_child1.alive());
    assert!(weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(!weak_child2.alive());
    assert!(!weak_child2.recursive_alive());
    assert_eq!(0, weak_child3.count());
    assert!(weak_child3.alive());
    assert!(weak_child3.recursive_alive());

    drop(child3);
    //    root
    //      \
    //      child1 (X)
    //      /       \
    // child2 (X)    child3 (X)
    assert_eq!(0, weak_root.count());
    assert!(weak_root.alive());
    assert!(weak_root.recursive_alive());
    assert_eq!(0, weak_child1.count());
    assert!(!weak_child1.alive());
    assert!(!weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(!weak_child2.alive());
    assert!(!weak_child2.recursive_alive());
    assert_eq!(0, weak_child3.count());
    assert!(!weak_child3.alive());
    assert!(!weak_child3.recursive_alive());

    drop(root);
    //    root (X)
    //      \
    //      child1 (X)
    //      /       \
    // child2 (X)    child3 (X)
    assert_eq!(0, weak_root.count());
    assert!(!weak_root.alive());
    assert!(!weak_root.recursive_alive());
    assert_eq!(0, weak_child1.count());
    assert!(!weak_child1.alive());
    assert!(!weak_child1.recursive_alive());
    assert_eq!(0, weak_child2.count());
    assert!(!weak_child2.alive());
    assert!(!weak_child2.recursive_alive());
    assert_eq!(0, weak_child3.count());
    assert!(!weak_child3.alive());
    assert!(!weak_child3.recursive_alive());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn join() {
    let (superroot, _) = JoinerToken::<BoxedError>::new(|_| None);

    let (root, _) = superroot.child_token(|_| None);

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

#[tokio::test(start_paused = true)]
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn recursive_finished() {
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
