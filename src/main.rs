use social_api::{
    application::like_service::LikeService,
    create_app,
    domain::external_validator::ExternalValidator,
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator, observability::init_observability,
        postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");

    let pool = init_db(db_url).await;

    init_observability();

    let redis_client = redis::Client::open(redis_url).unwrap();

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));

    let extern_validator: Arc<dyn ExternalValidator> = Arc::new(HttpExternalValidator::from_env());
    let like_service = Arc::new(LikeService::new(
        db_repo,
        cache_repo,
        extern_validator.clone(),
    ));

    let app = create_app(like_service, extern_validator);

    println!("Server ready on 0.0.0.0:8000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn init_db(db_url: String) -> sqlx::Pool<sqlx::Postgres> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations failed");
    pool
}
