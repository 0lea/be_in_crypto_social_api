use crate::api::dto::ErrorDetail;
use crate::api::dto::ErrorResponse;
use crate::domain::errors::DomainError;
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use serde_json::json;

pub struct ApiError(pub DomainError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(domain_error = %self.0, "Private ERROR");

        let (status, code, details) = match &self.0 {
            DomainError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", None),
            DomainError::ContentNotFound {
                content_type,
                content_id,
            } => (
                StatusCode::NOT_FOUND,
                "CONTENT_NOT_FOUND",
                Some(json!({ "content_type": content_type, "content_id": content_id })),
            ),
            DomainError::UnknownContentType { content_type } => (
                StatusCode::BAD_REQUEST,
                "CONTENT_TYPE_UNKNOWN",
                Some(json!({ "content_type": content_type })),
            ),
            DomainError::InvalidContentId(_) => {
                (StatusCode::BAD_REQUEST, "INVALID_CONTENT_ID", None)
            }
            DomainError::BatchTooLarge(_) => (StatusCode::BAD_REQUEST, "BATCH_TOO_LARGE", None),
            DomainError::InvalidCursor(_) => (StatusCode::BAD_REQUEST, "INVALID_CURSOR", None),
            DomainError::InvalidWindow(_) => (StatusCode::BAD_REQUEST, "INVALID_WINDOW", None),
            DomainError::RateLimited { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                Some(json!({ "retry_after": retry_after })),
                // TODO:
                // NOTA: Il middleware di rate limiting dovrebbe iniettare l'header `Retry-After`
            ),
            DomainError::DependencyUnavailable { .. } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                None,
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", None),
        };

        let body = Json(ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message: self.0.to_string(),
                request_id: "TODO_extract_from_extensions".to_string(),
                details,
            },
        });

        (status, body).into_response()
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        Self(err)
    }
}
