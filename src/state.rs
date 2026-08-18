use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

use crate::farmers::chain::IdentityChain;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub chain: Arc<dyn IdentityChain>,
    pub domain: String,
    pub session_ttl: Duration,
}

impl AppState {
    pub fn new(pool: PgPool, chain: Arc<dyn IdentityChain>) -> Self {
        Self {
            pool,
            chain,
            domain: "app.verdant.example".into(),
            session_ttl: Duration::from_secs(7 * 24 * 60 * 60),
        }
    }

    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = domain.into();
        self
    }

    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.session_ttl = ttl;
        self
    }
}
