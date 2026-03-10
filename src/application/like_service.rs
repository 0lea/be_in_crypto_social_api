use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::{
    api::dto::{
        BatchRequest, BatchStatusResponse, ContentCount, ContentItem, ContentStatus, CountResponse,
        PaginationCursor, StatusResponse, TopLikesResponse, UnlikeResponse, UserLikesResponse,
    },
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
            ..Default::default()
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
                        ..Default::default()
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

    /// As this is a performance-critical hot path, allocations and clones are strictly avoided
    /// except for the mandatory database fallback scenario.
    /// Implements a non-blocking "fire-and-forget" strategy for cache hydration.
    pub async fn get_likes_count_batch(
        &self,
        items: Vec<ContentItem>,
    ) -> Result<Vec<ContentCount>, DomainError> {
        if items.len() > 100 {
            return Err(DomainError::BatchTooLarge(items.len()));
        }

        let cache_res = self.cache_repo.get_counts_batch(&items).await;

        if let Ok(hash_map) = cache_res {
            // Decouple counts from the map to release the borrow on 'items'

            let counts_only: HashMap<ContentId, u64> = hash_map
                .into_iter()
                .map(|(id, (_, count))| (id, count))
                .collect();

            // Consume 'items' to build the response without additional allocations
            let res: Vec<ContentCount> = items
                .into_iter()
                .map(|item| {
                    let count = counts_only.get(&item.content_id).copied().unwrap_or(0);

                    ContentCount {
                        content_id: item.content_id,
                        content_type: item.content_type, // Move ownership: Zero-clone
                        count,
                    }
                })
                .collect();

            return Ok(res);
        }

        tracing::error!(
            "Cache miss or failure, falling back to DB: {:?}",
            cache_res.unwrap_err()
        );

        let content_count = self
            .db_repo
            .get_counts_batch(&items)
            .await
            .inspect_err(|_| {
                tracing::error!("Critical: Db fallback failure");
            })?;

        let cache_repo = self.cache_repo.clone();
        // Cache hydration: Clone occurs only in the worst-case scenario (cache miss)
        let content_count_cl = content_count.clone();

        // Fire and Forget: update cache asynchronously to avoid latency on the main path
        tokio::spawn(async move {
            if let Err(e) = cache_repo.set_counts_batch(content_count_cl).await {
                warn!("Failed to refresh cache from DB: {:?}", e);
            } else {
                warn!("Refresh cache from db succedes");
            }
        });

        Ok(content_count)
    }

    // since there is a limit of 100 pair, i choose to calc the delta between db_resp | request
    // in app layer insted of a join on the db to reduce the db load
    pub async fn get_batch_status(
        &self,
        user_id: &UserId,
        items: Vec<ContentItem>,
    ) -> Result<BatchStatusResponse, DomainError> {
        if items.len() > 100 {
            return Err(DomainError::BatchTooLarge(items.len()));
        }

        let db_likes = self.db_repo.get_likes_by_pairs(user_id, &items).await?;

        let mut liked_map: HashMap<(ContentType, ContentId), chrono::DateTime<Utc>> =
            HashMap::with_capacity(db_likes.len());
        for record in db_likes {
            liked_map.insert((record.content_type, record.content_id), record.created_at);
        }

        let results = items
            .into_iter()
            .map(|item| {
                let key = (item.content_type.clone(), item.content_id);

                if let Some(created_at) = liked_map.get(&key) {
                    ContentStatus {
                        content_type: item.content_type,
                        content_id: item.content_id,
                        liked: true,
                        liked_at: Some(*created_at),
                    }
                } else {
                    ContentStatus {
                        content_type: item.content_type,
                        content_id: item.content_id,
                        liked: false,
                        liked_at: None,
                    }
                }
            })
            .collect();

        Ok(BatchStatusResponse { results })
    }

    pub async fn get_user_liked_items(
        &self,
        user_id: UserId,
        content_type: Option<String>,
        cursor_str: Option<String>,
        limit: usize,
    ) -> Result<UserLikesResponse, DomainError> {
        let cursor = PaginationCursor::decode_opt(cursor_str)?;

        let mut items = self
            .db_repo
            .get_user_likes(user_id, content_type, cursor, limit + 1)
            .await?;

        // 3. Controlla se c'è un'altra pagina
        let mut next_cursor = None;
        if items.len() > limit {
            let last_item = &items[limit - 1];
            let next_cursor_obj = PaginationCursor {
                t: last_item.created_at,
                id: last_item.id, // Assumendo che il domain model abbia l'ID della riga
            };

            let serialized = serde_json::to_vec(&next_cursor_obj).unwrap();
            next_cursor = Some(general_purpose::STANDARD.encode(serialized));
            items.truncate(limit);
        }

        Ok(UserLikesResponse {
            items: items.into_iter().map(Into::into).collect(),
            next_cursor,
        })
    }

    pub async fn get_top_likes(
        &self,
        content_type: ContentType,
        window_str: String,
        limit: i64,
    ) -> Result<TopLikesResponse, DomainError> {
        let limit = limit.min(50);

        let since = match window_str.as_str() {
            "24h" => Some(chrono::Utc::now() - chrono::Duration::hours(24)),
            "7d" => Some(chrono::Utc::now() - chrono::Duration::days(7)),
            "30d" => Some(chrono::Utc::now() - chrono::Duration::days(30)),
            "all" => None,
            _ => return Err(DomainError::ValidationError("Invalid window".into())),
        };

        let (cached_data, is_fresh) = self
            .cache_repo
            .get_leaderboard_with_canary(&window_str, &content_type)
            .await
            .unwrap_or((None, false));

        if let Some(items) = cached_data {
            // 2. Se i dati sono vecchi (Canary morto)
            if !is_fresh {
                // PROVIAMO A PRENDERE IL LOCK
                if self
                    .cache_repo
                    .acquire_refresh_lock(&window_str, &content_type)
                    .await
                    .unwrap_or(false)
                {
                    tracing::info!(
                        "Lock acquired for {}. Refreshing in background.",
                        window_str
                    );

                    let db_repo = self.db_repo.clone();
                    let cache_repo = self.cache_repo.clone();
                    let w_str = window_str.clone();
                    let since_val = since;
                    let c_type = content_type.clone();

                    tokio::spawn(async move {
                        match db_repo
                            .get_top_likes(c_type.as_opt(), since_val, limit)
                            .await
                        {
                            Ok(new_items) => {
                                let canary_ttl = if w_str == "24h" { 30 } else { 300 };
                                let _ = cache_repo
                                    .set_leaderboard_with_canary(
                                        &w_str, &c_type, &new_items, canary_ttl,
                                    )
                                    .await;
                                tracing::info!("Background refresh completed for {}.", w_str);
                            }
                            Err(e) => tracing::error!("Background refresh failed: {}", e),
                        }
                    });
                } else {
                    tracing::debug!(
                        "Refresh already in progress for {}. Serving stale data.",
                        window_str
                    );
                }
            }

            return Ok(TopLikesResponse {
                window: window_str,
                content_type,
                items,
            });
        }

        let items = self
            .db_repo
            .get_top_likes(content_type.as_opt(), since, limit)
            .await?;

        let _ = self
            .cache_repo
            .set_leaderboard_with_canary(&window_str, &content_type, &items, 30)
            .await;

        Ok(TopLikesResponse {
            window: window_str,
            content_type,
            items,
        })
    }

    pub async fn full_health_check(&self) -> Result<(), DomainError> {
        self.db_repo.health_check().await?;
        self.cache_repo.health_check().await?;
        self.extern_repo.health_check().await?;
        Ok(())
    }
}
