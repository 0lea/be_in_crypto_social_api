use crate::{
    api::dto::{LikeRequest, LikeResponse},
    domain::{
        like::{ContentId, ContentType},
        user::UserId,
    },
};

#[derive(Debug)]
pub struct AddLikeCommand {
    pub user_id: UserId,
    pub content_id: ContentId,
    pub content_type: ContentType,
}

impl From<(UserId, LikeRequest)> for AddLikeCommand {
    fn from((user_id, req): (UserId, LikeRequest)) -> Self {
        Self {
            user_id,
            content_id: req.content_id,
            content_type: req.content_type,
        }
    }
}

#[derive(Default)]
pub struct AddLikeCommandResult {
    pub liked: bool,
    pub count: u64,
    pub liked_at: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
}

impl Into<LikeResponse> for AddLikeCommandResult {
    fn into(self) -> LikeResponse {
        LikeResponse {
            liked: self.liked,
            count: self.count,
            liked_at: self.liked_at,
            request_id: self.request_id,
        }
    }
}
