mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        api::dto::{ContentCount, ContentItem, PaginationCursor},
        application::{commands::AddLikeCommand, like_service::LikeService},
        domain::{
            errors::DomainError,
            external_validator::ExternalValidator,
            like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
            user::UserId,
        },
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


    async fn get_like(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<Like, DomainError>;

    async fn save(&self, like: &Like) -> Result<bool, DomainError>;
    async fn remove(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<bool, DomainError>;

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



    async fn get_likes_by_pairs(
        &self,
        user_id: &UserId,
        items: &[ContentItem],
    ) -> Result<Vec<Like>, DomainError> ;

        }
    }

    pub struct FakeCacheRepo {
        pub should_fail: bool,
        pub forced_count: u64,
    }

    impl FakeCacheRepo {
        pub fn new() -> Self {
            Self {
                should_fail: false,
                forced_count: 10,
            }
        }
    }

    #[async_trait]
    impl LikeCacheRepository for FakeCacheRepo {
        async fn increment(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Fake fail".into()));
            }
            Ok(self.forced_count)
        }

        async fn decrement(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Fake fail".into()));
            }
            Ok(self.forced_count)
        }

        async fn set_value(
            &self,
            _: &ContentType,
            _: &ContentId,
            _: u64,
        ) -> Result<(), DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Fake fail".into()));
            }
            Ok(())
        }

        async fn get_count(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Fake fail".into()));
            }
            Ok(self.forced_count)
        }

        async fn get_counts_batch<'a>(
            &'a self,
            items: &'a [ContentItem],
        ) -> Result<HashMap<ContentId, (&'a str, u64)>, DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Cache Miss/Fail simulated".into()));
            }

            let mut map = HashMap::new();
            for item in items {
                map.insert(
                    item.content_id,
                    (item.content_type.as_str(), self.forced_count),
                );
            }
            Ok(map)
        }

        async fn update_leaderboard(
            &self,
            _: &ContentType,
            _: &ContentId,
        ) -> Result<(), DomainError> {
            Ok(())
        }

        async fn get_leaderboard_window(
            &self,
            _: &ContentType,
            _: i64,
        ) -> Result<Vec<String>, DomainError> {
            Ok(vec![])
        }

        async fn set_counts_batch(&self, _: Vec<ContentCount>) -> Result<(), DomainError> {
            if self.should_fail {
                return Err(DomainError::CacheError("Fire and forget fail".into()));
            }
            Ok(())
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
        let mut val = MockValidator::new();
        let mut cache = FakeCacheRepo::new();
        cache.forced_count = 1;

        let u_id = UserId(Uuid::new_v4());
        let c_type = ContentType::new("post");
        let c_id = ContentId(Uuid::new_v4());

        val.expect_validate_content()
            .returning(|_, _| Ok(Uuid::new_v4()))
            .once();

        db.expect_save().returning(|_| Ok(false)).once();

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
        let mut cache = FakeCacheRepo::new();
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

        let like_cmd = AddLikeCommand {
            user_id: UserId(Uuid::new_v4()),
            content_type: ContentType::new("post"),
            content_id: ContentId(Uuid::new_v4()),
        };

        let service = LikeService::new(Arc::new(db), Arc::new(cache), Arc::new(val));
        let result = service.add_like(like_cmd).await;

        assert!(matches!(
            result,
            Err(DomainError::NotFound { id: _, resource: _ })
        ));
    }
}
