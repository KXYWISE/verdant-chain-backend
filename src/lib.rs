pub mod auth;
pub mod config;
pub mod error;
pub mod farmers;
pub mod ids;
pub mod indexer;
pub mod routes;
pub mod state;

use axum::Router;
use sqlx::PgPool;
use sqlx::migrate::MigrateError;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::farmers::chain::{IdentityChain, StubChain};

pub use config::Config;
pub use state::AppState;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub fn build_chain(config: &Config) -> Arc<dyn IdentityChain> {
    match config.chain.as_str() {
        "stub" => Arc::new(StubChain::new()),
        other => {
            tracing::warn!(chain = %other, "unknown chain backend, defaulting to stub");
            Arc::new(StubChain::new())
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(
            SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", routes::ApiDoc::openapi()),
        )
        .merge(routes::router())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
