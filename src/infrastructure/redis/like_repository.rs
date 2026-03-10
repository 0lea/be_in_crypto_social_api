use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType, LikeCacheRepository},
};

use async_trait::async_trait;
use redis::{AsyncCommands, Client, aio::MultiplexedConnection};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub struct RedisLikeRepository {
    client: Arc<Client>,
}

impl RedisLikeRepository {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn format_key(&self, c_type: &ContentType, c_id: &ContentId) -> String {
        format!("count:{}:{}", c_type.as_str(), c_id.0)
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

        let count_key = self.format_key(c_type, c_id);
        let lb_key = format!("leaderboard:{}", c_type.as_str());
        let score = chrono::Utc::now().timestamp();
        let member = c_id.0.to_string();

        let script = redis::Script::new(
            r#"
            local new_count = redis.call('INCR', KEYS[1])
            redis.call('ZADD', KEYS[2], ARGV[1], ARGV[2])
            return new_count
        "#,
        );

        let count: u64 = script
            .key(count_key)
            .key(lb_key)
            .arg(score)
            .arg(member)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(count)
    }

    async fn decrement(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let count_key = self.format_key(c_type, c_id);
        let lb_key = format!("leaderboard:{}", c_type.as_str());
        let score = chrono::Utc::now().timestamp();
        let member = c_id.0.to_string();

        let script = redis::Script::new(
            r#"
            local new_count = redis.call('DECR', KEYS[1])
            redis.call('ZADD', KEYS[2], ARGV[1], ARGV[2])
            return new_count
            "#,
        );

        let count: i64 = script
            .key(count_key)
            .key(lb_key)
            .arg(score)
            .arg(member)
            .invoke_async(&mut conn)
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

        let count_key = self.format_key(c_type, c_id);
        let lb_key = format!("leaderboard:{}", c_type.as_str());
        let score = chrono::Utc::now().timestamp();
        let member = c_id.0.to_string();

        // Usiamo SET per la chiave singola e ZADD per la leaderboard
        let script = redis::Script::new(
            r#"
                redis.call('SET', KEYS[1], ARGV[1])
                redis.call('ZADD', KEYS[2], ARGV[2], ARGV[3])
                return redis.status_reply("OK")
            "#,
        );

        let _: i64 = script
            .key(count_key)
            .key(lb_key)
            .arg(value)
            .arg(score)
            .arg(member)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_count(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let res: Result<u64, DomainError> = conn
            .get(self.format_key(c_type, c_id))
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()));

        res
    }

    async fn get_counts_batch(
        &self,
        c_type: &ContentType,
        ids: &[ContentId],
    ) -> Result<HashMap<Uuid, u64>, DomainError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.get_conn().await?;

        let keys: Vec<String> = ids.iter().map(|id| self.format_key(c_type, id)).collect();

        let values: Vec<Option<u64>> = conn
            .mget(keys)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        let mut result = HashMap::new();
        for (i, val) in values.into_iter().enumerate() {
            result.insert(ids[i].0, val.unwrap_or(0));
        }

        Ok(result)
    }

    async fn update_leaderboard(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let key = format!("leaderboard:{}", c_type.as_str());
        let score = chrono::Utc::now().timestamp();
        let member = c_id.0.to_string();

        let _: i64 = conn
            .zadd(key, member, score)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_leaderboard_window(
        &self,
        c_type: &ContentType,
        seconds: i64,
    ) -> Result<Vec<String>, DomainError> {
        let mut conn = self.get_conn().await?;

        let now = chrono::Utc::now().timestamp();
        let start = now - seconds;

        let ids: Vec<String> = conn
            .zrevrangebyscore(format!("leaderboard:{}", c_type.as_str()), now, start)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(ids)
    }
}
