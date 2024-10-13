use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AuthResponse {
    token: Option<String>,
    additional: Option<String>,
}

pub async fn login() -> impl IntoResponse {
    Json(AuthResponse { token: None, additional: None })
}
