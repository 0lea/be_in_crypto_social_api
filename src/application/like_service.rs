use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::{
    application::commands::{AddLikeCommand, AddLikeCommandResult},
    domain::{
        errors::DomainError,
        external_validator::ExternalValidator,
        like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
        user::UserId,
    },
};
use std::{collections::HashMap, sync::Arc};

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
        let count: u64;

        self.extern_repo
            .validate_content(&like_cmd.content_type, &like_cmd.content_id)
            .await?;

        let mut created_at = chrono::Utc::now();
        let like = Like {
            user_id: like_cmd.user_id.clone(),
            content_type: like_cmd.content_type.clone(),
            content_id: like_cmd.content_id.clone(),
            created_at,
        };

        let already_exists = self.db_repo.save(&like).await?;

        info!("already_exists {}", already_exists);
        if already_exists {
            let count_res = self
                .get_like_count(&like_cmd.content_type, &like_cmd.content_id)
                .await?;
            count = count_res.count;

            let existed_like = self
                .db_repo
                .get_like(
                    &like_cmd.user_id,
                    &like_cmd.content_type,
                    &like_cmd.content_id,
                )
                .await?;
            created_at = existed_like.created_at;
        } else {
            count = match self
                .cache_repo
                .increment(&like_cmd.content_type, &like_cmd.content_id)
                .await
            {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!("Cache failure, rolling back DB: {:?}", e);
                    let _ = self
                        .db_repo
                        .remove(
                            &like_cmd.user_id,
                            &like_cmd.content_type,
                            &like_cmd.content_id,
                        )
                        .await
                        .map_err(|e| error!("Error rolling back from add_like: {:?}", e));
                    return Err(e);
                }
            };
        }

        Ok(AddLikeCommandResult {
            liked: true,
            count,
            liked_at: created_at,
            already_existed: already_exists,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn remove_like(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<UnlikeResponse, DomainError> {
        self.extern_repo
            .validate_content(content_type, content_id)
            .await?;

        let was_liked = self
            .db_repo
            .remove(user_id, content_type, content_id)
            .await?;

        let mut current_count = self
            .cache_repo
            .get_count(content_type, content_id)
            .await
            .unwrap_or(0);

        if was_liked {
            match self.cache_repo.decrement(content_type, content_id).await {
                Ok(new_count) => current_count = new_count,
                Err(e) => {
                    tracing::error!("Cache failure, rolling back DB: {:?}", e);

                    let like = Like {
                        user_id: user_id.clone(),
                        content_type: content_type.clone(),
                        content_id: content_id.clone(),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = self
                        .db_repo
                        .save(&like)
                        .await
                        .map_err(|e| error!("Error rolling back from remove_like: {:?}", e));

                    return Err(e);
                }
            }
        }

        Ok(crate::api::dto::UnlikeResponse {
            liked: false,
            was_liked,
            count: current_count,
        })
    }

    pub async fn get_like_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<CountResponse, DomainError> {
        let cache_res = self.cache_repo.get_count(content_type, content_id).await;

        if let Ok(count) = cache_res
            && count != 0
        {
            debug!("found in cache, count {}", count);
            return Ok(CountResponse {
                content_type: content_type.to_string(),
                content_id: content_id.to_string(),
                count,
            });
        }

        tracing::error!("Cache  get like count failure, fallback on DB",);

        let count = self
            .db_repo
            .get_likes_count(content_type, content_id)
            .await?;

        if count > 0 {
            tracing::warn!("refreshing cache from db resp , fallback on DB",);
            let _ = self
                .cache_repo
                .set_value(content_type, content_id, count)
                .await
                .map_err(|err| error!("Cache fail refresh count from db value: {:?}", err));
        }

        Ok(CountResponse {
            content_type: content_type.to_string(),
            content_id: content_id.to_string(),
            count,
        })
    }

    pub async fn get_like_status(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<StatusResponse, DomainError> {
        let mut res = StatusResponse::default();

        let existing_like = self
            .db_repo
            .get_like(user_id, content_type, content_id)
            .await
            .map_err(|_| {
                warn!("ask status of not existed like");
            });

        res.liked = existing_like.is_ok();

        if let Ok(existing_like) = existing_like {
            res.liked_at = Some(existing_like.created_at);
        }

        Ok(res)
    }

        }







    }





    }
}
