use super::circuit_breaker::CircuitBreaker;
use crate::domain::{
    errors::DomainError,
    external_validator::ExternalValidator,
    like::{ContentId, ContentType},
    user::UserId,
};
use async_trait::async_trait;

pub struct HttpExternalValidator {
    client: reqwest::Client,
    profile_service_url: String,
    content_service_url: String,
    breaker: CircuitBreaker,
}

impl HttpExternalValidator {
    pub fn new() -> Self {
        let profile_url = std::env::var("PROFILE_API_URL").expect("missing profile url env");
        let content_url = std::env::var("CONTENT_API_POST_URL").expect("missing profile url env");

        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2)) // Timeout stretto! Fondamentale per la resilienza
                .build()
                .unwrap(),
            profile_service_url: profile_url,
            content_service_url: content_url,
            breaker: CircuitBreaker::from_env(),
        }
    }
}

#[async_trait]
impl ExternalValidator for HttpExternalValidator {
    async fn validate_user(&self, token: &UserId) -> Result<(), DomainError> {
        let url = format!("{}/v1/auth/validate", self.profile_service_url);

        let req_closure = async {
            let response = self
                .client
                .get(&url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|_| DomainError::ExternalServiceUnavailable {
                    service: "profile API".into(),
                })?;

            use reqwest::StatusCode;

            match response.status() {
                s if s.is_success() || s == StatusCode::NOT_FOUND => Ok(()),

                _ => Err(DomainError::ExternalServiceUnavailable {
                    service: "profile API".into(),
                }),
            }
        };

        self.breaker.call(req_closure).await
    }

    async fn validate_content(
        &self,
        c_type: &ContentType,
        c_id: &ContentId,
    ) -> Result<(), DomainError> {
        Ok(()) // Per ora mockiamo internamente per brevità
    }
}
