use super::circuit_breaker::CircuitBreaker;
use crate::{
    domain::{
        errors::DomainError,
        external_validator::{ExternalValidator, ValidationResult},
        like::{ContentId, ContentType, LikeCacheRepository},
        user::UserId,
    },
    infrastructure::dto::{ContentDto, UserDto},
};
use async_trait::async_trait;
use dashmap::DashSet;
use reqwest::StatusCode;
use std::{collections::HashMap, sync::Arc, time::Duration};
use uuid::Uuid;

const PROFILE_API: &str = "Profile API";

pub struct HttpExternalValidator {
    client: reqwest::Client,
    profile_service_url: String,
    content_apis: HashMap<String, String>,
    breaker: CircuitBreaker,
    cache_repo: Arc<dyn LikeCacheRepository>,
    single_flight_lock: Arc<DashSet<String>>,
}

impl HttpExternalValidator {
    pub fn new(
        profile_url: String,
        content_apis: HashMap<String, String>,
        cache_repo: Arc<dyn LikeCacheRepository>,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("Failed to create reqwest client"),
            profile_service_url: profile_url,
            content_apis,
            breaker: CircuitBreaker::from_env(),
            cache_repo,
            single_flight_lock: Arc::new(DashSet::with_capacity(100)),
        }
    }

    async fn get_cached_validation(&self, key: &str) -> Option<ValidationResult> {
        self.cache_repo
            .get_string(key)
            .await
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    async fn set_cached_validation(&self, key: &str, result: ValidationResult, ttl: u64) {
        if let Ok(json) = serde_json::to_string(&result) {
            let _ = self.cache_repo.set_string(key, &json, ttl).await;
        }
    }

    async fn call_profile_api(&self, token: &UserId) -> Result<Uuid, DomainError> {
        let url = format!("{}/v1/auth/validate", self.profile_service_url);
        let req_closure = async {
            let response = self
                .client
                .get(&url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| DomainError::DependencyUnavailable {
                    service: PROFILE_API.into(),
                })?;

            match response.status() {
                s if s.is_success() => {
                    let user_data: UserDto = response.json().await.map_err(|_| {
                        DomainError::DependencyDeserializeError {
                            service: PROFILE_API.into(),
                        }
                    })?;
                    Ok(user_data.user_id)
                }

                StatusCode::NOT_FOUND => Err(DomainError::DependencyNotFound {
                    service: PROFILE_API.into(),
                }),

                _ => Err(DomainError::DependencyUnavailable {
                    service: PROFILE_API.into(),
                }),
            }
        };
        self.breaker.call(req_closure).await
    }

    async fn call_content_api(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<Uuid, DomainError> {
        let base_url = self.content_apis.get(c_type.as_str()).ok_or_else(|| {
            DomainError::UnknownContentType {
                content_type: c_type.to_string(),
            }
        })?;

        let url = format!("{}/v1/{}/{}", base_url, c_type.as_str(), c_id);
        let service_name = format!("{} API", c_type.as_str().to_uppercase());

        let req_closure = async {
            let response = self.client.get(&url).send().await.map_err(|_| {
                DomainError::DependencyUnavailable {
                    service: service_name.clone(),
                }
            })?;

            match response.status() {
                s if s.is_success() => {
                    let content: ContentDto = response.json().await.map_err(|_| {
                        DomainError::DependencyDeserializeError {
                            service: service_name.clone(),
                        }
                    })?;
                    Ok(content.id)
                }
                StatusCode::NOT_FOUND => Err(DomainError::ContentNotFound {
                    content_type: c_type.to_string(),
                    content_id: c_id.to_string(),
                }),
                _ => Err(DomainError::DependencyUnavailable {
                    service: service_name.clone(),
                }),
            }
        };

        self.breaker.call(req_closure).await
    }
}

#[async_trait]
impl ExternalValidator for HttpExternalValidator {
    async fn validate_user(&self, token: &UserId) -> Result<Uuid, DomainError> {
        // hash token
        // let hash = blake3::hash(token.0.as_bytes());
        // let hex_str = hash.to_hex();
        let cache_key = format!("val:user:{}", token.0);
        if let Some(ValidationResult::Valid(user_id)) = self.get_cached_validation(&cache_key).await
        {
            return Ok(user_id);
        }

        let result = self.call_profile_api(token).await;

        if let Ok(user_id) = result {
            self.set_cached_validation(&cache_key, ValidationResult::Valid(user_id), 300)
                .await;
        }

        result
    }

    async fn validate_content(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<Uuid, DomainError> {
        let cache_key = format!("val:content:{}:{}", c_type, c_id);

        if let Some(res) = self.get_cached_validation(&cache_key).await {
            return match res {
                ValidationResult::Valid(uuid) => Ok(uuid),
                ValidationResult::NotFound => Err(DomainError::ContentNotFound {
                    content_type: c_type.as_str().into(),
                    content_id: c_id.0.into(),
                }),
            };
        }

        if !self.single_flight_lock.insert(cache_key.clone()) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            return Box::pin(self.validate_content(c_type, c_id)).await;
        }

        let result = self.call_content_api(c_type, c_id).await;

        match &result {
            Ok(uuid) => {
                self.set_cached_validation(&cache_key, ValidationResult::Valid(*uuid), 3600)
                    .await;
            }
            Err(DomainError::ContentNotFound {
                content_type: _,
                content_id: _,
            }) => {
                self.set_cached_validation(&cache_key, ValidationResult::NotFound, 300)
                    .await;
            }
            _ => {}
        }

        self.single_flight_lock.remove(&cache_key);
        result
    }

    async fn health_check(&self) -> Result<(), DomainError> {
        let url = format!("{}/v1/auth/validate", self.profile_service_url);

        let _ = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(1))
            .send()
            .await
            .map_err(|e| DomainError::DependencyHealtError(e.to_string()))?;

        Ok(())
    }
}
