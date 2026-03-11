use crate::{
    api::dto::SseLikeEvent,
    domain::like::{ContentId, ContentType, LikeCacheRepository},
};
use dashmap::DashMap;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct SseManager {
    channels: DashMap<String, broadcast::Sender<SseLikeEvent>>,
    cache_repo: Arc<dyn LikeCacheRepository>,
}

impl SseManager {
    pub fn new(cache_repo: Arc<dyn LikeCacheRepository>) -> Self {
        Self {
            channels: DashMap::new(),
            cache_repo,
        }
    }

    fn get_key(c_type: &ContentType, c_id: &ContentId) -> String {
        format!("{}:{}", c_type, c_id)
    }

    pub fn subscribe(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> broadcast::Receiver<SseLikeEvent> {
        let key = Self::get_key(c_type, c_id);

        let tx = self.channels.entry(key).or_insert_with(|| {
            let (tx, _) = broadcast::channel(100);
            tx
        });
        tx.subscribe()
    }

    pub async fn run_cache_event_listener(self: Arc<Self>) {
        loop {
            match self.cache_repo.get_async_pubsub_stream().await {
                Ok(mut stream) => {
                    tracing::info!("📡 SseManager: In ascolto sugli eventi...");

                    while let Some(payload) = stream.next().await {
                        let Ok(event) = serde_json::from_str::<SseLikeEvent>(&payload) else {
                            continue;
                        };

                        let (Some(c_type), Some(c_id)) = (&event.content_type, &event.content_id)
                        else {
                            continue;
                        };

                        let local_key = Self::get_key(c_type, c_id);
                        let Some(tx) = self.channels.get(&local_key) else {
                            continue;
                        };

                        let _ = tx.send(event);
                    }
                }
                Err(e) => {
                    tracing::error!("Errore sottoscrizione: {:?}. Riprovo...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
}
