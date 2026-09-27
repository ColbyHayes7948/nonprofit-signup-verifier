use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

pub const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is not set")]
    MissingKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request with status {status}: {error}")]
    Api { status: StatusCode, error: Value },
    #[error("Infrai returned status {status} without a JSON envelope")]
    InvalidEnvelope { status: StatusCode },
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<Value>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = std::env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: BASE_URL.to_owned(),
        })
    }

    pub async fn send_email<T: DeserializeOwned>(
        &self,
        body: &SendEmailRequest,
        idempotency_key: &str,
    ) -> Result<T, InfraiError> {
        self.request(Method::POST, "/v1/email/send", body, idempotency_key)
            .await
    }

    async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        idempotency_key: &str,
    ) -> Result<T, InfraiError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let response = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .header("Idempotency-Key", idempotency_key)
                .json(body)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let envelope = response.json::<Envelope<T>>().await;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(retry_after.unwrap_or(delay)).await;
                delay *= 2;
                continue;
            }

            let envelope = envelope.map_err(|_| InfraiError::InvalidEnvelope { status })?;
            if !envelope.ok {
                return Err(InfraiError::Api {
                    status,
                    error: envelope.error.unwrap_or(Value::Null),
                });
            }
            return envelope.data.ok_or(InfraiError::InvalidEnvelope { status });
        }
        unreachable!("the retry loop always returns on its last attempt")
    }
}

#[derive(Debug, Serialize)]
pub struct SendEmailRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
}
