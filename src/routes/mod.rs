use axum::Router;
use utoipa::OpenApi;

use crate::state::AppState;

pub mod farmers;
pub mod health;

use crate::farmers::{
    Farmer, FarmerMetadata, RegisterFarmerRequest, UpdateMetadataRequest, VerificationMarker,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        farmers::get_farmer_handler,
        farmers::register_farmer_handler,
        farmers::update_metadata_handler
    ),
    components(schemas(
        health::Health,
        Farmer,
        FarmerMetadata,
        VerificationMarker,
        RegisterFarmerRequest,
        UpdateMetadataRequest
    ))
)]
pub struct ApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(farmers::router())
}
