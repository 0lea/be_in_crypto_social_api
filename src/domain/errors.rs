use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Unexpected error")]
    InternalError(String),

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Missing, malformed, or invalid token")]
    Unauthorized,

    #[error("Content item does not exist or has been removed")]
    ContentNotFound {
        content_type: String,
        content_id: String,
    },

    #[error("content_type not in configured registry")]
    UnknownContentType { content_type: String },

    #[error("content_id is not a valid UUID v4")]
    InvalidContentId(String),

    #[error("Batch exceeds 100 items")]
    BatchTooLarge(usize),

    #[error("Pagination cursor is malformed or expired")]
    InvalidCursor(String),

    #[error("Time window not in [24h, 7d, 30d, all]")]
    InvalidWindow(String),

    #[error("Rate limit exceeded")]
    RateLimited { retry_after: u64 },

    #[error("External service unreachable (after circuit breaker opens)")]
    DependencyUnavailable { service: String },

    #[error("External service deserialize error: {service}")]
    DependencyDeserializeError { service: String },

    #[error("External service respond not found: {service} (Circuit Breaker close)")]
    DependencyNotFound { service: String },

    #[error("External service healt error: {0}")]
    DependencyHealtError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Database healt error: {0}")]
    DatabaseHealthError(String),

    #[error("Database error: not found: {0}")]
    DatabaseNotFound(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Cache miss")]
    CacheMiss,

    #[error("Cache healt error : {0}")]
    CacheHealthError(String),
}
