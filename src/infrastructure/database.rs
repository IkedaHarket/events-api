use ikeda_core::error::AppError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

/// Configuration options for the PostgreSQL connection pool.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Database connection URL (e.g. Neon connection string).
    pub url: String,
    /// Maximum concurrent connections in the pool.
    pub max_connections: u32,
    /// Minimum idle connections maintained.
    pub min_connections: u32,
    /// Connection acquisition timeout.
    pub acquire_timeout: Duration,
    /// Idle connection lifetime before closing.
    pub idle_timeout: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 20,
            min_connections: 5,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(600),
        }
    }
}

/// Creates and initializes a PostgreSQL connection pool.
#[allow(dead_code)]
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, AppError> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .connect(&config.url)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to connect to database: {e}")))
}

/// Begins a PostgreSQL transaction and securely configures `app.current_tenant`
/// as a local transaction variable for PostgreSQL Row-Level Security (RLS).
///
/// Because `is_local = true` is passed to `set_config`, the variable is discarded
/// automatically when the transaction commits or aborts, ensuring no leakage across pooled connections.
#[allow(dead_code)]
pub async fn begin_tenant_tx(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to begin transaction: {e}")))?;

    sqlx::query("SELECT set_config('app.current_tenant', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::DatabaseError(format!("Failed to set tenant context for RLS: {e}"))
        })?;

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::DatabaseConfig;
    use std::time::Duration;

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.acquire_timeout, Duration::from_secs(5));
        assert_eq!(config.idle_timeout, Duration::from_secs(600));
        assert!(config.url.is_empty());
    }
}
