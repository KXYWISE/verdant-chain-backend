use sqlx::PgPool;

use crate::ids::counter_id;
use crate::indexer::model::ChainEvent;

pub const VERIFICATION_PREFIX: &str = "va:verification";
pub const ESCROW_PREFIX: &str = "va:escrow";
pub const FINANCING_PREFIX: &str = "va:financing";

/// Decoded verification events (docs/events/verification.md).
#[derive(Debug, Clone)]
pub struct VerificationCreated {
    pub verification_id: u64,
    pub batch_hex: String,
    pub subject: String,
    pub proof_hash: String,
    pub issuer: String,
    pub issued_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct VerificationRevoked {
    pub verification_id: u64,
    pub revoked_ledger: i64,
}

/// Decoded escrow events (docs/events/escrow.md).
#[derive(Debug, Clone)]
pub struct EscrowCreated {
    pub escrow_id: u64,
    pub depositor: String,
    pub beneficiary: String,
    pub amount: i64,
    pub condition_kind: i32,
    pub condition_releaser: Option<String>,
    pub condition_timeout_ledger: Option<i64>,
    pub booking_ref_hex: String,
    pub created_ledger: i64,
    pub updated_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct EscrowDeposited {
    pub escrow_id: u64,
    pub amount: i64,
    pub updated_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct EscrowReleased {
    pub escrow_id: u64,
    pub released_amount: i64,
    pub updated_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct EscrowRefunded {
    pub escrow_id: u64,
    pub updated_ledger: i64,
}

/// Decoded financing events (docs/contracts/financing.md, accepted v1.0).
#[derive(Debug, Clone)]
pub struct FinancingCreated {
    pub financing_id: u64,
    pub funder: String,
    pub beneficiary: String,
    pub total_amount: i64,
    pub drawn_amount: i64,
    pub milestone_count: i32,
    pub milestones: serde_json::Value,
    pub drawn_ledger: Option<i64>,
    pub repaid_amount: i64,
    pub defaulted: bool,
    pub defaulted_ledger: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FinancingDeposited {
    pub financing_id: u64,
    /// Increment to the running drawn_amount (event semantics, accepted v1.0).
    pub amount: i64,
    pub updated_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct FinancingReleased {
    pub financing_id: u64,
    pub released_amount: i64,
    pub updated_ledger: i64,
}

#[derive(Debug, Clone)]
pub struct FinancingRefunded {
    pub financing_id: u64,
    pub refunded_amount: i64,
    pub defaulted: bool,
    pub defaulted_ledger: Option<i64>,
}

/// Raw JSON array access helpers for decoded payloads.
fn arr(data: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    data.as_array()
}

fn get_str(data: &serde_json::Value, i: usize) -> Option<String> {
    arr(data)?.get(i)?.as_str().map(str::to_string)
}

fn get_u64(data: &serde_json::Value, i: usize) -> Option<u64> {
    arr(data)?.get(i)?.as_u64()
}

fn get_i64(data: &serde_json::Value, i: usize) -> Option<i64> {
    arr(data)?.get(i)?.as_i64()
}

fn get_i32(data: &serde_json::Value, i: usize) -> Option<i32> {
    arr(data)?.get(i)?.as_i64().map(|v| v as i32)
}

fn get_bool(data: &serde_json::Value, i: usize) -> Option<bool> {
    arr(data)?.get(i)?.as_bool()
}

/// Renders a backend-issued UUIDv7 reference from its hex bytes:
/// `va:{kind}:{uuidv7}`. Falls back to the raw hex if not a valid UUID.
fn render_uuid7_ref(kind: &str, hex: &str) -> String {
    let uuid = uuid::Uuid::parse_str(hex).map(|u| u.to_string());
    format!("va:{kind}:{}", uuid.unwrap_or_else(|_| hex.to_string()))
}

pub fn decode_verification_created(event: &ChainEvent) -> Option<VerificationCreated> {
    let data = &event.data;
    Some(VerificationCreated {
        verification_id: get_u64(data, 0)?,
        batch_hex: get_str(data, 1)?,
        subject: get_str(data, 2)?,
        proof_hash: get_str(data, 3)?,
        issuer: get_str(data, 4)?,
        issued_ledger: get_i64(data, 5)?,
    })
}

pub fn decode_verification_revoked(event: &ChainEvent) -> Option<VerificationRevoked> {
    let data = &event.data;
    Some(VerificationRevoked {
        verification_id: get_u64(data, 0)?,
        revoked_ledger: get_i64(data, 3)?,
    })
}

pub fn decode_escrow_created(event: &ChainEvent) -> Option<EscrowCreated> {
    let data = &event.data;
    Some(EscrowCreated {
        escrow_id: get_u64(data, 0)?,
        depositor: get_str(data, 1)?,
        beneficiary: get_str(data, 2)?,
        amount: get_i64(data, 3)?,
        condition_kind: get_i32(data, 4)?,
        condition_releaser: get_str(data, 5),
        condition_timeout_ledger: get_i64(data, 6),
        booking_ref_hex: get_str(data, 7)?,
        created_ledger: get_i64(data, 8)?,
        updated_ledger: get_i64(data, 9)?,
    })
}

pub fn decode_escrow_deposited(event: &ChainEvent) -> Option<EscrowDeposited> {
    let data = &event.data;
    Some(EscrowDeposited {
        escrow_id: get_u64(data, 0)?,
        amount: get_i64(data, 2)?,
        updated_ledger: get_i64(data, 3)?,
    })
}

pub fn decode_escrow_released(event: &ChainEvent) -> Option<EscrowReleased> {
    let data = &event.data;
    Some(EscrowReleased {
        escrow_id: get_u64(data, 0)?,
        released_amount: get_i64(data, 2)?,
        updated_ledger: get_i64(data, 3)?,
    })
}

pub fn decode_escrow_refunded(event: &ChainEvent) -> Option<EscrowRefunded> {
    let data = &event.data;
    Some(EscrowRefunded {
        escrow_id: get_u64(data, 0)?,
        updated_ledger: get_i64(data, 3)?,
    })
}

/// Financing `FinancingCreated` carries the full `Financing` struct as data
/// (docs/contracts/financing.md §events). Financing id is also topic[0].
pub fn decode_financing_created(event: &ChainEvent) -> Option<FinancingCreated> {
    let data = &event.data;
    let financing_id = get_u64(data, 0)?;
    Some(FinancingCreated {
        financing_id,
        funder: get_str(data, 1)?,
        beneficiary: get_str(data, 2)?,
        total_amount: get_i64(data, 3)?,
        drawn_amount: get_i64(data, 4)?,
        milestone_count: get_i32(data, 5)?,
        milestones: arr(data)?.get(6)?.clone(),
        drawn_ledger: get_i64(data, 7),
        repaid_amount: get_i64(data, 8)?,
        defaulted: get_bool(data, 9)?,
        defaulted_ledger: get_i64(data, 10),
    })
}

pub fn decode_financing_deposited(event: &ChainEvent) -> Option<FinancingDeposited> {
    let data = &event.data;
    Some(FinancingDeposited {
        financing_id: get_u64(data, 0)?,
        amount: get_i64(data, 1)?,
        updated_ledger: get_i64(data, 2)?,
    })
}

pub fn decode_financing_released(event: &ChainEvent) -> Option<FinancingReleased> {
    let data = &event.data;
    Some(FinancingReleased {
        financing_id: get_u64(data, 0)?,
        released_amount: get_i64(data, 1)?,
        updated_ledger: get_i64(data, 2)?,
    })
}

pub fn decode_financing_refunded(event: &ChainEvent) -> Option<FinancingRefunded> {
    let data = &event.data;
    Some(FinancingRefunded {
        financing_id: get_u64(data, 0)?,
        refunded_amount: get_i64(data, 1)?,
        defaulted: get_bool(data, 2)?,
        defaulted_ledger: get_i64(data, 3),
    })
}

/// Applies a raw event to the projections it belongs to. Returns true if the
/// event was consumed by a builder. Idempotent for replays.
pub async fn apply_event(pool: &PgPool, event: &ChainEvent) -> Result<bool, sqlx::Error> {
    match (event.contract_name.as_str(), event.event_name.as_str()) {
        ("verification", "VerificationCreated") => {
            let decoded = decode_verification_created(event).ok_or(sqlx::Error::Protocol(
                "bad VerificationCreated payload".into(),
            ))?;
            upsert_verification(pool, &event.contract_id, &decoded).await?;
            Ok(true)
        }
        ("verification", "VerificationRevoked") => {
            let decoded = decode_verification_revoked(event).ok_or(sqlx::Error::Protocol(
                "bad VerificationRevoked payload".into(),
            ))?;
            revoke_verification(pool, &decoded).await?;
            Ok(true)
        }
        ("escrow", "EscrowCreated") => {
            let decoded = decode_escrow_created(event)
                .ok_or(sqlx::Error::Protocol("bad EscrowCreated payload".into()))?;
            upsert_escrow(pool, &event.contract_id, &decoded).await?;
            Ok(true)
        }
        ("escrow", "EscrowDeposited") => {
            let decoded = decode_escrow_deposited(event)
                .ok_or(sqlx::Error::Protocol("bad EscrowDeposited payload".into()))?;
            add_escrow_deposit(pool, &decoded).await?;
            Ok(true)
        }
        ("escrow", "EscrowReleased") => {
            let decoded = decode_escrow_released(event)
                .ok_or(sqlx::Error::Protocol("bad EscrowReleased payload".into()))?;
            release_escrow(pool, &decoded).await?;
            Ok(true)
        }
        ("escrow", "EscrowRefunded") => {
            let decoded = decode_escrow_refunded(event)
                .ok_or(sqlx::Error::Protocol("bad EscrowRefunded payload".into()))?;
            refund_escrow(pool, &decoded).await?;
            Ok(true)
        }
        ("identity", "FarmerRegistered") => {
            upsert_identity_farmer(pool, event).await?;
            Ok(true)
        }
        ("identity", "FarmerMetadataUpdated") => {
            update_identity_metadata(pool, event).await?;
            Ok(true)
        }
        ("identity", "VerificationMarkerSet") => {
            append_identity_marker(pool, event).await?;
            Ok(true)
        }
        ("financing", "FinancingCreated") => {
            let decoded = decode_financing_created(event)
                .ok_or(sqlx::Error::Protocol("bad FinancingCreated payload".into()))?;
            upsert_financing(pool, &event.contract_id, &decoded).await?;
            Ok(true)
        }
        ("financing", "FinancingDeposited") => {
            let decoded = decode_financing_deposited(event).ok_or(sqlx::Error::Protocol(
                "bad FinancingDeposited payload".into(),
            ))?;
            add_financing_deposit(pool, &decoded).await?;
            Ok(true)
        }
        ("financing", "FinancingReleased") => {
            let decoded = decode_financing_released(event).ok_or(sqlx::Error::Protocol(
                "bad FinancingReleased payload".into(),
            ))?;
            release_financing(pool, &decoded).await?;
            Ok(true)
        }
        ("financing", "FinancingRefunded") => {
            let decoded = decode_financing_refunded(event).ok_or(sqlx::Error::Protocol(
                "bad FinancingRefunded payload".into(),
            ))?;
            refund_financing(pool, &decoded).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn upsert_verification(
    pool: &PgPool,
    contract_id: &str,
    e: &VerificationCreated,
) -> Result<(), sqlx::Error> {
    let id = counter_id(VERIFICATION_PREFIX, e.verification_id);
    sqlx::query!(
        r#"
        INSERT INTO indexer.verification_projection
            (id, contract_id, batch_id, subject, proof_hash, issuer, issued_ledger,
             revoked, revoked_ledger)
        VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE, NULL)
        ON CONFLICT (id) DO UPDATE SET
            contract_id = EXCLUDED.contract_id,
            batch_id = EXCLUDED.batch_id,
            subject = EXCLUDED.subject,
            proof_hash = EXCLUDED.proof_hash,
            issuer = EXCLUDED.issuer,
            issued_ledger = EXCLUDED.issued_ledger,
            updated_at = NOW()
        "#,
        id,
        contract_id,
        render_uuid7_ref("batch", &e.batch_hex),
        e.subject,
        e.proof_hash,
        e.issuer,
        e.issued_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn revoke_verification(pool: &PgPool, e: &VerificationRevoked) -> Result<(), sqlx::Error> {
    let id = counter_id(VERIFICATION_PREFIX, e.verification_id);
    sqlx::query!(
        r#"
        UPDATE indexer.verification_projection
        SET revoked = TRUE, revoked_ledger = $2, updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.revoked_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn upsert_escrow(
    pool: &PgPool,
    contract_id: &str,
    e: &EscrowCreated,
) -> Result<(), sqlx::Error> {
    let id = counter_id(ESCROW_PREFIX, e.escrow_id);
    sqlx::query!(
        r#"
        INSERT INTO indexer.escrow_projection
            (id, contract_id, depositor, beneficiary, amount, released_amount,
             booking_ref, condition_kind, condition_releaser, condition_timeout_ledger,
             created_ledger, updated_ledger, status)
        VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9, $10, $11, 'open')
        ON CONFLICT (id) DO UPDATE SET
            contract_id = EXCLUDED.contract_id,
            depositor = EXCLUDED.depositor,
            beneficiary = EXCLUDED.beneficiary,
            amount = EXCLUDED.amount,
            booking_ref = EXCLUDED.booking_ref,
            condition_kind = EXCLUDED.condition_kind,
            condition_releaser = EXCLUDED.condition_releaser,
            condition_timeout_ledger = EXCLUDED.condition_timeout_ledger,
            created_ledger = EXCLUDED.created_ledger,
            updated_ledger = EXCLUDED.updated_ledger,
            updated_at = NOW()
        "#,
        id,
        contract_id,
        e.depositor,
        e.beneficiary,
        e.amount,
        render_uuid7_ref("booking", &e.booking_ref_hex),
        e.condition_kind,
        e.condition_releaser,
        e.condition_timeout_ledger,
        e.created_ledger,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn add_escrow_deposit(pool: &PgPool, e: &EscrowDeposited) -> Result<(), sqlx::Error> {
    let id = counter_id(ESCROW_PREFIX, e.escrow_id);
    sqlx::query!(
        r#"
        UPDATE indexer.escrow_projection
        SET amount = amount + $2, updated_ledger = $3, updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.amount,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn release_escrow(pool: &PgPool, e: &EscrowReleased) -> Result<(), sqlx::Error> {
    let id = counter_id(ESCROW_PREFIX, e.escrow_id);
    sqlx::query!(
        r#"
        UPDATE indexer.escrow_projection
        SET released_amount = released_amount + $2,
            updated_ledger = $3,
            status = CASE WHEN released_amount + $2 >= amount THEN 'released' ELSE 'open' END,
            updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.released_amount,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn refund_escrow(pool: &PgPool, e: &EscrowRefunded) -> Result<(), sqlx::Error> {
    let id = counter_id(ESCROW_PREFIX, e.escrow_id);
    sqlx::query!(
        r#"
        UPDATE indexer.escrow_projection
        SET status = 'refunded',
            released_amount = amount,
            updated_ledger = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn upsert_financing(
    pool: &PgPool,
    contract_id: &str,
    e: &FinancingCreated,
) -> Result<(), sqlx::Error> {
    let id = counter_id(FINANCING_PREFIX, e.financing_id);
    sqlx::query!(
        r#"
        INSERT INTO indexer.financing_projection
            (id, contract_id, funder, beneficiary, total_amount, drawn_amount,
             milestone_count, milestones, drawn_ledger, repaid_amount, defaulted,
             defaulted_ledger, updated_ledger, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                CASE WHEN $11 THEN 'defaulted' ELSE 'active' END)
        ON CONFLICT (id) DO UPDATE SET
            contract_id = EXCLUDED.contract_id,
            funder = EXCLUDED.funder,
            beneficiary = EXCLUDED.beneficiary,
            total_amount = EXCLUDED.total_amount,
            drawn_amount = EXCLUDED.drawn_amount,
            milestone_count = EXCLUDED.milestone_count,
            milestones = EXCLUDED.milestones,
            drawn_ledger = EXCLUDED.drawn_ledger,
            repaid_amount = EXCLUDED.repaid_amount,
            defaulted = EXCLUDED.defaulted,
            defaulted_ledger = EXCLUDED.defaulted_ledger,
            updated_ledger = EXCLUDED.updated_ledger,
            updated_at = NOW()
        "#,
        id,
        contract_id,
        e.funder,
        e.beneficiary,
        e.total_amount,
        e.drawn_amount,
        e.milestone_count,
        e.milestones,
        e.drawn_ledger,
        e.repaid_amount,
        e.defaulted,
        e.defaulted_ledger,
        e.drawn_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn add_financing_deposit(pool: &PgPool, e: &FinancingDeposited) -> Result<(), sqlx::Error> {
    let id = counter_id(FINANCING_PREFIX, e.financing_id);
    sqlx::query!(
        r#"
        UPDATE indexer.financing_projection
        SET drawn_amount = drawn_amount + $2, updated_ledger = $3, updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.amount,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn release_financing(pool: &PgPool, e: &FinancingReleased) -> Result<(), sqlx::Error> {
    let id = counter_id(FINANCING_PREFIX, e.financing_id);
    sqlx::query!(
        r#"
        UPDATE indexer.financing_projection
        SET drawn_amount = drawn_amount + $2, drawn_ledger = $3, updated_ledger = $3,
            updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.released_amount,
        e.updated_ledger,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn refund_financing(pool: &PgPool, e: &FinancingRefunded) -> Result<(), sqlx::Error> {
    let id = counter_id(FINANCING_PREFIX, e.financing_id);
    sqlx::query!(
        r#"
        UPDATE indexer.financing_projection
        SET status = CASE
                WHEN $2 THEN 'defaulted'
                ELSE 'refunded'
            END,
            defaulted = $2,
            defaulted_ledger = COALESCE($3, defaulted_ledger),
            drawn_amount = GREATEST(drawn_amount - $4, 0),
            updated_at = NOW()
        WHERE id = $1
        "#,
        id,
        e.defaulted,
        e.defaulted_ledger,
        e.refunded_amount,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Identity projection maintains the on-chain fields of the existing `farmers`
/// table (address, registered, created/updated ledger, verification_markers).
/// Data is the `Farmer` struct (object form, docs/contracts/farmer-identity.md):
/// `{ address, metadata_hash, verification_markers, created_ledger, updated_ledger }`.
fn identity_topics(event: &ChainEvent) -> Option<String> {
    event
        .topics
        .as_array()?
        .first()?
        .as_str()
        .map(str::to_string)
}

fn obj_i64(data: &serde_json::Value, key: &str) -> Option<i64> {
    data.get(key)?.as_i64()
}

async fn upsert_identity_farmer(pool: &PgPool, event: &ChainEvent) -> Result<(), sqlx::Error> {
    let Some(address) = identity_topics(event) else {
        return Err(sqlx::Error::Protocol("bad FarmerRegistered payload".into()));
    };
    let id = crate::farmers::model::farmer_id(&address);
    let created = obj_i64(&event.data, "created_ledger");
    let updated = obj_i64(&event.data, "updated_ledger");
    let markers = event.data.get("verification_markers").cloned();

    sqlx::query!(
        r#"
        INSERT INTO farmers
            (address, id, metadata, metadata_hash, registered, created_ledger,
             updated_ledger, verification_markers)
        VALUES ($1, $2, '{}'::jsonb, '', TRUE, $3, $4, $5)
        ON CONFLICT (address) DO UPDATE SET
            registered = TRUE,
            created_ledger = COALESCE(EXCLUDED.created_ledger, farmers.created_ledger),
            updated_ledger = COALESCE(EXCLUDED.updated_ledger, farmers.updated_ledger),
            verification_markers = CASE
                WHEN EXCLUDED.verification_markers IS NULL THEN farmers.verification_markers
                ELSE EXCLUDED.verification_markers END,
            updated_at = NOW()
        "#,
        address,
        id,
        created,
        updated,
        markers.unwrap_or_else(|| serde_json::json!([])),
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_identity_metadata(pool: &PgPool, event: &ChainEvent) -> Result<(), sqlx::Error> {
    let Some(address) = identity_topics(event) else {
        return Err(sqlx::Error::Protocol(
            "bad FarmerMetadataUpdated payload".into(),
        ));
    };
    let updated = obj_i64(&event.data, "updated_ledger");
    sqlx::query!(
        r#"
        UPDATE farmers
        SET updated_ledger = COALESCE($2, updated_ledger), updated_at = NOW()
        WHERE address = $1
        "#,
        address,
        updated,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn append_identity_marker(pool: &PgPool, event: &ChainEvent) -> Result<(), sqlx::Error> {
    let Some(address) = identity_topics(event) else {
        return Err(sqlx::Error::Protocol(
            "bad VerificationMarkerSet payload".into(),
        ));
    };
    let marker = event.data.clone();
    sqlx::query!(
        r#"
        UPDATE farmers
        SET verification_markers = verification_markers || $2::jsonb, updated_at = NOW()
        WHERE address = $1
        "#,
        address,
        marker,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ESCROW_PREFIX, FINANCING_PREFIX, VERIFICATION_PREFIX, decode_escrow_created,
        decode_escrow_deposited, decode_escrow_refunded, decode_escrow_released,
        decode_financing_created, decode_financing_deposited, decode_financing_refunded,
        decode_financing_released, decode_verification_created, decode_verification_revoked,
        render_uuid7_ref,
    };
    use crate::ids::{counter_id, parse_counter_id};
    use crate::indexer::model::ChainEvent;

    fn event(name: &str, data: serde_json::Value) -> ChainEvent {
        ChainEvent {
            contract_id: "C1".into(),
            contract_name: "verification".into(),
            event_name: name.into(),
            ledger_sequence: 100,
            operation_index: 0,
            event_index: 0,
            topics: json!([42]),
            data,
        }
    }

    #[test]
    fn decodes_verification_created() {
        let ev = event(
            "VerificationCreated",
            json!([
                42, "018f0c2a", "GSUBJECT", "0f1e2d3c", "GISSUER", 100_000, false, 0
            ]),
        );
        let decoded = decode_verification_created(&ev).unwrap();
        assert_eq!(decoded.verification_id, 42);
        assert_eq!(decoded.subject, "GSUBJECT");
        assert_eq!(decoded.issuer, "GISSUER");
        assert_eq!(decoded.issued_ledger, 100_000);
        assert_eq!(
            counter_id(VERIFICATION_PREFIX, decoded.verification_id),
            "va:verification:000000000042"
        );
    }

    #[test]
    fn decodes_verification_revoked() {
        let ev = event("VerificationRevoked", json!([42, "", true, 100_120]));
        let decoded = decode_verification_revoked(&ev).unwrap();
        assert_eq!(decoded.verification_id, 42);
        assert_eq!(decoded.revoked_ledger, 100_120);
    }

    #[test]
    fn decodes_escrow_created() {
        let ev = event(
            "EscrowCreated",
            json!([
                11,
                "GDEPOSITOR",
                "GBENEFICIARY",
                10_000_000_000i64,
                1,
                "GRELEASER",
                0,
                "018f0c2a",
                100_000,
                100_000
            ]),
        );
        let decoded = decode_escrow_created(&ev).unwrap();
        assert_eq!(decoded.escrow_id, 11);
        assert_eq!(decoded.depositor, "GDEPOSITOR");
        assert_eq!(decoded.condition_kind, 1);
        assert_eq!(decoded.booking_ref_hex, "018f0c2a");
        assert_eq!(
            counter_id(ESCROW_PREFIX, decoded.escrow_id),
            "va:escrow:000000000011"
        );
    }

    #[test]
    fn decodes_escrow_mutations() {
        let deposited = event("EscrowDeposited", json!([11, "GDEPOSITOR", 5_000, 100_010]));
        assert_eq!(decode_escrow_deposited(&deposited).unwrap().amount, 5_000);

        let released = event("EscrowReleased", json!([11, "GRELEASER", 3_000, 100_020]));
        assert_eq!(
            decode_escrow_released(&released).unwrap().released_amount,
            3_000
        );

        let refunded = event("EscrowRefunded", json!([11, "GDEPOSITOR", 2_000, 100_030]));
        assert_eq!(
            decode_escrow_refunded(&refunded).unwrap().updated_ledger,
            100_030
        );
    }

    #[test]
    fn decodes_financing_created() {
        let ev = event(
            "FinancingCreated",
            json!([
                7,
                "GFUNDER",
                "GBENEFICIARY",
                50_000_000_000i64,
                10_000_000_000i64,
                2,
                json!([{ "index": 1, "deadline_ledger": 100_100, "proof_hash": "aa", "proof_amount": 10_000_000_000i64 }]),
                100_000,
                0,
                false,
                null
            ]),
        );
        let decoded = decode_financing_created(&ev).unwrap();
        assert_eq!(decoded.financing_id, 7);
        assert_eq!(decoded.funder, "GFUNDER");
        assert_eq!(decoded.beneficiary, "GBENEFICIARY");
        assert_eq!(decoded.total_amount, 50_000_000_000);
        assert_eq!(decoded.drawn_amount, 10_000_000_000);
        assert_eq!(decoded.milestone_count, 2);
        assert_eq!(decoded.milestones[0]["proof_amount"], 10_000_000_000i64);
        assert_eq!(
            counter_id(FINANCING_PREFIX, decoded.financing_id),
            "va:financing:000000000007"
        );
    }

    #[test]
    fn decodes_financing_mutations() {
        let deposited = event("FinancingDeposited", json!([7, 5_000_000_000i64, 100_010]));
        assert_eq!(
            decode_financing_deposited(&deposited).unwrap().amount,
            5_000_000_000
        );

        let released = event("FinancingReleased", json!([7, 3_000_000_000i64, 100_020]));
        assert_eq!(
            decode_financing_released(&released)
                .unwrap()
                .released_amount,
            3_000_000_000
        );

        let refunded = event(
            "FinancingRefunded",
            json!([7, 2_000_000_000i64, true, 100_030]),
        );
        let decoded = decode_financing_refunded(&refunded).unwrap();
        assert_eq!(decoded.refunded_amount, 2_000_000_000);
        assert!(decoded.defaulted);
        assert_eq!(decoded.defaulted_ledger, Some(100_030));
    }

    #[test]
    fn renders_counter_ids_and_references() {
        assert_eq!(
            counter_id("va:verification", 7),
            "va:verification:000000000007"
        );
        assert_eq!(
            parse_counter_id("va:escrow:000000000011", "va:escrow"),
            Some(11)
        );
        assert_eq!(
            render_uuid7_ref("batch", "not-a-uuid"),
            "va:batch:not-a-uuid"
        );
        let uuid = "018f0c2a-0000-7000-8000-000000000000";
        assert_eq!(
            render_uuid7_ref("booking", uuid),
            format!("va:booking:{uuid}")
        );
    }
}
