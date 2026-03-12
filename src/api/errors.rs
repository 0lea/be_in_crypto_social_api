use crate::api::dto::ErrorDetail;
use crate::api::dto::ErrorResponse;
use crate::domain::errors::DomainError;
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use serde_json::json;

pub struct ApiError(pub DomainError, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (domain_error, request_id) = (&self.0, &self.1);
        tracing::error!(domain_error = %domain_error, request_id = %request_id );

        let (status, code, details) = match domain_error {
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
                message: domain_error.to_string(),
                request_id: request_id.to_string(),
                details,
            },
        });

        (status, body).into_response()
    }
}
