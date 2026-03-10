use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub read_database_url: String,
    pub database_url: String,
    pub redis_url: String,
    pub http_port: u16,

    pub profile_api_url: String,
    pub content_apis: HashMap<String, String>,

    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub redis_pool_size: u32,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let read_database_url =
            std::env::var("READ_DATABASE_URL").expect("READ_DATABASE_URL missing");
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
        let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");
        let profile_api_url = std::env::var("PROFILE_API_URL").expect("PROFILE_API_URL missing");
        let http_port = std::env::var("HTTP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("HTTP_PORT must be a valid u16");

        let mut content_apis = HashMap::new();
        for (key, value) in std::env::vars() {
            if key.starts_with("CONTENT_API_") && key.ends_with("_URL") {
                let content_type = key
                    .strip_prefix("CONTENT_API_")
                    .unwrap()
                    .strip_suffix("_URL")
                    .unwrap()
                    .to_lowercase();
                content_apis.insert(content_type, value);
            }
        }

        if content_apis.is_empty() {
            panic!("At least one CONTENT_API_*_URL must be configured!");
        }

        let db_max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".into())
            .parse()
            .unwrap();
        let db_min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| "5".into())
            .parse()
            .unwrap();
        let redis_pool_size = std::env::var("REDIS_POOL_SIZE")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .unwrap();

        Self {
            read_database_url,
            database_url,
            redis_url,
            http_port,
            profile_api_url,
            content_apis,
            db_max_connections,
            db_min_connections,
            redis_pool_size,
        }
    }
}
