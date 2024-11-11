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

use crate::{config::Config, instance::{StoredInstanceList, StoredInstance}};

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
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct JsonData {
    pub instances: HashMap<String, StoredInstance>,
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
    pub async fn list_instances(&self) -> Result<StoredInstanceList, Error> {
        Ok(self.data.instances.clone())
    }
    pub async fn new_instance(&mut self, id: String, inst: StoredInstance) -> Result<(), Error> {
        self.data.instances.insert(id, inst);

        self.update().await?;

        Ok(())
    }
    pub async fn get_instance<I: std::fmt::Display>(&self, id: I) -> Option<StoredInstance> {
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
