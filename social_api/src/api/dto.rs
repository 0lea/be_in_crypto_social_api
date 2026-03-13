use crate::{
    application::commands::AddLikeCommandResult,
    domain::{
        errors::DomainError,
        like::{ContentId, ContentType, Like},
        user::UserId,
    },
};
use base64::{Engine, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
    pub liked_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Deserialize)]
pub struct UserLikesQuery {
    pub content_type: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct UserLikesResponse {
    pub items: Vec<LikedItemDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LikedItemDto {
    pub content_type: String,
    pub content_id: String,
    pub liked_at: DateTime<Utc>,
}

impl From<Like> for LikedItemDto {
    fn from(like: Like) -> Self {
        Self {
            content_type: like.content_type.to_string(),
            content_id: like.content_id.to_string(),
            liked_at: like.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationCursor {
    pub t: DateTime<Utc>,
    pub id: Uuid,
}

impl PaginationCursor {
    pub fn decode_opt(cursor_str: Option<String>) -> Result<Option<Self>, DomainError> {
        cursor_str
            .map(|s| {
                let decoded = general_purpose::STANDARD
                    .decode(s)
                    .map_err(|_| DomainError::InvalidCursor("Bad Base64".into()))?;
                serde_json::from_slice(&decoded)
                    .map_err(|_| DomainError::InvalidCursor("Bad JSON".into()))
            })
            .transpose()
    }
}

// Leaderboard
#[derive(Debug, Deserialize)]
pub struct TopLikesQuery {
    pub content_type: Option<String>,
    pub window: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TopLikesResponse {
    pub window: String,
    pub content_type: ContentType,
    pub items: Vec<TopLikeItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLikeItem {
    pub content_type: String,
    pub content_id: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub content_type: ContentType,
    pub content_id: ContentId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SseEventType {
    Like,
    Unlike,
    Heartbeat,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseLikeEvent {
    pub event: SseEventType,
    pub content_type: Option<ContentType>,
    pub content_id: Option<ContentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

impl SseLikeEvent {
    pub fn from_like(like: Like, count: u64, e_type: SseEventType) -> Self {
        Self {
            content_id: Some(like.content_id),
            content_type: Some(like.content_type),
            count: Some(count),
            event: e_type,
            user_id: Some(like.user_id),
            timestamp: like.created_at,
        }
    }
}
