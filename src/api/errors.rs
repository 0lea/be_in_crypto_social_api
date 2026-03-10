use crate::api::dto::ErrorResponse;
use crate::domain::errors::DomainError;
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};

pub struct ApiError(pub DomainError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(domain_error= %self.0,  "Mapping to ApiError");

        let (status, message) = match self.0 {
            DomainError::Unauthorized => (StatusCode::UNAUTHORIZED, "Invalid session token"),
            DomainError::DependencyNotFound { service: _ } | DomainError::DatabaseNotFound(_) => {
                (StatusCode::NOT_FOUND, "Content not found")
            }
            DomainError::DependencyUnavailable { service: _ } => {
                (StatusCode::SERVICE_UNAVAILABLE, "Service unavaiable")
            }
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Errore interno del server",
            ),
        };

        let body = Json(ErrorResponse {
            error: status.canonical_reason().unwrap_or("Error").to_string(),
            message: message.to_string(),
            request_id: "context_id".into(),
        });

        (status, body).into_response()
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        Self(err)
    }
}
