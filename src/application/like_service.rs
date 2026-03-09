use std::sync::Arc;

use crate::{
    api::dto::{LikeRequest, LikeResponse},
    application::commands::{AddLikeCommand, AddLikeCommandResult},
    domain::{
        errors::DomainError,
        external_validator::ExternalValidator,
        like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
        user::UserId,
    },
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

    #[tracing::instrument(skip(self))]
    pub async fn add_like(
        &self,
        like_cmd: AddLikeCommand,
    ) -> Result<AddLikeCommandResult, DomainError> {
        self.extern_repo
            .validate_content(&like_cmd.content_type, &like_cmd.content_id)
            .await?;

        let like = Like {
            user_id: like_cmd.user_id.clone(),
            content_type: like_cmd.content_type.clone(),
            content_id: like_cmd.content_id.clone(),
            created_at: chrono::Utc::now(),
        };

        //TODO: to do it atomicly and in transaction + leaderboad update
        self.db_repo.save(&like).await?;
        let new_count = self
            .cache_repo
            .increment(&like_cmd.content_type, &like_cmd.content_id)
            .await?;

        Ok(AddLikeCommandResult {
            liked: true,
            count: new_count,
            liked_at: chrono::Utc::now(),
            request_id: "get_from_context".into(),
        })
    }
}
