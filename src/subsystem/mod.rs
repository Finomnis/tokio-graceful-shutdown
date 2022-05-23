mod data;
mod handle;
mod identifier;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use tokio_util::sync::CancellationToken;

use crate::err_types::ErrorHolder;
use crate::errors::PartialShutdownError;
use crate::runner::SubsystemRunner;
use crate::shutdown_token::ShutdownToken;
use crate::utils::ShutdownGuard;

use self::identifier::SubsystemIdentifier;

/// The data stored per subsystem, like name or nested subsystems
pub struct SubsystemData<ErrType: ErrorHolder = crate::BoxedError> {
    name: String,
    subsystems: Mutex<Option<Vec<SubsystemDescriptor<ErrType>>>>,
    shutdown_subsystems: tokio::sync::Mutex<Vec<SubsystemDescriptor<ErrType>>>,
    local_shutdown_token: ShutdownToken,
    global_shutdown_token: ShutdownToken,
    cancellation_token: CancellationToken,
    shutdown_guard: Weak<ShutdownGuard>,
}

/// The handle given to each subsystem through which the subsystem can interact with this crate.
pub struct SubsystemHandle<ErrType: ErrorHolder = crate::BoxedError> {
    data: Arc<SubsystemData<ErrType>>,
}
// Implement `Clone` manually because the compiler cannot derive `Clone
// from Generics that don't implement `Clone`.
// (https://stackoverflow.com/questions/72150623/)
impl<ErrType: ErrorHolder> Clone for SubsystemHandle<ErrType> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

/// A running subsystem. Can be used to stop the subsystem or get its return value.
struct SubsystemDescriptor<ErrType: ErrorHolder = crate::BoxedError> {
    id: SubsystemIdentifier,
    data: Arc<SubsystemData<ErrType>>,
    subsystem_runner: SubsystemRunner<ErrType>,
}

/// A nested subsystem. Can be used to perform a partial shutdown.
///
/// For more information, see [`SubsystemHandle::start()`] and [`SubsystemHandle::perform_partial_shutdown()`].
pub struct NestedSubsystem {
    id: SubsystemIdentifier,
}
