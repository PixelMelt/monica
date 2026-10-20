//! Engine-specific introspection of a database schema into one comparable
//! model. Used to check that the Rust migrations build the same schema as the
//! Laravel migrations did (`tests/schema_parity.rs`), and by the
//! `schema-dump` subcommand that produced the reference fixtures.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DbBackend, DbErr, QueryResult, Statement};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub tables: BTreeMap<String, Table>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    pub columns: BTreeMap<String, Column>,
    pub primary_key: Vec<String>,
    pub indexes: BTreeMap<String, Index>,
    pub foreign_keys: BTreeMap<String, ForeignKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub auto_increment: bool,
    /// MySQL only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub collation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub columns: Vec<String>,
    pub unique: bool,
    /// `btree`, `fulltext`, `gin`, ...
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_update: String,
    pub on_delete: String,
}

impl Schema {
    /// Drops the given tables (tooling tables like the migration log).
    pub fn without(mut self, tables: &[&str]) -> Self {
        for t in tables {
            self.tables.remove(*t);
        }
        self
    }

    /// Human-readable differences between `self` (expected) and `other`.
    pub fn diff(&self, other: &Schema) -> Vec<String> {
        let mut out = Vec::new();
        for (name, t) in &self.tables {
            match other.tables.get(name) {
                None => out.push(format!("missing table {name}")),
                Some(o) => diff_table(name, t, o, &mut out),
            }
        }
        for name in other.tables.keys() {
            if !self.tables.contains_key(name) {
                out.push(format!("unexpected table {name}"));
            }
        }
        out
    }
}

fn diff_map<T: PartialEq + std::fmt::Debug>(
    what: &str,
    table: &str,
    a: &BTreeMap<String, T>,
    b: &BTreeMap<String, T>,
    out: &mut Vec<String>,
) {
    for (k, v) in a {
        match b.get(k) {
            None => out.push(format!("{table}: missing {what} {k} ({v:?})")),
            Some(o) if o != v => out.push(format!("{table}: {what} {k}: expected {v:?}, got {o:?}")),
            _ => {}
        }
    }
    for (k, v) in b {
        if !a.contains_key(k) {
            out.push(format!("{table}: unexpected {what} {k} ({v:?})"));
        }
    }
}

fn diff_table(name: &str, a: &Table, b: &Table, out: &mut Vec<String>) {
    diff_map("column", name, &a.columns, &b.columns, out);
    if a.primary_key != b.primary_key {
        out.push(format!("{name}: primary key expected {:?}, got {:?}", a.primary_key, b.primary_key));
    }
    diff_map("index", name, &a.indexes, &b.indexes, out);
    diff_map("foreign key", name, &a.foreign_keys, &b.foreign_keys, out);
}

/// Foreign key rows collected before they become a [`ForeignKey`].
#[derive(Default)]
struct FkParts {
    /// (position, column, referenced column)
    cols: Vec<(i64, String, String)>,
    ref_table: String,
    on_update: String,
    on_delete: String,
}

impl FkParts {
    fn finish(mut self) -> ForeignKey {
        self.cols.sort();
        ForeignKey {
            columns: self.cols.iter().map(|c| c.1.clone()).collect(),
            ref_table: self.ref_table,
            ref_columns: self.cols.iter().map(|c| c.2.clone()).collect(),
            on_update: self.on_update,
            on_delete: self.on_delete,
        }
    }
}

/// Index rows collected before they become an [`Index`].
#[derive(Default)]
struct IndexParts {
    /// (position, column)
    cols: Vec<(i64, String)>,
    unique: bool,
    kind: String,
}

fn s(row: &QueryResult, col: &str) -> Result<String, DbErr> {
    row.try_get::<String>("", col)
}

fn opt(row: &QueryResult, col: &str) -> Result<Option<String>, DbErr> {
    row.try_get::<Option<String>>("", col)
}

fn int(row: &QueryResult, col: &str) -> Result<i64, DbErr> {
    row.try_get::<i64>("", col)
}

async fn rows<C: ConnectionTrait>(db: &C, sql: &str) -> Result<Vec<QueryResult>, DbErr> {
    db.query_all_raw(Statement::from_string(db.get_database_backend(), sql.to_owned())).await
}

/// Reads the schema of the database `db` is connected to.
pub async fn dump<C: ConnectionTrait>(db: &C) -> Result<Schema, DbErr> {
    match db.get_database_backend() {
        DbBackend::Sqlite => dump_sqlite(db).await,
        DbBackend::MySql => dump_mysql(db).await,
        DbBackend::Postgres => dump_postgres(db).await,
        other => Err(DbErr::Custom(format!("unsupported backend {other:?}"))),
    }
}

fn quote_sqlite(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

async fn dump_sqlite<C: ConnectionTrait>(db: &C) -> Result<Schema, DbErr> {
    let mut schema = Schema::default();
    let tables =
        rows(db, "SELECT name, sql FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .await?;
    for t in tables {
        let name = s(&t, "name")?;
        let create_sql = s(&t, "sql")?.to_lowercase();
        let q = quote_sqlite(&name);
        let mut table = Table::default();

        let mut pk: Vec<(i64, String)> = Vec::new();
        for c in rows(db, &format!("SELECT name, type, \"notnull\" AS nn, dflt_value, pk FROM pragma_table_info({q})"))
            .await?
        {
            let col = s(&c, "name")?;
            let data_type = s(&c, "type")?.to_lowercase();
            let pk_pos = int(&c, "pk")?;
            if pk_pos > 0 {
                pk.push((pk_pos, col.clone()));
            }
            let auto_increment = pk_pos > 0
                && data_type == "integer"
                && create_sql.contains(&format!("\"{}\" integer primary key autoincrement", col.to_lowercase()));
            table.columns.insert(
                col,
                Column {
                    data_type,
                    nullable: int(&c, "nn")? == 0 && pk_pos == 0,
                    default: opt(&c, "dflt_value")?,
                    auto_increment,
                    collation: None,
                },
            );
        }
        pk.sort();
        table.primary_key = pk.into_iter().map(|(_, c)| c).collect();

        for i in rows(db, &format!("SELECT name, \"unique\" AS uniq, origin FROM pragma_index_list({q})")).await? {
            if s(&i, "origin")? == "pk" {
                continue;
            }
            let iname = s(&i, "name")?;
            let mut cols: Vec<(i64, String)> = Vec::new();
            for c in rows(db, &format!("SELECT seqno, name FROM pragma_index_info({})", quote_sqlite(&iname))).await? {
                cols.push((int(&c, "seqno")?, s(&c, "name")?));
            }
            cols.sort();
            table.indexes.insert(
                iname,
                Index {
                    columns: cols.into_iter().map(|(_, c)| c).collect(),
                    unique: int(&i, "uniq")? == 1,
                    kind: "btree".into(),
                },
            );
        }

        let mut fks: BTreeMap<i64, FkParts> = BTreeMap::new();
        for f in rows(db, &format!("SELECT id, seq, \"table\" AS ref_table, \"from\" AS col, \"to\" AS ref_col, on_update, on_delete FROM pragma_foreign_key_list({q})")).await? {
            let e = fks.entry(int(&f, "id")?).or_default();
            e.cols.push((int(&f, "seq")?, s(&f, "col")?, opt(&f, "ref_col")?.unwrap_or_default()));
            e.ref_table = s(&f, "ref_table")?;
            e.on_update = s(&f, "on_update")?.to_lowercase();
            e.on_delete = s(&f, "on_delete")?.to_lowercase();
        }
        for parts in fks.into_values() {
            let fk = parts.finish();
            // SQLite foreign keys have no name; key them by their columns.
            table.foreign_keys.insert(fk.columns.join(","), fk);
        }
        schema.tables.insert(name, table);
    }
    Ok(schema)
}

async fn dump_mysql<C: ConnectionTrait>(db: &C) -> Result<Schema, DbErr> {
    let mut schema = Schema::default();
    for t in rows(
        db,
        "SELECT CAST(TABLE_NAME AS CHAR) AS name FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'",
    )
    .await?
    {
        schema.tables.insert(s(&t, "name")?, Table::default());
    }
    for c in rows(
        db,
        "SELECT CAST(TABLE_NAME AS CHAR) AS t, CAST(COLUMN_NAME AS CHAR) AS c, CAST(COLUMN_TYPE AS CHAR) AS ty, \
         CAST(IS_NULLABLE AS CHAR) AS nullable, CAST(COLUMN_DEFAULT AS CHAR) AS dflt, CAST(EXTRA AS CHAR) AS extra, \
         CAST(COLLATION_NAME AS CHAR) AS coll \
         FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE()",
    )
    .await?
    {
        let Some(table) = schema.tables.get_mut(&s(&c, "t")?) else { continue };
        let extra = s(&c, "extra")?.to_lowercase();
        table.columns.insert(
            s(&c, "c")?,
            Column {
                data_type: s(&c, "ty")?.to_lowercase(),
                nullable: s(&c, "nullable")? == "YES",
                default: opt(&c, "dflt")?,
                auto_increment: extra.contains("auto_increment"),
                collation: opt(&c, "coll")?,
            },
        );
    }
    let mut idx: BTreeMap<(String, String), IndexParts> = BTreeMap::new();
    for i in rows(
        db,
        "SELECT CAST(TABLE_NAME AS CHAR) AS t, CAST(INDEX_NAME AS CHAR) AS i, CAST(NON_UNIQUE AS SIGNED) AS non_unique, \
         CAST(SEQ_IN_INDEX AS SIGNED) AS seq, CAST(COLUMN_NAME AS CHAR) AS c, CAST(INDEX_TYPE AS CHAR) AS kind \
         FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = DATABASE()",
    )
    .await?
    {
        let e = idx.entry((s(&i, "t")?, s(&i, "i")?)).or_default();
        e.cols.push((int(&i, "seq")?, s(&i, "c")?));
        e.unique = int(&i, "non_unique")? == 0;
        e.kind = s(&i, "kind")?.to_lowercase();
    }
    for ((t, i), mut parts) in idx {
        let Some(table) = schema.tables.get_mut(&t) else {
            continue;
        };
        parts.cols.sort();
        let columns: Vec<String> = parts.cols.into_iter().map(|(_, c)| c).collect();
        if i == "PRIMARY" {
            table.primary_key = columns;
        } else {
            table.indexes.insert(i, Index { columns, unique: parts.unique, kind: parts.kind });
        }
    }
    let mut fks: BTreeMap<(String, String), FkParts> = BTreeMap::new();
    for f in rows(
        db,
        "SELECT CAST(k.TABLE_NAME AS CHAR) AS t, CAST(k.CONSTRAINT_NAME AS CHAR) AS n, CAST(k.ORDINAL_POSITION AS SIGNED) AS pos, \
         CAST(k.COLUMN_NAME AS CHAR) AS c, CAST(k.REFERENCED_TABLE_NAME AS CHAR) AS rt, \
         CAST(k.REFERENCED_COLUMN_NAME AS CHAR) AS rc, CAST(r.UPDATE_RULE AS CHAR) AS upd, CAST(r.DELETE_RULE AS CHAR) AS del \
         FROM information_schema.KEY_COLUMN_USAGE k \
         JOIN information_schema.REFERENTIAL_CONSTRAINTS r \
           ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
         WHERE k.TABLE_SCHEMA = DATABASE() AND k.REFERENCED_TABLE_NAME IS NOT NULL",
    )
    .await?
    {
        let e = fks.entry((s(&f, "t")?, s(&f, "n")?)).or_default();
        e.cols.push((int(&f, "pos")?, s(&f, "c")?, s(&f, "rc")?));
        e.ref_table = s(&f, "rt")?;
        e.on_update = s(&f, "upd")?.to_lowercase();
        e.on_delete = s(&f, "del")?.to_lowercase();
    }
    for ((t, n), parts) in fks {
        let Some(table) = schema.tables.get_mut(&t) else {
            continue;
        };
        table.foreign_keys.insert(n, parts.finish());
    }
    Ok(schema)
}

async fn dump_postgres<C: ConnectionTrait>(db: &C) -> Result<Schema, DbErr> {
    let mut schema = Schema::default();
    for t in rows(
        db,
        "SELECT c.relname::text AS name FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'public' AND c.relkind = 'r'",
    )
    .await?
    {
        schema.tables.insert(s(&t, "name")?, Table::default());
    }
    for c in rows(
        db,
        "SELECT c.relname::text AS t, a.attname::text AS col, format_type(a.atttypid, a.atttypmod) AS ty, \
         a.attnotnull AS notnull, pg_get_expr(d.adbin, d.adrelid) AS dflt, a.attidentity::text AS ident \
         FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
         WHERE n.nspname = 'public' AND c.relkind = 'r' AND a.attnum > 0 AND NOT a.attisdropped",
    )
    .await?
    {
        let Some(table) = schema.tables.get_mut(&s(&c, "t")?) else { continue };
        let default = opt(&c, "dflt")?;
        let auto_increment =
            default.as_deref().is_some_and(|d| d.starts_with("nextval(")) || !s(&c, "ident")?.is_empty();
        table.columns.insert(
            s(&c, "col")?,
            Column {
                data_type: s(&c, "ty")?,
                nullable: !c.try_get::<bool>("", "notnull")?,
                default,
                auto_increment,
                collation: None,
            },
        );
    }
    for i in rows(
        db,
        "SELECT t.relname::text AS t, i.relname::text AS i, ix.indisprimary AS is_primary, ix.indisunique AS is_unique, \
         am.amname::text AS kind, \
         ARRAY(SELECT pg_get_indexdef(ix.indexrelid, k + 1, true) FROM generate_subscripts(ix.indkey, 1) AS k ORDER BY k)::text[] AS cols \
         FROM pg_index ix JOIN pg_class t ON t.oid = ix.indrelid JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_am am ON am.oid = i.relam JOIN pg_namespace n ON n.oid = t.relnamespace \
         WHERE n.nspname = 'public'",
    )
    .await?
    {
        let Some(table) = schema.tables.get_mut(&s(&i, "t")?) else { continue };
        let columns: Vec<String> = i.try_get::<Vec<String>>("", "cols")?;
        if i.try_get::<bool>("", "is_primary")? {
            table.primary_key = columns;
        } else {
            table.indexes.insert(
                s(&i, "i")?,
                Index { columns, unique: i.try_get::<bool>("", "is_unique")?, kind: s(&i, "kind")? },
            );
        }
    }
    for f in rows(
        db,
        "SELECT c.conrelid::regclass::text AS t, c.conname::text AS n, \
         ARRAY(SELECT a.attname::text FROM unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord) \
               JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum ORDER BY k.ord)::text[] AS cols, \
         c.confrelid::regclass::text AS rt, \
         ARRAY(SELECT a.attname::text FROM unnest(c.confkey) WITH ORDINALITY AS k(attnum, ord) \
               JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum ORDER BY k.ord)::text[] AS rcols, \
         c.confupdtype::text AS upd, c.confdeltype::text AS del \
         FROM pg_constraint c JOIN pg_namespace n ON n.oid = c.connamespace \
         WHERE c.contype = 'f' AND n.nspname = 'public'",
    )
    .await?
    {
        let Some(table) = schema.tables.get_mut(&s(&f, "t")?) else { continue };
        let action = |code: String| {
            match code.as_str() {
                "a" => "no action",
                "r" => "restrict",
                "c" => "cascade",
                "n" => "set null",
                "d" => "set default",
                _ => "?",
            }
            .to_owned()
        };
        table.foreign_keys.insert(
            s(&f, "n")?,
            ForeignKey {
                columns: f.try_get::<Vec<String>>("", "cols")?,
                ref_table: s(&f, "rt")?,
                ref_columns: f.try_get::<Vec<String>>("", "rcols")?,
                on_update: action(s(&f, "upd")?),
                on_delete: action(s(&f, "del")?),
            },
        );
    }
    Ok(schema)
}
