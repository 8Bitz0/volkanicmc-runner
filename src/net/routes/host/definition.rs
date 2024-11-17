use axum::{extract::State, Json,};
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{AppState, instance::{InstanceType, VolkanicSource}};

use super::get_host;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostDefinition {
    #[serde(rename = "type")]
    pub i_type: HostInstanceType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum HostInstanceType {
    #[serde(rename = "volkanic-construct")]
    VolkanicConstruct {
        base64: String,
    },
}

impl From<InstanceType> for HostInstanceType {
    fn from(i_type: InstanceType) -> Self {
        match i_type {
            InstanceType::Volkanic { source } => {
                match source {
                    VolkanicSource::Base64(base64) => {
                        HostInstanceType::VolkanicConstruct { base64 }
                    }
                }
            }
        }
    }
}

pub async fn get_def(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<HostDefinition>, StatusCode> {
    let instance_id = get_host(headers, state.clone()).await?;

    let instance = state.instances.lock().await.get_instance(&instance_id).await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let definition = HostDefinition {
        i_type: instance.inst_type.clone().into()
    };

    Ok(Json(definition))
}
