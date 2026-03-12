pub mod api;
pub mod application;
pub mod domain;
pub mod infrastructure;

use crate::{
    api::{
        handlers::{
            delete_unlike, get_count, get_count_batch, get_status, get_status_batch, get_top_likes,
            get_user_likes, post_like, sse_stream,
        },
        health,
        middleware::{auth_middleware, rate_limit_layer, tracing_middleware},
    },
    application::like_service::LikeService,
    domain::{external_validator::ExternalValidator, rate_limit::RateLimiter},
    infrastructure::config::Config,
};
use axum::{
    Router,
    middleware::{from_fn, from_fn_with_state},
    routing::{delete, get, post},
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::{Arc, OnceLock};
use tokio_util::sync::CancellationToken;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

// global static var soince Prometheus is a process singleton an multiple call to it will fail
// (es. tests..)
static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

#[derive(Clone)]
pub struct RateLimitConfig {
    pub limiter: Arc<dyn RateLimiter>,
    pub limit: u64,
    pub window: u64,
}

pub fn create_app(
    like_service: Arc<LikeService>,
    extern_validator: Arc<dyn ExternalValidator>,
    rate_limiter: Arc<dyn RateLimiter>,
    config: &Config,
    c_token: CancellationToken,
) -> axum::Router {
    let read_limit = RateLimitConfig {
        limiter: rate_limiter.clone(),
        limit: config.rate_limit_read_per_minute,
        window: 60,
    };

    let write_limit = RateLimitConfig {
        limiter: rate_limiter.clone(),
        limit: config.rate_limit_write_per_minute,
        window: 60,
    };

    let recorder_handle = METRICS_HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install prometheus recorder")
    });
    let metrics_handle = recorder_handle.clone();

    let metrics_cleaner = recorder_handle.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            tokio::select! {
                _ = c_token.cancelled() => {
                    break
                }
                _ = interval.tick() => {
                    metrics_cleaner.run_upkeep();
                }
            }
        }
    });

    let public_routes = Router::new()
        .route("/likes/batch/counts", post(get_count_batch))
        .route("/likes/{content_type}/{content_id}/count", get(get_count))
        .route("/likes/top", get(get_top_likes))
        .route("/likes/stream", get(sse_stream))
        .layer(from_fn_with_state(read_limit, rate_limit_layer));

    let protected_routes = Router::new()
        .route("/likes", post(post_like))
        .route("/likes/{content_type}/{content_id}", delete(delete_unlike))
        .route("/likes/{content_type}/{content_id}/status", get(get_status))
        .route("/likes/user", get(get_user_likes))
        .route("/likes/batch/statuses", post(get_status_batch))
        .layer(from_fn_with_state(write_limit, rate_limit_layer))
        .layer(from_fn_with_state(
            extern_validator.clone(),
            auth_middleware,
        ));

    let health_routes = health::healt_router();
    let v1_routes = public_routes.merge(protected_routes);

    Router::new()
        .nest("/health", health_routes)
        .nest("/v1", v1_routes)
        .route(
            "/metrics",
            get(|| async move { metrics_handle.clone().render() }),
        )
        .with_state(like_service)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(from_fn(tracing_middleware))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
