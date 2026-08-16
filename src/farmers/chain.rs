use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct ChainFarmer {
    pub address: String,
    pub created_ledger: i64,
    pub updated_ledger: i64,
    pub verification_markers: Vec<FarmerVerificationMarker>,
}

#[derive(Debug, Clone)]
pub struct FarmerVerificationMarker {
    pub kind: String,
    pub issuer: String,
    pub issued_ledger: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("already registered")]
    AlreadyRegistered,
    #[error("farmer not found")]
    NotFound,
    #[error("internal: {0}")]
    Internal(String),
}

#[async_trait]
pub trait IdentityChain: Send + Sync {
    async fn register_farmer(
        &self,
        address: &str,
        metadata_hash: &str,
    ) -> Result<ChainFarmer, ChainError>;
    async fn update_metadata(
        &self,
        address: &str,
        metadata_hash: &str,
    ) -> Result<ChainFarmer, ChainError>;
    async fn is_registered(&self, address: &str) -> Result<bool, ChainError>;
}

#[derive(Debug, Default)]
pub struct StubChain {
    inner: Arc<Mutex<HashMap<String, ChainFarmer>>>,
}

impl StubChain {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl IdentityChain for StubChain {
    async fn register_farmer(
        &self,
        address: &str,
        _metadata_hash: &str,
    ) -> Result<ChainFarmer, ChainError> {
        let mut store = self.inner.lock().await;
        if store.contains_key(address) {
            return Err(ChainError::AlreadyRegistered);
        }
        let ledger = 100_000 + store.len() as i64;
        let farmer = ChainFarmer {
            address: address.into(),
            created_ledger: ledger,
            updated_ledger: ledger,
            verification_markers: vec![],
        };
        store.insert(address.into(), farmer.clone());
        Ok(farmer)
    }

    async fn update_metadata(
        &self,
        address: &str,
        _metadata_hash: &str,
    ) -> Result<ChainFarmer, ChainError> {
        let mut store = self.inner.lock().await;
        let len = store.len();
        let farmer = store.get_mut(address).ok_or(ChainError::NotFound)?;
        farmer.updated_ledger = 100_000 + len as i64;
        Ok(farmer.clone())
    }

    async fn is_registered(&self, address: &str) -> Result<bool, ChainError> {
        let store = self.inner.lock().await;
        Ok(store.contains_key(address))
    }
}
