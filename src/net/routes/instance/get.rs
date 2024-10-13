use axum::{extract::State, Json, response::IntoResponse};

use crate::AppState;

pub async fn list_instances(State(state): State<AppState>) -> impl IntoResponse {
    let instances_lock = state.instances.lock().await;

    let instances = instances_lock.list_instance().await.unwrap();

    Json(instances)
}
