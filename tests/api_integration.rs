use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::HashMap, ops::Add, sync::Arc};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use social_api::{
    application::like_service::LikeService,
    create_app,
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator,
        postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};

async fn auth_ok(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", format!("Bearer {}", user_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "valid": true,
        "user_id": user_id,
        "display_name": "test_user"
        })))
        .mount(&mock_server)
        .await;
}

async fn auth_err(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", "Bearer tok_bad"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;
}

async fn post_err(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;
}

async fn bonus_hunter_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/bonus_hunter/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "bonus_hunter",
            })))
        .mount(&mock_server)
        .await;
}

async fn post_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "post",
            })))
        .mount(&mock_server)
        .await;
}

async fn setup_test_app(pool: PgPool, mock_url: String) -> axum::Router {
    dotenvy::dotenv().ok();
    let redis_url = std::env::var("REDIS_TEST_URL").unwrap_or("redis://127.0.0.1:6379/2".into());
    let redis_client = redis::Client::open(redis_url).unwrap();

    let mut conn = redis_client
        .get_connection()
        .expect("Failed to connect to Redis for cleanup");
    let _: () = redis::cmd("FLUSHDB")
        .query(&mut conn)
        .expect("Failed to flush Redis");

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));
    let validator = Arc::new(HttpExternalValidator::new(mock_url.clone(), mock_url));
    let like_service = Arc::new(LikeService::new(db_repo, cache_repo, validator.clone()));
    create_app(like_service, validator)
}

#[sqlx::test]
#[test_log::test]
async fn test_full_like_lifecycle(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";
    let test_post_id = &uuid::Uuid::new_v4().to_string();

    auth_ok(test_user_id, &mock_server).await;
    post_ok(test_post_id, &mock_server).await;

    // 1. create Like
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["liked"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // indeponent
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["already_existed"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // Stato
    let res = server
        .get(&format!("/v1/likes/post/{}/status", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["liked"], true);

    // recount
    let res = server
        .get(&format!("/v1/likes/post/{}/count", test_post_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["content_type"], "post");
    assert_eq!(res.json::<Value>()["content_id"], test_post_id.as_str());
    assert_eq!(res.json::<Value>()["count"], 1);

    // 4. Rimozione Like (Unlike)
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["was_liked"], true);
    assert_eq!(res.json::<Value>()["count"], 0);
}

#[sqlx::test]
#[test_log::test]
async fn test_edge_cases(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";
    let test_user_id_2 = "550e8400-e29b-41d4-a716-446655440002";
    let test_post_id = &uuid::Uuid::new_v4().to_string();

    // unexistent
    auth_ok(test_user_id, &mock_server).await;
    post_err(test_post_id, &mock_server).await;

    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::NOT_FOUND);

    // Unlike of unexistent like
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::NOT_FOUND);

    // create one
    mock_server.reset().await;
    auth_ok(test_user_id, &mock_server).await;
    post_ok(test_post_id, &mock_server).await;
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["liked"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // user 2 delete unliked post
    auth_ok(test_user_id_2, &mock_server).await;
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id_2))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["was_liked"], false);

    // Token Invalid
    auth_err("tok_bad", &mock_server).await;
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", "Bearer tok_bad")
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::UNAUTHORIZED);
}
}
        .await;

    // Deve fallire perché manca l'header Authorization gestito dal tuo middleware
    response.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}
