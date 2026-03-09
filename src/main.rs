mod api;
mod application;
mod domain;
mod infrastructure;

use crate::api::middleware;
use crate::domain::external_validator::ExternalValidator;
use crate::infrastructure::observability::init_observability;
use crate::infrastructure::postgres::like_repository::PostgresLikeRepository;
use crate::infrastructure::redis::like_repository::RedisLikeRepository;
use crate::{
    application::like_service::LikeService,
    infrastructure::clients::http_external_validator::HttpExternalValidator,
};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::{Router, routing::post};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .unwrap();
    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));

    init_observability();

    let redis_client = redis::Client::open(redis_url).unwrap();
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));
    let extern_validator: Arc<dyn ExternalValidator> = Arc::new(HttpExternalValidator::new());
    let like_service = Arc::new(LikeService::new(
        db_repo,
        cache_repo,
        extern_validator.clone(),
    ));

    // let public_routes = Router::new()
    //     .route("/health", get(health_check));

    // .route("/likes", delete(api::handlers::delete_unlike))
    let protected_routes = Router::new()
        .route("/likes", post(api::handlers::post_like))
        .layer(from_fn_with_state(
            extern_validator.clone(),
            middleware::auth_middleware,
        ));

    let app = Router::new()
        // .merge(public_routes)
        .nest("/v1", protected_routes) // Tutte le rotte protette sotto /v1
        .with_state(like_service)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(from_fn(middleware::tracing_middleware))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    //test
    //test
    println!("🚀 Server ready on 0.0.0.0:8000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
