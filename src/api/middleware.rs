use crate::{
    RateLimitConfig,
    api::errors::ApiError,
    domain::{
        errors::DomainError, external_validator::ExternalValidator, rate_limit::RateLimiter,
        user::UserId,
    },
};
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
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

    let mut user_id = UserId(token);
    user_id.0 = validator.validate_user(&user_id).await?;

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

fn extract_key(req: &Request<Body>) -> String {
    if let Some(user_id) = req.extensions().get::<UserId>() {
        return format!("rl:user:{}", user_id);
    }
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return format!("rl:ip:{}", addr.ip());
    }

    "rl:unknown".to_string()
}

pub async fn rate_limit_layer(
    State(cfg): State<RateLimitConfig>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let key = extract_key(&req);

    match cfg.limiter.check_limit(&key, cfg.limit, cfg.window).await {
        Ok(status) => {
            let mut response = if !status.allowed {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response()
            } else {
                next.run(req).await
            };

            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Limit", HeaderValue::from(cfg.limit));
            headers.insert("X-RateLimit-Remaining", HeaderValue::from(status.remaining));
            headers.insert("X-RateLimit-Reset", HeaderValue::from(status.reset_after));

            if !status.allowed {
                headers.insert("Retry-After", HeaderValue::from(status.reset_after));
            }
            response
        }
        Err(e) => {
            tracing::error!("Rate limiter error: {:?}", e);
            next.run(req).await
        }
    }
}
