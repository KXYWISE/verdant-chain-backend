use axum::Router;
use utoipa::OpenApi;

use crate::state::AppState;

pub mod auth;
pub mod farmers;
pub mod health;

use crate::auth::model::{Challenge, ChallengeRequest, Session, SessionRow, VerifyRequest};
use crate::farmers::{
    Farmer, FarmerMetadata, RegisterFarmerRequest, UpdateMetadataRequest, VerificationMarker,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        farmers::get_farmer_handler,
        farmers::register_farmer_handler,
        farmers::update_metadata_handler,
        farmers::search_farmers_handler,
        auth::challenge_handler,
        auth::verify_handler,
        auth::session_handler
    ),
    components(schemas(
        health::Health,
        Farmer,
        FarmerMetadata,
        VerificationMarker,
        RegisterFarmerRequest,
        UpdateMetadataRequest,
        Challenge,
        ChallengeRequest,
        Session,
        SessionRow,
        VerifyRequest
    ))
)]
pub struct ApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(farmers::router())
        .merge(auth::router())
}
