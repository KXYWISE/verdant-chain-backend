use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    pub status: String,
    pub database: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service health and database connectivity", body = Health)
    )
)]
pub async fn health(State(state): State<AppState>) -> Result<Json<Health>, AppError> {
    let database_up = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    Ok(Json(Health {
        status: if database_up { "ok" } else { "degraded" }.to_string(),
        database: if database_up { "up" } else { "down" }.to_string(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}
