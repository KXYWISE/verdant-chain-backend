use sqlx::PgPool;
use std::sync::Arc;

use crate::farmers::chain::IdentityChain;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub chain: Arc<dyn IdentityChain>,
}

impl AppState {
    pub fn new(pool: PgPool, chain: Arc<dyn IdentityChain>) -> Self {
        Self { pool, chain }
    }
}
