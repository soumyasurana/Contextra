use errors::ContextraError;
use sqlx::{PgPool as SqlxPgPool, postgres::PgPoolOptions};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PgPool {
    pool: SqlxPgPool,
}

impl PgPool {
    pub async fn connect(database_url: &str) -> Result<Self, ContextraError> {
        Self::connect_with_limit(database_url, 5).await
    }

    pub async fn connect_with_limit(
        database_url: &str,
        max_connections: u32,
    ) -> Result<Self, ContextraError> {
        if max_connections == 0 {
            return Err(ContextraError::Validation(
                "database max_connections must be positive".into(),
            ));
        }

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await
            .map_err(|e| ContextraError::StorageError(format!("Failed to connect to DB: {}", e)))?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<(), ContextraError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                ContextraError::StorageError(format!("Failed to run migrations: {}", e))
            })?;
        Ok(())
    }

    pub fn inner(&self) -> &SqlxPgPool {
        &self.pool
    }
}
