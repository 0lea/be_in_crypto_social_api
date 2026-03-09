use crate::{
    api::{
        dto::{LikeRequest, LikeResponse},
        errors::ApiError,
    },
    application::{commands::AddLikeCommand, like_service::LikeService},
    domain::{errors::DomainError, user::UserId},
};
use axum::{
    Extension, Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

#[tracing::instrument(skip(service))]
pub async fn post_like(
    Extension(user_id): Extension<UserId>,
    State(service): State<Arc<LikeService>>,
    Json(payload): Json<LikeRequest>,
    // payload: Result<Json<LikeRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    // let Json(payload) = match payload {
    //     Ok(p) => p,
    //     Err(rejection) => {
    //         tracing::error!("❌ Errore JSON: {}", rejection.body_text());
    //         return Err(ApiError(DomainError::ValidationError(
    //             rejection.body_text(),
    //         )));
    //     }
    // };

    let command: AddLikeCommand = (user_id, payload).into();
    let res: LikeResponse = service.add_like(command).await?.into();

    Ok((StatusCode::CREATED, Json(res)))
}
