use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
    user::UserId,
};

pub struct LikeService {
    db_repo: Arc<dyn LikeDbRepository>,
    cache_repo: Arc<dyn LikeCacheRepository>,
}

impl LikeService {
    pub fn new(
        db_repo: Arc<dyn LikeDbRepository>,
        cache_repo: Arc<dyn LikeCacheRepository>,
    ) -> Self {
        Self {
            db_repo,
            cache_repo,
        }
    }

    pub async fn add_like(
        &self,
        user_id: UserId,
        c_type: ContentType,
        c_id: ContentId,
    ) -> Result<(), DomainError> {
        let like = Like {
            user_id,
            content_type: c_type.clone(),
            content_id: c_id.clone(),
            created_at: chrono::Utc::now(),
        };

        //TODO: to do it atomicly and in transaction + leaderboad update
        self.db_repo.save(&like).await?;
        self.cache_repo.increment(&c_type, &c_id).await?;

        Ok(())
    }
}
