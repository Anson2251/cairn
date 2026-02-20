use sha2::Digest;
use sqlx::{migrate::MigrateDatabase, postgres::PgPoolOptions, PgPool};
use tracing::info;

use crate::config::DatabaseConfig;
use crate::error::AppResult;

pub async fn create_pool(config: &DatabaseConfig) -> AppResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .connect(&config.url)
        .await?;

    info!("Database pool created successfully");
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> AppResult<()> {
    info!("Running database migrations...");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT NOW(),
            success BOOLEAN NOT NULL,
            checksum BYTEA NOT NULL,
            execution_time BIGINT NOT NULL
        )
        "#
    )
    .execute(pool)
    .await?;

    let migrations = vec![
        (1, "Create users table", include_str!("migrations/001_users.sql")),
        (2, "Create OAuth accounts table", include_str!("migrations/002_oauth_accounts.sql")),
        (3, "Create invite codes table", include_str!("migrations/003_invite_codes.sql")),
        (4, "Create sketches table", include_str!("migrations/004_sketches.sql")),
        (5, "Create routes table", include_str!("migrations/005_routes.sql")),
        (6, "Create shares table", include_str!("migrations/006_shares.sql")),
        (7, "Create public links table", include_str!("migrations/007_public_links.sql")),
        (8, "Create sync log table", include_str!("migrations/008_sync_log.sql")),
        (9, "Create refresh tokens table", include_str!("migrations/009_refresh_tokens.sql")),
    ];

    for (version, description, sql) in migrations {
        let already_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = $1 AND success = true)"
        )
        .bind(version as i64)
        .fetch_one(pool)
        .await?;

        if already_applied {
            info!("Migration {} ({}) already applied, skipping", version, description);
            continue;
        }

        info!("Applying migration {}: {}", version, description);

        let mut transaction = pool.begin().await?;

        for statement in sql.split(';').filter(|s| !s.trim().is_empty()) {
            if let Err(e) = sqlx::query(statement).execute(&mut *transaction).await {
                transaction.rollback().await?;
                return Err(e.into());
            }
        }

        let checksum = sha2::Sha256::digest(sql.as_bytes());
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES ($1, $2, true, $3, 0)"
        )
        .bind(version as i64)
        .bind(description)
        .bind(checksum.as_slice())
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        info!("Migration {} applied successfully", version);
    }

    info!("All migrations completed");
    Ok(())
}

pub async fn create_database_if_not_exists(database_url: &str) -> AppResult<()> {
    use sqlx::Postgres;

    if Postgres::database_exists(database_url).await? {
        info!("Database already exists");
        return Ok(());
    }

    info!("Creating database...");
    Postgres::create_database(database_url).await?;
    info!("Database created successfully");
    Ok(())
}
