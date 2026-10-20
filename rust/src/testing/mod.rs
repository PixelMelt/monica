//! Test harness shared by unit and integration tests: an empty migrated
//! database per test on the engine chosen by `MONICA_TEST_DB` (`sqlite`,
//! the default, `mysql` or `postgres`), and model factories.
//!
//! MySQL and Postgres need a server; their admin URLs come from
//! `MONICA_TEST_MYSQL_URL` and `MONICA_TEST_POSTGRES_URL` (defaults match the
//! containers in `rust/README.md`).

pub mod factory;
pub mod pushover;

use crate::db::blueprint::Engine;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};

pub struct TestDb {
    pub conn: DatabaseConnection,
    pub engine: Engine,
    /// MySQL/Postgres: the database name, returned to the pool on drop.
    name: Option<String>,
    file: Option<std::path::PathBuf>,
}

pub fn engine() -> Engine {
    match std::env::var("MONICA_TEST_DB").as_deref() {
        Ok("mysql") => Engine::MySql,
        Ok("postgres") | Ok("pgsql") => Engine::Postgres,
        _ => Engine::Sqlite,
    }
}

fn server_url(engine: Engine) -> String {
    let base = if engine == Engine::MySql {
        std::env::var("MONICA_TEST_MYSQL_URL").unwrap_or_else(|_| "mysql://root:root@127.0.0.1:33306".into())
    } else {
        std::env::var("MONICA_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432".into())
    };
    base.trim_end_matches('/').to_owned()
}

async fn admin(engine: Engine) -> DatabaseConnection {
    let base = server_url(engine);
    let url = if engine == Engine::MySql { base } else { format!("{base}/postgres") };
    crate::db::connect(&url).await.expect("admin connection")
}

/// Migrated MySQL/Postgres databases of this process that a finished test
/// emptied, ready for the next test. Migrating takes seconds on those
/// engines; emptying takes milliseconds. Names, not connections: each
/// `#[tokio::test]` has its own runtime, and a pool can't outlive its runtime.
static POOL: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Drops `monica_test_<pid>_*` databases left by processes that are gone
/// (a test that panics never returns its database).
async fn drop_stale(engine: Engine) {
    static DONE: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    DONE.get_or_init(|| async {
        let admin = admin(engine).await;
        let sql = if engine == Engine::MySql {
            "SELECT CAST(SCHEMA_NAME AS CHAR) AS name FROM information_schema.SCHEMATA WHERE SCHEMA_NAME LIKE 'monica_test_%'"
        } else {
            "SELECT datname::text AS name FROM pg_database WHERE datname LIKE 'monica_test_%'"
        };
        let rows = admin.query_all_raw(Statement::from_string(admin.get_database_backend(), sql)).await.unwrap_or_default();
        for row in rows {
            let Ok(name) = row.try_get::<String>("", "name") else { continue };
            let pid = name.strip_prefix("monica_test_").and_then(|r| r.split('_').next()).and_then(|p| p.parse::<u32>().ok());
            let alive = pid.is_some_and(|p| std::path::Path::new(&format!("/proc/{p}")).exists());
            if !alive {
                let _ = admin.execute_unprepared(&drop_sql(engine, &name)).await;
            }
        }
    })
    .await;
}

fn drop_sql(engine: Engine, name: &str) -> String {
    if engine == Engine::Postgres {
        format!("drop database if exists {name} with (force)")
    } else {
        format!("drop database if exists {name}")
    }
}

impl TestDb {
    /// A new database with no migration applied (tests of a migration, which
    /// then run the rest so the database can be pooled).
    pub async fn empty() -> TestDb {
        let engine = engine();
        if engine == Engine::Sqlite {
            let path = std::env::temp_dir().join(format!("monica_test_{}.sqlite", uuid::Uuid::new_v4().simple()));
            let conn = crate::db::connect(&format!("sqlite://{}?mode=rwc", path.display())).await.expect("sqlite");
            return TestDb { conn, engine, name: None, file: Some(path) };
        }
        drop_stale(engine).await;
        let name = format!("monica_test_{}_{}", std::process::id(), uuid::Uuid::new_v4().simple());
        admin(engine).await.execute_unprepared(&format!("create database {name}")).await.expect("create database");
        let conn = crate::db::connect(&format!("{}/{name}", server_url(engine))).await.expect("test database");
        TestDb { conn, engine, name: Some(name), file: None }
    }

    /// A migrated database, recycled from an earlier test when possible.
    /// At most four migrate at once, so a parallel test run doesn't swamp
    /// the MySQL or Postgres server with DDL.
    pub async fn migrated() -> TestDb {
        let engine = engine();
        if engine == Engine::Sqlite {
            let path = std::env::temp_dir().join(format!("monica_test_{}.sqlite", uuid::Uuid::new_v4().simple()));
            let conn = crate::db::connect(&format!("sqlite://{}?mode=rwc", path.display())).await.expect("sqlite");
            crate::db::migrate(&conn).await.expect("migrations");
            return TestDb { conn, engine, name: None, file: Some(path) };
        }
        drop_stale(engine).await;
        let base = server_url(engine);
        let pooled = POOL.lock().unwrap().pop();
        if let Some(name) = pooled {
            let conn = crate::db::connect(&format!("{base}/{name}")).await.expect("test database");
            return TestDb { conn, engine, name: Some(name), file: None };
        }
        static MIGRATING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
        let _permit = MIGRATING.acquire().await.expect("semaphore");
        let name = format!("monica_test_{}_{}", std::process::id(), uuid::Uuid::new_v4().simple());
        admin(engine).await.execute_unprepared(&format!("create database {name}")).await.expect("create database");
        let conn = crate::db::connect(&format!("{base}/{name}")).await.expect("test database");
        crate::db::migrate(&conn).await.expect("migrations");
        TestDb { conn, engine, name: Some(name), file: None }
    }

    /// Ends the test's use of the database: SQLite files are deleted;
    /// MySQL/Postgres databases are emptied, reseeded and pooled. Call at
    /// the end of each test.
    pub async fn drop(self) {
        if let Some(path) = &self.file {
            let _ = self.conn.close().await;
            let _ = std::fs::remove_file(path);
            return;
        }
        let Some(name) = self.name.clone() else { return };
        match self.reset().await {
            Ok(()) => {
                let _ = self.conn.close().await;
                POOL.lock().unwrap().push(name);
            }
            Err(_) => {
                let _ = self.conn.close().await;
                let _ = admin(self.engine).await.execute_unprepared(&drop_sql(self.engine, &name)).await;
            }
        }
    }

    /// Deletes every row and puts back what the migrations seed.
    async fn reset(&self) -> Result<(), sea_orm::DbErr> {
        let backend = self.conn.get_database_backend();
        let sql = if self.engine == Engine::MySql {
            "SELECT CAST(TABLE_NAME AS CHAR) AS name FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'"
        } else {
            "SELECT tablename::text AS name FROM pg_tables WHERE schemaname = 'public'"
        };
        let tables: Vec<String> = self
            .conn
            .query_all_raw(Statement::from_string(backend, sql))
            .await?
            .iter()
            .filter_map(|r| r.try_get::<String>("", "name").ok())
            .filter(|t| t != "seaql_migrations")
            .collect();
        let tx = self.conn.begin().await?;
        if self.engine == Engine::MySql {
            tx.execute_unprepared("SET FOREIGN_KEY_CHECKS = 0").await?;
            for t in &tables {
                tx.execute_unprepared(&format!("DELETE FROM `{t}`")).await?;
            }
            tx.execute_unprepared("ALTER TABLE `currencies` AUTO_INCREMENT = 1").await?;
            tx.execute_unprepared("SET FOREIGN_KEY_CHECKS = 1").await?;
        } else {
            let list = tables.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ");
            tx.execute_unprepared(&format!("TRUNCATE {list} RESTART IDENTITY CASCADE")).await?;
        }
        crate::db::migrations::seed(&tx, self.engine).await?;
        tx.commit().await
    }
}
