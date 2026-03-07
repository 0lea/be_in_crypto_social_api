use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone)]
pub struct Like {
    pub user_id: UserId,
    pub display_name: String,
}
// "valid": true,
