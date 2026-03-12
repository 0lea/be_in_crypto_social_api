use std::sync::Arc;

use serde_json::json;
use social_api::{
    application::{like_service::LikeService, sse::SseManager},
    create_app,
    domain::{
        external_validator::ExternalValidator,
        like::{ContentId, ContentType},
        user::UserId,
    },
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator, config::Config,
        postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

pub struct TestContext {
    pub base_url: String,
    pub cache: Arc<RedisLikeRepository>,
    pub db: Arc<PostgresLikeRepository>,
    pub mock_server: MockServer,
    pub router: axum::Router,
}

pub async fn setup_test_context(
    pool: PgPool,
    config_override: impl FnOnce(&mut Config),
) -> TestContext {
    let mock_server = MockServer::start().await;
    let mut config = Config::from_env();

    config.rate_limit_write_per_minute = 1000;
    config.rate_limit_read_per_minute = 5000;
    config.profile_api_url = mock_server.uri();
    config
        .content_apis
        .insert("post".to_string(), mock_server.uri());
    config
        .content_apis
        .insert("bonus_hunter".to_string(), mock_server.uri());
    config.rate_limit_write_per_minute = 10000;

    config.redis_url = std::env::var("REDIS_TEST_URL").unwrap_or("redis://127.0.0.1:6379/2".into());
    // Applichiamo le modifiche specifiche del test (es. per testare il rate limit)
    config_override(&mut config);

    let redis_client = redis::Client::open(config.redis_url.clone()).unwrap();
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)).await);
    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));

    let extern_validator: Arc<dyn ExternalValidator> = Arc::new(HttpExternalValidator::new(
        config.profile_api_url.clone(),
        config.content_apis.clone(),
        cache_repo.clone(),
    ));

    let c_token = CancellationToken::new();
    let sse_manager = Arc::new(SseManager::new(cache_repo.clone(), c_token.clone()));

    let sse_worker = sse_manager.clone();
    tokio::spawn(async move {
        sse_worker.run_cache_event_listener().await;
    });
    let like_service = Arc::new(LikeService::new(
        db_repo.clone(),
        cache_repo.clone(),
        extern_validator.clone(),
        sse_manager,
        c_token.clone(),
    ));

    let app = create_app(
        like_service,
        extern_validator,
        cache_repo.clone(),
        &config,
        c_token,
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app_cl = app.clone();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app_cl.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    TestContext {
        base_url: format!("http://{}", addr),
        cache: cache_repo,
        db: db_repo,
        mock_server,
        router: app,
    }
}

pub async fn auth_ok(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", format!("Bearer {}", user_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "valid": true,
        "user_id": user_id,
        "display_name": "test_user"
        })))
        .mount(mock_server)
        .await;
}

pub async fn auth_err(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", format!("Bearer {}", user_id)))
        .respond_with(ResponseTemplate::new(401))
        .mount(mock_server)
        .await;
}

pub async fn post_err(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(404))
        .mount(mock_server)
        .await;
}

pub async fn bonus_hunter_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/bonus_hunter/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "bonus_hunter",
            })))
        .mount(mock_server)
        .await;
}

pub async fn post_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "post",
            })))
        .mount(mock_server)
        .await;
}

pub async fn insert_old_like(
    pool: &PgPool,
    user_id: UserId,
    content_id: ContentId,
    c_type: ContentType,
    hours_ago: i32,
) {
    sqlx::query!(
        "INSERT INTO likes (user_id, content_id, content_type, created_at) 
         VALUES ($1, $2, $3, NOW() - make_interval(hours => $4))",
        user_id.0,
        content_id.0,
        c_type.as_str(),
        hours_ago
    )
    .execute(pool)
    .await
    .unwrap();
}
