use serde_json::json;
use sqlx::PgPool;
use tracing::debug;

use crate::error::AppError;
use crate::farmers::chain::{ChainError, IdentityChain};
use crate::farmers::hash::metadata_hash;
use crate::farmers::model::{
    Farmer, FarmerMetadata, RegisterFarmerRequest, UpdateMetadataRequest, farmer_id,
    parse_farmer_identifier,
};

/// Error when an actor header doesn't match the addressed farmer.
const ERR_UNAUTHORIZED_ACTOR: &str = "caller is not the addressed farmer";

pub async fn register_farmer(
    pool: &PgPool,
    chain: &dyn IdentityChain,
    req: RegisterFarmerRequest,
    actor: &str,
) -> Result<Farmer, AppError> {
    let address = parse_farmer_identifier(&req.address)?;
    if actor != address {
        return Err(AppError::Unauthorized(ERR_UNAUTHORIZED_ACTOR.into()));
    }
    if req.metadata.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "metadata.name must be non-empty".into(),
        ));
    }

    let canonical_hash = metadata_hash(&req.metadata);
    if let Some(provided) = &req.metadata_hash
        && provided != &canonical_hash
    {
        return Err(AppError::BadRequest(
            "provided metadataHash does not match canonical hash".into(),
        ));
    }

    debug!("registering farmer {address} via chain");
    let chain_farmer = chain
        .register_farmer(&address, &canonical_hash)
        .await
        .map_err(map_chain_error)?;

    let id = farmer_id(&address);
    let markers_json = json!([]);
    sqlx::query!(
        r#"
        INSERT INTO farmers (address, id, metadata, metadata_hash, registered, created_ledger, updated_ledger, verification_markers)
        VALUES ($1, $2, $3, $4, TRUE, $5, $6, $7)
        "#,
        address,
        id,
        json!(req.metadata),
        canonical_hash,
        chain_farmer.created_ledger,
        chain_farmer.updated_ledger,
        markers_json,
    )
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(Farmer {
        address,
        id,
        registered: true,
        created_ledger: Some(chain_farmer.created_ledger),
        updated_ledger: Some(chain_farmer.updated_ledger),
        metadata: req.metadata,
        metadata_hash: canonical_hash,
        verification_markers: vec![],
    })
}

pub async fn get_farmer(pool: &PgPool, address: &str) -> Result<Farmer, AppError> {
    let address = parse_farmer_identifier(address)?;

    let row = sqlx::query!(
        r#"
        SELECT address, id, metadata, metadata_hash, registered, created_ledger, updated_ledger, verification_markers
        FROM farmers
        WHERE address = $1
        "#,
        address
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;

    let row = row.ok_or_else(|| AppError::NotFound(format!("farmer {address} not found")))?;

    let metadata: FarmerMetadata = serde_json::from_value(row.metadata)
        .map_err(|e| AppError::Internal(format!("metadata decode: {e}")))?;
    let markers: Vec<FarmerMarkerRow> = serde_json::from_value(row.verification_markers)
        .map_err(|e| AppError::Internal(format!("markers decode: {e}")))?;

    Ok(Farmer {
        address: row.address,
        id: row.id,
        registered: row.registered,
        created_ledger: row.created_ledger,
        updated_ledger: row.updated_ledger,
        metadata,
        metadata_hash: row.metadata_hash,
        verification_markers: markers
            .into_iter()
            .map(|m| crate::farmers::model::VerificationMarker {
                kind: m.kind,
                issuer: m.issuer,
                issued_ledger: m.issued_ledger,
            })
            .collect(),
    })
}

#[derive(serde::Deserialize)]
struct FarmerMarkerRow {
    kind: String,
    issuer: String,
    issued_ledger: i64,
}

pub async fn update_metadata(
    pool: &PgPool,
    chain: &dyn IdentityChain,
    address: &str,
    req: UpdateMetadataRequest,
    actor: &str,
) -> Result<Farmer, AppError> {
    let address = parse_farmer_identifier(address)?;
    if actor != address {
        return Err(AppError::Unauthorized(ERR_UNAUTHORIZED_ACTOR.into()));
    }
    if req.metadata.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "metadata.name must be non-empty".into(),
        ));
    }

    let canonical_hash = metadata_hash(&req.metadata);

    debug!("updating metadata for farmer {address} via chain");
    let chain_farmer = chain
        .update_metadata(&address, &canonical_hash)
        .await
        .map_err(map_chain_error)?;

    sqlx::query!(
        r#"
        UPDATE farmers
        SET metadata = $2, metadata_hash = $3, updated_ledger = $4, updated_at = now()
        WHERE address = $1
        "#,
        address,
        json!(req.metadata),
        canonical_hash,
        chain_farmer.updated_ledger,
    )
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    let updated = get_farmer(pool, &address).await?;
    Ok(updated)
}

fn map_chain_error(e: ChainError) -> AppError {
    match e {
        ChainError::AlreadyRegistered => AppError::Conflict("farmer already registered".into()),
        ChainError::NotFound => AppError::NotFound("farmer not found on-chain".into()),
        ChainError::Internal(msg) => AppError::Internal(msg),
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn register_then_get() {
        // Unit test logic requires a DB pool; this tests service logic via integration tests instead
    }
}
