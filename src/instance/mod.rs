use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex};
use tracing::error;
use uuid::Uuid;

use crate::storage;

mod docker;
mod volkanic;

pub use docker::DockerInstanceProvider;
pub use volkanic::VolkanicSource;

/// Maximum allowed number of attempts to generate a unique UUID for
/// a new instance.
/// 
/// This is unlikely to ever be an issue, as there is a total of 2^128
/// possible combinations.
const MAX_UUID_GEN_ITER: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Generic(String),
    #[error("Docker error: {0}")]
    Docker(bollard::errors::Error),
    #[error("Storage error: {0}")]
    Storage(storage::Error),
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),
    #[error("Ran out of unique IDs")]
    ExhaustedUniqueIds,
    #[error("Container ID not found (container exists)")]
    ContainerIdNotFound,
    #[error("No container state")]
    NoContainerState,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PubInstance {
    pub name: String,
    #[serde(rename = "type")]
    pub inst_type: InstanceType,
    pub status: InstanceStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredInstance {
    pub name: String,
    pub inst_type: InstanceType,
    pub host_com_token: String,
    pub container_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum InstanceType {
    #[serde(rename = "volkanic")]
    Volkanic { source: VolkanicSource },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum InstanceStatus {
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "creating")]
    Creating(u8),
    #[serde(rename = "deleting")]
    Deleting,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "stopping")]
    Stopping,
}

#[derive(Debug, Clone)]
struct Instance {
    pub name: Arc<Mutex<String>>,
    pub inst_type: Arc<Mutex<InstanceType>>,
    pub status: Arc<Mutex<InstanceStatus>>,
    pub host_com_token: Arc<Mutex<String>>,
    pub last_con: Arc<Mutex<Option<chrono::NaiveDateTime>>>,
    pub container_id: Arc<Mutex<Option<String>>>,
}

pub type PubInstanceList = HashMap<String, PubInstance>;
pub type StoredInstanceList = HashMap<String, StoredInstance>;
type InstanceList = HashMap<String, Instance>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub inst_type: InstanceType,
}

async fn unique_id(keys: Vec<String>) -> Result<String, Error> {
    for _ in 0..MAX_UUID_GEN_ITER {
        let new_id = Uuid::new_v4().to_string();

        if !keys.contains(&new_id) {
            return Ok(new_id);
        }
    }

    error!(r#"Unable to create unique UUID. Either you have an astronomical
    number of instances, or there may be a much deeper issue within the
    built-in libraries or hardware."#);

    Err(Error::ExhaustedUniqueIds)
}
