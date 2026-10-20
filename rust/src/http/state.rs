//! Shared state of the HTTP handlers.

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use super::ratelimit::RateLimiter;
use crate::config::Config;
use crate::mail::Mailer;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub mailer: Mailer,
    pub limiter: RateLimiter,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config) -> anyhow::Result<AppState> {
        let mailer = Mailer::new(&config)?;
        Ok(AppState { db, config: Arc::new(config), mailer, limiter: RateLimiter::default() })
    }
}
