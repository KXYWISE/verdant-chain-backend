use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to obtain a SEP-40 signing challenge.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChallengeRequest {
    /// Stellar public key (G…) the caller claims.
    pub address: String,
}

/// SEP-40 challenge issued by the backend.
#[derive(Debug, Serialize, ToSchema)]
pub struct Challenge {
    /// App domain (must match the wallet allowlist).
    pub domain: String,
    /// Single-use random nonce.
    pub nonce: String,
    /// ISO-8601 timestamp of issuance.
    pub timestamp: String,
    /// Stellar public key the challenge is for.
    pub address: String,
}

/// Payload the wallet signs and returns to `/auth/verify`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyRequest {
    /// Stellar public key (G…).
    pub address: String,
    /// Domain from the issued challenge.
    pub domain: String,
    /// Nonce from the issued challenge.
    pub nonce: String,
    /// Timestamp from the issued challenge.
    pub timestamp: String,
    /// Base64 ed25519 signature over the SEP-40 signed payload.
    pub signature: String,
}

/// Established backend session.
#[derive(Debug, Serialize, ToSchema)]
pub struct Session {
    /// Opaque bearer token; send as `Authorization: Bearer <token>`.
    pub token: String,
    /// Stellar public key the session is bound to.
    pub address: String,
    /// Roles granted to the session.
    pub roles: Vec<String>,
    /// Session expiry (ISO-8601).
    pub expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow, Debug, Serialize, ToSchema)]
pub struct SessionRow {
    pub address: String,
    pub roles: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

pub const CHALLENGE_TTL_MINUTES: i64 = 5;
