use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType},
    user::UserId,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ValidationResult {
    Valid(Uuid),
    NotFound,
}

#[async_trait]
pub trait ExternalValidator: Send + Sync {
    async fn validate_user(&self, token: &UserId) -> Result<Uuid, DomainError>;

    async fn validate_content(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<Uuid, DomainError>;

    async fn health_check(&self) -> Result<(), DomainError>;
}
