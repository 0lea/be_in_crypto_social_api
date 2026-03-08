use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Unauthorized: invalid or expired token")]
    Unauthorized,

    #[error("Resource not found: {resource} with ID {id}")]
    NotFound { resource: String, id: String },

    #[error("External service unavailable: {service} (Circuit Breaker active)")]
    ExternalServiceUnavailable { service: String },

    #[error("Database internal error: {0}")]
    DatabaseError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Infrastructure error: {0}")]
    InfrastructureError(String),

    #[error("Rate limit exceeded. Please retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
