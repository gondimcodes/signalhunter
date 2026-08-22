use crate::config::DatabaseConfig;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

pub mod queries;

pub type DbPool = MySqlPool;

pub async fn create_pool(cfg: &DatabaseConfig) -> Result<DbPool, sqlx::Error> {
    let url = cfg.connection_url();

    MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(Duration::from_secs(cfg.connect_timeout_sec))
        .idle_timeout(Duration::from_secs(cfg.idle_timeout_sec))
        .connect(&url)
        .await
}
