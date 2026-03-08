use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::errors::DomainError;
use crate::domain::like::{ContentId, ContentType, Like};
use crate::domain::user::UserId;

pub trait LikeDbRepository: Send + Sync {
    async fn save(&self, like: &Like) -> Result<(), DomainError>;
    async fn remove(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError>;

    async fn get_user_likes(
        &self,
        user_id: &UserId,
        cursor: Option<DateTime<Utc>>,
        limit: u64,
    ) -> Result<Vec<Like>, DomainError>;
}

pub trait LikeCacheRepository: Send + Sync {
    async fn increment(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError>;

    async fn decrement(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError>;

    async fn get_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError>;

    async fn get_counts_batch(
        &self,
        content_type: &ContentType,
        ids: &[ContentId],
    ) -> Result<HashMap<Uuid, u64>, DomainError>;

    async fn update_leaderboard(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError>;

    async fn get_leaderboard_window(
        &self,
        c_type: &ContentType,
        seconds: i64,
    ) -> Result<Vec<String>, DomainError>;
}
