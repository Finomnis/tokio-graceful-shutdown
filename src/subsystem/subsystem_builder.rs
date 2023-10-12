use std::{borrow::Cow, future::Future, marker::PhantomData};

use crate::{ErrTypeTraits, ErrorAction, SubsystemHandle};

pub struct SubsystemBuilder<'a, ErrType, Err, Fut, Subsys>
where
    ErrType: ErrTypeTraits,
    Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
    Fut: 'static + Future<Output = Result<(), Err>> + Send,
    Err: Into<ErrType>,
{
    pub(crate) name: Cow<'a, str>,
    pub(crate) subsystem: Subsys,
    pub(crate) failure_action: ErrorAction,
    pub(crate) panic_action: ErrorAction,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (Fut, ErrType, Err)>,
}

impl<'a, ErrType, Err, Fut, Subsys> SubsystemBuilder<'a, ErrType, Err, Fut, Subsys>
where
    ErrType: ErrTypeTraits,
    Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
    Fut: 'static + Future<Output = Result<(), Err>> + Send,
    Err: Into<ErrType>,
{
    pub fn new(name: impl Into<Cow<'a, str>>, subsystem: Subsys) -> Self {
        Self {
            name: name.into(),
            subsystem,
            failure_action: ErrorAction::Forward,
            panic_action: ErrorAction::Forward,
            _phantom: Default::default(),
        }
    }

    pub fn on_failure(mut self, action: ErrorAction) -> Self {
        self.failure_action = action;
        self
    }

    pub fn on_panic(mut self, action: ErrorAction) -> Self {
        self.panic_action = action;
        self
    }
}
