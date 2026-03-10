use crate::{
    api::dto::{BatchCountResponse, ContentCount, ContentItem},
    domain::{
        errors::DomainError,
        like::{ContentId, ContentType, LikeCacheRepository},
    },
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
            .map(|i| self.format_key(&i.content_type, &i.content_id))
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
            let key = self.format_key(&i.content_type, &i.content_id);
            pipe.set_ex(key, i.count, 3600);
        }

        let mut conn = self.get_conn().await?;
        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| DomainError::CacheError(e.to_string()))?;

        Ok(())
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

    async fn health_check(&self) -> Result<(), DomainError> {
        let mut conn = self.get_conn().await?;

        conn.ping::<()>()
            .await
            .map_err(|e| DomainError::CacheHealthError(e.to_string()))?;

        Ok(())
    }
}
