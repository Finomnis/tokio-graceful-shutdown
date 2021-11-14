use crate::initiate_shutdown;
use anyhow::Result;
use std::future::Future;

/// Executes an async submodule.
///
/// When the submodule returns an error,
/// a program shutdown gets triggered.
pub fn start_submodule(
    submodule: impl Future<Output = Result<()>> + Send + 'static,
) -> tokio::task::JoinHandle<Result<()>> {
    async fn submodule_executor(submodule: impl Future<Output = Result<()>>) -> Result<()> {
        let result = submodule.await;
        if let Err(e) = &result {
            log::error!("Submodule Error: {}", e);
            initiate_shutdown();
        }
        result
    }

    tokio::spawn(submodule_executor(submodule))
}

#[macro_export]
/// Waits for given submodule handles. Times out after given duration.
macro_rules! wait_for_submodule_shutdown {
    ($duration:expr, $($handles : expr),* $(,) ?) => {{
        use anyhow::anyhow;

        // Flattens JoinHandle<T> to Future<Result<T>>, to enable proper error early stopping in try_join.
        async fn flatten(handle: tokio::task::JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
            match handle.await {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(err)) => Err(err),
                Err(err) => Err(anyhow::Error::new(err)),
            }
        }

        let task_joiner = async {
            tokio::try_join!(
                $(flatten($handles)),*
            ).and(Ok(()))
        };

        let result = tokio::select! {
            e = task_joiner => e,
            _ = tokio::time::sleep($duration) => Err(anyhow::anyhow!("Subsystem shutdown took too long!"))
        };

        match result {
            Err(e) => {
                log::error!("Submodule Error: {:?}", e);
                Err(anyhow!("Submodule failure."))
            }
            Ok(()) => {
                log::info!("Subsystems shut down successfully.");
                Ok(())
            }
        }
    }};
}
