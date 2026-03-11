use crate::{
    api::dto::{ContentCount, ContentItem, SseLikeEvent, TopLikeItem},
    domain::{
        errors::DomainError,
        like::{ContentId, ContentType, LikeCacheRepository},
    },
};
use futures_util::StreamExt;

use async_trait::async_trait;
use futures_util::stream::BoxStream;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection};
use std::{collections::HashMap, sync::Arc};

const SSE_EVENTS_KEY: &str = "sse:events";

enum LeadKeyType {
    Data,
    Canary,
    Lock,
}

impl LeadKeyType {
    fn as_str(&self) -> &str {
        match self {
            LeadKeyType::Data => "data",
            LeadKeyType::Canary => "canary",
            LeadKeyType::Lock => "lock",
        }
    }
}
pub struct RedisLikeRepository {
    client: Arc<Client>,
}

impl RedisLikeRepository {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn format_count_key(&self, c_type: &ContentType, c_id: &ContentId) -> String {
        format!("count:{}:{}", c_type.as_str(), c_id.0)
    }

    fn format_channel_key(&self, c_type: &ContentType, c_id: &ContentId) -> String {
        format!("{}:{}:{}", SSE_EVENTS_KEY, c_type.as_str(), c_id.0)
    }

    fn format_all_channels_key(&self) -> String {
        format!("{}:*", SSE_EVENTS_KEY)
    }

    fn format_lead_key(&self, k_type: LeadKeyType, window: &str, c_type: &ContentType) -> String {
        format!(
            "leaderboard:{}:{}:{}",
            k_type.as_str(),
            window,
            c_type.as_str()
        )
    }

    async fn get_conn(&self) -> Result<MultiplexedConnection, DomainError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| DomainError::CacheError("Redis down".into()))
    }
}

#[async_trait]
impl LikeCacheRepository for RedisLikeRepository {
    async fn increment(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let count_key = self.format_count_key(c_type, c_id);

        let count: u64 = conn
            .incr(count_key, 1)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(count)
    }

    async fn decrement(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let count_key = self.format_count_key(c_type, c_id);
        let count: i64 = conn
            .decr(count_key, 1)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(count.max(0) as u64)
    }

    async fn set_value(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
        value: u64,
    ) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;
        let count_key = self.format_count_key(c_type, c_id);
        let _: i64 = conn
            .set(count_key, value)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_count(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let res: Result<u64, DomainError> = conn
            .get(self.format_count_key(c_type, c_id))
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()));

        res
    }

    async fn get_counts_batch<'a>(
        &'a self,
        items: &'a [ContentItem],
    ) -> Result<HashMap<ContentId, (&'a str, u64)>, DomainError> {
        if items.is_empty() {
            return Ok(HashMap::new());
        }
        let mut conn = self.get_conn().await?;

        let keys: Vec<String> = items
            .iter()
            .map(|i| self.format_count_key(&i.content_type, &i.content_id))
            .collect();

        let values: Vec<Option<u64>> = conn
            .mget(keys)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        let mut result = HashMap::with_capacity(items.len());

        for (item, opt_count) in items.iter().zip(values.into_iter()) {
            match opt_count {
                Some(count) => {
                    result.insert(item.content_id, (item.content_type.as_str(), count));
                }
                None => return Err(DomainError::CacheMiss),
            }
        }

        Ok(result)
    }

    async fn set_counts_batch(&self, counts: Vec<ContentCount>) -> Result<(), DomainError> {
        if counts.is_empty() {
            return Ok(());
        }

        let mut pipe = redis::pipe();

        for i in counts {
            let key = self.format_count_key(&i.content_type, &i.content_id);
            pipe.set_ex(key, i.count, 3600);
        }

        let mut conn = self.get_conn().await?;
        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_leaderboard_with_canary(
        &self,
        window: &str,
        c_type: &ContentType,
    ) -> Result<(Option<Vec<TopLikeItem>>, bool), DomainError> {
        let mut conn = self.get_conn().await?;

        let data_key = self.format_lead_key(LeadKeyType::Data, window, c_type);
        let canary_key = self.format_lead_key(LeadKeyType::Canary, window, c_type);

        let result: (Option<String>, Option<String>) = redis::cmd("MGET")
            .arg(&data_key)
            .arg(&canary_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        let (data_json, canary_exists) = result;

        let items = data_json.and_then(|j| serde_json::from_str(&j).ok());
        let is_fresh = canary_exists.is_some();

        Ok((items, is_fresh))
    }

    async fn set_leaderboard_with_canary(
        &self,
        window: &str,
        c_type: &ContentType,
        items: &[TopLikeItem],
        canary_ttl: u64,
    ) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let data_key = self.format_lead_key(LeadKeyType::Data, window, c_type);
        let canary_key = self.format_lead_key(LeadKeyType::Canary, window, c_type);

        let json_str = serde_json::to_string(items).unwrap_or_default();

        // Aggiorniamo i dati (senza TTL o TTL lungo) e resettiamo il canary
        let mut pipe = redis::pipe();
        pipe.set(&data_key, json_str)
            .set_ex(&canary_key, "alive", canary_ttl);

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn acquire_refresh_lock(
        &self,
        window: &str,
        c_type: &ContentType,
    ) -> Result<bool, DomainError> {
        let mut conn = self.get_conn().await?;
        let lock_key = self.format_lead_key(LeadKeyType::Lock, window, c_type);

        let acquired: Option<String> = redis::cmd("SET")
            .arg(&lock_key)
            .arg("locked")
            .arg("NX")
            .arg("EX")
            .arg(10)
            .query_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(acquired.is_some()) // Ritorna true se abbiamo preso il lock
    }

    async fn health_check(&self) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        conn.ping::<()>()
            .await
            .map_err(|e| DomainError::CacheHealthError(e.to_string()))?;

        Ok(())
    }

    // SSE

    async fn publish_like_event(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
        event: &SseLikeEvent,
    ) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let channel = self.format_channel_key(c_type, c_id);

        let payload = serde_json::to_string(event)
            .map_err(|e| DomainError::CacheError(format!("JSON error: {}", e)))?;

        let _: () = redis::cmd("PUBLISH")
            .arg(&channel)
            .arg(&payload)
            .query_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_async_pubsub_stream(&self) -> Result<BoxStream<'static, String>, DomainError> {
        let mut pubsub = self
            .client
            .get_async_pubsub()
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        let all_chan_key = self.format_all_channels_key();
        pubsub
            .psubscribe(all_chan_key)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        let stream = pubsub
            .into_on_message()
            .map(|msg| msg.get_payload::<String>().unwrap_or_default());

        Ok(stream.boxed())
    }
}
