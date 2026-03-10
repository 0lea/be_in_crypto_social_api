use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType},
    user::UserId,
};

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
