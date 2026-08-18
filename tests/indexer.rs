use serde_json::{Value, json};
use std::sync::Arc;

use verdant_backend::indexer::chain::StubEventSource;
use verdant_backend::indexer::model::ChainEvent;
use verdant_backend::indexer::service;
use verdant_backend::indexer::store;
use verdant_backend::{connect, migrate};

fn test_address(n: u8) -> String {
    let payload = [n; 32];
    stellar_strkey::ed25519::PublicKey::from_payload(&payload)
        .unwrap()
        .to_string()
        .to_string()
}

fn event(
    contract_name: &str,
    event_name: &str,
    ledger: i64,
    data: Value,
    topics: Value,
) -> ChainEvent {
    ChainEvent {
        contract_id: "CCONTRACT".into(),
        contract_name: contract_name.into(),
        event_name: event_name.into(),
        ledger_sequence: ledger,
        operation_index: 0,
        event_index: 0,
        topics,
        data,
    }
}

async fn test_pool() -> sqlx::PgPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let pool = connect(&database_url)
        .await
        .expect("connect to test database");
    migrate(&pool).await.expect("run migrations");
    sqlx::query(
        "TRUNCATE indexer.indexed_events, indexer.indexer_cursors, \
         indexer.verification_projection, indexer.escrow_projection, farmers, documents",
    )
    .execute(&pool)
    .await
    .expect("clean indexer tables");
    pool
}

#[tokio::test]
async fn verification_projection_builds_from_events() {
    let pool = test_pool().await;
    let source = Arc::new(StubEventSource::new());

    source.push(event(
        "verification",
        "VerificationCreated",
        100_000,
        json!([
            42,
            "018f0c2a-0000-7000-8000-000000000000",
            "GSUBJECT",
            "0f1e2d3c",
            "GISSUER",
            100_000,
            false,
            0
        ]),
        json!([42]),
    ));
    source.push(event(
        "verification",
        "VerificationRevoked",
        100_120,
        json!([42, "", true, 100_120]),
        json!([42]),
    ));
    // Recent events within the trusted cutoff (head - 10) are ingested but not
    // finalized. head = 100_140 → floor = 100_130.
    source.push(event(
        "verification",
        "VerificationCreated",
        100_135,
        json!([
            43,
            "018f0c2b-0000-7000-8000-000000000000",
            "GSUBJECT",
            "aabbcc",
            "GISSUER",
            100_135,
            false,
            0
        ]),
        json!([43]),
    ));
    source.set_head(100_140);

    let ingested = service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("ingest");
    assert_eq!(ingested, 3);

    let cursor = store::get_cursor(&pool, "CCONTRACT").await.unwrap();
    assert_eq!(cursor.ledger_sequence, 100_135);

    // Events at ledger <= head - 10 (100_130) are finalized.
    let row = sqlx::query!(
        r#"
        SELECT id, subject, issuer, issued_ledger, revoked, revoked_ledger, batch_id
        FROM indexer.verification_projection
        WHERE id = 'va:verification:000000000042'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.subject, "GSUBJECT");
    assert_eq!(row.issuer, "GISSUER");
    assert_eq!(row.issued_ledger, 100_000);
    assert!(row.revoked);
    assert_eq!(row.revoked_ledger, Some(100_120));
    assert_eq!(
        row.batch_id,
        "va:batch:018f0c2a-0000-7000-8000-000000000000"
    );

    let not_finalized = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM indexer.verification_projection WHERE id = 'va:verification:000000000043'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(not_finalized, 0);
}

#[tokio::test]
async fn escrow_projection_accumulates_deposits_releases_refunds() {
    let pool = test_pool().await;
    let source = Arc::new(StubEventSource::new());

    source.push(event(
        "escrow",
        "EscrowCreated",
        100_000,
        json!([
            11,
            "GDEPOSITOR",
            "GBENEFICIARY",
            10_000_000_000i64,
            1,
            "GRELEASER",
            0,
            "018f0c2a-0000-7000-8000-000000000000",
            100_000,
            100_000
        ]),
        json!([11, "GDEPOSITOR"]),
    ));
    source.push(event(
        "escrow",
        "EscrowDeposited",
        100_010,
        json!([11, "GDEPOSITOR", 5_000_000_000i64, 100_010]),
        json!([11, "GDEPOSITOR"]),
    ));
    source.push(event(
        "escrow",
        "EscrowReleased",
        100_020,
        json!([11, "GRELEASER", 3_000_000_000i64, 100_020]),
        json!([11, "GRELEASER"]),
    ));
    source.push(event(
        "escrow",
        "EscrowRefunded",
        100_030,
        json!([11, "GDEPOSITOR", 12_000_000_000i64, 100_030]),
        json!([11, "GDEPOSITOR"]),
    ));
    source.set_head(100_040);

    service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("ingest");

    let row = sqlx::query!(
        r#"
        SELECT depositor, beneficiary, amount, released_amount, booking_ref, status
        FROM indexer.escrow_projection
        WHERE id = 'va:escrow:000000000011'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.depositor, "GDEPOSITOR");
    assert_eq!(row.beneficiary, "GBENEFICIARY");
    // deposit added to amount, released subtracted, refund closes at amount
    assert_eq!(row.amount, 15_000_000_000);
    assert_eq!(row.released_amount, 15_000_000_000);
    assert_eq!(row.status, "refunded");
    assert_eq!(
        row.booking_ref,
        Some("va:booking:018f0c2a-0000-7000-8000-000000000000".into())
    );
}

#[tokio::test]
async fn identity_projection_upserts_farmers_onchain_fields() {
    let pool = test_pool().await;
    let source = Arc::new(StubEventSource::new());
    let addr = test_address(1);

    source.push(event(
        "identity",
        "FarmerRegistered",
        100_000,
        json!({
            "address": addr,
            "metadata_hash": "deadbeef",
            "verification_markers": [],
            "created_ledger": 100_000,
            "updated_ledger": 100_000
        }),
        json!([addr]),
    ));
    source.push(event(
        "identity",
        "VerificationMarkerSet",
        100_050,
        json!({ "kind": "organic", "issuer": "GISSUER", "issued_ledger": 100_050 }),
        json!([addr]),
    ));
    source.set_head(100_060);

    service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("ingest");

    let row = sqlx::query!(
        r#"
        SELECT address, id, registered, created_ledger, updated_ledger, verification_markers
        FROM farmers WHERE address = $1
        "#,
        addr,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.id, format!("va:farmer:{addr}"));
    assert!(row.registered);
    assert_eq!(row.created_ledger, Some(100_000));
    assert_eq!(row.updated_ledger, Some(100_000));
    let markers: Vec<Value> = serde_json::from_value(row.verification_markers).unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0]["kind"], "organic");
}

#[tokio::test]
async fn re_ingest_is_idempotent() {
    let pool = test_pool().await;
    let source = Arc::new(StubEventSource::new());

    source.push(event(
        "verification",
        "VerificationCreated",
        100_000,
        json!([
            42,
            "018f0c2a-0000-7000-8000-000000000000",
            "GSUBJECT",
            "0f1e2d3c",
            "GISSUER",
            100_000,
            false,
            0
        ]),
        json!([42]),
    ));
    source.set_head(100_020);

    service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("first ingest");
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM indexer.indexed_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Replaying the same source must not duplicate raw events or rows.
    let reingested = service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("second ingest");
    assert_eq!(reingested, 0);

    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM indexer.indexed_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM indexer.verification_projection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn rewind_drops_events_and_resumes_from_divergent_ledger() {
    let pool = test_pool().await;
    let source = Arc::new(StubEventSource::new());

    source.push(event(
        "verification",
        "VerificationCreated",
        100_000,
        json!([
            1,
            "018f0c2a-0000-7000-8000-000000000000",
            "GSUBJECT",
            "aa",
            "GISSUER",
            100_000,
            false,
            0
        ]),
        json!([1]),
    ));
    source.set_head(100_020);
    service::ingest(&pool, source.as_ref(), "CCONTRACT")
        .await
        .expect("ingest");

    store::rewind(&pool, "CCONTRACT", 100_010)
        .await
        .expect("rewind");

    // Only events at or after the divergent ledger are dropped.
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM indexer.indexed_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let cursor = store::get_cursor(&pool, "CCONTRACT").await.unwrap();
    assert_eq!(cursor, verdant_backend::indexer::model::Cursor::START);
}
