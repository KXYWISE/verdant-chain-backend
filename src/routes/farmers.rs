use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::auth::extractor::AuthUser;
use crate::error::AppError;
use crate::farmers::model::{Farmer, RegisterFarmerRequest, UpdateMetadataRequest};
use crate::farmers::service::{
    FarmerSearchResponse, get_farmer, register_farmer, search_farmers, update_metadata,
};
use crate::state::AppState;

#[derive(Deserialize, Serialize, Debug)]
pub struct SearchFarmersQuery {
    #[serde(default)]
    pub q: Option<String>,
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

/// GET /api/v1/farmers/:address
#[utoipa::path(
    get,
    path = "/api/v1/farmers/{address}",
    params(
        ("address" = String, Path, description = "Stellar public key (G…) or va:farmer:G… form")
    ),
    responses(
        (status = 200, description = "Farmer identity record + off-chain profile", body = Farmer),
        (status = 400, description = "Invalid address format"),
        (status = 404, description = "Unknown farmer"),
        (status = 500, description = "Indexer / infrastructure failure")
    )
)]
async fn get_farmer_handler(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<Farmer>, AppError> {
    get_farmer(&state.pool, &address).await.map(Json)
}

/// POST /api/v1/farmers/register
#[utoipa::path(
    post,
    path = "/api/v1/farmers/register",
    request_body = RegisterFarmerRequest,
    responses(
        (status = 201, description = "Farmer registered", body = Farmer),
        (status = 400, description = "Invalid address, empty name, or bad metadataHash"),
        (status = 401, description = "Bearer token missing/invalid or authenticated address does not match farmer"),
        (status = 409, description = "Farmer already registered")
    )
)]
async fn register_farmer_handler(
    State(state): State<AppState>,
    actor: AuthUser,
    Json(req): Json<RegisterFarmerRequest>,
) -> Result<(axum::http::StatusCode, Json<Farmer>), AppError> {
    let farmer = register_farmer(&state.pool, state.chain.as_ref(), req, &actor.address).await?;
    Ok((axum::http::StatusCode::CREATED, Json(farmer)))
}

/// PUT /api/v1/farmers/:address/metadata
#[utoipa::path(
    put,
    path = "/api/v1/farmers/{address}/metadata",
    params(
        ("address" = String, Path, description = "Stellar public key (G…) or va:farmer:G… form")
    ),
    request_body = UpdateMetadataRequest,
    responses(
        (status = 200, description = "Updated farmer record", body = Farmer),
        (status = 400, description = "Invalid address or empty name"),
        (status = 401, description = "Bearer token missing/invalid or authenticated address does not match farmer"),
        (status = 404, description = "Unknown farmer")
    )
)]
async fn update_metadata_handler(
    State(state): State<AppState>,
    actor: AuthUser,
    Path(address): Path<String>,
    Json(req): Json<UpdateMetadataRequest>,
) -> Result<Json<Farmer>, AppError> {
    let farmer = update_metadata(
        &state.pool,
        state.chain.as_ref(),
        &address,
        req,
        &actor.address,
    )
    .await?;
    Ok(Json(farmer))
}

/// GET /api/v1/farmers
#[utoipa::path(
    get,
    path = "/api/v1/farmers",
    params(
        ("q" = Option<String>, Query, description = "Substring search on name, region, district (case-insensitive)"),
        ("page" = Option<i64>, Query, description = "1-indexed page number (default 1)"),
        ("page_size" = Option<i64>, Query, description = "Results per page, max 100 (default 20)")
    ),
    responses(
        (status = 200, description = "Search farmers (AgriScout directory)", body = FarmerSearchResponse),
        (status = 400, description = "Invalid pagination params"),
        (status = 500, description = "Infrastructure failure")
    )
)]
async fn search_farmers_handler(
    State(state): State<AppState>,
    Query(query): Query<SearchFarmersQuery>,
) -> Result<Json<FarmerSearchResponse>, AppError> {
    search_farmers(&state.pool, query.q, query.page, query.page_size)
        .await
        .map(Json)
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_farmer_handler,
        register_farmer_handler,
        update_metadata_handler,
        search_farmers_handler
    ),
    components(schemas(
        Farmer,
        FarmerSearchResponse,
        crate::farmers::service::FarmerSearchItem,
        crate::farmers::service::Pagination,
        crate::farmers::model::FarmerMetadata,
        crate::farmers::model::VerificationMarker,
        crate::farmers::model::RegisterFarmerRequest,
        crate::farmers::model::UpdateMetadataRequest
    ))
)]
pub struct FarmersApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/farmers",
            axum::routing::get(search_farmers_handler),
        )
        .route(
            "/api/v1/farmers/{address}",
            axum::routing::get(get_farmer_handler),
        )
        .route(
            "/api/v1/farmers/register",
            axum::routing::post(register_farmer_handler),
        )
        .route(
            "/api/v1/farmers/{address}/metadata",
            axum::routing::put(update_metadata_handler),
        )
}
