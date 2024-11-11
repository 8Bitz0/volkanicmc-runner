use bollard::Docker;
use rand::Rng;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error};

use crate::global_event::GlobalEvent;
use crate::storage::JsonStorageProvider;

use super::{
    Error,
    Instance,
    InstanceList,
    InstanceRequest,
    InstanceStatus,
    PubInstance,
    PubInstanceList,
    StoredInstance
};

const MAX_TOKEN_GEN_ITER: usize = 128;

pub struct DockerInstanceProvider {
    // config: Arc<Mutex<ConfigFile>>,
    instances: Arc<Mutex<InstanceList>>,
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

        info!("Loading instances from storage");

        let mut instances: InstanceList = HashMap::new();

        for i in storage.lock().await.list_instances().await.map_err(Error::Storage)? {
            let id = i.0.clone();
            let inst = i.1;

            let new_instance = Instance {
                name: inst.name.clone(),
                inst_type: inst.inst_type.clone(),
                status: InstanceStatus::Inactive,
                host_com_token: inst.host_com_token.clone(),
                last_con: None,
            };

            info!(instance_id = id.as_str(), "Loaded instance from storage");

            instances.insert(id, new_instance);
        }

        info!("Connected to Docker");

        Ok(DockerInstanceProvider {
            // config,
            instances: Arc::new(Mutex::new(instances)),
            g_event_tx,
            storage,
            docker_handle: Arc::new(docker_handle),
        })
    }
    pub async fn list_instance(&self) -> Result<PubInstanceList, Error> {
        let mut list = PubInstanceList::new();

        for i in self.instances.lock().await.iter() {
            list.insert(i.0.clone(), to_pub_instance(i.1));
        }

        Ok(list)
    }
    /// Returns the ID of the new instance
    pub async fn new_instance(&self, inst: InstanceRequest) -> Result<String, Error> {
        let id = super::unique_id(self.instances.lock().await.keys().cloned().collect()).await?;

        let token = unique_token(self.instances.lock().await.values().map(|i| i.host_com_token.clone()).collect()).await?;

        let new_instance = Instance {
            name: inst.name.clone(),
            inst_type: inst.inst_type.clone(),
            status: InstanceStatus::Inactive,
            host_com_token: token.clone(),
            last_con: None,
        };

        self.instances.lock().await.insert(id.clone(), new_instance);

        self.storage.lock().await.new_instance(id.clone(), StoredInstance {
            name: inst.name,
            inst_type: inst.inst_type,
            host_com_token: token.clone(),
        }).await.map_err(Error::Storage)?;

        Ok(id)
    }
    /// Returns whether the instance was deleted or not
    pub async fn del_instance<I: std::fmt::Display>(&self, id: I) -> Result<bool, Error> {
        self.instances.lock().await.remove(&id.to_string());

        let deleted = self.storage.lock().await.del_instance(id.to_string()).await.map_err(Error::Storage)?;

        Ok(deleted)
    }
    pub async fn get_instance<I: std::fmt::Display>(&self, id: I) -> Option<PubInstance> {
        self.instances.lock().await.get(&id.to_string()).map(to_pub_instance)
    }
    pub async fn find_token(&self, token: &str) -> Option<String> {
        for i in self.instances.lock().await.iter() {
            if i.1.host_com_token == token {
                return Some(i.0.clone());
            }
        }

        None
    }
    pub async fn set_last_con(&self, id: &str) -> Result<(), Error> {
        let mut instances = self.instances.lock().await;
        let inst = instances.get_mut(id).ok_or(Error::InstanceNotFound(id.to_string()))?;

        inst.last_con = Some(chrono::Utc::now().naive_utc());

        Ok(())
    }
}

fn to_pub_instance(inst: &Instance) -> PubInstance {
    PubInstance {
        name: inst.name.clone(),
        inst_type: inst.inst_type.clone(),
        status: inst.status.clone(),
    }
}

async fn unique_token(keys: Vec<String>) -> Result<String, Error> {
    for _ in 0..MAX_TOKEN_GEN_ITER {
        let new_id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        if !keys.contains(&new_id) {
            return Ok(new_id);
        }
    }

    error!(r#"Unable to create unique token. Either you have an astronomical
    number of instances, or there may be a much deeper issue within the
    built-in libraries or hardware."#);

    Err(Error::ExhaustedUniqueIds)
}
