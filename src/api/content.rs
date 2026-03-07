use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ContentResponse {
    pub exists: bool,
    pub id: Uuid,
}

pub async fn check_content(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> (StatusCode, Json<ContentResponse>) {
    if state.valid_content.contains(&id) {
        (StatusCode::OK, Json(ContentResponse { exists: true, id }))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ContentResponse { exists: false, id }),
        )
    }
}
