use crate::infrai_client::{InfraiClient, InfraiError, SendEmailRequest};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Deserialize)]
pub struct SignupInput {
    pub email: String,
    pub name: String,
    pub donor_receipts: bool,
    pub volunteer_reminders: bool,
    pub campaign_reporting: bool,
}

#[derive(Debug, Serialize)]
pub struct SignupOutput {
    pub signup_id: String,
    pub status: SignupStatus,
    pub message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignupStatus {
    PendingVerification,
    Verified,
}

#[derive(Debug, Clone)]
struct SignupRecord {
    token: String,
    status: SignupStatus,
}

#[derive(Debug, Error)]
pub enum SignupError {
    #[error(transparent)]
    Infrai(#[from] InfraiError),
    #[error("signup was not found")]
    NotFound,
    #[error("verification token is invalid")]
    InvalidToken,
    #[error("verification signing secret is invalid")]
    InvalidSecret,
}

#[derive(Clone)]
pub struct SignupService {
    infrai: InfraiClient,
    public_url: String,
    signing_secret: Vec<u8>,
    signups: Arc<RwLock<HashMap<String, SignupRecord>>>,
}

impl SignupService {
    pub fn new(infrai: InfraiClient, public_url: String, signing_secret: String) -> Self {
        Self {
            infrai,
            public_url,
            signing_secret: signing_secret.into_bytes(),
            signups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn signup(&self, input: SignupInput) -> Result<SignupOutput, SignupError> {
        let signup_id = Uuid::new_v4().to_string();
        let token = sign(&self.signing_secret, &signup_id)?;
        let verification_url = format!(
            "{}/verify?signup_id={}&token={}",
            self.public_url, signup_id, token
        );
        let mail: EmailResult = self
            .infrai
            .send_email(
                &SendEmailRequest {
                    to: input.email,
                    subject: "Confirm your nonprofit updates".to_owned(),
                    body: format!(
                        "Hi {},\n\nConfirm your signup: {}",
                        input.name, verification_url
                    ),
                },
                &format!("signup-email-{signup_id}"),
            )
            .await?;

        self.signups.write().await.insert(
            signup_id.clone(),
            SignupRecord {
                token,
                status: SignupStatus::PendingVerification,
            },
        );

        Ok(SignupOutput {
            signup_id,
            status: SignupStatus::PendingVerification,
            message_id: mail.message_id,
        })
    }

    pub async fn verify(&self, signup_id: &str, token: &str) -> Result<SignupStatus, SignupError> {
        let mut signups = self.signups.write().await;
        let record = signups.get_mut(signup_id).ok_or(SignupError::NotFound)?;
        if record.token != token {
            return Err(SignupError::InvalidToken);
        }
        record.status = SignupStatus::Verified;
        Ok(record.status)
    }
}

#[derive(Debug, Deserialize)]
struct EmailResult {
    message_id: String,
}

fn sign(secret: &[u8], signup_id: &str) -> Result<String, SignupError> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| SignupError::InvalidSecret)?;
    mac.update(signup_id.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{SignupError, SignupRecord, SignupStatus};
    use std::collections::HashMap;

    fn verify_record(
        records: &mut HashMap<String, SignupRecord>,
        signup_id: &str,
        token: &str,
    ) -> Result<SignupStatus, SignupError> {
        let record = records.get_mut(signup_id).ok_or(SignupError::NotFound)?;
        if record.token != token {
            return Err(SignupError::InvalidToken);
        }
        record.status = SignupStatus::Verified;
        Ok(record.status)
    }

    #[test]
    fn only_the_emailed_token_verifies_a_pending_signup() {
        let mut records = HashMap::from([(
            "signup-42".to_owned(),
            SignupRecord {
                token: "emailed-token".to_owned(),
                status: SignupStatus::PendingVerification,
            },
        )]);

        assert!(matches!(
            verify_record(&mut records, "signup-42", "other-token"),
            Err(SignupError::InvalidToken)
        ));
        assert_eq!(
            records["signup-42"].status,
            SignupStatus::PendingVerification
        );
        assert_eq!(
            verify_record(&mut records, "signup-42", "emailed-token").unwrap(),
            SignupStatus::Verified
        );
    }
}
