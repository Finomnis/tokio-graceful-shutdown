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
