use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Unauthorized: invalid or expired token")]
    Unauthorized,

    #[error("Resource not found: {resource} with ID {id}")]
    NotFound { resource: String, id: String },

    #[error("Error max batch size exceeded: {0}")]
    BatchTooLarge(usize),

    #[error("External service unavailable: {service} (Circuit Breaker active)")]
    DependencyUnavailable { service: String },

    #[error("External service respond not found: {service} (Circuit Breaker close)")]
    DependencyNotFound { service: String },

    #[error("External service deserialize error: {service} (Circuit Breaker close)")]
    DependencyDeserializeError { service: String },

    #[error("Database internal error: {0}")]
    DatabaseError(String),

    #[error("Database error: not found: {0}")]
    DatabaseNotFound(String),

    #[error("Database indeponet error: {0}")]
    AlreadyExists(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Cache missing")]
    CacheMiss,

    #[error("Infrastructure error: {0}")]
    InfrastructureError(String),

    #[error("Rate limit exceeded. Please retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
