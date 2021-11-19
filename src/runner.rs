use crate::{AsyncSubsystem, SubsystemHandle};

pub async fn run_subsystem<S: AsyncSubsystem + 'static + Send>(
    name: String,
    mut subsystem: S,
    subsystem_handle: SubsystemHandle,
) {
    let shutdown_token = subsystem_handle.shutdown_token();

    let result = subsystem.run(subsystem_handle).await;
    match result {
        Ok(()) => (),
        Err(e) => {
            log::error!("Error in submodule '{}': {:?}", name, e);
            shutdown_token.shutdown();
        }
    };
}