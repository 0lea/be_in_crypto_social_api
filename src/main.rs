use social_api::{
    application::{like_service::LikeService, sse::SseManager},
    create_app,
    domain::external_validator::ExternalValidator,
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator, config::Config,
        observability::init_observability, postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // 1. Caricamento fail-fast della config
    let config = Config::from_env();

    init_observability();

    // 2. Inizializzazione Pool connessioni usando i valori della config
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to Database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations failed");

    let redis_client =
        redis::Client::open(config.redis_url.clone()).expect("Failed to connect to Redis");

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));
    let extern_validator: Arc<dyn ExternalValidator> = Arc::new(HttpExternalValidator::new(
        config.profile_api_url.clone(),
        config.content_apis.clone(),
    ));
    let sse_manager = Arc::new(SseManager::new(cache_repo.clone()));

    let rate_limiter = cache_repo.clone();

    let sse_worker = sse_manager.clone();
    tokio::spawn(async move {
        sse_worker.run_cache_event_listener().await;
    });

    let like_service = Arc::new(LikeService::new(
        db_repo,
        cache_repo,
        extern_validator.clone(),
        sse_manager,
    ));

    let app = create_app(like_service, extern_validator, rate_limiter, &config);
    let addr = format!("0.0.0.0:{}", config.http_port);
    println!("Server ready on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
