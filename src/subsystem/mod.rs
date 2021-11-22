mod data;
mod handle;

use std::sync::Arc;
use std::sync::Mutex;

use crate::runner::SubsystemRunner;
use crate::shutdown_token::ShutdownToken;

/// The data stored per subsystem, like name or nested subsystems
pub struct SubsystemData {
    name: String,
    subsystems: Mutex<Option<Vec<SubsystemDescriptor>>>,
    shutdown_subsystems: tokio::sync::Mutex<Vec<SubsystemDescriptor>>,
    shutdown_token: ShutdownToken,
}

/// The handle through which every subsystem can interact with this crate.
#[derive(Clone)]
pub struct SubsystemHandle {
    shutdown_token: ShutdownToken,
    data: Arc<SubsystemData>,
}

/// A running subsystem. Can be used to stop the subsystem or get its return value.
struct SubsystemDescriptor {
    data: Arc<SubsystemData>,
    subsystem_runner: SubsystemRunner,
}
