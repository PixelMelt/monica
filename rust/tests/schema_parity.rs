//! The Rust migrations build the same schema as `database/migrations`, then
//! change what `m20261020_000001_usernames` changes. The fixtures are
//! `monica schema-dump` of databases migrated by Laravel (`php artisan
//! migrate`), minus the tables of removed features.

mod common;

use common::TestDb;
use monica::db::blueprint::Engine;
use monica::db::id::Id;
use monica::db::schema_dump::{self, Schema};
use sea_orm::{ConnectionTrait, Statement};

fn fixture(e: Engine) -> Schema {
    let json = match e {
        Engine::Sqlite => include_str!("fixtures/schema/sqlite.json"),
        Engine::MySql => include_str!("fixtures/schema/mysql.json"),
        Engine::Postgres => include_str!("fixtures/schema/postgres.json"),
    };
    serde_json::from_str(json).unwrap()
}

/// `m20261020_000001_usernames`: `users.email` and its unique index become
/// a nullable `username` of the same type, and `email_verified_at` goes.
/// `m20261020_000002_leftovers`: the columns and tables of removed features
/// go.
fn with_usernames(mut schema: Schema) -> Schema {
    let users = schema.tables.get_mut("users").unwrap();
    let mut username = users.columns.remove("email").unwrap();
    username.nullable = true;
    users.columns.insert("username".into(), username);
    users.columns.remove("email_verified_at").unwrap();
    let mut index = users.indexes.remove("users_email_unique").unwrap();
    index.columns = vec!["username".into()];
    users.indexes.insert("users_username_unique".into(), index);
    for (table, columns) in [
        ("accounts", &["storage_limit_in_mb"][..]),
        ("users", &["invitation_code", "invitation_accepted_at", "help_shown"][..]),
        ("user_notification_channels", &["verification_token"][..]),
    ] {
        for column in columns {
            schema.tables.get_mut(table).unwrap().columns.remove(*column).unwrap();
        }
    }
    for table in ["password_resets", "cache", "cache_locks", "jobs", "failed_jobs"] {
        schema.tables.remove(table).unwrap();
    }
    schema
}

#[tokio::test]
async fn migrations_build_the_laravel_schema() {
    let db = TestDb::migrated().await;
    let got = schema_dump::dump(&db.conn).await.unwrap().without(&["seaql_migrations"]);
    let diff = with_usernames(fixture(db.engine)).diff(&got);
    db.drop().await;
    assert!(diff.is_empty(), "{} differences:\n{}", diff.len(), diff.join("\n"));
}

#[tokio::test]
async fn migrations_seed_the_currencies() {
    let db = TestDb::migrated().await;
    let rows = db
        .conn
        .query_all_raw(Statement::from_string(
            db.conn.get_database_backend(),
            "select id, code from currencies order by id",
        ))
        .await
        .unwrap();
    let got: Vec<(Id, String)> =
        rows.iter().map(|r| (r.try_get::<Id>("", "id").unwrap(), r.try_get::<String>("", "code").unwrap())).collect();
    db.drop().await;
    let expected: Vec<String> = serde_json::from_str(include_str!("fixtures/currencies.json")).unwrap();
    assert_eq!(got.len(), expected.len());
    for (i, (id, code)) in got.iter().enumerate() {
        assert_eq!(*id, Id(i as i64 + 1));
        assert_eq!(code, &expected[i]);
    }
}
