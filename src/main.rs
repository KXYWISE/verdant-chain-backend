use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use verdant_backend::{AppState, Config, app, connect, migrate};

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

    let state = AppState::new(pool);
    let app = app(state);

    let listener = tokio::net::TcpListener::bind(config.addr()).await?;
    tracing::info!(addr = %config.addr(), "verdant-backend listening");
    axum::serve(listener, app).await?;

    Ok(())
}
