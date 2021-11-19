use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::runner::run_subsystem;
use crate::shutdown_token::ShutdownToken;

pub struct SubsystemData {
    name: String,
    subsystems: RwLock<Vec<Arc<SubsystemData>>>,
    subsystem_joinhandles: RwLock<Vec<Arc<JoinHandle<()>>>>,
    shutdown_token: ShutdownToken,
}

pub struct SubsystemHandle {
    shutdown_token: ShutdownToken,
    data: Arc<SubsystemData>,
}

impl SubsystemData {
    pub fn new(name: &'static str, shutdown_token: ShutdownToken) -> Self {
        Self {
            name: name.to_string(),
            subsystems: RwLock::new(Vec::new()),
            subsystem_joinhandles: RwLock::new(Vec::new()),
            shutdown_token,
        }
    }

    pub async fn add_subsystem(
        &self,
        subsystem: Arc<SubsystemData>,
        joinhandle: Arc<JoinHandle<()>>,
    ) {
        self.subsystem_joinhandles.write().await.push(joinhandle);
        self.subsystems.write().await.push(subsystem);
    }

    pub async fn perform_shutdown(&self) -> Result<()> {
        Ok(())
    }
}

impl SubsystemHandle {
    pub fn new(data: Arc<SubsystemData>) -> Self {
        Self {
            shutdown_token: data.shutdown_token.clone(),
            data,
        }
    }

    pub async fn start<S: AsyncSubsystem + 'static + Send>(
        &mut self,
        name: &'static str,
        subsystem: S,
    ) -> &mut Self {
        let shutdown_token = self.shutdown_token.clone();

        // Create subsystem data structure
        let new_subsystem = Arc::new(SubsystemData::new(name, shutdown_token.clone()));

        // Create handle
        let subsystem_handle = SubsystemHandle::new(new_subsystem.clone());

        // Spawn new task
        let join_handle = Arc::new(tokio::spawn(run_subsystem(
            self.data.name.clone() + name,
            subsystem,
            subsystem_handle,
        )));

        // Store subsystem data
        self.data.add_subsystem(new_subsystem, join_handle).await;

        self
    }

    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown_token.clone()
    }

    pub async fn on_shutdown_requested(&self) {
        self.shutdown_token.wait_for_shutdown().await
    }
}

// impl SubsystemHandle {
//     pub fn new(subsystem: Box<dyn AsyncSubsystem>, shutdown_token: ShutdownToken) -> Self {
//         Self {
//             subsystem,
//             shutdown_token,
//         }
//     }

//     pub fn start<S: AsyncSubsystem + 'static + Send>(
//         &mut self,
//         name: &'static str,
//         subsystem: S,
//     ) -> &mut Self {
//         let boxed_subsys = Box::new(subsystem);
//         let shutdown_token = self.shutdown_token.clone();

//         tokio::spawn(async move {
//             let handle = Box::new(SubsystemHandle::new(boxed_subsys, shutdown_token));
//             handle.run();
//         });

//         self
//     }

//     pub async fn on_shutdown_request(&self) {
//         self.shutdown_token.wait_for_shutdown().await
//     }

//     pub fn initiate_shutdown(&self) {
//         self.shutdown_token.shutdown();
//     }

//     pub fn shutdown_token(&self) -> ShutdownToken {
//         self.shutdown_token.clone()
//     }

//     fn run(mut self) {}
// }

#[async_trait]
pub trait AsyncSubsystem {
    async fn run(&mut self, inst: SubsystemHandle) -> Result<()>;
}
