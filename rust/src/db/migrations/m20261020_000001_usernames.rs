//! Single-user install: users sign in with a username instead of an email.
//! Each user's username starts as their email in lower case (usernames are
//! kept in lower case), then `email` and `email_verified_at` go (SQLite can
//! drop them in place because the email's unique index is a separate
//! index). Email and Telegram notification channels go too, with their
//! schedules and logs: reminders are sent through Pushover.

use sea_orm_migration::prelude::*;

use crate::db::blueprint::Engine;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261020_000001_usernames"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let statements: &[&str] = match Engine::from_backend(manager.get_database_backend()) {
            Engine::Sqlite => &[
                r#"alter table "users" add column "username" varchar"#,
                r#"update "users" set "username" = lower("email")"#,
                r#"create unique index "users_username_unique" on "users" ("username")"#,
                r#"drop index "users_email_unique""#,
                r#"alter table "users" drop column "email""#,
                r#"alter table "users" drop column "email_verified_at""#,
            ],
            Engine::MySql => &[
                "alter table `users` add `username` varchar(255) null",
                "update `users` set `username` = lower(`email`)",
                "alter table `users` add unique `users_username_unique`(`username`)",
                "alter table `users` drop index `users_email_unique`",
                "alter table `users` drop column `email`, drop column `email_verified_at`",
            ],
            Engine::Postgres => &[
                r#"alter table "users" add column "username" varchar(255) null"#,
                r#"update "users" set "username" = lower("email")"#,
                r#"alter table "users" add constraint "users_username_unique" unique ("username")"#,
                r#"alter table "users" drop column "email""#,
                r#"alter table "users" drop column "email_verified_at""#,
            ],
        };
        for sql in statements {
            db.execute_unprepared(sql).await?;
        }
        db.execute_unprepared("delete from user_notification_channels where type in ('email', 'telegram')").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Statement};
    use sea_orm_migration::MigratorTrait;

    use crate::db::migrations::Migrator;
    use crate::testing::TestDb;

    async fn strings(db: &TestDb, sql: &str, column: &str) -> Vec<String> {
        let rows = db.conn.query_all_raw(Statement::from_string(db.conn.get_database_backend(), sql)).await.unwrap();
        rows.iter().map(|r| r.try_get::<String>("", column).unwrap()).collect()
    }

    #[tokio::test]
    async fn usernames_are_the_emails_in_lower_case_and_email_channels_go() {
        let db = TestDb::empty().await;
        Migrator::up(&db.conn, Some(1)).await.unwrap();
        let account = uuid::Uuid::new_v4();
        let user = uuid::Uuid::new_v4();
        for sql in [
            format!("insert into accounts (id, storage_limit_in_mb) values ('{account}', 0)"),
            format!("insert into users (id, account_id, email) values ('{user}', '{account}', 'Ann@Example.com')"),
            format!(
                "insert into user_notification_channels (user_id, type, content, active) values \
                 ('{user}', 'email', 'ann@example.com', true), ('{user}', 'telegram', '42', true), \
                 ('{user}', 'pushover', '{{}}', true)"
            ),
        ] {
            db.conn.execute_unprepared(&sql).await.unwrap();
        }

        Migrator::up(&db.conn, None).await.unwrap();

        assert_eq!(strings(&db, "select username from users", "username").await, ["ann@example.com"]);
        assert_eq!(strings(&db, "select type from user_notification_channels", "type").await, ["pushover"]);
        db.drop().await;
    }
}
