use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::json;
use std::sync::Arc;

use crate::{
    api::{errors::ApiError, request_id::ReqCtx},
    application::like_service::LikeService,
};

pub fn healt_router() -> Router<Arc<LikeService>> {
    Router::new()
        .route("/live", get(liveness))
        .route("/ready", get(readiness))
}

async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "up" })))
}

pub async fn readiness(
    State(service): State<Arc<LikeService>>,
    ctx: ReqCtx,
) -> Result<impl IntoResponse, ApiError> {
    service
        .full_health_check()
        .await
        .map_err(|e| ApiError(e, ctx.id))?;

    Ok(())
}
