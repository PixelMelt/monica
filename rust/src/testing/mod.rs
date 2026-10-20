//! Test harness shared by unit and integration tests: a fresh database per
//! test on the engine chosen by `MONICA_TEST_DB` (`sqlite`, the default,
//! `mysql` or `postgres`), and model factories.
//!
//! MySQL and Postgres need a server; their admin URLs come from
//! `MONICA_TEST_MYSQL_URL` and `MONICA_TEST_POSTGRES_URL` (defaults match the
//! containers in `rust/README.md`).

pub mod factory;

use crate::db::blueprint::Engine;
use sea_orm::{ConnectionTrait, DatabaseConnection};

pub struct TestDb {
    pub conn: DatabaseConnection,
    pub engine: Engine,
    admin: Option<(DatabaseConnection, String)>,
    file: Option<std::path::PathBuf>,
}

pub fn engine() -> Engine {
    match std::env::var("MONICA_TEST_DB").as_deref() {
        Ok("mysql") => Engine::MySql,
        Ok("postgres") | Ok("pgsql") => Engine::Postgres,
        _ => Engine::Sqlite,
    }
}

impl TestDb {
    /// An empty database.
    pub async fn empty() -> TestDb {
        let engine = engine();
        let name = format!("monica_test_{}", uuid::Uuid::new_v4().simple());
        match engine {
            Engine::Sqlite => {
                let path = std::env::temp_dir().join(format!("{name}.sqlite"));
                let conn = crate::db::connect(&format!("sqlite://{}?mode=rwc", path.display())).await.expect("sqlite");
                TestDb { conn, engine, admin: None, file: Some(path) }
            }
            Engine::MySql | Engine::Postgres => {
                let base = if engine == Engine::MySql {
                    std::env::var("MONICA_TEST_MYSQL_URL")
                        .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:33306".into())
                } else {
                    std::env::var("MONICA_TEST_POSTGRES_URL")
                        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432".into())
                };
                let base = base.trim_end_matches('/').to_owned();
                let admin_url = if engine == Engine::MySql { base.clone() } else { format!("{base}/postgres") };
                let admin = crate::db::connect(&admin_url).await.expect("admin connection");
                admin.execute_unprepared(&format!("create database {name}")).await.expect("create database");
                let conn = crate::db::connect(&format!("{base}/{name}")).await.expect("test database");
                TestDb { conn, engine, admin: Some((admin, name)), file: None }
            }
        }
    }

    /// A migrated database.
    pub async fn migrated() -> TestDb {
        let db = TestDb::empty().await;
        crate::db::migrate(&db.conn).await.expect("migrations");
        db
    }

    /// Drops the database. Call at the end of each test.
    pub async fn drop(self) {
        let _ = self.conn.close().await;
        if let Some(path) = self.file {
            let _ = std::fs::remove_file(path);
        }
        if let Some((admin, name)) = self.admin {
            let sql = if self.engine == Engine::Postgres {
                format!("drop database if exists {name} with (force)")
            } else {
                format!("drop database if exists {name}")
            };
            admin.execute_unprepared(&sql).await.expect("drop database");
        }
    }
}
