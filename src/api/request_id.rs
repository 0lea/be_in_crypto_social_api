use axum::{extract::FromRequestParts, http::request::Parts};

use crate::api::errors::ApiError;

#[derive(Debug)]
pub struct ReqCtx {
    pub id: String,
}

impl<S> FromRequestParts<S> for ReqCtx
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let id = parts
            .extensions
            .get::<tower_http::request_id::RequestId>()
            .map(|id| id.header_value().to_str().unwrap_or("unknown"))
            .unwrap_or("unknown")
            .to_string();

        Ok(ReqCtx { id })
    }
}
