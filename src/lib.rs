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
        middleware::{auth_middleware, tracing_middleware},
    },
    application::like_service::LikeService,
    domain::external_validator::ExternalValidator,
};
use axum::{
    Router,
    middleware::{from_fn, from_fn_with_state},
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

pub fn create_app(
    like_service: Arc<LikeService>,
    extern_validator: Arc<dyn ExternalValidator>,
) -> axum::Router {
    let public_routes = Router::new()
        .route("/likes/batch/counts", post(get_count_batch))
        .route("/likes/{content_type}/{content_id}/count", get(get_count))
        .route("/likes/top", get(get_top_likes))
        .route("/likes/stream", get(sse_stream));

    let protected_routes = Router::new()
        .route("/likes", post(post_like))
        .route("/likes/{content_type}/{content_id}", delete(delete_unlike))
        .route("/likes/{content_type}/{content_id}/status", get(get_status))
        .route("/likes/user", get(get_user_likes))
        .route("/likes/batch/statuses", post(get_status_batch))
        .layer(from_fn_with_state(
            extern_validator.clone(),
            auth_middleware,
        ));

    let health_routes: Router<Arc<LikeService>> = health::healt_router();
    let v1_routes = public_routes.merge(protected_routes);

    Router::new()
        .nest("/health", health_routes)
        .nest("/v1", v1_routes)
        .with_state(like_service)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(from_fn(tracing_middleware))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
