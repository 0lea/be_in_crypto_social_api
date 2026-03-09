use axum_test::TestServer;
use serde_json::json;
use social_api::{
    application::like_service::LikeService,
    create_app,
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator,
        postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

async fn setup_test_app() -> axum::Router {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL per i test mancante");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL per i test mancante");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .unwrap();

    let redis_client = redis::Client::open(redis_url).unwrap();

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));
    let validator = Arc::new(HttpExternalValidator::new());

    let like_service = Arc::new(LikeService::new(db_repo, cache_repo, validator.clone()));
    create_app(like_service, validator)
}

#[tokio::test]
async fn test_real_flow_integration() {
    let app = setup_test_app().await;
    let server = TestServer::new(app);

    let content_id = "731b0395-4888-4822-b516-05b4b7bf2089";

    let response = server
        .post("/v1/likes")
        .add_header(
            "Authorization",
            "Bearer 550e8400-e29b-41d4-a716-446655440001",
        )
        .json(&json!({
            "content_type": "post",
            "content_id": content_id
        }))
        .await;

    response.assert_status(axum::http::StatusCode::CREATED);

    let body = response.json::<serde_json::Value>();
    assert!(body["liked"].as_bool().unwrap());
    assert!(body["count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_post_like_unauthorized() {
    let app = setup_test_app().await;
    let server = TestServer::new(app);

    let response = server
        .post("/v1/likes")
        .json(&json!({
            "content_type": "post",
            "content_id": "731b0395-4888-4822-b516-05b4b7bf2089"
        }))
        .await;

    // Deve fallire perché manca l'header Authorization gestito dal tuo middleware
    response.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}
