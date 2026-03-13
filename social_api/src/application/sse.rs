use crate::{
    api::dto::SseLikeEvent,
    domain::like::{ContentId, ContentType, LikeCacheRepository},
};
use dashmap::DashMap;
use futures_util::{Stream, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub struct SseManager {
    channels: DashMap<String, broadcast::Sender<SseLikeEvent>>,
    cache_repo: Arc<dyn LikeCacheRepository>,
    c_token: CancellationToken,
}

impl SseManager {
    pub fn new(cache_repo: Arc<dyn LikeCacheRepository>, c_token: CancellationToken) -> Self {
        Self {
            channels: DashMap::new(),
            cache_repo,
            c_token: c_token.clone(),
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

    ///Entry point
    pub async fn run_cache_event_listener(&self) {
        tracing::info!("SseManager: Worker started");

        while !self.c_token.is_cancelled() {
            match self.cache_repo.get_async_pubsub_stream().await {
                Ok(mut stream) => {
                    tracing::info!("SseManager: Redis PubSub connected");

                    if self.process_stream(&mut stream).await.is_break() {
                        return;
                    }

                    tracing::warn!("SseManager: brocken stream , try reconnect...");
                }
                Err(e) => {
                    tracing::error!(
                        "SseManager: Redis connection erro : {:?}. retry in 2s...",
                        e
                    );
                }
            }

            tokio::select! {
                _ = self.c_token.cancelled() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {}
            }
        }
    }

    async fn process_stream(
        &self,
        stream: &mut (impl Stream<Item = String> + Unpin),
    ) -> std::ops::ControlFlow<()> {
        loop {
            tokio::select! {
                _ = self.c_token.cancelled() => {
                    tracing::info!("SseManager: Shutdown during streaming");
                    return std::ops::ControlFlow::Break(());
                }

                maybe_msg = stream.next() => {
                    match maybe_msg {
                        Some(payload) => self.handle_incoming_payload(payload),
                        None => return std::ops::ControlFlow::Continue(()),
                    }
                }
            }
        }
    }

    fn handle_incoming_payload(&self, payload: String) {
        let Ok(event) = serde_json::from_str::<SseLikeEvent>(&payload) else {
            tracing::warn!("SseManager: invalid payload received: {}", payload);
            return;
        };

        // extact c_type & c_it
        let (Some(c_type), Some(c_id)) = (&event.content_type, &event.content_id) else {
            return;
        };

        let chan_key = Self::get_key(c_type, c_id);

        // get connectect channel -> clients
        let Some(tx) = self.channels.get(&chan_key) else {
            return;
        };

        if tx.send(event).is_err() && tx.receiver_count() == 0 {
            tracing::info!(
                "SseManager: removing chan {} as it has 0 subscribers",
                chan_key
            );
            self.channels.remove(&chan_key);
        }
    }
}
