use chrono::{DateTime, Utc};

use crate::domain::errors::DomainError;
use crate::domain::like::model::{ContentId, ContentType, Like};
use crate::domain::user::model::UserId;

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
