use async_trait::async_trait;

use crate::domain::errors::DomainError;

#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check_limit(
        &self,
        key: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<RateLimitStatus, DomainError>;
}

pub struct RateLimitStatus {
    pub allowed: bool,
    pub remaining: u64,
    pub limit: u64,
    pub reset_after: u64,
}
