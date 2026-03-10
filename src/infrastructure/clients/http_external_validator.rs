use super::circuit_breaker::CircuitBreaker;
use crate::{
    domain::{
        errors::DomainError,
        external_validator::ExternalValidator,
        like::{ContentId, ContentType},
        user::UserId,
    },
    infrastructure::dto::{ContentDto, UserDto},
};
use async_trait::async_trait;
use reqwest::StatusCode;
use std::collections::HashMap;
use uuid::Uuid;

const PROFILE_API: &str = "Profile API";

pub struct HttpExternalValidator {
    client: reqwest::Client,
    profile_service_url: String,
    content_apis: HashMap<String, String>, // Astrazione dinamica dei content types
    breaker: CircuitBreaker,
}

impl HttpExternalValidator {
    pub fn new(profile_url: String, content_apis: HashMap<String, String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("Failed to create reqwest client"),
            profile_service_url: profile_url,
            content_apis,
            breaker: CircuitBreaker::from_env(),
        }
    }
}

#[async_trait]
impl ExternalValidator for HttpExternalValidator {
    async fn validate_user(&self, token: &UserId) -> Result<Uuid, DomainError> {
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

    async fn validate_content(
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
