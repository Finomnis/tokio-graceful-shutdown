use tracing_test::traced_test;

use super::*;

#[test]
#[traced_test]
fn normal() {
    let (sender, receiver) = mpsc::unbounded_channel();
    let mut error_collector = ErrorCollector::<String>::new(receiver);

    sender
        .send(SubsystemError::Panicked(Arc::from("ABC")))
        .unwrap();
    sender
        .send(SubsystemError::Panicked(Arc::from("def")))
        .unwrap();

    let received = error_collector.finish();
    assert_eq!(
        received.iter().map(|e| e.name()).collect::<Vec<_>>(),
        vec!["ABC", "def"]
    );
}

#[test]
#[traced_test]
fn double_finish() {
    let (sender, receiver) = mpsc::unbounded_channel();
    let mut error_collector = ErrorCollector::<String>::new(receiver);

    sender
        .send(SubsystemError::Panicked(Arc::from("ABC")))
        .unwrap();
    sender
        .send(SubsystemError::Panicked(Arc::from("def")))
        .unwrap();

    let received = error_collector.finish();
    assert_eq!(
        received.iter().map(|e| e.name()).collect::<Vec<_>>(),
        vec!["ABC", "def"]
    );

    let received = error_collector.finish();
    assert_eq!(
        received.iter().map(|e| e.name()).collect::<Vec<_>>(),
        vec!["ABC", "def"]
    );
}

#[test]
#[traced_test]
fn no_finish() {
    let (sender, receiver) = mpsc::unbounded_channel();
    let error_collector = ErrorCollector::<String>::new(receiver);

    sender
        .send(SubsystemError::Panicked(Arc::from("ABC")))
        .unwrap();
    sender
        .send(SubsystemError::Panicked(Arc::from("def")))
        .unwrap();

    drop(error_collector);

    assert!(logs_contain("An error got dropped: Panicked(\"ABC\")"));
    assert!(logs_contain("An error got dropped: Panicked(\"def\")"));
}
