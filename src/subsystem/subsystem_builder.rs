use std::borrow::Cow;

use crate::ErrorAction;

/// Configures a subsystem before it gets spawned through
/// [`SubsystemHandle::start`](crate::SubsystemHandle::start).
pub struct SubsystemBuilder<'a, Subsys> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) subsystem: Subsys,
    pub(crate) failure_action: ErrorAction,
    pub(crate) panic_action: ErrorAction,
    pub(crate) detached: bool,
}

impl<'a, Subsys> SubsystemBuilder<'a, Subsys> {
    /// Creates a new SubsystemBuilder from a given subsystem
    /// function.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the subsystem. Primarily to identify the
    ///   subsystem in error messages.
    /// * `subsystem` - The subsystem function that the subsystem will execute.
    pub fn new(name: impl Into<Cow<'a, str>>, subsystem: Subsys) -> Self {
        Self {
            name: name.into(),
            subsystem,
            failure_action: ErrorAction::Forward,
            panic_action: ErrorAction::Forward,
            detached: false,
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

    /// Detaches the subsystem from the parent, causing a shutdown request to not
    /// be propagated from the parent to the child automatically.
    ///
    /// If this option is set, the parent needs to call [`initiate_shutdown()`](crate::NestedSubsystem::initiate_shutdown)
    /// on the child during shutdown, otherwise the child will not
    /// react to the shutdown request. So use this option with care.
    pub fn detached(mut self) -> Self {
        self.detached = true;
        self
    }
}
