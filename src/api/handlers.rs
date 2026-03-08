use crate::api::errors::ApiError;
use crate::application::like_service::LikeService;
use crate::domain::like::{ContentId, ContentType};
use crate::{api::dto::LikeRequest, domain::user::UserId};
use axum::response::IntoResponse;
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

pub async fn post_like(
    State(service): State<Arc<LikeService>>,
    Json(payload): Json<LikeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    service
        .add_like(
            UserId(payload.user_id),
            ContentType::new(&payload.content_type),
            ContentId(payload.content_id),
        )
        .await?;

    Ok(StatusCode::CREATED)
}
