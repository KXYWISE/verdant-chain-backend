use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::error::AppError;
use crate::farmers::model::{Farmer, RegisterFarmerRequest, UpdateMetadataRequest};
use crate::farmers::service::{get_farmer, register_farmer, update_metadata};
use crate::state::AppState;

const ACTOR_HEADER: &str = "x-verdant-actor";

fn actor_from_headers(headers: &HeaderMap) -> Result<String, AppError> {
    headers
        .get(ACTOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Unauthorized("missing X-Verdant-Actor header".into()))
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
        (status = 401, description = "Actor header missing or does not match farmer"),
        (status = 409, description = "Farmer already registered")
    )
)]
async fn register_farmer_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterFarmerRequest>,
) -> Result<(axum::http::StatusCode, Json<Farmer>), AppError> {
    let actor = actor_from_headers(&headers)?;
    let farmer = register_farmer(&state.pool, state.chain.as_ref(), req, &actor).await?;
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
        (status = 401, description = "Actor header missing or does not match farmer"),
        (status = 404, description = "Unknown farmer")
    )
)]
async fn update_metadata_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(address): Path<String>,
    Json(req): Json<UpdateMetadataRequest>,
) -> Result<Json<Farmer>, AppError> {
    let actor = actor_from_headers(&headers)?;
    let farmer = update_metadata(&state.pool, state.chain.as_ref(), &address, req, &actor).await?;
    Ok(Json(farmer))
}

#[derive(OpenApi)]
#[openapi(
    paths(get_farmer_handler, register_farmer_handler, update_metadata_handler),
    components(schemas(
        Farmer,
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
