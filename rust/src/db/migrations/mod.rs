//! Database migrations.
//!
//! `m20250101_000001_initial` builds the schema of `database/migrations` as
//! it stands after the last Laravel migration, minus the tables of removed
//! features (Telescope, Pulse, WebAuthn keys, Socialite user tokens). Later
//! schema changes are new migrations in this module.

mod initial;
mod m20250101_000001_initial;

use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20250101_000001_initial::Migration)]
    }
}
