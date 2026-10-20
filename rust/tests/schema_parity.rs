//! The Rust migrations build the same schema as `database/migrations`.
//! The fixtures are `monica schema-dump` of databases migrated by Laravel
//! (`php artisan migrate`), minus the tables of removed features.

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

#[tokio::test]
async fn migrations_build_the_laravel_schema() {
    let db = TestDb::migrated().await;
    let got = schema_dump::dump(&db.conn).await.unwrap().without(&["seaql_migrations"]);
    let diff = fixture(db.engine).diff(&got);
    db.drop().await;
    assert!(diff.is_empty(), "{} differences:\n{}", diff.len(), diff.join("\n"));
}

#[tokio::test]
async fn migrations_seed_the_currencies() {
    let db = TestDb::migrated().await;
    let rows = db
        .conn
        .query_all_raw(Statement::from_string(db.conn.get_database_backend(), "select id, code from currencies order by id"))
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
