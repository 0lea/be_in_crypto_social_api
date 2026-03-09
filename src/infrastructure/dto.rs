use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UserDto {
    pub user_id: Uuid,
    pub display_name: String,
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContentDto {
    pub id: Uuid,
    pub title: String,
    pub content_type: String,
}
