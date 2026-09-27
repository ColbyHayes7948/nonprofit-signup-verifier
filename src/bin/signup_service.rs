use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nonprofit_signup_verifier::{
    infrai_client::{InfraiClient, InfraiError},
    nonprofit_signup::{SignupError, SignupInput, SignupOutput, SignupService, SignupStatus},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    let infrai = InfraiClient::from_env().expect("INFRAI_API_KEY must be set");
    let public_url = std::env::var("PUBLIC_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let signing_secret =
        std::env::var("SIGNUP_SIGNING_SECRET").expect("SIGNUP_SIGNING_SECRET must be set");
    let service = SignupService::new(infrai, public_url, signing_secret);
    let app = Router::new()
        .route("/signup", post(signup))
        .route("/verify", get(verify))
        .with_state(service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("port 3000 must be available");
    println!("signup service listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.expect("server stopped");
}

async fn signup(
    State(service): State<SignupService>,
    Json(input): Json<SignupInput>,
) -> Result<Json<SignupOutput>, ServiceError> {
    service.signup(input).await.map(Json).map_err(ServiceError)
}

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    signup_id: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct VerifyOutput {
    status: SignupStatus,
}

async fn verify(
    State(service): State<SignupService>,
    Query(query): Query<VerifyQuery>,
) -> Result<Json<VerifyOutput>, ServiceError> {
    let status = service.verify(&query.signup_id, &query.token).await?;
    Ok(Json(VerifyOutput { status }))
}

struct ServiceError(SignupError);

impl From<SignupError> for ServiceError {
    fn from(error: SignupError) -> Self {
        Self(error)
    }
}

impl axum::response::IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.0 {
            SignupError::NotFound => StatusCode::NOT_FOUND,
            SignupError::InvalidToken => StatusCode::UNPROCESSABLE_ENTITY,
            SignupError::Infrai(InfraiError::Api { status, .. }) if status.is_client_error() => {
                *status
            }
            SignupError::InvalidSecret
            | SignupError::Infrai(InfraiError::MissingKey)
            | SignupError::Infrai(InfraiError::Transport(_))
            | SignupError::Infrai(InfraiError::InvalidEnvelope { .. })
            | SignupError::Infrai(InfraiError::Api { .. }) => StatusCode::BAD_GATEWAY,
        };
        let body: Value = json!({ "error": self.0.to_string() });
        (status, Json(body)).into_response()
    }
}
