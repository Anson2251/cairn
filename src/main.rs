use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use cairn::{
    config::AppConfig,
    create_router, db, AppState, auth::jwt::JwtManager,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file first, before initializing tracing
    match dotenvy::dotenv() {
        Ok(path) => eprintln!(".env file loaded from: {:?}", path),
        Err(e) => eprintln!("No .env file found or failed to load: {}", e),
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Cairn server...");

    let config = AppConfig::new()?;
    
    info!(
        "Configuration loaded - Server: {}:{}, Database: {}, Redis: {}",
        config.server.host,
        config.server.port,
        config.database.url,
        config.redis.url
    );

    db::create_database_if_not_exists(&config.database.url).await?;
    let db_pool = db::create_pool(&config.database).await?;
    db::run_migrations(&db_pool).await?;

    let redis_client = redis::Client::open(config.redis.url.clone())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;

    let jwt = JwtManager::new(&config.jwt)?;

    let state = Arc::new(AppState {
        db: db_pool,
        redis: redis_conn,
        jwt: Arc::new(jwt),
        config: config.clone(),
    });

    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
