use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType},
    user::UserId,
};

#[async_trait]
pub trait ExternalValidator: Send + Sync {
    async fn validate_user(&self, token: &str) -> Result<UserId, DomainError>;
    async fn validate_content(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError>;
}
