use crate::domain::like::{ContentId, ContentType};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct LikeRequest {
    pub content_type: ContentType,
    pub content_id: ContentId,
}

#[derive(Debug, Serialize)]
pub struct LikeResponse {
    pub liked: bool,
    pub count: u64,
    pub liked_at: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub request_id: String,
}
