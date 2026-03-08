use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LikeRequest {
    pub user_id: Uuid,
    pub content_type: String,
    pub content_id: Uuid,
}
