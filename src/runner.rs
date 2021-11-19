use crate::{AsyncSubsystem, SubsystemHandle};

pub async fn run_subsystem<S: AsyncSubsystem + 'static + Send>(
    name: String,
    mut subsystem: S,
    subsystem_handle: SubsystemHandle,
) -> Result<(), ()> {
    let shutdown_token = subsystem_handle.shutdown_token();

    let result = subsystem.run(subsystem_handle).await;
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            log::error!("Error in subsystem '{}': {:?}", name, e);
            shutdown_token.shutdown();
            Err(())
        }
    }
}
