// mod tests {
//     use async_trait::async_trait;
//     use chrono::{DateTime, Utc};
//
//     use crate::{
//         LikeService,
//         api::dto::{ContentCount, ContentItem, PaginationCursor, TopLikeItem},
//         domain::{
//             errors::DomainError,
//             like::{ContentId, ContentType, Like, LikeCacheRepository, LikeDbRepository},
//             user::UserId,
//         },
//     };
//     use std::{collections::HashMap, sync::Arc, sync::Mutex};
//     // ... (altri import già presenti)
//
//     // --- MANUAL DB MOCK ---
//     pub struct FakeDbRepo {
//         // Usiamo un Mutex per simulare il comportamento del DB e contare le chiamate
//         pub call_count: Arc<Mutex<i32>>,
//         pub items_to_return: Vec<TopLikeItem>,
//         pub should_fail: bool,
//     }
//
//     impl FakeDbRepo {
//         pub fn new() -> Self {
//             Self {
//                 call_count: Arc::new(Mutex::new(0)),
//                 items_to_return: vec![],
//                 should_fail: false,
//             }
//         }
//     }
//
//     #[async_trait]
//     impl LikeDbRepository for FakeDbRepo {
//         async fn get_top_likes(
//             &self,
//             _content_type: Option<&str>,
//             _since: Option<DateTime<Utc>>,
//             _limit: i64,
//         ) -> Result<Vec<TopLikeItem>, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::DatabaseError("DB Fail".into()));
//             }
//             let mut count = self.call_count.lock().unwrap();
//             *count += 1;
//             Ok(self.items_to_return.clone())
//         }
//
//         // Implementazioni dummy per il resto del trait
//         async fn get_like(
//             &self,
//             _: &UserId,
//             _: &ContentType,
//             _: &ContentId,
//         ) -> Result<Like, DomainError> {
//             unimplemented!()
//         }
//         async fn save(&self, _: &Like) -> Result<bool, DomainError> {
//             Ok(true)
//         }
//         async fn remove(
//             &self,
//             _: &UserId,
//             _: &ContentType,
//             _: &ContentId,
//         ) -> Result<bool, DomainError> {
//             Ok(true)
//         }
//         async fn get_likes_count(
//             &self,
//             _: &ContentType,
//             _: &ContentId,
//         ) -> Result<u64, DomainError> {
//             Ok(0)
//         }
//         async fn get_user_likes(
//             &self,
//             _: UserId,
//             _: Option<String>,
//             _: Option<PaginationCursor>,
//             _: usize,
//         ) -> Result<Vec<Like>, DomainError> {
//             Ok(vec![])
//         }
//         async fn get_counts_batch(
//             &self,
//             _: &[ContentItem],
//         ) -> Result<Vec<ContentCount>, DomainError> {
//             Ok(vec![])
//         }
//         async fn get_likes_by_pairs(
//             &self,
//             _: &UserId,
//             _: &[ContentItem],
//         ) -> Result<Vec<Like>, DomainError> {
//             Ok(vec![])
//         }
//         async fn health_check(&self) -> Result<(), DomainError> {
//             Ok(())
//         }
//     }
//
//     // --- AGGIORNAMENTO FAKE CACHE PER LEADERBOARD ---
//     // Aggiungiamo campi al tuo FakeCacheRepo per gestire il Canary e il Lock
//     pub struct FakeCacheRepo {
//         pub should_fail: bool,
//         pub forced_count: u64,
//         pub cached_items: Arc<Mutex<Option<Vec<TopLikeItem>>>>,
//         pub is_fresh: Arc<Mutex<bool>>,
//         pub lock_acquired: Arc<Mutex<bool>>,
//     }
//
//     impl FakeCacheRepo {
//         pub fn new() -> Self {
//             Self {
//                 should_fail: false,
//                 forced_count: 10,
//                 cached_items: Arc::new(Mutex::new(None)),
//                 is_fresh: Arc::new(Mutex::new(false)),
//                 lock_acquired: Arc::new(Mutex::new(false)),
//             }
//         }
//     }
//
//     #[async_trait]
//     impl LikeCacheRepository for FakeCacheRepo {
//         async fn increment(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fake fail".into()));
//             }
//             Ok(self.forced_count)
//         }
//
//         async fn decrement(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fake fail".into()));
//             }
//             Ok(self.forced_count)
//         }
//
//         async fn set_value(
//             &self,
//             _: &ContentType,
//             _: &ContentId,
//             _: u64,
//         ) -> Result<(), DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fake fail".into()));
//             }
//             Ok(())
//         }
//
//         async fn get_count(&self, _: &ContentType, _: &ContentId) -> Result<u64, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fake fail".into()));
//             }
//             Ok(self.forced_count)
//         }
//
//         async fn get_counts_batch<'a>(
//             &'a self,
//             items: &'a [ContentItem],
//         ) -> Result<HashMap<ContentId, (&'a str, u64)>, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Cache Miss/Fail simulated".into()));
//             }
//             let mut map = HashMap::new();
//             for item in items {
//                 map.insert(
//                     item.content_id,
//                     (item.content_type.as_str(), self.forced_count),
//                 );
//             }
//             Ok(map)
//         }
//
//         async fn update_leaderboard(
//             &self,
//             _: &ContentType,
//             _: &ContentId,
//         ) -> Result<(), DomainError> {
//             Ok(())
//         }
//
//         async fn get_leaderboard_window(
//             &self,
//             _: &ContentType,
//             _: i64,
//         ) -> Result<Vec<String>, DomainError> {
//             Ok(vec![])
//         }
//
//         async fn set_counts_batch(&self, _: Vec<ContentCount>) -> Result<(), DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fire and forget fail".into()));
//             }
//             Ok(())
//         }
//
//         async fn health_check(&self) -> Result<(), DomainError> {
//             Ok(())
//         }
//
//         async fn get_leaderboard_with_canary(
//             &self,
//             window: &str,
//             c_type: Option<&str>,
//         ) -> Result<(Option<Vec<TopLikeItem>>, bool), DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fire and forget fail".into()));
//             }
//             Ok((None, false))
//         }
//
//         async fn set_leaderboard_with_canary(
//             &self,
//             window: &str,
//             c_type: Option<&str>,
//             items: &[TopLikeItem],
//             canary_ttl: u64,
//         ) -> Result<(), DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fire and forget fail".into()));
//             }
//             Ok(())
//         }
//
//         async fn acquire_refresh_lock(
//             &self,
//             window: &str,
//             c_type: &ContentType,
//         ) -> Result<bool, DomainError> {
//             if self.should_fail {
//                 return Err(DomainError::CacheError("Fire and forget fail".into()));
//             }
//             Ok(true)
//         }
//     }
//
//     // --- TEST CASE: LEADERBOARD COLD START ---
//     #[tokio::test]
//     async fn test_get_top_likes_cold_start() {
//         let db = Arc::new(FakeDbRepo::new());
//         let cache = Arc::new(FakeCacheRepo::new());
//         let val = Arc::new(MockValidator::new()); // Questo può restare Mockall se non ha riferimenti complessi
//
//         // Setup dati DB
//         let expected_items = vec![TopLikeItem {
//             content_id: "post1".into(),
//             content_type: "post".into(),
//             count: 100,
//         }];
//
//         // Usiamo un trucco per impostare i dati nel repo manuale
//         let mut db_manual = FakeDbRepo::new();
//         db_manual.items_to_return = expected_items.clone();
//         let db = Arc::new(db_manual);
//
//         let service = LikeService::new(db.clone(), cache.clone(), val.clone());
//
//         // 1. Eseguiamo la chiamata
//         let result = service
//             .get_top_likes(ContentType::new("post"), "24h".into(), 10)
//             .await
//             .unwrap();
//
//         // 2. Verifichiamo
//         assert_eq!(result.items.len(), 1);
//         assert_eq!(result.items[0].count, 100);
//
//         // Verifichiamo che il DB sia stato chiamato (visto che la cache era vuota)
//         assert_eq!(*db.call_count.lock().unwrap(), 1);
//     }
//
//     // --- TEST CASE: STALE WHILE REVALIDATE (LOCK) ---
//     #[tokio::test]
//     async fn test_leaderboard_stale_data_triggers_refresh() {
//         let db = Arc::new(FakeDbRepo::new());
//         let cache = Arc::new(FakeCacheRepo::new());
//         let val = Arc::new(MockValidator::new());
//
//         // Inizializziamo la cache con dati "vecchi" (is_fresh = false)
//         {
//             let mut items = cache.cached_items.lock().unwrap();
//             *items = Some(vec![TopLikeItem {
//                 content_id: "old".into(),
//                 content_type: "post".into(),
//                 count: 5,
//             }]);
//             let mut fresh = cache.is_fresh.lock().unwrap();
//             *fresh = false;
//         }
//
//         let service = LikeService::new(db.clone(), cache.clone(), val.clone());
//
//         // Chiamata: deve restituire i dati vecchi IMMEDIATAMENTE
//         let result = service
//             .get_top_likes(ContentType::new("post"), "24h".into(), 10)
//             .await
//             .unwrap();
//
//         assert_eq!(result.items[0].content_id, "old");
//
//         // Aspettiamo un attimo per il tokio::spawn del background refresh
//         tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
//
//         // Verifichiamo che il DB sia stato chiamato dal task in background
//         assert_eq!(*db.call_count.lock().unwrap(), 1);
//     }
// }
