use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage;

mod docker;
mod volkanic;

pub use docker::DockerInstanceProvider;
pub use volkanic::VolkanicSource;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Docker error: {0}")]
    Docker(bollard::errors::Error),
    #[error("Storage error: {0}")]
    Storage(storage::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Instance {
    pub name: String,
    #[serde(rename = "type")]
    pub inst_type: InstanceType,
    pub status: InstanceStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum InstanceType {
    #[serde(rename = "volkanic")]
    Volkanic { source: VolkanicSource },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

pub type InstanceList = HashMap<String, Instance>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub inst_type: InstanceType,
}
