use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::shutdown_token::ShutdownToken;

pub struct SubsystemHandle {
    children: HashMap<&'static str, Box<SubsystemHandle>>,
    subsystem: Box<dyn AsyncSubsystem>,
    shutdown_token: ShutdownToken,
}

impl SubsystemHandle {
    pub fn new(subsystem: Box<dyn AsyncSubsystem>, shutdown_token: ShutdownToken) -> Self {
        Self {
            children: HashMap::new(),
            subsystem,
            shutdown_token,
        }
    }

    pub fn start<S: AsyncSubsystem + 'static>(
        &mut self,
        name: &'static str,
        subsystem: S,
    ) -> &mut Self {
        if let Some(_) = self.children.insert(
            name,
            Box::new(SubsystemHandle::new(
                Box::new(subsystem),
                self.shutdown_token.clone(),
            )),
        ) {
            panic!("Subsystem with name '{}' already exists!", name);
        }

        self
    }

    pub async fn on_shutdown_request(&self) {
        self.shutdown_token.wait_for_shutdown().await
    }

    pub fn initiate_shutdown(&self) {
        self.shutdown_token.shutdown();
    }

    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown_token.clone()
    }
}

#[async_trait(?Send)]
pub trait AsyncSubsystem {
    async fn run(&mut self, inst: &mut SubsystemHandle) -> Result<()>;
}
