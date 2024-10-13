use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum VkMode {
    #[serde(rename = "no-auth")]
    NoAuth,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VkInfo {
    pub version: String,
    pub protocol: u32,
    pub mode: VkMode,
}

impl VkInfo {
    fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol: crate::net::http::PROTOCOL_VER,
            mode: VkMode::NoAuth,
        }
    }
}

pub async fn info() -> impl IntoResponse {
    Json(VkInfo::new())
}
