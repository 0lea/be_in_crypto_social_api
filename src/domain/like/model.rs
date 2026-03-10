use core::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::user::model::UserId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Copy)]
pub struct ContentId(pub Uuid);

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ContentId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ContentType(String);

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ContentType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl ContentType {
    pub fn new(t: &str) -> Self {
        Self(t.to_lowercase())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct Like {
    pub user_id: UserId,
    pub content_type: ContentType,
    pub content_id: ContentId,
    pub created_at: DateTime<Utc>,
}
