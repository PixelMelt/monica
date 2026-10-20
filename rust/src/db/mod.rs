//! Database connection and schema.

pub mod blueprint;
pub mod id;
pub mod migrations;
pub mod schema_dump;
pub mod timestamp;
pub mod uuid;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

/// Connects to `url` (`sqlite://…`, `mysql://…` or `postgres://…`).
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(url.to_owned());
    opts.sqlx_logging(false);
    let db = Database::connect(opts).await?;
    uuid::set_backend(sea_orm::ConnectionTrait::get_database_backend(&db));
    Ok(db)
}

/// The current time in UTC, to the second: what Laravel writes to
/// `timestamp` columns (Postgres' `timestamp(0)` would round fractions).
pub fn now() -> timestamp::Timestamp {
    use chrono::Timelike;
    timestamp::Timestamp(chrono::Utc::now().naive_utc().with_nanosecond(0).expect("0 is a valid nanosecond"))
}

/// Applies every pending migration.
pub async fn migrate(db: &DatabaseConnection) -> Result<(), DbErr> {
    migrations::Migrator::up(db, None).await
}
