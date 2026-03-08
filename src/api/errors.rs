use crate::domain::errors::DomainError;
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use serde_json::json;

pub struct ApiError(pub DomainError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(
            error_type = ?self.0,
            "Request failed"
        );
        let (status, error_message) = match self.0 {
            // DomainError::NotFound => (StatusCode::NOT_FOUND, "Risorsa non trovata"),
            // DomainError::Unauthorized => (StatusCode::UNAUTHORIZED, "Non autorizzato"),
            // DomainError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Troppe richieste"),
            // DomainError::ExternalServiceUnavailable => {
            //     (StatusCode::SERVICE_UNAVAILABLE, "Servizio esterno giù")
            // }
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Errore interno del server",
            ),
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.0.to_string()
        }));

        (status, body).into_response()
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        Self(err)
    }
}
