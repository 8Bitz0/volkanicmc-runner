use async_stream::stream;
use axum::{extract::State, response::sse::{Event, Sse}};
use futures_util::Stream;
use tokio::sync::broadcast;
use tracing::debug;

use crate::{AppState, global_event::GlobalEvent};

pub async fn global_event_sub(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    struct Guard {
        g_event_rx: broadcast::Receiver<GlobalEvent>,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            debug!("Client dropped event listener");
        }
    }

    debug!("Client requested event listener");

    let g_event_tx = state.g_event_tx.clone();

    let stream = stream! {
        let mut guard = Guard {
            g_event_rx: g_event_tx.subscribe(),
        };
        
        loop {
            let g_event = guard.g_event_rx.recv().await.unwrap();

            yield Event::default().json_data(g_event);
        }
    };

    Sse::new(stream)
}
