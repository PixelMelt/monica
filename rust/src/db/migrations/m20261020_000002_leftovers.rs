//! What the single-user install no longer uses: the storage quota
//! (`accounts.storage_limit_in_mb`), invitations (`users.invitation_code`,
//! `invitation_accepted_at`), the help links (`users.help_shown`), email
//! channel verification (`user_notification_channels.verification_token`),
//! and Laravel's password reset, cache and queue tables. Accounts are
//! managed by instance administrators: an install without one makes its
//! oldest account administrator the instance administrator.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

use crate::db::blueprint::Engine;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261020_000002_leftovers"
    }
}

/// Columns that go, by table.
const COLUMNS: &[(&str, &[&str])] = &[
    ("accounts", &["storage_limit_in_mb"]),
    ("users", &["invitation_code", "invitation_accepted_at", "help_shown"]),
    ("user_notification_channels", &["verification_token"]),
];

/// Tables that go.
const TABLES: &[&str] = &["password_resets", "cache", "cache_locks", "jobs", "failed_jobs"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let e = Engine::from_backend(manager.get_database_backend());
        let q = |name: &str| if e == Engine::MySql { format!("`{name}`") } else { format!("\"{name}\"") };
        for (table, columns) in COLUMNS {
            for column in *columns {
                db.execute_unprepared(&format!("alter table {} drop column {}", q(table), q(column))).await?;
            }
        }
        for table in TABLES {
            db.execute_unprepared(&format!("drop table {}", q(table))).await?;
        }

        let backend = manager.get_database_backend();
        let admins = db
            .query_one_raw(Statement::from_string(
                backend,
                "select count(*) as n from users where is_instance_administrator = true",
            ))
            .await?
            .map(|r| r.try_get::<i64>("", "n"))
            .transpose()?
            .unwrap_or(0);
        if admins == 0 {
            let oldest = db
                .query_one_raw(Statement::from_string(
                    backend,
                    "select id from users where is_account_administrator = true \
                     order by case when created_at is null then 1 else 0 end, created_at, id",
                ))
                .await?;
            if let Some(row) = oldest {
                let id: crate::db::uuid::Uuid = row.try_get("", "id")?;
                let sql = format!("update users set is_instance_administrator = true where id = '{id}'");
                db.execute_unprepared(&sql).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Statement};
    use sea_orm_migration::MigratorTrait;

    use crate::db::migrations::Migrator;
    use crate::testing::TestDb;

    #[tokio::test]
    async fn the_oldest_account_administrator_becomes_instance_administrator() {
        let db = TestDb::empty().await;
        Migrator::up(&db.conn, Some(2)).await.unwrap();
        let account = uuid::Uuid::new_v4();
        let (member, old_admin, new_admin) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        for sql in [
            format!("insert into accounts (id) values ('{account}')"),
            format!(
                "insert into users (id, account_id, username, is_account_administrator, created_at) values \
                 ('{member}', '{account}', 'member', false, '2020-01-01 00:00:00'), \
                 ('{new_admin}', '{account}', 'new', true, '2022-01-01 00:00:00'), \
                 ('{old_admin}', '{account}', 'old', true, '2021-01-01 00:00:00')"
            ),
        ] {
            db.conn.execute_unprepared(&sql).await.unwrap();
        }

        Migrator::up(&db.conn, None).await.unwrap();

        let rows = db
            .conn
            .query_all_raw(Statement::from_string(
                db.conn.get_database_backend(),
                "select username from users where is_instance_administrator = true",
            ))
            .await
            .unwrap();
        let admins: Vec<String> = rows.iter().map(|r| r.try_get("", "username").unwrap()).collect();
        assert_eq!(admins, ["old"]);
        db.drop().await;
    }
}
