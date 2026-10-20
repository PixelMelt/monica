//! Shared state of the HTTP handlers.

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use super::ratelimit::RateLimiter;
use crate::config::Config;
use crate::mail::Mailer;
use crate::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub mailer: Mailer,
    pub limiter: RateLimiter,
    pub storage: Storage,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config) -> anyhow::Result<AppState> {
        let mailer = Mailer::new(&config)?;
        let storage = Storage::new(&config.data_dir);
        Ok(AppState { db, config: Arc::new(config), mailer, limiter: RateLimiter::default(), storage })
    }
}
