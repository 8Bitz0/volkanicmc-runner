use async_stream::stream;
use axum::{extract::State, response::sse::{Event, Sse}};
use hyper::{HeaderMap, StatusCode};
use futures_util::Stream;
use tokio::sync::broadcast;
use tracing::debug;

use crate::AppState;
use crate::instance::HostEvent;

use super::get_host;

pub async fn host_event_sub(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, axum::Error>>>, StatusCode> {
    struct Guard {
        host_event_rx: broadcast::Receiver<HostEvent>,
    }
    
    impl Drop for Guard {
        fn drop(&mut self) {
            debug!("Client dropped event listener");
        }
    }
    
    let instance_id = get_host(headers, state.clone()).await?;
    
    let host_event_tx = state.instances.lock().await.get_host_event_tx(instance_id).await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    debug!("Client requested event listener");

    let stream = stream! {
        let mut guard = Guard {
            host_event_rx: host_event_tx.subscribe(),
        };
        
        loop {
            let g_event = guard.host_event_rx.recv().await.unwrap();

            yield Event::default().json_data(g_event);
        }
    };

    Ok(Sse::new(stream))
}
