use axum::extract::State;
use axum::{Json, Router};
use serde::Deserialize;

use crate::auth::model::{Challenge, ChallengeRequest, Session, VerifyRequest};
use crate::auth::service;
use crate::error::AppError;
use crate::state::AppState;

/// POST /api/v1/auth/challenge
#[utoipa::path(
    post,
    path = "/api/v1/auth/challenge",
    request_body = ChallengeRequest,
    responses(
        (status = 200, description = "SEP-40 signing challenge", body = Challenge),
        (status = 400, description = "Invalid address format"),
        (status = 500, description = "Infrastructure failure")
    )
)]
pub async fn challenge_handler(
    State(state): State<AppState>,
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<Challenge>, AppError> {
    service::issue_challenge(&state.pool, &state.domain, req)
        .await
        .map(Json)
}

/// POST /api/v1/auth/verify
#[utoipa::path(
    post,
    path = "/api/v1/auth/verify",
    request_body = VerifyRequest,
    responses(
        (status = 200, description = "Session established", body = Session),
        (status = 400, description = "Malformed payload"),
        (status = 401, description = "Invalid signature / address mismatch"),
        (status = 409, description = "Nonce already used / expired"),
        (status = 500, description = "Infrastructure failure")
    )
)]
pub async fn verify_handler(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<Session>, AppError> {
    service::verify(&state.pool, &state.domain, state.session_ttl, req)
        .await
        .map(Json)
}

#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub token: String,
}

/// GET /api/v1/auth/session — resolve a bearer token to its session.
#[utoipa::path(
    get,
    path = "/api/v1/auth/session",
    params(
        ("token" = String, Query, description = "Opaque bearer token")
    ),
    responses(
        (status = 200, description = "Session details", body = crate::auth::model::SessionRow),
        (status = 401, description = "No session / expired")
    )
)]
pub async fn session_handler(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<SessionQuery>,
) -> Result<Json<crate::auth::model::SessionRow>, AppError> {
    service::session_by_token(&state.pool, &query.token)
        .await
        .map(Json)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/auth/challenge",
            axum::routing::post(challenge_handler),
        )
        .route("/api/v1/auth/verify", axum::routing::post(verify_handler))
        .route("/api/v1/auth/session", axum::routing::get(session_handler))
}
