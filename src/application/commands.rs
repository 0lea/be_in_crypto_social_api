use crate::{
    api::dto::LikeRequest,
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
    pub already_existed: bool,
    pub count: u64,
    pub liked_at: chrono::DateTime<chrono::Utc>,
}
