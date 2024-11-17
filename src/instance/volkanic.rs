use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum VolkanicSource {
    #[serde(rename = "base64")]
    Base64(String),
}
