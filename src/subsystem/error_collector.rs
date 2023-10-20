use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{errors::SubsystemError, ErrTypeTraits};

pub(crate) enum ErrorCollector<ErrType: ErrTypeTraits> {
    Collecting(mpsc::UnboundedReceiver<SubsystemError<ErrType>>),
    Finished(Arc<[SubsystemError<ErrType>]>),
}

impl<ErrType: ErrTypeTraits> ErrorCollector<ErrType> {
    pub(crate) fn new(receiver: mpsc::UnboundedReceiver<SubsystemError<ErrType>>) -> Self {
        Self::Collecting(receiver)
    }

    pub(crate) fn finish(&mut self) -> Arc<[SubsystemError<ErrType>]> {
        match self {
            ErrorCollector::Collecting(receiver) => {
                let mut errors = vec![];
                receiver.close();
                while let Ok(e) = receiver.try_recv() {
                    errors.push(e);
                }
                let errors = errors.into_boxed_slice().into();
                *self = ErrorCollector::Finished(Arc::clone(&errors));
                errors
            }
            ErrorCollector::Finished(errors) => Arc::clone(errors),
        }
    }
}

impl<ErrType: ErrTypeTraits> Drop for ErrorCollector<ErrType> {
    fn drop(&mut self) {
        if let Self::Collecting(receiver) = self {
            receiver.close();
            while let Ok(e) = receiver.try_recv() {
                tracing::warn!("An error got dropped: {e:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {

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
}
