use base64::Engine as _;
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::RngCore;
use sqlx::PgPool;
use tracing::debug;

use crate::auth::model::{
    CHALLENGE_TTL_MINUTES, Challenge, ChallengeRequest, Session, SessionRow, VerifyRequest,
};
use crate::error::AppError;
use stellar_strkey::ed25519::PublicKey;

const ERR_NONCE_UNUSED_OR_EXPIRED: &str = "nonce missing, already used, or expired";

/// Issue a single-use SEP-40 signing challenge for `address`.
pub async fn issue_challenge(
    pool: &PgPool,
    domain: &str,
    req: ChallengeRequest,
) -> Result<Challenge, AppError> {
    validate_address(&req.address)?;

    let nonce = new_nonce();
    let timestamp = Utc::now();
    let expires_at = timestamp + Duration::minutes(CHALLENGE_TTL_MINUTES);

    sqlx::query(
        "INSERT INTO auth_challenges (nonce, domain, address, issued_at, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&nonce)
    .bind(domain)
    .bind(&req.address)
    .bind(timestamp)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    debug!(address = %req.address, "issued auth challenge");
    Ok(Challenge {
        domain: domain.to_string(),
        nonce,
        timestamp: timestamp.to_rfc3339(),
        address: req.address,
    })
}

/// Verify the wallet's signature over the issued challenge and create a session.
pub async fn verify(
    pool: &PgPool,
    domain: &str,
    session_ttl: std::time::Duration,
    req: VerifyRequest,
) -> Result<Session, AppError> {
    validate_address(&req.address)?;

    let nonce_row = sqlx::query_as::<_, NonceRow>(
        "SELECT domain, expires_at, used
         FROM auth_challenges
         WHERE nonce = $1 AND address = $2 AND domain = $3",
    )
    .bind(&req.nonce)
    .bind(&req.address)
    .bind(&req.domain)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::Conflict(ERR_NONCE_UNUSED_OR_EXPIRED.into()))?;

    if nonce_row.used || nonce_row.expires_at <= Utc::now() {
        return Err(AppError::Conflict(ERR_NONCE_UNUSED_OR_EXPIRED.into()));
    }
    if nonce_row.domain != domain {
        return Err(AppError::BadRequest(
            "challenge issued for a different domain".into(),
        ));
    }

    let message = sep40_message(&req.domain, &req.address, &req.nonce, &req.timestamp);
    verify_signature(&req.address, &req.signature, message.as_bytes())
        .map_err(|e| AppError::Unauthorized(format!("invalid signature: {e}")))?;

    sqlx::query("UPDATE auth_challenges SET used = TRUE WHERE nonce = $1")
        .bind(&req.nonce)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;

    let token = new_nonce();
    let token_hash = hash_token(&token);
    let expires_at = Utc::now()
        + Duration::from_std(session_ttl).map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query(
        "INSERT INTO auth_sessions (token_hash, address, roles, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&token_hash)
    .bind(&req.address)
    .bind(&["farmer".to_string()])
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    debug!(address = %req.address, "established auth session");
    Ok(Session {
        token,
        address: req.address,
        roles: vec!["farmer".into()],
        expires_at,
    })
}

/// Look up a session by its opaque bearer token (SHA-256 lookup).
pub async fn session_by_token(pool: &PgPool, token: &str) -> Result<SessionRow, AppError> {
    let token_hash = hash_token(token);
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT address, roles, expires_at FROM auth_sessions WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;

    let row = row.ok_or_else(|| AppError::Unauthorized("no session for token".into()))?;
    if row.expires_at <= Utc::now() {
        return Err(AppError::Unauthorized("session expired".into()));
    }
    Ok(row)
}

fn validate_address(address: &str) -> Result<(), AppError> {
    PublicKey::from_string(address)
        .map(|_| ())
        .map_err(|e| AppError::BadRequest(format!("invalid stellar address: {e}")))
}

fn new_nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Build the exact SEP-40 signed-payload text the wallet signs.
fn sep40_message(domain: &str, address: &str, nonce: &str, timestamp: &str) -> String {
    format!(
        "{domain} wants you to sign in with your Stellar account:\n{address}\n\nNonce: {nonce}\nIssued At: {timestamp}\n"
    )
}

fn verify_signature(address: &str, signature_b64: &str, message: &[u8]) -> Result<(), String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| format!("base64: {e}"))?;
    let signature = Signature::from_slice(&raw).map_err(|e| format!("signature length: {e}"))?;

    let pubkey = PublicKey::from_string(address).map_err(|e| format!("address: {e}"))?;
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey.0).map_err(|e| format!("verifying key: {e}"))?;

    verifying_key
        .verify(message, &signature)
        .map_err(|e| format!("ed25519: {e}"))
}

#[derive(sqlx::FromRow)]
struct NonceRow {
    domain: String,
    expires_at: chrono::DateTime<Utc>,
    used: bool,
}
