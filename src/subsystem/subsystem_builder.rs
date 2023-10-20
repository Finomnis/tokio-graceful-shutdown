use std::{borrow::Cow, future::Future, marker::PhantomData};

use crate::{ErrTypeTraits, ErrorAction, SubsystemHandle};

/// Configures a subsystem before it gets spawned through
/// [`SubsystemHandle::start`].
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
    /// Creates a new SubsystemBuilder from a given subsystem
    /// function.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the subsystem. Primarily to identify the
    ///            subsystem in error messages.
    /// * `subsystem` - The subsystem function that the subsystem will execute.
    pub fn new(name: impl Into<Cow<'a, str>>, subsystem: Subsys) -> Self {
        Self {
            name: name.into(),
            subsystem,
            failure_action: ErrorAction::Forward,
            panic_action: ErrorAction::Forward,
            _phantom: Default::default(),
        }
    }

    /// Sets the way this subsystem should react to failures,
    /// meaning if it or one of its children return an `Err` value.
    ///
    /// The default is [`ErrorAction::Forward`].
    ///
    /// For more information, see [`ErrorAction`].
    pub fn on_failure(mut self, action: ErrorAction) -> Self {
        self.failure_action = action;
        self
    }

    /// Sets the way this subsystem should react if it or one
    /// of its children panic.
    ///
    /// The default is [`ErrorAction::Forward`].
    ///
    /// For more information, see [`ErrorAction`].
    pub fn on_panic(mut self, action: ErrorAction) -> Self {
        self.panic_action = action;
        self
    }
}
