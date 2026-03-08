mod api;
mod application;
mod domain;
mod infrastructure;

use crate::application::like_service::LikeService;
use crate::infrastructure::postgres::like_repository::PostgresLikeRepository;
use crate::infrastructure::redis::like_repository::RedisLikeRepository;
use axum::{Router, routing::post};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

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

    let redis_client = redis::Client::open(redis_url).unwrap();
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));

    let like_service = Arc::new(LikeService::new(db_repo, cache_repo));

    let app = Router::new()
        .route("/likes", post(api::handlers::post_like))
        .with_state(like_service);

    println!("🚀 Server ready on 0.0.0.0:8000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
