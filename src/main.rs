mod api;
mod application;
mod domain;
mod infrastructure;

use crate::infrastructure::observability::init_observability;
use crate::infrastructure::postgres::like_repository::PostgresLikeRepository;
use crate::infrastructure::redis::like_repository::RedisLikeRepository;
use crate::{
    application::like_service::LikeService,
    infrastructure::clients::http_external_validator::HttpExternalValidator,
};
use axum::{Router, body::Body, http::Request, response::Response, routing::post};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::Span;

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
    let extern_validator = Arc::new(HttpExternalValidator::new());

    let like_service = Arc::new(LikeService::new(db_repo, cache_repo, extern_validator));

    let middleware_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    let request_id = request
                        .extensions()
                        .get::<RequestId>()
                        .map(|id| id.header_value().to_str().unwrap_or("unknown"))
                        .unwrap_or("unknown");

                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                })
                .on_response(|_response: &Response, latency: Duration, _span: &Span| {
                    tracing::info!(latency_ms = latency.as_millis(), "Richiesta completata");
                }),
        )
        .layer(PropagateRequestIdLayer::x_request_id());

    let app = Router::new()
        .route("/likes", post(api::handlers::post_like))
        .layer(middleware_stack)
        .with_state(like_service);

    println!("🚀 Server ready on 0.0.0.0:8000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
