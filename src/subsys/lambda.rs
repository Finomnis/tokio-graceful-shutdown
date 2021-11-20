use crate::{AsyncSubsystem, SubsystemHandle};
use anyhow::Result;
use async_trait::async_trait;
use std::future::Future;

pub struct LambdaSubsystem<
    Fut: Future<Output = Result<()>> + Send,
    T: FnOnce(SubsystemHandle) -> Fut + Send,
> {
    func: T,
}

#[async_trait]
impl<Fut: Future<Output = Result<()>> + Send, T: FnOnce(SubsystemHandle) -> Fut + Send>
    AsyncSubsystem for LambdaSubsystem<Fut, T>
{
    async fn run(mut self, subsys: SubsystemHandle) -> Result<()> {
        let result = (self.func)(subsys);
        result.await
    }
}

impl<Fut: Future<Output = Result<()>> + Send, T: FnOnce(SubsystemHandle) -> Fut + Send>
    LambdaSubsystem<Fut, T>
{
    pub fn new(func: T) -> Self {
        Self { func }
    }
}
