use bollard::{
    container::{self, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions, StartContainerOptions},
    Docker
};
use futures_util::future::join_all;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::{sync::{broadcast, Mutex}, task::JoinHandle};
use tracing::{debug, info, error};

use crate::{
    config::ConfigFile,
    global_event::GlobalEvent,
    storage::JsonStorageProvider,
};

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

const HOST_IMAGE: &str = "ghcr.io/8bitz0/volkanicmc-host:0.2.0";
const MAX_TOKEN_GEN_ITER: usize = 128;
const RUNNER_ADDR_PREFIX: &str = "http://host.docker.internal:";
const INSTANCE_CHECK_INTERVAL_MS: u64 = 750;
const INSTANCE_CHECK_CONTAINER_ID_LOCK_TIMEOUT_MS: u64 = 50;

#[derive(Clone)]
pub struct DockerInstanceProvider {
    config: Arc<Mutex<ConfigFile>>,
    instances: Arc<Mutex<InstanceList>>,
    g_event_tx: broadcast::Sender<GlobalEvent>,
    storage: Arc<Mutex<JsonStorageProvider>>,
    docker_handle: Arc<Docker>,
    bg_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum HostEvent {
    /// Expects the host to gracefully stop the instance and wait
    #[serde(rename = "stop")]
    Stop,
}

impl DockerInstanceProvider {
    pub async fn new(
        config: Arc<Mutex<ConfigFile>>,
        g_event_tx: broadcast::Sender<GlobalEvent>,
        storage: Arc<Mutex<JsonStorageProvider>>,
    ) -> Result<Self, Error> {
        let docker_handle = Docker::connect_with_local_defaults().map_err(Error::Docker)?;

        info!("Connected to Docker");

        info!("Loading instances from storage");

        let provider = DockerInstanceProvider {
            config,
            instances: Arc::new(Mutex::new(HashMap::new())),
            g_event_tx,
            storage: storage.clone(),
            docker_handle: Arc::new(docker_handle),
            bg_handle: Arc::new(Mutex::new(None)),
        };

        let total_to_load: usize = storage.lock().await.list_instances().await.map_err(Error::Storage)?.len();

        for i in storage.lock().await.list_instances().await.map_err(Error::Storage)? {
            let id = i.0.clone();
            let inst = i.1;

            let new_instance = Instance {
                name: Arc::new(Mutex::new(inst.name.clone())),
                inst_type: Arc::new(Mutex::new(inst.inst_type.clone())),
                status: Arc::new(Mutex::new(InstanceStatus::Inactive)),
                host_com_token: Arc::new(Mutex::new(inst.host_com_token.clone())),
                last_con: Arc::new(Mutex::new(None)),
                container_id: Arc::new(Mutex::new(inst.container_id.clone())),
                host_com_tx: broadcast::channel(255).0,
            };

            debug!(instance_id = id.as_str(), "Loaded instance from storage");

            provider.instances.lock().await.insert(id.clone(), new_instance);
        }

        match total_to_load.cmp(&1) {
            std::cmp::Ordering::Greater => info!("Loaded {} instances from storage", total_to_load),
            std::cmp::Ordering::Equal => info!("Loaded {} instance from storage", total_to_load),
            std::cmp::Ordering::Less => info!("No instances loaded from storage"),
        }

        // Start background tasks for instance provider
        provider.start_bg().await?;

        Ok(provider)
    }
    pub async fn list_instance(&self) -> Result<PubInstanceList, Error> {
        let mut list = PubInstanceList::new();

        for i in self.instances.lock().await.iter() {
            list.insert(i.0.clone(), to_pub_instance(i.1).await);
        }

        Ok(list)
    }
    /// Returns the ID of the new instance
    pub async fn new_instance(&self, inst: InstanceRequest) -> Result<String, Error> {
        let id = super::unique_id(self.instances.lock().await.keys().cloned().collect()).await?;

        let tokens: Vec<_> = join_all(
            self.instances.lock().await.values()
                .map(|i| async move { i.host_com_token.lock().await.clone() })
        ).await;
        let token = unique_token(tokens).await?;

        let new_instance = Instance {
            name: Arc::new(Mutex::new(inst.name.clone())),
            inst_type: Arc::new(Mutex::new(inst.inst_type.clone())),
            status: Arc::new(Mutex::new(InstanceStatus::Inactive)),
            host_com_token: Arc::new(Mutex::new(token.clone())),
            last_con: Arc::new(Mutex::new(None)),
            container_id: Arc::new(Mutex::new(None)),
            host_com_tx: broadcast::channel(255).0,
        };

        self.instances.lock().await.insert(id.clone(), new_instance.clone());

        self.storage.lock().await.update_instance(id.clone(), StoredInstance {
            name: inst.name,
            inst_type: inst.inst_type,
            host_com_token: token.clone(),
            container_id: None,
        }).await.map_err(Error::Storage)?;

        let _ = self.g_event_tx
            .send(GlobalEvent::ModifyInstance { id: id.clone(), instance: to_pub_instance(&new_instance).await });

        Ok(id)
    }
    /// Returns whether the instance was deleted or not
    pub async fn del_instance(&self, id: impl std::fmt::Display) -> Result<(), Error> {
        let id = id.to_string();

        let provider = self.clone();

        self.set_inst_status(&id, InstanceStatus::Deleting).await?;

        tokio::spawn(async move {
            {
                // Lock instances
                let mut instances = provider.instances.lock().await.clone();
                let inst = instances.get_mut(&id).ok_or(Error::InstanceNotFound(id.clone()))?.clone();
    
                // Drop instances lock preventing blocking other operations
                drop(instances);
    
                if (inst.container_id.lock().await.clone()).is_some() {
                    match provider.delete_container(&id, &inst).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error deleting container: {}", e);
    
                            provider.set_inst_status(&id, InstanceStatus::Inactive).await?;
    
                            return Err(e);
                        }
                    };
                } 
            }
    
            provider.instances.lock().await.remove(&id.to_string());
    
            match provider.storage.lock().await.del_instance(id.to_string()).await {
                Ok(d) => d,
                Err(e) => {
                    error!("Error deleting instance from storage: {}", e);
    
                    provider.set_inst_status(&id, InstanceStatus::Inactive).await?;
    
                    return Err(Error::Storage(e));
                }
            };
    
            let _ = provider.g_event_tx.send(GlobalEvent::DeleteInstance { id: id.to_string() });

            Ok(())
        });

        Ok(())
    }
    pub async fn get_instance(&self, id: impl std::fmt::Display) -> Option<PubInstance> {
        if let Some(inst) = self.instances.lock().await.get(&id.to_string()) {
            Some(to_pub_instance(inst).await)
        } else {
            None
        }
    }
    pub async fn start_instance<I: std::fmt::Display>(&self, id: I) {
        let id = id.to_string();

        let provider = self.clone();

        tokio::spawn(async move {
            match provider.start_host(id.to_string()).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Error starting instance: {}", e);
                }
            };
        });
    }
    pub async fn stop_instance<I: std::fmt::Display>(&self, id: I) {
        let id = id.to_string();

        let provider = self.clone();

        tokio::spawn(async move {
            match provider.stop_host(id.to_string()).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Error stopping instance: {}", e);
                }
            };
        });
    }
    pub async fn find_token(&self, token: &str) -> Option<String> {
        for i in self.instances.lock().await.iter() {
            if i.1.host_com_token.lock().await.clone() == token {
                return Some(i.0.clone());
            }
        }

        None
    }
    pub async fn set_last_con(&self, id: &str) -> Result<(), Error> {
        let mut instances = self.instances.lock().await;
        let inst = instances.get_mut(id).ok_or(Error::InstanceNotFound(id.to_string()))?;

        inst.last_con = Arc::new(Mutex::new(Some(chrono::Utc::now().naive_utc())));

        Ok(())
    }
    pub async fn get_host_event_tx(&self, id: impl std::fmt::Display) -> Option<broadcast::Sender<HostEvent>> {
        let instances = self.instances.lock().await;
        let inst = instances.get(&id.to_string())?;
        
        Some(inst.host_com_tx.clone())
    }
    async fn start_host(&self, id: impl std::fmt::Display) -> Result<(), Error> {
        let id = id.to_string();

        let mut instances = self.instances.lock().await.clone();
        let inst = instances.get_mut(&id).ok_or(Error::InstanceNotFound(id.clone()))?;

        self.set_inst_status_in(&id, inst, InstanceStatus::Starting).await?;
        
        let mut previous_created = false;
        loop {
            let container_id = inst.container_id.lock().await.clone();

            let inspect_r = match &container_id {
                Some(id) => self.docker_handle.inspect_container(id, None).await.ok(),
                None => None,
            };
    
            match inspect_r {
                Some(c) => {
                    match c.id {
                        Some(container_id) => {
                            self.docker_handle.start_container(&container_id, None::<StartContainerOptions<String>>).await.map_err(Error::Docker)?;
                        },
                        None => {
                            error!("Container ID not found in inspect response");
                            self.set_inst_status_in(&id, inst, InstanceStatus::Inactive).await?;
    
                            return Err(Error::ContainerIdNotFound);
                        }
                    };
                }
                None => {
                    if previous_created {
                        error!("Container ID not found (container was created)");
                        self.set_inst_status_in(&id, inst, InstanceStatus::Inactive).await?;
    
                        return Err(Error::ContainerIdNotFound);
                    }

                    debug!("Error inspecting container");
                    info!("Creating container for instance {}", id);
    
                    self.create_container(&id, inst).await?;

                    previous_created = true;

                    continue;
                } 
            };

            break;
        };

        self.set_inst_status_in(&id, inst, InstanceStatus::Running).await?;

        Ok(())
    }
    async fn stop_host(&self, id: impl std::fmt::Display) -> Result<(), Error> {
        let id = id.to_string();

        let mut instances = self.instances.lock().await.clone();
        let inst = instances.get_mut(&id).ok_or(Error::InstanceNotFound(id.clone()))?;

        if *inst.status.lock().await != InstanceStatus::Inactive {
            self.set_inst_status_in(&id, inst, InstanceStatus::Stopping).await?;
        }

        let container_id = inst.container_id.lock().await.clone();

        if let Some(container_id) = container_id {
            match inst.host_com_tx.send(HostEvent::Stop) {
                Ok(_) => {}
                Err(e) => {
                    error!("Error while sending host communication signal: {}", e);
                }
            };
            
            // self.docker_handle.stop_container(&container_id, None::<container::StopContainerOptions>).await.map_err(Error::Docker)?;
        } else {
            error!("No container attached to instance {}", id);
        };

        self.set_inst_status_in(&id, inst, InstanceStatus::Inactive).await?;

        Ok(())
    }
    async fn create_container(
        &self,
        id: impl std::fmt::Display,
        inst: &Instance
    ) -> Result<String, Error> {
        let mut container_id_lock = inst.container_id.lock().await;

        let container_r = self.docker_handle.create_container(Some(CreateContainerOptions {
            name: new_container_name().await,
            ..Default::default()
        }), container::Config {
            image: Some(HOST_IMAGE),
            env: Some(vec![
                &format!("TOKEN={}", inst.host_com_token.lock().await),
                &format!("RUNNER_URL={}", get_runner_addr(self.config.lock().await.config.port).await),
            ]),
            ..Default::default()
        }).await.map_err(Error::Docker)?;

        for w in container_r.warnings {
            error!("Warning received while creating container: {}", w);
        }

        *container_id_lock = Some(container_r.id.clone());

        self.storage.lock().await.update_instance(id.to_string(), StoredInstance {
            name: inst.name.lock().await.clone(),
            inst_type: inst.inst_type.lock().await.clone(),
            host_com_token: inst.host_com_token.lock().await.clone(),
            container_id: container_id_lock.clone(),
        }).await.map_err(Error::Storage)?;

        Ok(container_r.id)
    }
    async fn delete_container(
        &self,
        id: impl std::fmt::Display,
        inst: &Instance,
    ) -> Result<(), Error> {
        let mut container_id_lock = inst.container_id.lock().await;
        let container_id = container_id_lock.clone().ok_or(Error::ContainerIdNotFound)?;

        let c = self.docker_handle.inspect_container(&container_id, None).await.map_err(Error::Docker)?;

        if c.state.ok_or(Error::NoContainerState)?.running.unwrap_or(false) {
            debug!("Stopping container {} for deletion...", container_id);
            self.docker_handle.stop_container(&container_id, None::<container::StopContainerOptions>).await.map_err(Error::Docker)?;
        }

        debug!("Deleting container {}...", container_id);

        self.docker_handle.remove_container(
            &container_id,
            None::<RemoveContainerOptions>,
        ).await.map_err(Error::Docker)?;

        *container_id_lock = None;

        self.storage.lock().await.update_instance(id.to_string(), StoredInstance {
            name: inst.name.lock().await.clone(),
            inst_type: inst.inst_type.lock().await.clone(),
            host_com_token: inst.host_com_token.lock().await.clone(),
            container_id: None,
        }).await.map_err(Error::Storage)?;

        Ok(())
    }
    async fn set_inst_status(&self, id: impl std::fmt::Display, status: InstanceStatus) -> Result<(), Error> {
        let instances = self.instances.lock().await.clone();
        let inst = instances.get(&id.to_string()).ok_or(Error::InstanceNotFound(id.to_string()))?;

        *inst.status.lock().await = status;

        let _ = self.g_event_tx.send(GlobalEvent::ModifyInstance { id: id.to_string(), instance: to_pub_instance(inst).await });

        Ok(())
    }
    async fn set_inst_status_in(&self, id: impl std::fmt::Display, inst: &Instance, status: InstanceStatus) -> Result<(), Error> {
        *inst.status.lock().await = status;

        let _ = self.g_event_tx.send(GlobalEvent::ModifyInstance { id: id.to_string(), instance: to_pub_instance(inst).await });

        Ok(())
    }
    async fn start_bg(&self) -> Result<(), Error> {
        let provider = self.clone();

        let handle = tokio::spawn(async move {
            loop {
                match provider.bg_loop().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Error in background loop: {}", e);
                    }
                };
            }
        });

        *self.bg_handle.lock().await = Some(handle);

        Ok(())
    }
    async fn bg_loop(&self) -> Result<(), Error> {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(INSTANCE_CHECK_INTERVAL_MS)).await;

            debug!("Checking instances");

            for instance in self.instances.lock().await.clone().iter() {
                let id = instance.0;
                let inst = instance.1;

                debug!("Checking instance {}", id);
    
                let container_id = tokio::select! {
                    container = inst.container_id.lock() => container.clone(),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(INSTANCE_CHECK_CONTAINER_ID_LOCK_TIMEOUT_MS)) => {
                        debug!("Timeout waiting for container_id lock for instance {}", id);
                        continue;
                    }
                };

                if let Some(container_id) = container_id {
                    debug!("Checking container {}", container_id);

                    let containers = self.docker_handle.list_containers(Some(ListContainersOptions::<String> {
                        all: true,
                        ..Default::default()
                    })).await.map_err(Error::Docker)?;

                    let con = containers.iter().find(|c| c.id.as_ref() == Some(&container_id));

                    match con {
                        Some(_) => {
                            debug!("Container {} found", container_id);
                            let c = self.docker_handle.inspect_container(&container_id, None).await.map_err(Error::Docker)?;

                            if c.state.and_then(|s| s.running).unwrap_or(false) {
                                debug!("Container {} running", container_id);
                                if *inst.status.lock().await == InstanceStatus::Inactive {
                                    self.set_inst_status_in(&id, inst, InstanceStatus::Running).await?;
                                }
                            } else {
                                debug!("Container {} not running", container_id);

                                if *inst.status.lock().await == InstanceStatus::Running {
                                    self.set_inst_status_in(&id, inst, InstanceStatus::Inactive).await?;
                                }
                            };
                        }
                        None => {
                            error!("Container {} not found (was attached to instance: {})", container_id, id);
                            *inst.container_id.lock().await = None;
                            
                            self.set_inst_status_in(&id, inst, InstanceStatus::Inactive).await?;

                            self.storage.lock().await.update_instance(id.clone(), StoredInstance {
                                name: inst.name.lock().await.clone(),
                                inst_type: inst.inst_type.lock().await.clone(),
                                host_com_token: inst.host_com_token.lock().await.clone(),
                                container_id: None,
                            }).await.map_err(Error::Storage)?;
                        }
                    };
                } else {
                    debug!("No container attached to instance {}", id);
                };
            };
        };
    }
}

async fn to_pub_instance(inst: &Instance) -> PubInstance {
    PubInstance {
        name: inst.name.lock().await.clone(),
        inst_type: inst.inst_type.lock().await.clone(),
        status: inst.status.lock().await.clone(),
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

async fn new_container_name() -> String {
    let rand_name_suffix: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(48)
            .map(char::from)
            .collect();
    
    format!("vk-{}", rand_name_suffix)
}

async fn get_runner_addr(port: u16) -> String {
    format!("{}{}", RUNNER_ADDR_PREFIX, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unique_token() {
        let keys = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let token = unique_token(keys).await.unwrap();

        assert_eq!(token.len(), 64);
    }
    #[tokio::test]
    async fn test_new_container_name() {
        let name = new_container_name().await;

        if name.len() > 64 {
            panic!("Container name too long: {}", name);
        }
    }
}
