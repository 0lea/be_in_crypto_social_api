use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    external_validator::ExternalValidator,
    like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
    user::UserId,
};

pub struct LikeService {
    db_repo: Arc<dyn LikeDbRepository>,
    cache_repo: Arc<dyn LikeCacheRepository>,
    extern_repo: Arc<dyn ExternalValidator>,
}

impl LikeService {
    pub fn new(
        db_repo: Arc<dyn LikeDbRepository>,
        cache_repo: Arc<dyn LikeCacheRepository>,
        extern_repo: Arc<dyn ExternalValidator>,
    ) -> Self {
        Self {
            db_repo,
            cache_repo,
            extern_repo,
        }
    }

    #[tracing::instrument(
        skip(self, user_id), 
        fields(content_type = %c_type, content_id = %c_id)
    )]
    pub async fn add_like(
        &self,
        user_id: UserId,
        c_type: ContentType,
        c_id: ContentId,
    ) -> Result<(), DomainError> {
        self.extern_repo.validate_user(&user_id).await?;
        // TODO:
        // self.validator.validate_content(&c_type, &c_id).await?;

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
