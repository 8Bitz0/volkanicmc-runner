use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::debug;

use crate::instance::Instance;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum GlobalEvent {
    #[serde(rename = "modify-instance")]
    ModifyInstance { id: String, instance: Instance },
}

pub fn init_channel() -> broadcast::Sender<GlobalEvent> {
    let (tx, rx) = broadcast::channel(4096);

    tokio::spawn(async move {
        let mut rx = rx;

        loop {
            let event = rx.recv().await.unwrap();

            debug!("Global event issued: {:#?}", event);
        }
    });

    tx
}