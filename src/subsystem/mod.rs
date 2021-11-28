mod data;
mod handle;
mod identifier;

use std::sync::Arc;
use std::sync::Mutex;

use crate::runner::SubsystemRunner;
use crate::shutdown_token::ShutdownToken;

use self::identifier::SubsystemIdentifier;

/// The data stored per subsystem, like name or nested subsystems
pub struct SubsystemData {
    name: String,
    subsystems: Mutex<Option<Vec<SubsystemDescriptor>>>,
    shutdown_subsystems: tokio::sync::Mutex<Vec<SubsystemDescriptor>>,
    local_shutdown_token: ShutdownToken,
    global_shutdown_token: ShutdownToken,
}

/// The handle given to each subsystem through which the subsystem can interact with this crate.
#[derive(Clone)]
pub struct SubsystemHandle {
    data: Arc<SubsystemData>,
}

/// A running subsystem. Can be used to stop the subsystem or get its return value.
struct SubsystemDescriptor {
    id: SubsystemIdentifier,
    data: Arc<SubsystemData>,
    subsystem_runner: SubsystemRunner,
}

pub struct NestedSubsystem {}
