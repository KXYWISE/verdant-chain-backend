pub mod health;

use axum::Router;
use utoipa::OpenApi;

use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(paths(health::health), components(schemas(health::Health)))]
pub struct ApiDoc;

pub fn router() -> Router<AppState> {
    Router::new().merge(health::router())
}
