use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum VolkanicSource {
    #[serde(rename = "url")]
    Url(String),
    #[serde(rename = "base64")]
    Base64(String),
}
