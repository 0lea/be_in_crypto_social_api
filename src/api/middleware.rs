use crate::{
    api::errors::ApiError,
    application::like_service::LikeService,
    domain::{errors::DomainError, external_validator::ExternalValidator, user::UserId},
};
use axum::{
    body::Body,
    extract::State,
    http::{Request, header},
    middleware::Next,
    response::Response,
};
use std::{sync::Arc, time::Instant};
use tracing::{Instrument, info_span};
use uuid::Uuid;

pub async fn auth_middleware(
    State(validator): State<Arc<dyn ExternalValidator>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(DomainError::Unauthorized)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(DomainError::Unauthorized.into());
    }

    let raw_token = &auth_header[7..];
    let token = Uuid::parse_str(raw_token).map_err(|_| DomainError::Unauthorized)?;

    let user_id = UserId(token);
    let user_id = validator
        .validate_user(&user_id)
        .await
        .map_err(|_| DomainError::Unauthorized)?;

    req.extensions_mut().insert(user_id);

    Ok(next.run(req).await)
}

pub async fn tracing_middleware(request: Request<Body>, next: Next) -> Response {
    let start = Instant::now();

    let request_id = request
        .extensions()
        .get::<tower_http::request_id::RequestId>()
        .map(|id| id.header_value().to_str().unwrap_or("unknown"))
        .unwrap_or("unknown")
        .to_string();

    let method = request.method().clone();
    let uri = request.uri().clone();

    let span = info_span!(
        "http_request",
        %method,
        %uri,
        request_id = %request_id,
    );

    let response = async move { next.run(request).await }
        .instrument(span.clone())
        .await;

    let latency = start.elapsed().as_millis();
    let status = response.status();

    let _enter = span.enter();
    if status.is_client_error() || status.is_server_error() {
        tracing::error!(status = %status.as_u16(), latency_ms = %latency, "Request failed");
    } else {
        tracing::info!(status = %status.as_u16(), latency_ms = %latency, "Request successful");
    }

    response
}
