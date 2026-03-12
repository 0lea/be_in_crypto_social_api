use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    // Database & Redis
    pub read_database_url: String,
    pub database_url: String,
    pub redis_url: String,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub redis_pool_size: u32,

    // Server
    pub http_port: u16,
    pub log_level: String,
    pub shutdown_timeout_secs: u64,

    // External APIs
    pub profile_api_url: String,
    pub content_apis: HashMap<String, String>,

    // Rate Limiting
    pub rate_limit_write_per_minute: u64,
    pub rate_limit_read_per_minute: u64,

    // Cache TTLs (Il match che cercavi)
    pub cache_ttl_like_counts_secs: i64,
    pub cache_ttl_content_validation_secs: i64,
    // pub cache_ttl_user_status_secs: usize,

    // Circuit Breaker
    pub cb_failure_threshold: u64,
    pub cb_recovery_timeout_secs: u64,
    pub cb_success_threshold: u64,

    // Background Tasks & SSE
    pub sse_heartbeat_interval_secs: u64,
    pub leaderboard_refresh_interval_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let get_u64 = |key: &str, default: &str| {
            std::env::var(key)
                .unwrap_or_else(|_| default.to_string())
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("{} must be a valid u64", key))
        };

        let get_i64 = |key: &str, default: &str| {
            std::env::var(key)
                .unwrap_or_else(|_| default.to_string())
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("{} must be a valid usize", key))
        };

        let read_database_url =
            std::env::var("READ_DATABASE_URL").expect("READ_DATABASE_URL missing");
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
        let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");
        let db_max_connections = get_u64("DB_MAX_CONNECTIONS", "20") as u32;
        let db_min_connections = get_u64("DB_MIN_CONNECTIONS", "5") as u32;
        let db_acquire_timeout_secs = get_u64("DB_ACQUIRE_TIMEOUT_SECS", "5");
        let redis_pool_size = get_u64("REDIS_POOL_SIZE", "10") as u32;

        let http_port = std::env::var("HTTP_PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse::<u16>()
            .expect("HTTP_PORT must be u16");
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into());
        let shutdown_timeout_secs = get_u64("SHUTDOWN_TIMEOUT_SECS", "30");

        let profile_api_url = std::env::var("PROFILE_API_URL").expect("PROFILE_API_URL missing");
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

        let rate_limit_write_per_minute = get_u64("RATE_LIMIT_WRITE_PER_MINUTE", "30");
        let rate_limit_read_per_minute = get_u64("RATE_LIMIT_READ_PER_MINUTE", "1000");

        let cache_ttl_like_counts_secs = get_i64("CACHE_TTL_LIKE_COUNTS_SECS", "300");
        let cache_ttl_content_validation_secs =
            get_i64("CACHE_TTL_CONTENT_VALIDATION_SECS", "3600");
        // let cache_ttl_user_status_secs = get_usize("CACHE_TTL_USER_STATUS_SECS", "60");

        // 6. Circuit Breaker
        let cb_failure_threshold = get_u64("CIRCUIT_BREAKER_FAILURE_THRESHOLD", "5");
        let cb_recovery_timeout_secs = get_u64("CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECS", "30");
        let cb_success_threshold = get_u64("CIRCUIT_BREAKER_SUCCESS_THRESHOLD", "3");

        // 7. Background Tasks
        let sse_heartbeat_interval_secs = get_u64("SSE_HEARTBEAT_INTERVAL_SECS", "15");
        let leaderboard_refresh_interval_secs = get_u64("LEADERBOARD_REFRESH_INTERVAL_SECS", "60");

        Self {
            read_database_url,
            database_url,
            redis_url,
            db_max_connections,
            db_min_connections,
            db_acquire_timeout_secs,
            redis_pool_size,
            http_port,
            log_level,
            shutdown_timeout_secs,
            profile_api_url,
            content_apis,
            rate_limit_write_per_minute,
            rate_limit_read_per_minute,
            cache_ttl_like_counts_secs,
            cache_ttl_content_validation_secs,
            // cache_ttl_user_status_secs,
            cb_failure_threshold,
            cb_recovery_timeout_secs,
            cb_success_threshold,
            sse_heartbeat_interval_secs,
            leaderboard_refresh_interval_secs,
        }
    }
}

// use std::collections::HashMap;
//
// #[derive(Debug, Clone)]
// pub struct Config {
//     pub read_database_url: String,
//     pub database_url: String,
//     pub redis_url: String,
//     pub http_port: u16,
//
//     pub profile_api_url: String,
//     pub content_apis: HashMap<String, String>,
//
//     pub db_max_connections: u32,
//     pub db_min_connections: u32,
//     pub redis_pool_size: u32,
//     pub rate_limit_write_per_minute: u64,
//     pub rate_limit_read_per_minute: u64,
//     pub circuit_breaker_failure_threshold: u64,
// }
//
// impl Config {
//     pub fn from_env() -> Self {
//         dotenvy::dotenv().ok();
//
//         let read_database_url =
//             std::env::var("READ_DATABASE_URL").expect("READ_DATABASE_URL missing");
//         let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
//         let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL missing");
//         let profile_api_url = std::env::var("PROFILE_API_URL").expect("PROFILE_API_URL missing");
//         let http_port = std::env::var("HTTP_PORT")
//             .unwrap_or_else(|_| "8080".to_string())
//             .parse()
//             .expect("HTTP_PORT must be a valid u16");
//
//         let mut content_apis = HashMap::new();
//         for (key, value) in std::env::vars() {
//             if key.starts_with("CONTENT_API_") && key.ends_with("_URL") {
//                 let content_type = key
//                     .strip_prefix("CONTENT_API_")
//                     .unwrap()
//                     .strip_suffix("_URL")
//                     .unwrap()
//                     .to_lowercase();
//                 content_apis.insert(content_type, value);
//             }
//         }
//
//         if content_apis.is_empty() {
//             panic!("At least one CONTENT_API_*_URL must be configured!");
//         }
//
//         let db_max_connections = std::env::var("DB_MAX_CONNECTIONS")
//             .unwrap_or_else(|_| "20".into())
//             .parse()
//             .unwrap();
//         let db_min_connections = std::env::var("DB_MIN_CONNECTIONS")
//             .unwrap_or_else(|_| "5".into())
//             .parse()
//             .unwrap();
//         let redis_pool_size = std::env::var("REDIS_POOL_SIZE")
//             .unwrap_or_else(|_| "10".into())
//             .parse()
//             .unwrap();
//
//         let rate_limit_write_per_minute = std::env::var("RATE_LIMIT_WRITE_PER_MINUTE")
//             .unwrap_or_else(|_| "30".into())
//             .parse()
//             .expect("RATE_LIMIT_WRITE_PER_MINUTE must be u64");
//
//         let rate_limit_read_per_minute = std::env::var("RATE_LIMIT_READ_PER_MINUTE")
//             .unwrap_or_else(|_| "1000".into())
//             .parse()
//             .expect("RATE_LIMIT_READ_PER_MINUTE must be u64");
//
//         let circuit_breaker_failure_threshold = std::env::var("CIRCUIT_BREAKER_FAILURE_THRESHOLD")
//             .unwrap_or_else(|_| "1000".into())
//             .parse()
//             .expect("CIRCUIT_BREAKER_FAILURE_THRESHOLD must be u64");
//
//         Self {
//             read_database_url,
//             database_url,
//             redis_url,
//             http_port,
//             profile_api_url,
//             content_apis,
//             db_max_connections,
//             db_min_connections,
//             redis_pool_size,
//             rate_limit_write_per_minute,
//             rate_limit_read_per_minute,
//             circuit_breaker_failure_threshold,
//         }
//     }
// }
