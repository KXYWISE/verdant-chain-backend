use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub log_level: String,
    pub database_url: String,
    pub chain: String,
    pub domain: String,
    pub session_ttl: std::time::Duration,
    /// Soroban RPC endpoint used by the real event indexer
    /// (`VERDANT_BACKEND_RPC_URL`), empty when running on the stub chain.
    pub rpc_url: String,
    /// contract_id (`C…`) → indexer contract name
    /// (`VERDANT_BACKEND_INDEXER_CONTRACTS`, `name:contract,name:contract`).
    pub indexer_contracts: HashMap<String, String>,
}

/// Parses `name:contract,name:contract` into contract-id → name.
fn parse_contracts(raw: &str) -> HashMap<String, String> {
    raw.split(',')
        .filter_map(|pair| {
            let (name, contract) = pair.split_once(':')?;
            let name = name.trim();
            let contract = contract.trim();
            (!name.is_empty() && !contract.is_empty())
                .then(|| (contract.to_string(), name.to_string()))
        })
        .collect()
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("VERDANT_BACKEND_HOST")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(IpAddr::from([127, 0, 0, 1]));
        let port = env::var("VERDANT_BACKEND_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(8080);
        let log_level =
            env::var("VERDANT_BACKEND_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        let chain = env::var("VERDANT_BACKEND_CHAIN").unwrap_or_else(|_| "stub".to_string());
        let domain =
            env::var("VERDANT_BACKEND_DOMAIN").unwrap_or_else(|_| "app.verdant.example".into());
        let session_ttl_secs = env::var("VERDANT_BACKEND_SESSION_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7 * 24 * 60 * 60);
        let rpc_url = env::var("VERDANT_BACKEND_RPC_URL").unwrap_or_else(|_| "".to_string());
        let indexer_contracts = parse_contracts(
            &env::var("VERDANT_BACKEND_INDEXER_CONTRACTS").unwrap_or_else(|_| "".to_string()),
        );
        Ok(Self {
            host,
            port,
            log_level,
            database_url,
            chain,
            domain,
            session_ttl: std::time::Duration::from_secs(session_ttl_secs),
            rpc_url,
            indexer_contracts,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("DATABASE_URL environment variable is not set")]
    MissingDatabaseUrl,
}

#[cfg(test)]
mod tests {
    use super::parse_contracts;

    #[test]
    fn parses_indexer_contracts() {
        let map = parse_contracts("verification:CDEBUG,escrow:C2,financing:C3");
        assert_eq!(map.get("CDEBUG"), Some(&"verification".to_string()));
        assert_eq!(map.get("C2"), Some(&"escrow".to_string()));
        assert_eq!(map.get("C3"), Some(&"financing".to_string()));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn skips_empty_contract_pairs() {
        let map = parse_contracts("verification:CDEBUG,,escrow:");
        assert_eq!(map.len(), 1);
    }
}
