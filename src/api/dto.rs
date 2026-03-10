use crate::{
    application::commands::AddLikeCommandResult,
    domain::like::{ContentId, ContentType},
};
use serde::{Deserialize, Serialize};

// crud
#[derive(Deserialize, Debug)]
pub struct LikeRequest {
    pub content_type: ContentType,
    pub content_id: ContentId,
}

#[derive(Debug, Serialize)]
pub struct LikeResponse {
    pub liked: bool,
    pub already_existed: bool,
    pub count: u64,
    pub liked_at: chrono::DateTime<chrono::Utc>,
}

impl From<AddLikeCommandResult> for LikeResponse {
    fn from(value: AddLikeCommandResult) -> Self {
        LikeResponse {
            already_existed: value.already_existed,
            liked: value.liked,
            count: value.count,
            liked_at: value.liked_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UnlikeResponse {
    pub liked: bool,
    pub was_liked: bool,
    pub count: u64,
}
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub request_id: String,
}
