//! `Id` and `Uuid` round-trip on every engine (see their module docs).

mod common;

use common::TestDb;
use monica::db::id::Id;
use monica::db::uuid::Uuid;
use sea_orm::{ConnectionTrait, Statement, Value};

#[tokio::test]
async fn uuid_and_id_keys_round_trip() {
    let db = TestDb::migrated().await;
    let backend = db.conn.get_database_backend();
    let account = Uuid::new();
    let q = |sql: &str, values: Vec<Value>| Statement::from_sql_and_values(backend, sql, values);
    let sql = |mysql_or_sqlite: &str| {
        if backend == sea_orm::DbBackend::Postgres {
            let mut n = 0;
            mysql_or_sqlite
                .chars()
                .map(|c| {
                    if c == '?' {
                        n += 1;
                        format!("${n}")
                    } else {
                        c.to_string()
                    }
                })
                .collect()
        } else {
            mysql_or_sqlite.to_owned()
        }
    };

    db.conn
        .execute_raw(q(
            &sql("insert into accounts (id, storage_limit_in_mb) values (?, ?)"),
            vec![account.into(), 7.into()],
        ))
        .await
        .unwrap();
    db.conn
        .execute_raw(q(
            &sql("insert into templates (account_id, name) values (?, ?)"),
            vec![account.into(), "Default".into()],
        ))
        .await
        .unwrap();

    let row = db
        .conn
        .query_one_raw(q(&sql("select id, storage_limit_in_mb from accounts where id = ?"), vec![account.into()]))
        .await
        .unwrap()
        .expect("account found by uuid");
    assert_eq!(row.try_get::<Uuid>("", "id").unwrap(), account);

    let row = db
        .conn
        .query_one_raw(q(&sql("select id, account_id from templates where account_id = ?"), vec![account.into()]))
        .await
        .unwrap()
        .expect("template found by account uuid");
    let id = row.try_get::<Id>("", "id").unwrap();
    assert_eq!(row.try_get::<Uuid>("", "account_id").unwrap(), account);

    let row = db
        .conn
        .query_one_raw(q(&sql("select name from templates where id = ?"), vec![id.into()]))
        .await
        .unwrap()
        .expect("template found by id");
    assert_eq!(row.try_get::<String>("", "name").unwrap(), "Default");
    db.drop().await;
}
