use bollard::Docker;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::info;

use crate::global_event::GlobalEvent;
use crate::instance::InstanceList;
use crate::storage::JsonStorageProvider;

use super::{Error, Instance, InstanceRequest};

pub struct DockerInstanceProvider {
    // config: Arc<Mutex<ConfigFile>>,
    g_event_tx: broadcast::Sender<GlobalEvent>,
    storage: Arc<Mutex<JsonStorageProvider>>,
    docker_handle: Arc<Docker>,
}

impl DockerInstanceProvider {
    pub async fn new(
        /* config: Arc<Mutex<ConfigFile>>, */
        g_event_tx: broadcast::Sender<GlobalEvent>,
        storage: Arc<Mutex<JsonStorageProvider>>,
    ) -> Result<Self, Error> {
        let docker_handle = Docker::connect_with_local_defaults().map_err(Error::Docker)?;

        info!("Connected to Docker");

        Ok(DockerInstanceProvider {
            // config,
            g_event_tx,
            storage,
            docker_handle: Arc::new(docker_handle),
        })
    }
    pub async fn list_instance(&self) -> Result<InstanceList, Error> {
        self.storage.lock().await.list_instances().await.map_err(Error::Storage)
    }
    /// Returns the ID of the new instance
    pub async fn new_instance(&self, inst: InstanceRequest) -> Result<String, Error> {
        self.storage.lock().await.new_instance(inst).await.map_err(Error::Storage)
    }
    /// Returns whether the instance was deleted or not
    pub async fn del_instance<I: std::fmt::Display>(&self, id: I) -> Result<bool, Error> {
        let deleted = self.storage.lock().await.del_instance(id.to_string()).await.map_err(Error::Storage)?;

        Ok(deleted)
    }
    pub async fn get_instance<I: std::fmt::Display>(&self, id: I) -> Option<Instance> {
        self.storage.lock().await.get_instance(id.to_string()).await
    }
}
