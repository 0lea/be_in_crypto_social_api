use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use tracing::error;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ContentResponse {
    id: Uuid,
    content_type: String,
}

pub async fn check_content(
    Path((content_type, id)): Path<(String, Uuid)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if state.valid_content.contains(&id) {
        (StatusCode::OK, Json(ContentResponse { id, content_type })).into_response()
    } else {
        (StatusCode::NOT_FOUND).into_response()
    }
}
