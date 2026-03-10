use crate::{
    api::{
        dto::{
            BatchRequest, LikeRequest, LikeResponse, TopLikesQuery, TopLikesResponse,
            UserLikesQuery, UserLikesResponse,
        },
        errors::ApiError,
    },
    application::{commands::AddLikeCommand, like_service::LikeService},
    domain::{
        like::{ContentId, ContentType},
        user::UserId,
    },
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
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

#[tracing::instrument(skip(service))]
pub async fn delete_unlike(
    Extension(user_id): Extension<UserId>,
    State(service): State<Arc<LikeService>>,
    Path((c_type, c_id)): Path<(ContentType, ContentId)>,
) -> Result<impl IntoResponse, ApiError> {
    let res = service.remove_like(&user_id, &c_type, &c_id).await?;
    Ok((StatusCode::OK, Json(res)))
}

#[tracing::instrument(skip(service))]
pub async fn get_count(
    State(service): State<Arc<LikeService>>,
    Path((c_type, c_id)): Path<(ContentType, ContentId)>,
) -> Result<impl IntoResponse, ApiError> {
    let res = service.get_like_count(&c_type, &c_id).await?;
    Ok((StatusCode::OK, Json(res)))
}

#[tracing::instrument(skip(service))]
pub async fn get_status(
    Extension(user_id): Extension<UserId>,
    State(service): State<Arc<LikeService>>,
    Path((c_type, c_id)): Path<(ContentType, ContentId)>,
) -> Result<impl IntoResponse, ApiError> {
    let res = service.get_like_status(&user_id, &c_type, &c_id).await?;
    Ok((StatusCode::OK, Json(res)))
}

#[tracing::instrument(skip(service))]
pub async fn get_count_batch(
    State(service): State<Arc<LikeService>>,
    Json(payload): Json<BatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let res = service.get_likes_count_batch(payload.items).await?;
    Ok((StatusCode::OK, Json(res)))
}

#[tracing::instrument(skip(service))]
pub async fn get_status_batch(
    Extension(user_id): Extension<UserId>,
    State(service): State<Arc<LikeService>>,
    Json(payload): Json<BatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let res = service.get_batch_status(&user_id, payload.items).await?;
    Ok((StatusCode::OK, Json(res)))
}

#[tracing::instrument(skip(service))]
pub async fn get_user_likes(
    State(service): State<Arc<LikeService>>,
    Extension(user_id): Extension<UserId>,
    Query(query): Query<UserLikesQuery>,
) -> Result<Json<UserLikesResponse>, ApiError> {
    let limit = query.limit.unwrap_or(20);

    let response = service
        .get_user_liked_items(user_id, query.content_type, query.cursor, limit)
        .await?;

    Ok(Json(response))
}

#[tracing::instrument(skip(service))]
pub async fn get_top_likes(
    State(service): State<Arc<LikeService>>,
    Query(query): Query<TopLikesQuery>,
) -> Result<Json<TopLikesResponse>, ApiError> {
    let limit = query.limit.unwrap_or(10);
    let c_type: ContentType = query.content_type.unwrap_or("all".into()).into();
    let response = service.get_top_likes(c_type, query.window, limit).await?;
    Ok(Json(response))
}
