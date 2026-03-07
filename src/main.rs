mod api;
mod state;

use axum::{Router, routing::get};

use crate::api::{content, profile};

#[tokio::main]

async fn main() {
    let mock_type = std::env::var("MOCK_TYPE").expect("MOCK_TYPE must be defined as env var");
    let port = std::env::var("PORT").expect("PORT must be defined as env var");

    let mut app = Router::new();

    app = match mock_type.as_str() {
        "profile" => {
            println!("Starting Profile API Mock on port {}", port);
            app.route("/v1/auth/validate", get(profile::validate_token))
        }

        "content" => {
            println!("Starting Content Mock API on port {}", port);
            app.route("/v1/post/:id", get(content::check_content))
                .route("/v1/bonus_hunter/:id", get(content::check_content))
                .route("/v1/top_picks/:id", get(content::check_content))
        }
        _ => panic!("MOCK_TYPE '{}' unkown", mock_type),
    };

    let app_state = state::AppState::new();

    let app = app.with_state(app_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("Mock service running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Error opening tcp listener");
    axum::serve(listener, app).await.unwrap();
}
