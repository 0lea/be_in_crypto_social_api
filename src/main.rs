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
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    init_observability();

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

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool.clone())));
    let cache_repo = RedisLikeRepository::new(Arc::new(redis_client)).await;
    let cache_repo_a = Arc::new(cache_repo);

    let extern_validator: Arc<dyn ExternalValidator> = Arc::new(HttpExternalValidator::new(
        config.profile_api_url.clone(),
        config.content_apis.clone(),
        cache_repo_a.clone(),
    ));

    let rate_limiter = cache_repo_a.clone();

    let shutdown_token = CancellationToken::new();

    let sse_token = shutdown_token.clone();
    let sse_manager = Arc::new(SseManager::new(cache_repo_a.clone(), sse_token));
    let sse_worker = sse_manager.clone();
    tokio::spawn(async move {
        sse_worker.run_cache_event_listener().await;
    });

    let service_c_token = shutdown_token.clone();
    let like_service = Arc::new(LikeService::new(
        db_repo,
        cache_repo_a,
        extern_validator.clone(),
        sse_manager,
        service_c_token,
    ));

    let app = create_app(
        like_service,
        extern_validator,
        rate_limiter,
        &config,
        shutdown_token.clone(),
    );
    let addr = format!("0.0.0.0:{}", config.http_port);
    println!("Server ready on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    let pool_for_shutdown = pool.clone();
    let token_for_shutdown = shutdown_token.clone();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(pool_for_shutdown, token_for_shutdown))
    .await
    .unwrap();
}

async fn shutdown_signal(pool: sqlx::PgPool, token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => { println!("SIGINT received"); },
        _ = terminate => { println!("SIGTERM received"); },
    }

    println!("Starting shutdown...");

    token.cancel();

    tokio::time::timeout(Duration::from_secs(2), pool.close())
        .await
        .ok();

    println!("database pool closed");
}
