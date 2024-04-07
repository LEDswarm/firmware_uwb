use async_channel::{unbounded, Sender, Receiver};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ledswarm_protocol::InternalMessage;

pub struct EventBus {
    subscribers: Arc<Mutex<HashMap<String, Vec<Sender<(String, InternalMessage)>>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn subscribe(&self, tag: &str) -> Receiver<(String, InternalMessage)> {
        let (tx, rx) = unbounded();
        let mut subs = self.subscribers.lock().unwrap();
        subs.entry(tag.to_string()).or_default().push(tx);
        rx
    }

    async fn publish(&self, tag: &str, event: InternalMessage) {
        let subs = self.subscribers.lock().unwrap();
        if let Some(subscribers) = subs.get(tag) {
            for tx in subscribers {
                let _ = tx.send((tag.to_string(), event.clone())).await;
            }
        }
    }
}