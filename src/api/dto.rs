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
pub struct CountResponse {
    pub content_type: String,
    pub content_id: String,
    pub count: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct StatusResponse {
    pub liked: bool,
    pub liked_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub request_id: String,
}

// batch
#[derive(Debug, serde::Deserialize)]
pub struct BatchRequest {
    pub items: Vec<ContentItem>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ContentItem {
    pub content_type: ContentType,
    pub content_id: ContentId,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ContentCount {
    pub content_type: ContentType,
    pub content_id: ContentId,
    pub count: u64,
}
#[derive(Debug, serde::Serialize)]
pub struct BatchCountResponse {
    pub results: Vec<ContentCount>,
}

#[derive(Debug, serde::Serialize)]
pub struct BatchStatusResponse {
    pub results: Vec<ContentStatus>,
}

#[derive(Debug, serde::Serialize)]
pub struct ContentStatus {
    pub content_type: ContentType,
    pub content_id: ContentId,
    pub liked: bool,
    pub liked_at: Option<chrono::DateTime<chrono::Utc>>,
}
