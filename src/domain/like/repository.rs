use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::api::dto::{BatchRequest, ContentCount, ContentItem, PaginationCursor, TopLikeItem};
use crate::domain::errors::DomainError;
use crate::domain::like::{ContentId, ContentType, Like};
use crate::domain::user::UserId;

#[async_trait]
pub trait LikeDbRepository: Send + Sync {
    async fn save(&self, like: &Like) -> Result<bool, DomainError>;
    async fn remove(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<bool, DomainError>;

    async fn get_like(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<Like, DomainError>;

    async fn get_likes_by_pairs(
        &self,
        user_id: &UserId,
        items: &[ContentItem],
    ) -> Result<Vec<Like>, DomainError>;

    async fn get_likes_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError>;

    async fn get_user_likes(
        &self,
        user_id: UserId,
        content_type: Option<String>,
        cursor: Option<PaginationCursor>,
        limit: usize,
    ) -> Result<Vec<Like>, DomainError>;

    async fn get_counts_batch(
        &self,
        items: &[ContentItem],
    ) -> Result<Vec<ContentCount>, DomainError>;

    async fn get_top_likes(
        &self,
        content_type: Option<&str>,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<TopLikeItem>, DomainError>;

    async fn health_check(&self) -> Result<(), DomainError>;
}
#[async_trait]
pub trait LikeCacheRepository: Send + Sync {
    async fn increment(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError>;

    async fn decrement(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError>;

    async fn set_value(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
        value: u64,
    ) -> Result<(), DomainError>;

    async fn get_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError>;

    async fn get_counts_batch<'a>(
        &'a self,
        items: &'a [ContentItem],
    ) -> Result<HashMap<ContentId, (&'a str, u64)>, DomainError>;

    async fn set_counts_batch(&self, counts: Vec<ContentCount>) -> Result<(), DomainError>;

    async fn get_leaderboard_with_canary(
        &self,
        window: &str,
        c_type: &ContentType,
    ) -> Result<(Option<Vec<TopLikeItem>>, bool), DomainError>;

    async fn set_leaderboard_with_canary(
        &self,
        window: &str,
        c_type: &ContentType,
        items: &[TopLikeItem],
        canary_ttl: u64,
    ) -> Result<(), DomainError>;

    async fn acquire_refresh_lock(
        &self,
        window: &str,
        c_type: &ContentType,
    ) -> Result<bool, DomainError>;

    async fn health_check(&self) -> Result<(), DomainError>;
}
