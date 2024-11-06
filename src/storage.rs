use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
};
use tokio::{
    fs,
    io::AsyncWriteExt,
};
use tracing::error;
use uuid::Uuid;

use crate::{config::Config, instance::{Instance, InstanceList, InstanceRequest, InstanceStatus}};

/// Maximum allowed number of attempts to generate a unique UUID for
/// a new instance.
/// 
/// This is unlikely to ever be an issue, as there a total of 2^128
/// possible combinations.
const MAX_UUID_GEN_ITER: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("Expected file, found directory: {0}")]
    FoundDirectory(PathBuf),
    #[error("JSON decode error: {0}")]
    JsonDecode(serde_jsonc::Error),
    #[error("JSON encode error: {0}")]
    JsonEncode(serde_jsonc::Error),
    #[error("No JSON storage path set in config")]
    NoStoragePath,
    #[error("Ran out of unique IDs")]
    ExhaustedUniqueIds,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct JsonData {
    pub instances: HashMap<String, Instance>,
    /// Maps Docker container IDs to instance IDs
    #[serde(rename = "docker-container-map")]
    pub docker_con_map: HashMap<String, String>,
}

pub struct JsonStorageProvider {
    path: PathBuf,
    data: JsonData,
}

impl JsonStorageProvider {
    pub async fn new(config: Config) -> Result<Self, Error> {
        let json_path = match &config.storage.path {
            Some(o) => o.clone(),
            None => {
                return Err(Error::NoStoragePath);
            }
        };

        let mut store_file = Self {
            path: json_path.clone(),
            data: JsonData::default(),
        };

        if store_file.path.is_file() {
            store_file.load().await?;
        } else if json_path.is_dir() {
            return Err(Error::FoundDirectory(store_file.path.clone()));
        } else {
            store_file.update().await?;
        }

        Ok(store_file)
    }
    pub async fn list_instances(&self) -> Result<InstanceList, Error> {
        Ok(self.data.instances.clone())
    }
    pub async fn new_instance(&mut self, inst: InstanceRequest) -> Result<String, Error> {
        let id = unique_id(self).await?;

        let new_instance = Instance {
            name: inst.name,
            inst_type: inst.inst_type,
            status: InstanceStatus::Creating(0),
        };

        self.data.instances.insert(id.clone(), new_instance);

        self.update().await?;

        Ok(id)
    }
    pub async fn get_instance<I: std::fmt::Display>(&self, id: I) -> Option<Instance> {
        self.data.instances.get(&id.to_string()).cloned()
    }
    /// `true` is returned if the instance was removed from storage
    pub async fn del_instance<I: std::fmt::Display>(&mut self, id: I) -> Result<bool, Error> {
        let deleted = self.data.instances.remove(&id.to_string()).is_some();

        self.update().await?;

        Ok(deleted)
    }
    async fn load(&mut self) -> Result<(), Error> {
        let json_raw = fs::read_to_string(&self.path).await.map_err(Error::Io)?;

        self.data = serde_jsonc::from_str(&json_raw).map_err(Error::JsonDecode)?;

        Ok(())
    }
    async fn update(&self) -> Result<(), Error> {
        let mut f = fs::File::create(&self.path).await.map_err(Error::Io)?;

        let config_raw = serde_jsonc::to_string_pretty(&self.data).map_err(Error::JsonEncode)?;

        f.write_all(config_raw.as_bytes()).await.map_err(Error::Io)?;

        Ok(())
    }
}

async fn unique_id(storage: &JsonStorageProvider) -> Result<String, Error> {
    for _ in 0..MAX_UUID_GEN_ITER {
        let new_id = Uuid::new_v4().to_string();

        if !storage.data.instances.contains_key(&new_id) {
            return Ok(new_id);
        }
    }

    error!(r#"Unable to create unique UUID. Either you have an astronomical
    number of instances, or there may be a much deeper issue within the
    built-in libraries or hardware."#);

    Err(Error::ExhaustedUniqueIds)
}
