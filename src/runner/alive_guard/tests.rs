use std::sync::atomic::{AtomicU32, Ordering};
use tracing_test::traced_test;

use super::*;

#[test]
#[traced_test]
fn finished_callback() {
    let alive_guard = AliveGuard::new();

    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::clone(&counter);

    alive_guard.on_finished(move || {
        counter2.fetch_add(1, Ordering::Relaxed);
    });

    drop(alive_guard);

    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
#[traced_test]
fn cancel_callback() {
    let alive_guard = AliveGuard::new();

    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::clone(&counter);

    alive_guard.on_finished(|| {});
    alive_guard.on_cancel(move || {
        counter2.fetch_add(1, Ordering::Relaxed);
    });

    drop(alive_guard);

    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
#[traced_test]
fn both_callbacks() {
    let alive_guard = AliveGuard::new();

    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = Arc::clone(&counter);
    let counter3 = Arc::clone(&counter);

    alive_guard.on_finished(move || {
        counter2.fetch_add(1, Ordering::Relaxed);
    });
    alive_guard.on_cancel(move || {
        counter3.fetch_add(1, Ordering::Relaxed);
    });

    drop(alive_guard);

    assert_eq!(counter.load(Ordering::Relaxed), 2);
}

#[test]
#[traced_test]
fn no_callback() {
    let alive_guard = AliveGuard::new();
    drop(alive_guard);

    assert!(logs_contain(
        "No `finished` callback was registered in AliveGuard! This should not happen, please report this at https://github.com/Finomnis/tokio-graceful-shutdown/issues."
    ));
}
