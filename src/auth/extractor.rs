use axum::extract::FromRequestParts;
use axum::http::HeaderValue;
use axum::http::request::Parts;

use crate::auth::service;
use crate::error::AppError;
use crate::state::AppState;

const BEARER_PREFIX: &str = "Bearer ";

/// Extracts the authenticated Stellar address from the session bearer token.
pub struct AuthUser {
    pub address: String,
    pub roles: Vec<String>,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts.headers.get("authorization"))?;
        let session = service::session_by_token(&state.pool, token).await?;
        Ok(AuthUser {
            address: session.address,
            roles: session.roles,
        })
    }
}

fn bearer_token(value: Option<&HeaderValue>) -> Result<&str, AppError> {
    let value =
        value.ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;
    let value = value
        .to_str()
        .map_err(|_| AppError::Unauthorized("invalid Authorization header".into()))?;
    let token = value
        .strip_prefix(BEARER_PREFIX)
        .ok_or_else(|| AppError::Unauthorized("expected Bearer token".into()))?;
    if token.is_empty() {
        return Err(AppError::Unauthorized("empty bearer token".into()));
    }
    Ok(token)
}
