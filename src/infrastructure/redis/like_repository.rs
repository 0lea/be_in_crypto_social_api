use crate::{
    api::dto::{ContentCount, ContentItem, SseLikeEvent, TopLikeItem},
    domain::{
        errors::DomainError,
        like::{ContentId, ContentType, LikeCacheRepository},
        rate_limit::{RateLimitStatus, RateLimiter},
    },
    infrastructure::config::Config,
};
use async_trait::async_trait;
use futures_util::StreamExt;
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
    config: Config,
}

impl RedisLikeRepository {
    pub fn new(client: Arc<Client>) -> Self {
        let config = Config::from_env();
        Self { client, config }
    }

    async fn get_conn(&self) -> Result<MultiplexedConnection, DomainError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| DomainError::CacheError("Redis down".into()))
    }

    fn get_ttl_for_key(&self, key: &str) -> i64 {
        // Estraiamo la parte prima del primo ":" (es. "count", "leaderboard", etc.)
        let prefix = key.split(':').next().unwrap_or("");

        match prefix {
            "count" => self.config.cache_ttl_like_counts_secs,
            "leaderboard" => self.config.cache_ttl_like_counts_secs,
            "validation" => self.config.cache_ttl_content_validation_secs,
            // "user_status" => self.config.cache_ttl_user_status_secs,
            _ => 300,
        }
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
}

#[async_trait]
impl LikeCacheRepository for RedisLikeRepository {
    async fn increment(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;
        let mut pipe = redis::pipe();
        let count_key = self.format_count_key(c_type, c_id);
        let ttl = self.get_ttl_for_key(&count_key);

        let count: u64 = pipe
            .incr(&count_key, 1)
            .expire(count_key, ttl)
            .query_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(count)
    }

    async fn decrement(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;
        let mut pipe = redis::pipe();
        let count_key = self.format_count_key(c_type, c_id);
        let ttl = self.get_ttl_for_key(&count_key);

        let count: u64 = pipe
            .decr(&count_key, 1)
            .expire(count_key, ttl)
            .query_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(count)
    }

    async fn set_value(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
        value: u64,
    ) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;
        let count_key = self.format_count_key(c_type, c_id);
        let ttl = self.get_ttl_for_key(&count_key) as u64;

        let _: () = conn
            .set_ex(count_key, value, ttl)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_count(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<Option<u64>, DomainError> {
        let mut conn = self.get_conn().await?;
        let key = self.format_count_key(c_type, c_id);

        conn.get(key)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))
    }

    async fn get_counts_batch<'a>(
        &'a self,
        items: &'a [ContentItem],
    ) -> Result<HashMap<ContentId, u64>, DomainError> {
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
            if let Some(count) = opt_count {
                result.insert(item.content_id, count);
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

    async fn publish_like_event(&self, event: &SseLikeEvent) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let c_type = event.content_type.clone().ok_or(DomainError::SseKeyError)?;
        let c_id = event.content_id.ok_or(DomainError::SseKeyError)?;

        let channel = self.format_channel_key(&c_type, &c_id);

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

#[async_trait]
impl RateLimiter for RedisLikeRepository {
    async fn check_limit(
        &self,
        key: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<RateLimitStatus, DomainError> {
        let mut conn = self.get_conn().await?;

        let script = redis::Script::new(include_str!("./rate_limit.lua"));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // ARGV[1]: capacity -> limit
        // ARGV[2]: fill_rate -> tokens per second (limit / window)
        // ARGV[3]: now
        // ARGV[4]: cost -> 1
        let fill_rate = limit as f64 / window_secs as f64;

        let result: Vec<i64> = script
            .key(key)
            .arg(limit) // ARGV[1]
            .arg(fill_rate) // ARGV[2]
            .arg(now) // ARGV[3]
            .arg(1) // ARGV[4] (costo singola chiamata)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        // result[0] -> remaining_tokens
        // result[1] -> seconds_until_full (reset_after)
        // result[2] -> allowed (1 or 0)

        Ok(RateLimitStatus {
            allowed: result[2] == 1,
            remaining: result[0] as u64,
            limit,
            reset_after: result[1] as u64,
        })
    }
}
