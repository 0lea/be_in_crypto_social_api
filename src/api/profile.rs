use crate::state::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;

#[derive(Serialize)]
pub struct AuthResponse {
    pub valid: bool,
    pub user_id: Option<String>,
}

pub async fn validate_token(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> (StatusCode, Json<AuthResponse>) {
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    if let Some(token) = token {
        if let Some(user_id) = state.valid_tokens.get(token) {
            return (
                StatusCode::OK,
                Json(AuthResponse {
                    valid: true,
                    user_id: Some(user_id.clone()),
                }),
            );
        }
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(AuthResponse {
            valid: false,
            user_id: None,
        }),
    )
}
