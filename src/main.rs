mod api;
mod state;

use std::time::Duration;

use crate::api::{content, profile};
use axum::{
    Router,
    http::{Request, Response},
    routing::get,
};
use tower_http::trace::TraceLayer;
use tracing::{Span, info, info_span};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer().compact())
        .init();

    tracing::info!("Logger init!");

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
            app.route("/v1/{content_type}/{id}", get(content::check_content))
        }
        _ => panic!("MOCK_TYPE '{}' unkown", mock_type),
    };

    let app_state = state::AppState::new();

    app = app.layer(
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                info_span!(
                    "mock_request",
                    method = %request.method(),
                    uri = %request.uri(),
                )
            })
            .on_request(|request: &Request<_>, _span: &Span| {
                info!("--> In arrivo: {} {}", request.method(), request.uri());
            })
            .on_response(|response: &Response<_>, latency: Duration, _span: &Span| {
                info!(
                    "<-- Risposta: {} in {}ms",
                    response.status(),
                    latency.as_millis()
                );
            }),
    );
    let app = app.with_state(app_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("Mock service running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Error opening tcp listener");
    axum::serve(listener, app).await.unwrap();
}
