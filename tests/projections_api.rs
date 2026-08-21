use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use verdant_backend::farmers::chain::StubChain;
use verdant_backend::{AppState, app, connect, migrate};

async fn test_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let pool = connect(&database_url).await.expect("connect");
    migrate(&pool).await.expect("migrate");
    // Clean indexer projections
    sqlx::query(
        "TRUNCATE indexer.verification_projection, indexer.escrow_projection, indexer.financing_projection, indexer.indexed_events, indexer.indexer_cursors",
    )
    .execute(&pool)
    .await
    .expect("truncate");
    let chain = Arc::new(StubChain::new());
    AppState::new(pool, chain)
}

async fn seed_verification(pool: &sqlx::PgPool) {
    sqlx::query(
        r#"INSERT INTO indexer.verification_projection
           (id, contract_id, batch_id, subject, proof_hash, issuer, issued_ledger, revoked, revoked_ledger)
           VALUES ('va:verification:000000000042', 'CCONTRACT', 'va:batch:018f0c2a-0000-7000-8000-000000000000',
                   'GSUBJECT', '0f1e2d3c', 'GISSUER', 100000, FALSE, NULL)"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO indexer.verification_projection
           (id, contract_id, batch_id, subject, proof_hash, issuer, issued_ledger, revoked, revoked_ledger)
           VALUES ('va:verification:000000000043', 'CCONTRACT', 'va:batch:018f0c2b-0000-7000-8000-000000000001',
                   'GSUBJECT2', 'aabbcc', 'GISSUER', 100135, TRUE, 100120)"#,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_escrow(pool: &sqlx::PgPool) {
    sqlx::query(
        r#"INSERT INTO indexer.escrow_projection
           (id, contract_id, depositor, beneficiary, amount, released_amount, booking_ref, condition_kind, condition_releaser, condition_timeout_ledger, created_ledger, updated_ledger, status)
           VALUES ('va:escrow:000000000011', 'CCONTRACT', 'GDEPOSITOR', 'GBENEFICIARY', 10000000000, 0,
                   'va:booking:018f0c2a-0000-7000-8000-000000000000', 1, 'GRELEASER', NULL, 100000, 100000, 'open')"#,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_financing(pool: &sqlx::PgPool) {
    sqlx::query(
        r#"INSERT INTO indexer.financing_projection
           (id, contract_id, funder, beneficiary, total_amount, drawn_amount, milestone_count, milestones, drawn_ledger, repaid_amount, defaulted, defaulted_ledger, updated_ledger, status)
           VALUES ('va:financing:000000000007', 'CCONTRACT', 'GFUNDER', 'GBENEFICIARY', 50000000000, 10000000000, 2, '[{"index":1}]'::jsonb, 100000, 0, FALSE, NULL, 100000, 'active')"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO indexer.financing_projection
           (id, contract_id, funder, beneficiary, total_amount, drawn_amount, milestone_count, milestones, drawn_ledger, repaid_amount, defaulted, defaulted_ledger, updated_ledger, status)
           VALUES ('va:financing:000000000008', 'CCONTRACT', 'GFUNDER2', 'GBENEF2', 30000000000, 5000000000, 1, '[]'::jsonb, 100010, 0, TRUE, 100030, 100030, 'defaulted')"#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn verifications_get_by_id_ok() {
    let state = test_state().await;
    seed_verification(&state.pool).await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications/va:verification:000000000042")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["id"], "va:verification:000000000042");
    assert_eq!(json["subject"], "GSUBJECT");
    assert_eq!(json["revoked"], false);
}

#[tokio::test]
async fn verifications_get_bare_counter_ok() {
    let state = test_state().await;
    seed_verification(&state.pool).await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications/42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn verifications_get_invalid_id_returns_400() {
    let state = test_state().await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications/not-an-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn verifications_get_not_found_returns_404() {
    let state = test_state().await;
    seed_verification(&state.pool).await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications/va:verification:000000000099")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn verifications_list_filters_and_pagination() {
    let state = test_state().await;
    seed_verification(&state.pool).await;
    // Filter by subject
    let resp = app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications?subject=GSUBJECT&page=1&pageSize=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["pagination"]["total"], 1);
    assert_eq!(json["items"].as_array().unwrap().len(), 1);
    // Filter by revoked
    let resp2 = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/verifications?revoked=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["pagination"]["total"], 1);
    assert_eq!(json2["items"][0]["id"], "va:verification:000000000043");
}

#[tokio::test]
async fn escrows_get_and_list() {
    let state = test_state().await;
    seed_escrow(&state.pool).await;
    let resp = app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/escrows/va:escrow:000000000011")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["depositor"], "GDEPOSITOR");
    // list filter by depositor
    let resp2 = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/escrows?depositor=GDEPOSITOR")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["pagination"]["total"], 1);
}

#[tokio::test]
async fn financings_get_and_list_with_status_filter() {
    let state = test_state().await;
    seed_financing(&state.pool).await;
    let resp = app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/financings/va:financing:000000000007")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["funder"], "GFUNDER");
    // list filter by status
    let resp2 = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/financings?status=defaulted")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["pagination"]["total"], 1);
    assert_eq!(json2["items"][0]["id"], "va:financing:000000000008");
}

#[tokio::test]
async fn escrows_get_invalid_returns_400() {
    let state = test_state().await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/escrows/bad-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn financings_get_not_found_returns_404() {
    let state = test_state().await;
    seed_financing(&state.pool).await;
    let resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/financings/va:financing:000000000099")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
