mod error_collector;
mod nested_subsystem;
mod subsystem_builder;
mod subsystem_finished_future;
mod subsystem_handle;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

pub use subsystem_builder::SubsystemBuilder;
pub use subsystem_handle::SubsystemHandle;

pub(crate) use subsystem_handle::root_handle;

use crate::{utils::JoinerTokenRef, BoxedError, ErrTypeTraits, ErrorAction};

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
pub struct NestedSubsystem<ErrType: ErrTypeTraits = BoxedError> {
    joiner: JoinerTokenRef,
    cancellation_token: CancellationToken,
    errors: Mutex<error_collector::ErrorCollector<ErrType>>,
    error_actions: Arc<ErrorActions>,
    abort_handle: tokio::task::AbortHandle,
}

pub(crate) struct ErrorActions {
    pub(crate) on_failure: Atomic<ErrorAction>,
    pub(crate) on_panic: Atomic<ErrorAction>,
}

/// A future that is resolved once the corresponding subsystem is finished.
///
/// Returned by [`NestedSubsystem::finished`].
#[must_use = "futures do nothing unless polled"]
pub struct SubsystemFinishedFuture {
    future: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
}
