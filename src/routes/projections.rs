use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Shared pagination helpers (mirrors farmers search)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Pagination {
    pub page: i64,
    #[serde(rename = "pageSize")]
    pub page_size: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

fn normalize_page(page: i64) -> i64 {
    page.max(1)
}
fn normalize_page_size(page_size: i64) -> i64 {
    page_size.clamp(1, 100)
}

// ---------------------------------------------------------------------------
// Verification projection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Verification {
    pub id: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    #[serde(rename = "batchId")]
    pub batch_id: String,
    pub subject: String,
    #[serde(rename = "proofHash")]
    pub proof_hash: String,
    pub issuer: String,
    #[serde(rename = "issuedLedger")]
    pub issued_ledger: i64,
    pub revoked: bool,
    #[serde(rename = "revokedLedger")]
    pub revoked_ledger: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationListResponse {
    pub items: Vec<Verification>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListVerificationsQuery {
    #[serde(default)]
    #[param(example = "va:batch:018f0c2a-0000-7000-8000-000000000000")]
    pub batch_id: Option<String>,
    #[param(example = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF")]
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub revoked: Option<bool>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size", rename = "pageSize")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    20
}

fn normalize_verification_id(input: &str) -> Result<String, AppError> {
    if let Some(counter_part) = input.strip_prefix("va:verification:") {
        if counter_part.parse::<u64>().is_ok() {
            return Ok(input.to_string());
        }
        return Err(AppError::BadRequest(format!(
            "invalid verification id: {input}"
        )));
    }
    // Accept bare counter as convenience
    if let Ok(counter) = input.parse::<u64>() {
        return Ok(crate::ids::counter_id("va:verification", counter));
    }
    Err(AppError::BadRequest(format!(
        "invalid verification id: {input}"
    )))
}

/// GET /api/v1/verifications/:id
#[utoipa::path(
    get,
    path = "/api/v1/verifications/{id}",
    params(
        ("id" = String, Path, description = "Verification id va:verification:<12-digit> or bare counter")
    ),
    responses(
        (status = 200, description = "Verification projection", body = Verification),
        (status = 400, description = "Invalid id format"),
        (status = 404, description = "Verification not found")
    )
)]
async fn get_verification_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Verification>, AppError> {
    let normalized = normalize_verification_id(&id)?;
    let row = sqlx::query_as::<_, VerificationRow>(
        r#"SELECT id, contract_id, batch_id, subject, proof_hash, issuer, issued_ledger, revoked, revoked_ledger
           FROM indexer.verification_projection WHERE id = $1"#,
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("verification {id} not found")))?;
    Ok(Json(row.into()))
}

/// GET /api/v1/verifications
#[utoipa::path(
    get,
    path = "/api/v1/verifications",
    params(ListVerificationsQuery),
    responses(
        (status = 200, description = "List verifications", body = VerificationListResponse),
        (status = 400, description = "Invalid pagination"),
        (status = 500, description = "Infrastructure failure")
    )
)]
async fn list_verifications_handler(
    State(state): State<AppState>,
    Query(q): Query<ListVerificationsQuery>,
) -> Result<Json<VerificationListResponse>, AppError> {
    let page = normalize_page(q.page);
    let page_size = normalize_page_size(q.page_size);
    let offset = (page - 1) * page_size;

    // Build dynamic WHERE — use Option binds; NULL means no filter
    let rows = sqlx::query_as::<_, VerificationRow>(
        r#"SELECT id, contract_id, batch_id, subject, proof_hash, issuer, issued_ledger, revoked, revoked_ledger
           FROM indexer.verification_projection
           WHERE ($1::text IS NULL OR batch_id = $1)
             AND ($2::text IS NULL OR subject = $2)
             AND ($3::text IS NULL OR issuer = $3)
             AND ($4::boolean IS NULL OR revoked = $4)
           ORDER BY issued_ledger DESC, id ASC
           LIMIT $5 OFFSET $6"#,
    )
    .bind(&q.batch_id)
    .bind(&q.subject)
    .bind(&q.issuer)
    .bind(q.revoked)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM indexer.verification_projection
           WHERE ($1::text IS NULL OR batch_id = $1)
             AND ($2::text IS NULL OR subject = $2)
             AND ($3::text IS NULL OR issuer = $3)
             AND ($4::boolean IS NULL OR revoked = $4)"#,
    )
    .bind(&q.batch_id)
    .bind(&q.subject)
    .bind(&q.issuer)
    .bind(q.revoked)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total_pages = ((total + page_size - 1) / page_size).max(1);
    Ok(Json(VerificationListResponse {
        items: rows.into_iter().map(Into::into).collect(),
        pagination: Pagination {
            page,
            page_size,
            total,
            total_pages,
        },
    }))
}

#[derive(sqlx::FromRow)]
struct VerificationRow {
    id: String,
    contract_id: String,
    batch_id: String,
    subject: String,
    proof_hash: String,
    issuer: String,
    issued_ledger: i64,
    revoked: bool,
    revoked_ledger: Option<i64>,
}
impl From<VerificationRow> for Verification {
    fn from(r: VerificationRow) -> Self {
        Self {
            id: r.id,
            contract_id: r.contract_id,
            batch_id: r.batch_id,
            subject: r.subject,
            proof_hash: r.proof_hash,
            issuer: r.issuer,
            issued_ledger: r.issued_ledger,
            revoked: r.revoked,
            revoked_ledger: r.revoked_ledger,
        }
    }
}

// ---------------------------------------------------------------------------
// Escrow projection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Escrow {
    pub id: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    pub depositor: String,
    pub beneficiary: String,
    pub amount: i64,
    #[serde(rename = "releasedAmount")]
    pub released_amount: i64,
    #[serde(rename = "bookingRef")]
    pub booking_ref: Option<String>,
    #[serde(rename = "conditionKind")]
    pub condition_kind: i32,
    #[serde(rename = "conditionReleaser")]
    pub condition_releaser: Option<String>,
    #[serde(rename = "conditionTimeoutLedger")]
    pub condition_timeout_ledger: Option<i64>,
    #[serde(rename = "createdLedger")]
    pub created_ledger: i64,
    #[serde(rename = "updatedLedger")]
    pub updated_ledger: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EscrowListResponse {
    pub items: Vec<Escrow>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListEscrowsQuery {
    pub depositor: Option<String>,
    pub beneficiary: Option<String>,
    #[serde(rename = "bookingRef")]
    pub booking_ref: Option<String>,
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size", rename = "pageSize")]
    pub page_size: i64,
}

fn normalize_escrow_id(input: &str) -> Result<String, AppError> {
    if let Some(counter_part) = input.strip_prefix("va:escrow:") {
        if counter_part.parse::<u64>().is_ok() {
            return Ok(input.to_string());
        }
        return Err(AppError::BadRequest(format!("invalid escrow id: {input}")));
    }
    if let Ok(counter) = input.parse::<u64>() {
        return Ok(crate::ids::counter_id("va:escrow", counter));
    }
    Err(AppError::BadRequest(format!("invalid escrow id: {input}")))
}

/// GET /api/v1/escrows/:id
#[utoipa::path(
    get,
    path = "/api/v1/escrows/{id}",
    params(
        ("id" = String, Path, description = "Escrow id va:escrow:<12-digit> or bare counter")
    ),
    responses(
        (status = 200, description = "Escrow projection", body = Escrow),
        (status = 400, description = "Invalid id format"),
        (status = 404, description = "Escrow not found")
    )
)]
async fn get_escrow_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Escrow>, AppError> {
    let normalized = normalize_escrow_id(&id)?;
    let row = sqlx::query_as::<_, EscrowRow>(
        r#"SELECT id, contract_id, depositor, beneficiary, amount, released_amount, booking_ref, condition_kind, condition_releaser, condition_timeout_ledger, created_ledger, updated_ledger, status
           FROM indexer.escrow_projection WHERE id = $1"#,
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("escrow {id} not found")))?;
    Ok(Json(row.into()))
}

/// GET /api/v1/escrows
#[utoipa::path(
    get,
    path = "/api/v1/escrows",
    params(ListEscrowsQuery),
    responses(
        (status = 200, description = "List escrows", body = EscrowListResponse),
        (status = 500, description = "Infrastructure failure")
    )
)]
async fn list_escrows_handler(
    State(state): State<AppState>,
    Query(q): Query<ListEscrowsQuery>,
) -> Result<Json<EscrowListResponse>, AppError> {
    let page = normalize_page(q.page);
    let page_size = normalize_page_size(q.page_size);
    let offset = (page - 1) * page_size;

    let rows = sqlx::query_as::<_, EscrowRow>(
        r#"SELECT id, contract_id, depositor, beneficiary, amount, released_amount, booking_ref, condition_kind, condition_releaser, condition_timeout_ledger, created_ledger, updated_ledger, status
           FROM indexer.escrow_projection
           WHERE ($1::text IS NULL OR depositor = $1)
             AND ($2::text IS NULL OR beneficiary = $2)
             AND ($3::text IS NULL OR booking_ref = $3)
             AND ($4::text IS NULL OR status = $4)
           ORDER BY created_ledger DESC, id ASC
           LIMIT $5 OFFSET $6"#,
    )
    .bind(&q.depositor)
    .bind(&q.beneficiary)
    .bind(&q.booking_ref)
    .bind(&q.status)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM indexer.escrow_projection
           WHERE ($1::text IS NULL OR depositor = $1)
             AND ($2::text IS NULL OR beneficiary = $2)
             AND ($3::text IS NULL OR booking_ref = $3)
             AND ($4::text IS NULL OR status = $4)"#,
    )
    .bind(&q.depositor)
    .bind(&q.beneficiary)
    .bind(&q.booking_ref)
    .bind(&q.status)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total_pages = ((total + page_size - 1) / page_size).max(1);
    Ok(Json(EscrowListResponse {
        items: rows.into_iter().map(Into::into).collect(),
        pagination: Pagination {
            page,
            page_size,
            total,
            total_pages,
        },
    }))
}

#[derive(sqlx::FromRow)]
struct EscrowRow {
    id: String,
    contract_id: String,
    depositor: String,
    beneficiary: String,
    amount: i64,
    released_amount: i64,
    booking_ref: Option<String>,
    condition_kind: i32,
    condition_releaser: Option<String>,
    condition_timeout_ledger: Option<i64>,
    created_ledger: i64,
    updated_ledger: i64,
    status: String,
}
impl From<EscrowRow> for Escrow {
    fn from(r: EscrowRow) -> Self {
        Self {
            id: r.id,
            contract_id: r.contract_id,
            depositor: r.depositor,
            beneficiary: r.beneficiary,
            amount: r.amount,
            released_amount: r.released_amount,
            booking_ref: r.booking_ref,
            condition_kind: r.condition_kind,
            condition_releaser: r.condition_releaser,
            condition_timeout_ledger: r.condition_timeout_ledger,
            created_ledger: r.created_ledger,
            updated_ledger: r.updated_ledger,
            status: r.status,
        }
    }
}

// ---------------------------------------------------------------------------
// Financing projection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Financing {
    pub id: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    pub funder: String,
    pub beneficiary: String,
    #[serde(rename = "totalAmount")]
    pub total_amount: i64,
    #[serde(rename = "drawnAmount")]
    pub drawn_amount: i64,
    #[serde(rename = "milestoneCount")]
    pub milestone_count: i32,
    pub milestones: serde_json::Value,
    #[serde(rename = "drawnLedger")]
    pub drawn_ledger: Option<i64>,
    #[serde(rename = "repaidAmount")]
    pub repaid_amount: i64,
    pub defaulted: bool,
    #[serde(rename = "defaultedLedger")]
    pub defaulted_ledger: Option<i64>,
    #[serde(rename = "updatedLedger")]
    pub updated_ledger: Option<i64>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FinancingListResponse {
    pub items: Vec<Financing>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListFinancingsQuery {
    pub funder: Option<String>,
    pub beneficiary: Option<String>,
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size", rename = "pageSize")]
    pub page_size: i64,
}

fn normalize_financing_id(input: &str) -> Result<String, AppError> {
    if let Some(counter_part) = input.strip_prefix("va:financing:") {
        if counter_part.parse::<u64>().is_ok() {
            return Ok(input.to_string());
        }
        return Err(AppError::BadRequest(format!(
            "invalid financing id: {input}"
        )));
    }
    if let Ok(counter) = input.parse::<u64>() {
        return Ok(crate::ids::counter_id("va:financing", counter));
    }
    Err(AppError::BadRequest(format!(
        "invalid financing id: {input}"
    )))
}

/// GET /api/v1/financings/:id
#[utoipa::path(
    get,
    path = "/api/v1/financings/{id}",
    params(
        ("id" = String, Path, description = "Financing id va:financing:<12-digit> or bare counter")
    ),
    responses(
        (status = 200, description = "Financing projection", body = Financing),
        (status = 400, description = "Invalid id format"),
        (status = 404, description = "Financing not found")
    )
)]
async fn get_financing_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Financing>, AppError> {
    let normalized = normalize_financing_id(&id)?;
    let row = sqlx::query_as::<_, FinancingRow>(
        r#"SELECT id, contract_id, funder, beneficiary, total_amount, drawn_amount, milestone_count, milestones, drawn_ledger, repaid_amount, defaulted, defaulted_ledger, updated_ledger, status
           FROM indexer.financing_projection WHERE id = $1"#,
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("financing {id} not found")))?;
    Ok(Json(row.into()))
}

/// GET /api/v1/financings
#[utoipa::path(
    get,
    path = "/api/v1/financings",
    params(ListFinancingsQuery),
    responses(
        (status = 200, description = "List financings", body = FinancingListResponse),
        (status = 500, description = "Infrastructure failure")
    )
)]
async fn list_financings_handler(
    State(state): State<AppState>,
    Query(q): Query<ListFinancingsQuery>,
) -> Result<Json<FinancingListResponse>, AppError> {
    let page = normalize_page(q.page);
    let page_size = normalize_page_size(q.page_size);
    let offset = (page - 1) * page_size;

    let rows = sqlx::query_as::<_, FinancingRow>(
        r#"SELECT id, contract_id, funder, beneficiary, total_amount, drawn_amount, milestone_count, milestones, drawn_ledger, repaid_amount, defaulted, defaulted_ledger, updated_ledger, status
           FROM indexer.financing_projection
           WHERE ($1::text IS NULL OR funder = $1)
             AND ($2::text IS NULL OR beneficiary = $2)
             AND ($3::text IS NULL OR status = $3)
           ORDER BY updated_ledger DESC NULLS LAST, id ASC
           LIMIT $4 OFFSET $5"#,
    )
    .bind(&q.funder)
    .bind(&q.beneficiary)
    .bind(&q.status)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM indexer.financing_projection
           WHERE ($1::text IS NULL OR funder = $1)
             AND ($2::text IS NULL OR beneficiary = $2)
             AND ($3::text IS NULL OR status = $3)"#,
    )
    .bind(&q.funder)
    .bind(&q.beneficiary)
    .bind(&q.status)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    let total_pages = ((total + page_size - 1) / page_size).max(1);
    Ok(Json(FinancingListResponse {
        items: rows.into_iter().map(Into::into).collect(),
        pagination: Pagination {
            page,
            page_size,
            total,
            total_pages,
        },
    }))
}

#[derive(sqlx::FromRow)]
struct FinancingRow {
    id: String,
    contract_id: String,
    funder: String,
    beneficiary: String,
    total_amount: i64,
    drawn_amount: i64,
    milestone_count: i32,
    milestones: serde_json::Value,
    drawn_ledger: Option<i64>,
    repaid_amount: i64,
    defaulted: bool,
    defaulted_ledger: Option<i64>,
    updated_ledger: Option<i64>,
    status: String,
}
impl From<FinancingRow> for Financing {
    fn from(r: FinancingRow) -> Self {
        Self {
            id: r.id,
            contract_id: r.contract_id,
            funder: r.funder,
            beneficiary: r.beneficiary,
            total_amount: r.total_amount,
            drawn_amount: r.drawn_amount,
            milestone_count: r.milestone_count,
            milestones: r.milestones,
            drawn_ledger: r.drawn_ledger,
            repaid_amount: r.repaid_amount,
            defaulted: r.defaulted,
            defaulted_ledger: r.defaulted_ledger,
            updated_ledger: r.updated_ledger,
            status: r.status,
        }
    }
}

// ---------------------------------------------------------------------------
// Router + OpenAPI doc
// ---------------------------------------------------------------------------

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        get_verification_handler,
        list_verifications_handler,
        get_escrow_handler,
        list_escrows_handler,
        get_financing_handler,
        list_financings_handler
    ),
    components(schemas(
        Verification,
        VerificationListResponse,
        Escrow,
        EscrowListResponse,
        Financing,
        FinancingListResponse,
        Pagination
    ))
)]
pub struct ProjectionsApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/verifications/{id}",
            axum::routing::get(get_verification_handler),
        )
        .route(
            "/api/v1/verifications",
            axum::routing::get(list_verifications_handler),
        )
        .route(
            "/api/v1/escrows/{id}",
            axum::routing::get(get_escrow_handler),
        )
        .route("/api/v1/escrows", axum::routing::get(list_escrows_handler))
        .route(
            "/api/v1/financings/{id}",
            axum::routing::get(get_financing_handler),
        )
        .route(
            "/api/v1/financings",
            axum::routing::get(list_financings_handler),
        )
}
