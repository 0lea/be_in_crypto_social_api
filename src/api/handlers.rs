use crate::{
    api::{
        dto::{LikeRequest, LikeResponse},
        errors::ApiError,
    },
    application::{commands::AddLikeCommand, like_service::LikeService},
    domain::user::UserId,
};
use axum::{Extension, response::IntoResponse};
use axum::{Json, extract::State};
use std::sync::Arc;

pub async fn post_like(
    Extension(user_id): Extension<UserId>,
    State(service): State<Arc<LikeService>>,
    Json(payload): Json<LikeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let command: AddLikeCommand = (user_id, payload).into();
    let res: LikeResponse = service.add_like(command).await?.into();

    Ok(Json(res))
}
