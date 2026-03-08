use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType, LikeCacheRepository},
};

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

impl LikeCacheRepository for RedisLikeRepository {
    async fn increment(&self, c_type: &ContentType, c_id: &ContentId) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let key = self.format_key(c_type, c_id);

        conn.incr::<_, i64, ()>(key, 1)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn decrement(&self, c_type: &ContentType, c_id: &ContentId) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        let key = self.format_key(c_type, c_id);

        //TODO: check logic: when 0 set to -1 ??
        conn.decr::<_, i64, ()>(key, 1)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_count(&self, c_type: &ContentType, c_id: &ContentId) -> Result<u64, DomainError> {
        let mut conn = self.get_conn().await?;

        let val: Option<u64> = conn
            .get(self.format_key(c_type, c_id))
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(val.unwrap_or(0))
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
