use std::sync::Arc;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use verdant_backend::{AppState, Config, app, build_chain, connect, migrate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let config = Config::from_env()?;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = connect(&config.database_url).await?;
    migrate(&pool).await?;

    if !config.rpc_url.is_empty() && !config.indexer_contracts.is_empty() {
        let source = Arc::new(verdant_backend::indexer::RpcEventSource::new(
            &config.rpc_url,
            config.indexer_contracts.clone(),
        )?);
        let contract_ids: Vec<String> = config.indexer_contracts.keys().cloned().collect();
        verdant_backend::indexer::service::spawn_indexer(pool.clone(), source, contract_ids);
        tracing::info!(
            rpc_url = %config.rpc_url,
            contracts = ?config.indexer_contracts.keys().collect::<Vec<_>>(),
            "indexer subscriber started"
        );
    } else {
        tracing::info!("indexer running on stub chain (no RPC URL configured)");
    }

    let chain = build_chain(&config);
    let state = AppState::new(pool, chain)
        .with_domain(&config.domain)
        .with_session_ttl(config.session_ttl);
    let app = app(state);

    let listener = tokio::net::TcpListener::bind(config.addr()).await?;
    tracing::info!(addr = %config.addr(), "verdant-backend listening");
    axum::serve(listener, app).await?;

    Ok(())
}
