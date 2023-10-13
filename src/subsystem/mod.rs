mod error_collector;
mod nested_subsystem;
mod subsystem_builder;
mod subsystem_handle;

use std::sync::{Arc, Mutex};

pub use subsystem_builder::SubsystemBuilder;
pub use subsystem_handle::SubsystemHandle;

pub(crate) use subsystem_handle::root_handle;

use crate::{utils::JoinerTokenRef, ErrTypeTraits, ErrorAction};

use atomic::Atomic;
use tokio_util::sync::CancellationToken;

/// A nested subsystem.
///
/// Can be used to control the subsystem or wait for it to finish.
///
/// Dropping this value does not perform any action - the subsystem
/// will be neither cancelled, shut down or detached.
///
/// For more information, look through the examples directory in
/// the source code.
pub struct NestedSubsystem<ErrType: ErrTypeTraits> {
    joiner: JoinerTokenRef,
    cancellation_token: CancellationToken,
    errors: Mutex<error_collector::ErrorCollector<ErrType>>,
    error_actions: Arc<ErrorActions>,
}

pub(crate) struct ErrorActions {
    pub(crate) on_failure: Atomic<ErrorAction>,
    pub(crate) on_panic: Atomic<ErrorAction>,
}
