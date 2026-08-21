use axum::Router;
use utoipa::OpenApi;

use crate::state::AppState;

pub mod auth;
pub mod farmers;
pub mod health;
pub mod projections;

use crate::auth::model::{Challenge, ChallengeRequest, Session, SessionRow, VerifyRequest};
use crate::farmers::{
    Farmer, FarmerMetadata, RegisterFarmerRequest, UpdateMetadataRequest, VerificationMarker,
};
use crate::routes::projections::{
    Escrow, EscrowListResponse, Financing, FinancingListResponse, Pagination, Verification,
    VerificationListResponse,
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
        auth::session_handler,
        projections::get_verification_handler,
        projections::list_verifications_handler,
        projections::get_escrow_handler,
        projections::list_escrows_handler,
        projections::get_financing_handler,
        projections::list_financings_handler
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
        VerifyRequest,
        Verification,
        VerificationListResponse,
        Escrow,
        EscrowListResponse,
        Financing,
        FinancingListResponse,
        Pagination
    ))
)]
pub struct ApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(farmers::router())
        .merge(auth::router())
        .merge(projections::router())
}
