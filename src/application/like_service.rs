use crate::{
    application::commands::{AddLikeCommand, AddLikeCommandResult},
    domain::{
        errors::DomainError,
        external_validator::ExternalValidator,
        like::{Like, LikeCacheRepository, LikeDbRepository},
    },
};
use std::sync::Arc;

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

        self.db_repo.save(&like).await?;

        let new_count = match self
            .cache_repo
            .increment(&like_cmd.content_type, &like_cmd.content_id)
            .await
        {
            Ok(count) => count,
            Err(e) => {
                tracing::error!("Cache failure, rolling back DB: {:?}", e);
                //TODO: to do it atomicly and in transaction + leaderboad update
                // let _ = self.db_repo.delete(user_id, &c_type, &c_id).await;
                return Err(e);
            }
        };

        Ok(AddLikeCommandResult {
            liked: true,
            count: new_count,
            liked_at: chrono::Utc::now(),
            request_id: "get_from_context".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::domain::{
        errors::DomainError,
        like::{ContentId, ContentType},
        user::UserId,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use mockall::{mock, predicate::*};
    use uuid::Uuid;

    // Generiamo i Mock automaticamente
    mock! {
        pub DbRepo {}
        #[async_trait]
        impl LikeDbRepository for DbRepo {


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
    }

    mock! {
        pub CacheRepo {}
        #[async_trait]
        impl LikeCacheRepository for CacheRepo {
            async fn increment(
                &self,
                content_type: &ContentType,
                content_id: &ContentId,
            ) -> Result<u64, DomainError>;

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
    }

    mock! {
        pub Validator {}
        #[async_trait]
        impl ExternalValidator for Validator {
            async fn validate_user(&self, token: &UserId) -> Result<Uuid, DomainError>;
            async fn validate_content(
                &self,
                content_type: &ContentType,
                content_id: &ContentId,
            ) -> Result<Uuid, DomainError>;
        }
    }

    #[tokio::test]
    async fn test_add_like_success() {
        let mut db = MockDbRepo::new();
        let mut cache = MockCacheRepo::new();
        let mut val = MockValidator::new();

        let u_id = UserId(Uuid::new_v4());
        let c_type = ContentType::new("post");
        let c_id = ContentId(Uuid::new_v4());

        // Aspettative: validazione ok, db ok, cache ok
        val.expect_validate_content()
            .returning(|_, _| Ok(Uuid::new_v4()))
            .once();
        db.expect_save().returning(|_| Ok(())).once();
        cache.expect_increment().returning(|_, _| Ok(1)).once();

        let service = LikeService::new(Arc::new(db), Arc::new(cache), Arc::new(val));

        let like_cmd = AddLikeCommand {
            user_id: u_id,
            content_type: c_type,
            content_id: c_id,
        };
        let result = service.add_like(like_cmd).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.count, 1);
        assert!(response.liked);
    }

    #[tokio::test]
    async fn test_add_like_fails_if_content_invalid() {
        let mut db = MockDbRepo::new();
        let mut cache = MockCacheRepo::new();
        let mut val = MockValidator::new();

        val.expect_validate_content()
            .returning(|_, _| {
                Err(DomainError::NotFound {
                    id: "test_id".into(),
                    resource: "test".into(),
                })
            })
            .once();

        db.expect_save().never();
        cache.expect_increment().never();

        let like_cmd = AddLikeCommand {
            user_id: UserId(Uuid::new_v4()),
            content_type: ContentType::new("post"),
            content_id: ContentId(Uuid::new_v4()),
        };
        let service = LikeService::new(Arc::new(db), Arc::new(cache), Arc::new(val));
        let result = service.add_like(like_cmd).await;

        assert!(matches!(
            result,
            Err(DomainError::NotFound { id, resource })
        ));
    }
}
