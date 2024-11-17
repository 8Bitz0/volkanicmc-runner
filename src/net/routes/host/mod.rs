use axum::extract::State;
use hyper::{HeaderMap, StatusCode};

use crate::AppState;

pub mod definition;

pub async fn auth(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let _ = get_host(headers, state).await?;

    Ok(StatusCode::OK)
}

pub async fn heartbeat(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let instance_id = get_host(headers, state.clone()).await?;

    state.instances.lock().await.set_last_con(&instance_id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

async fn get_host(
    headers: HeaderMap,
    state: AppState,
) -> Result<String, StatusCode> {
    let auth_header = headers.get("Authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .split_whitespace()
        .nth(1)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    state.instances.lock().await.find_token(token).await
        .ok_or(StatusCode::UNAUTHORIZED)
}
