pub mod api;
pub mod application;
pub mod domain;
pub mod infrastructure;

use std::sync::Arc;

use crate::{
    api::{
        handlers::post_like,
        middleware::{auth_middleware, tracing_middleware},
    },
    application::like_service::LikeService,
    domain::external_validator::ExternalValidator,
};
use axum::{
    Router,
    middleware::{from_fn, from_fn_with_state},
    routing::post,
};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

pub fn create_app(
    like_service: Arc<LikeService>,
    extern_validator: Arc<dyn ExternalValidator>,
) -> axum::Router {
    // let public_routes = Router::new()
    //     .route("/health", get(health_check));

    // .route("/likes", delete(api::handlers::delete_unlike))
    let protected_routes =
        Router::new()
            .route("/likes", post(post_like))
            .layer(from_fn_with_state(
                extern_validator.clone(),
                auth_middleware,
            ));

    Router::new()
        // .merge(public_routes)
        .nest("/v1", protected_routes)
        .with_state(like_service)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(from_fn(tracing_middleware))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}
