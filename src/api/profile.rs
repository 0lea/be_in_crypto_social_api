use crate::state::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response}, // Aggiunto Response
};
use serde::Serialize;

#[derive(Serialize)]
struct AuthResponse {
    user_id: String,
    display_name: String,
    valid: bool,
}

#[derive(Serialize)]
struct AuthErrResponse {
    valid: bool,
    error: String,
}

pub async fn validate_token(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    if let Some(token) = token
        && let Some(dispaly_name) = state.valid_tokens.get(token)
    {
        tracing::info!(token = %token, user_id = %token, "Token valid");

        return (
            StatusCode::OK,
            Json(AuthResponse {
                valid: true,
                user_id: token.to_string(),
                display_name: dispaly_name.to_owned(),
            }),
        )
            .into_response();
    }

    tracing::warn!("validaton token failed");

    (
        StatusCode::UNAUTHORIZED,
        Json(AuthErrResponse {
            valid: false,
            error: "invalid_token".to_string(),
        }),
    )
        .into_response()
}
