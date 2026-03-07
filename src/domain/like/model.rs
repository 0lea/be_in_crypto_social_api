use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::user::model::UserId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentType(String);

impl ContentType {
    pub fn new(t: &str) -> Self {
        Self(t.to_lowercase())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct Like {
    pub user_id: UserId,
    pub content_type: ContentType,
    pub content_id: ContentId,
    pub created_at: DateTime<Utc>,
}
