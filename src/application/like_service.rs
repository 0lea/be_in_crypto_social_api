use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use futures_util::lock::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::{
    api::dto::{
        BatchStatusResponse, ContentCount, ContentItem, ContentStatus, CountResponse,
        PaginationCursor, SseEventType, SseLikeEvent, StatusResponse, TopLikesResponse,
        UnlikeResponse, UserLikesResponse,
    },
    application::{
        commands::{AddLikeCommand, AddLikeCommandResult},
        sse::SseManager,
    },
    domain::{
        errors::DomainError,
        external_validator::ExternalValidator,
        like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
        user::UserId,
    },
};
use std::{collections::HashMap, sync::Arc};

enum RefreshType {
    Count,
    Leaderboard,
}

pub struct LikeService {
    db_repo: Arc<dyn LikeDbRepository>,
    cache_repo: Arc<dyn LikeCacheRepository>,
    extern_repo: Arc<dyn ExternalValidator>,
    pub sse_manager: Arc<SseManager>,
    flight_locks: DashMap<String, Arc<Mutex<()>>>,
    pub cancellation_token: CancellationToken,
}

impl LikeService {
    pub fn new(
        db_repo: Arc<dyn LikeDbRepository>,
        cache_repo: Arc<dyn LikeCacheRepository>,
        extern_repo: Arc<dyn ExternalValidator>,
        sse_manager: Arc<SseManager>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            db_repo,
            cache_repo,
            extern_repo,
            sse_manager,
            flight_locks: DashMap::with_capacity(100),
            cancellation_token,
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_like(
        &self,
        like_cmd: AddLikeCommand,
    ) -> Result<AddLikeCommandResult, DomainError> {
        let count: u64;
        // validate content
        self.extern_repo
            .validate_content(&like_cmd.content_type, &like_cmd.content_id)
            .await?;

        let mut created_at = chrono::Utc::now();
        let like = Like {
            user_id: like_cmd.user_id.clone(),
            content_type: like_cmd.content_type.clone(),
            content_id: like_cmd.content_id,
            created_at,
            ..Default::default()
        };

        // write on db
        let already_exists = self.db_repo.save(&like).await?;

        debug!(
            "already_exists {}, c_type:{}, c_id:{}",
            already_exists, like_cmd.content_type, like_cmd.content_id
        );

        if already_exists {
            // get curr like count for resp, if err return ( means cache && db are in err)
            count = self
                .get_like_count_with_fallback(&like_cmd.content_type, &like_cmd.content_id)
                .await?;

            // take created at form db, needed for response
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
            match self
                .cache_repo
                .increment(&like_cmd.content_type, &like_cmd.content_id)
                .await
            {
                Ok(new_count) => {
                    // Cache up, use his count
                    count = new_count;
                }
                Err(e) => {
                    tracing::error!(
                        "Resilience Mode: Cache fail, fetching count from DB: {:?}",
                        e
                    );
                    // cache fail, go to db if DB fail, return
                    count = self
                        .db_repo
                        .get_likes_count(&like_cmd.content_type, &like_cmd.content_id)
                        .await?
                }
            }
        }

        self.send_sse_event(
            &like_cmd.content_id,
            &like_cmd.content_type,
            &like_cmd.user_id,
            count,
            SseEventType::Like,
        );
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
        // validate content so we are sure is not a fake content
        self.extern_repo
            .validate_content(content_type, content_id)
            .await?;

        // remove from db returning if it was an axisting like
        let was_liked = self
            .db_repo
            .remove(user_id, content_type, content_id)
            .await?;

        // get count in cache, fallback DB, if fail we are unable to return consistent count, return err
        let mut curr_count = self
            .get_like_count_with_fallback(content_type, content_id)
            .await?;

        if !was_liked {
            // not present in db, no decr no sse
            return Ok(crate::api::dto::UnlikeResponse {
                liked: false,
                was_liked,
                count: curr_count,
            });
        }

        if curr_count != 0 {
            // avoid set cache to -1
            match self.cache_repo.decrement(content_type, content_id).await {
                Ok(new_count) => curr_count = new_count,
                Err(e) => {
                    tracing::error!(
                        "Resilience Mode: Cache fail, fetching count from DB: {:?}",
                        e
                    );
                    // cache fail, go to db if DB fail, return
                    curr_count = self
                        .db_repo
                        .get_likes_count(content_type, content_id)
                        .await?
                }
            }
        }

        self.send_sse_event(
            content_id,
            content_type,
            user_id,
            curr_count,
            SseEventType::Unlike,
        );

        Ok(UnlikeResponse {
            liked: false,
            was_liked,
            count: curr_count,
        })
    }

    pub async fn get_like_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<CountResponse, DomainError> {
        let count = self
            .get_like_count_with_fallback(content_type, content_id)
            .await?;

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

    pub async fn get_likes_count_batch(
        &self,
        items: Vec<ContentItem>,
    ) -> Result<Vec<ContentCount>, DomainError> {
        if items.len() > 100 {
            return Err(DomainError::BatchTooLarge(items.len()));
        }

        let cached_counts = self
            .cache_repo
            .get_counts_batch(&items)
            .await
            .unwrap_or_default();

        let mut results = Vec::with_capacity(items.len());
        let mut missing_items = Vec::new();

        for item in items {
            if let Some(&count) = cached_counts.get(&item.content_id) {
                results.push(ContentCount {
                    content_id: item.content_id,
                    content_type: item.content_type,
                    count,
                });
            } else {
                missing_items.push(item);
            }
        }

        if missing_items.is_empty() {
            return Ok(results);
        }

        let mut to_fetch_from_db = Vec::new();
        let mut locked_keys = Vec::new();

        for item in missing_items {
            let lock_key =
                Self::format_lock_key(RefreshType::Count, &item.content_type, &item.content_id);

            if self
                .flight_locks
                .insert(lock_key.clone(), Arc::new(Mutex::new(())))
                .is_none()
            {
                locked_keys.push(lock_key);
                to_fetch_from_db.push(item);
            }
        }

        if !to_fetch_from_db.is_empty() {
            let db_counts = self.db_repo.get_counts_batch(&to_fetch_from_db).await?;

            results.extend(db_counts.clone());

            let cache_repo = self.cache_repo.clone();
            let rehydration_set = self.flight_locks.clone();

            tokio::spawn(async move {
                let _ = cache_repo.set_counts_batch(db_counts).await;
                // Rilasciamo i lock solo DOPO la scrittura in cache
                for key in locked_keys {
                    rehydration_set.remove(&key);
                }
            });
        }

        Ok(results)
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

        // check if there is one more page
        let mut next_cursor = None;
        if items.len() > limit {
            let last_item = &items[limit - 1];
            let next_cursor_obj = PaginationCursor {
                t: last_item.created_at,
                id: last_item.id,
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
            if !is_fresh {
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

    /// send fire-and-forget SSE event
    fn send_sse_event(
        &self,
        content_id: &ContentId,
        content_type: &ContentType,
        user_id: &UserId,
        count: u64,
        e_type: SseEventType,
    ) {
        let event = SseLikeEvent {
            content_id: Some(content_id.to_owned()),
            content_type: Some(content_type.clone()),
            user_id: Some(user_id.clone()),
            event: e_type,
            count: Some(count),
            timestamp: chrono::Utc::now(),
        };
        let cache_cl = self.cache_repo.clone();
        tokio::spawn(async move { cache_cl.publish_like_event(&event).await });
    }

    async fn get_cache_like_count(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<u64, DomainError> {
        let count = self.cache_repo.get_count(c_type, c_id).await?;

        let Some(count) = count else {
            return Err(DomainError::CacheMiss);
        };

        debug!(
            "found count in cache for {} {}, count: {}",
            c_type, c_id, count
        );

        Ok(count)
    }

    /// this method fail if cache & DB fail, otherwise return count
    async fn get_like_count_with_fallback(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<u64, DomainError> {
        if let Ok(count) = self.get_cache_like_count(c_type, c_id).await {
            return Ok(count);
        }

        let lock_key = Self::format_lock_key(RefreshType::Count, c_type, c_id);
        let lock = self
            .flight_locks
            .entry(lock_key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // DOUBLE-CHECK
        if let Ok(count) = self.get_cache_like_count(c_type, c_id).await {
            self.flight_locks.remove(&lock_key); // Pulizia
            return Ok(count);
        }

        tracing::warn!("Cache miss, fetching from DB");

        let count = self.db_repo.get_likes_count(c_type, c_id).await?;

        // sync cache refresh
        let _ = self.cache_repo.set_value(c_type, c_id, count).await;

        //unlock others
        self.flight_locks.remove(&lock_key);
        Ok(count)
    }

    fn format_lock_key(rtype: RefreshType, c_type: &ContentType, c_id: &ContentId) -> String {
        match rtype {
            RefreshType::Count => format!("lock:count:{}:{}", c_type.as_str(), c_id.0),
            RefreshType::Leaderboard => format!("lock:leaderboard:{}", c_type.as_str()),
        }
    }
}
