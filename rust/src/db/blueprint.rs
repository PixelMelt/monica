//! A port of the subset of Laravel's schema `Blueprint` and its SQLite, MySQL
//! and Postgres grammars that `database/migrations` uses, so the Rust
//! migrations read like the PHP ones and build the same schema on every
//! engine (same native types, defaults, index and constraint names).
//!
//! Tables are rendered in order; foreign key constraints are collected and
//! added after every table exists (MySQL and Postgres), or inlined in the
//! `create table` (SQLite, as Laravel does).

use sea_orm::DbBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Sqlite,
    MySql,
    Postgres,
}

impl Engine {
    pub fn from_backend(b: DbBackend) -> Self {
        match b {
            DbBackend::MySql => Engine::MySql,
            DbBackend::Postgres => Engine::Postgres,
            _ => Engine::Sqlite,
        }
    }

    /// `ScoutHelper::isFullTextIndex()` with `FULL_TEXT_INDEX=true`.
    pub fn full_text(self) -> bool {
        matches!(self, Engine::MySql | Engine::Postgres)
    }

    fn wrap(self, name: &str) -> String {
        match self {
            Engine::MySql => format!("`{name}`"),
            _ => format!("\"{name}\""),
        }
    }

    fn columnize(self, cols: &[String]) -> String {
        cols.iter()
            .map(|c| self.wrap(c))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    String(u32),
    Char(u32),
    Text,
    MediumText,
    LongText,
    Integer,
    BigInteger,
    SmallInteger,
    TinyInteger,
    MediumInteger,
    Boolean,
    DateTime,
    Timestamp,
    Date,
    Time,
    Uuid,
    Double,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Default {
    Str(String),
    Bool(bool),
    Int(i64),
    CurrentTimestamp,
}

#[derive(Debug, Clone)]
pub struct Column {
    name: String,
    kind: Kind,
    nullable: bool,
    default: Option<Default>,
    unsigned: bool,
    auto_increment: bool,
    fluent: Option<IndexKind>,
}

impl Column {
    pub fn nullable(&mut self) -> &mut Self {
        self.nullable = true;
        self
    }

    pub fn default(&mut self, value: impl Into<Default>) -> &mut Self {
        self.default = Some(value.into());
        self
    }

    pub fn use_current(&mut self) -> &mut Self {
        self.default = Some(Default::CurrentTimestamp);
        self
    }

    /// `->unique()` on the column: added after the table's other commands.
    pub fn unique(&mut self) -> &mut Self {
        self.fluent = Some(IndexKind::Unique);
        self
    }

    /// `->index()` on the column: added after the table's other commands.
    pub fn index(&mut self) -> &mut Self {
        self.fluent = Some(IndexKind::Index);
        self
    }

    fn sql(&self, e: Engine, has_primary_command: bool) -> String {
        let serial = self.auto_increment
            && matches!(
                self.kind,
                Kind::BigInteger
                    | Kind::Integer
                    | Kind::SmallInteger
                    | Kind::TinyInteger
                    | Kind::MediumInteger
            );
        let ty = match (e, self.kind) {
            (Engine::Sqlite, Kind::String(_) | Kind::Char(_) | Kind::Uuid) => "varchar".into(),
            (Engine::Sqlite, Kind::Text | Kind::MediumText | Kind::LongText) => "text".into(),
            (
                Engine::Sqlite,
                Kind::Integer
                | Kind::BigInteger
                | Kind::SmallInteger
                | Kind::TinyInteger
                | Kind::MediumInteger,
            ) => "integer".into(),
            (Engine::Sqlite, Kind::Boolean) => "tinyint(1)".into(),
            (Engine::Sqlite, Kind::DateTime | Kind::Timestamp) => "datetime".into(),
            (Engine::Sqlite, Kind::Date) => "date".into(),
            (Engine::Sqlite, Kind::Time) => "time".into(),
            (Engine::Sqlite, Kind::Double) => "double".into(),

            (Engine::MySql, Kind::String(n)) => format!("varchar({n})"),
            (Engine::MySql, Kind::Char(n)) => format!("char({n})"),
            (Engine::MySql, Kind::Text) => "text".into(),
            (Engine::MySql, Kind::MediumText) => "mediumtext".into(),
            (Engine::MySql, Kind::LongText) => "longtext".into(),
            (Engine::MySql, Kind::Integer) => "int".into(),
            (Engine::MySql, Kind::BigInteger) => "bigint".into(),
            (Engine::MySql, Kind::SmallInteger) => "smallint".into(),
            (Engine::MySql, Kind::TinyInteger) => "tinyint".into(),
            (Engine::MySql, Kind::MediumInteger) => "mediumint".into(),
            (Engine::MySql, Kind::Boolean) => "tinyint(1)".into(),
            (Engine::MySql, Kind::DateTime) => "datetime".into(),
            (Engine::MySql, Kind::Timestamp) => "timestamp".into(),
            (Engine::MySql, Kind::Date) => "date".into(),
            (Engine::MySql, Kind::Time) => "time".into(),
            (Engine::MySql, Kind::Uuid) => "char(36)".into(),
            (Engine::MySql, Kind::Double) => "double".into(),

            (Engine::Postgres, Kind::String(n)) => format!("varchar({n})"),
            (Engine::Postgres, Kind::Char(n)) => format!("char({n})"),
            (Engine::Postgres, Kind::Text | Kind::MediumText | Kind::LongText) => "text".into(),
            (Engine::Postgres, Kind::Integer | Kind::MediumInteger) => {
                if serial { "serial" } else { "integer" }.into()
            }
            (Engine::Postgres, Kind::BigInteger) => {
                if serial { "bigserial" } else { "bigint" }.into()
            }
            (Engine::Postgres, Kind::SmallInteger | Kind::TinyInteger) => {
                if serial { "smallserial" } else { "smallint" }.into()
            }
            (Engine::Postgres, Kind::Boolean) => "boolean".into(),
            (Engine::Postgres, Kind::DateTime | Kind::Timestamp) => {
                "timestamp(0) without time zone".into()
            }
            (Engine::Postgres, Kind::Date) => "date".into(),
            (Engine::Postgres, Kind::Time) => "time(0) without time zone".into(),
            (Engine::Postgres, Kind::Uuid) => "uuid".into(),
            (Engine::Postgres, Kind::Double) => "double precision".into(),
        };
        let mut sql = format!("{} {ty}", e.wrap(&self.name));
        let default = self.default.as_ref().map(|d| match d {
            Default::CurrentTimestamp => " default CURRENT_TIMESTAMP".to_owned(),
            Default::Bool(b) => format!(" default '{}'", *b as i32),
            Default::Int(i) => format!(" default '{i}'"),
            Default::Str(s) => format!(" default '{}'", s.replace('\'', "''")),
        });
        match e {
            Engine::Sqlite => {
                if serial {
                    sql.push_str(" primary key autoincrement");
                }
                if !self.nullable {
                    sql.push_str(" not null");
                }
                sql.push_str(&default.unwrap_or_default());
            }
            Engine::MySql => {
                if self.unsigned {
                    sql.push_str(" unsigned");
                }
                sql.push_str(if self.nullable { " null" } else { " not null" });
                sql.push_str(&default.unwrap_or_default());
                if serial {
                    sql.push_str(if has_primary_command {
                        " auto_increment"
                    } else {
                        " auto_increment primary key"
                    });
                }
            }
            Engine::Postgres => {
                if serial && !has_primary_command {
                    sql.push_str(" primary key");
                }
                sql.push_str(if self.nullable { " null" } else { " not null" });
                sql.push_str(&default.unwrap_or_default());
            }
        }
        sql
    }
}

impl From<&str> for Default {
    fn from(s: &str) -> Self {
        Default::Str(s.to_owned())
    }
}
impl From<bool> for Default {
    fn from(b: bool) -> Self {
        Default::Bool(b)
    }
}
impl From<i64> for Default {
    fn from(i: i64) -> Self {
        Default::Int(i)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    Index,
    Unique,
    FullText,
}

#[derive(Debug, Clone)]
pub struct Foreign {
    name: String,
    table: String,
    columns: Vec<String>,
    on: String,
    references: Vec<String>,
    on_delete: Option<&'static str>,
    on_update: Option<&'static str>,
}

impl Foreign {
    pub fn references(&mut self, cols: &[&str]) -> &mut Self {
        self.references = cols.iter().map(|c| c.to_string()).collect();
        self
    }

    pub fn on(&mut self, table: &str) -> &mut Self {
        self.on = table.to_owned();
        self
    }

    pub fn cascade_on_delete(&mut self) -> &mut Self {
        self.on_delete = Some("cascade");
        self
    }

    pub fn null_on_delete(&mut self) -> &mut Self {
        self.on_delete = Some("set null");
        self
    }

    pub fn on_delete(&mut self, action: &'static str) -> &mut Self {
        self.on_delete = Some(action);
        self
    }

    fn actions(&self) -> String {
        let mut sql = String::new();
        if let Some(a) = self.on_delete {
            sql.push_str(&format!(" on delete {a}"));
        }
        if let Some(a) = self.on_update {
            sql.push_str(&format!(" on update {a}"));
        }
        sql
    }

    fn clause(&self, e: Engine) -> String {
        format!(
            "foreign key ({}) references {} ({}){}",
            e.columnize(&self.columns),
            e.wrap(&self.on),
            e.columnize(&self.references),
            self.actions()
        )
    }
}

#[derive(Debug)]
enum Command {
    Index {
        name: String,
        kind: IndexKind,
        columns: Vec<String>,
    },
    Foreign(usize),
}

/// The key type of the model a `foreignIdFor` points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Int,
    Uuid,
}

#[derive(Debug)]
pub struct Blueprint {
    table: String,
    columns: Vec<Column>,
    primary: Option<Vec<String>>,
    commands: Vec<Command>,
    foreigns: Vec<Foreign>,
}

impl Blueprint {
    fn new(table: &str) -> Self {
        Blueprint {
            table: table.to_owned(),
            columns: Vec::new(),
            primary: None,
            commands: Vec::new(),
            foreigns: Vec::new(),
        }
    }

    fn add(&mut self, name: &str, kind: Kind) -> &mut Column {
        self.columns.push(Column {
            name: name.to_owned(),
            kind,
            nullable: false,
            default: None,
            unsigned: false,
            auto_increment: false,
            fluent: None,
        });
        self.columns.last_mut().unwrap()
    }

    fn index_name(&self, kind: &str, cols: &[&str]) -> String {
        format!("{}_{}_{}", self.table, cols.join("_"), kind)
            .to_lowercase()
            .replace(['-', '.'], "_")
    }

    // ------------------------------------------------------------- columns

    /// `$table->id()`: unsigned big integer auto-increment primary key.
    pub fn id(&mut self) -> &mut Column {
        self.big_increments("id")
    }

    pub fn big_increments(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::BigInteger);
        c.unsigned = true;
        c.auto_increment = true;
        c
    }

    pub fn increments(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::Integer);
        c.unsigned = true;
        c.auto_increment = true;
        c
    }

    pub fn string(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::String(255))
    }

    pub fn string_len(&mut self, name: &str, len: u32) -> &mut Column {
        self.add(name, Kind::String(len))
    }

    pub fn char(&mut self, name: &str, len: u32) -> &mut Column {
        self.add(name, Kind::Char(len))
    }

    pub fn text(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Text)
    }

    pub fn medium_text(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::MediumText)
    }

    pub fn long_text(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::LongText)
    }

    pub fn integer(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Integer)
    }

    pub fn unsigned_integer(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::Integer);
        c.unsigned = true;
        c
    }

    pub fn big_integer(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::BigInteger)
    }

    pub fn unsigned_big_integer(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::BigInteger);
        c.unsigned = true;
        c
    }

    pub fn small_integer(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::SmallInteger)
    }

    pub fn unsigned_tiny_integer(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::TinyInteger);
        c.unsigned = true;
        c
    }

    pub fn unsigned_medium_integer(&mut self, name: &str) -> &mut Column {
        let c = self.add(name, Kind::MediumInteger);
        c.unsigned = true;
        c
    }

    pub fn boolean(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Boolean)
    }

    pub fn datetime(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::DateTime)
    }

    pub fn timestamp(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Timestamp)
    }

    pub fn date(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Date)
    }

    pub fn time(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Time)
    }

    pub fn uuid(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Uuid)
    }

    pub fn double(&mut self, name: &str) -> &mut Column {
        self.add(name, Kind::Double)
    }

    /// `$table->timestamps()`: nullable `created_at` and `updated_at`.
    pub fn timestamps(&mut self) {
        self.timestamp("created_at").nullable();
        self.timestamp("updated_at").nullable();
    }

    /// `$table->softDeletes()`: nullable `deleted_at`.
    pub fn soft_deletes(&mut self) {
        self.timestamp("deleted_at").nullable();
    }

    /// `$table->rememberToken()`.
    pub fn remember_token(&mut self) {
        self.string_len("remember_token", 100).nullable();
    }

    /// `$table->morphs($name)`. `AppServiceProvider` sets
    /// `Builder::defaultMorphKeyType('uuid')`, so the id column is a uuid.
    pub fn morphs(&mut self, name: &str) {
        self.string(&format!("{name}_type"));
        self.uuid(&format!("{name}_id"));
        self.index(&[&format!("{name}_type"), &format!("{name}_id")]);
    }

    /// `$table->nullableNumericMorphs($name)`.
    pub fn nullable_numeric_morphs(&mut self, name: &str) {
        self.string(&format!("{name}_type")).nullable();
        self.unsigned_big_integer(&format!("{name}_id")).nullable();
        self.index(&[&format!("{name}_type"), &format!("{name}_id")]);
    }

    /// `$table->foreignIdFor(Model::class, $column)`: an unsigned big integer
    /// (or a uuid, for models with uuid keys) named `column`.
    pub fn foreign_id_for(&mut self, column: &str, key: Key) -> ForeignId<'_> {
        match key {
            Key::Int => {
                self.unsigned_big_integer(column);
            }
            Key::Uuid => {
                self.uuid(column);
            }
        }
        ForeignId {
            bp: self,
            column: column.to_owned(),
        }
    }

    // ------------------------------------------------------------- keys

    pub fn primary(&mut self, cols: &[&str]) {
        self.primary = Some(cols.iter().map(|c| c.to_string()).collect());
    }

    pub fn index(&mut self, cols: &[&str]) {
        let name = self.index_name("index", cols);
        self.index_named(cols, &name);
    }

    pub fn index_named(&mut self, cols: &[&str], name: &str) {
        self.commands.push(Command::Index {
            name: name.to_owned(),
            kind: IndexKind::Index,
            columns: cols.iter().map(|c| c.to_string()).collect(),
        });
    }

    pub fn unique(&mut self, cols: &[&str]) {
        let name = self.index_name("unique", cols);
        self.unique_named(cols, &name);
    }

    pub fn unique_named(&mut self, cols: &[&str], name: &str) {
        self.commands.push(Command::Index {
            name: name.to_owned(),
            kind: IndexKind::Unique,
            columns: cols.iter().map(|c| c.to_string()).collect(),
        });
    }

    /// `$table->fullText($cols)`. Callers wrap it in `if e.full_text()`, as the
    /// PHP wraps it in `ScoutHelper::isFullTextIndex()`.
    pub fn full_text(&mut self, cols: &[&str]) {
        let name = self.index_name("fulltext", cols);
        self.commands.push(Command::Index {
            name,
            kind: IndexKind::FullText,
            columns: cols.iter().map(|c| c.to_string()).collect(),
        });
    }

    /// `$table->foreign($cols)`; chain `.references(..).on(..)`.
    pub fn foreign(&mut self, cols: &[&str]) -> &mut Foreign {
        let name = self.index_name("foreign", cols);
        self.foreigns.push(Foreign {
            name,
            table: self.table.clone(),
            columns: cols.iter().map(|c| c.to_string()).collect(),
            on: String::new(),
            references: vec!["id".into()],
            on_delete: None,
            on_update: None,
        });
        self.commands
            .push(Command::Foreign(self.foreigns.len() - 1));
        self.foreigns.last_mut().unwrap()
    }
}

/// What `foreignIdFor` returns, to chain `->nullable()->constrained()`.
pub struct ForeignId<'a> {
    bp: &'a mut Blueprint,
    column: String,
}

impl<'a> ForeignId<'a> {
    pub fn nullable(self) -> Self {
        self.bp.columns.last_mut().unwrap().nullable = true;
        self
    }

    /// `->constrained($table)`: foreign key to `table.id`.
    pub fn constrained(self, table: &str) -> &'a mut Foreign {
        let col = self.column.clone();
        let bp = self.bp;
        bp.foreign(&[&col]).on(table)
    }

    /// The column itself, without a constraint.
    pub fn column(self) -> &'a mut Column {
        self.bp.columns.last_mut().unwrap()
    }
}

/// The whole schema: tables in creation order.
#[derive(Debug, Default)]
pub struct Schema {
    tables: Vec<Blueprint>,
}

impl Schema {
    pub fn create(&mut self, table: &str, f: impl FnOnce(&mut Blueprint)) {
        let mut bp = Blueprint::new(table);
        f(&mut bp);
        // Blueprint::addFluentIndexes runs at toSql time, after the commands
        // the callback added.
        let fluent: Vec<(String, IndexKind)> = bp
            .columns
            .iter()
            .filter_map(|c| c.fluent.map(|k| (c.name.clone(), k)))
            .collect();
        for (col, kind) in fluent {
            match kind {
                IndexKind::Unique => bp.unique(&[&col]),
                _ => bp.index(&[&col]),
            }
        }
        self.tables.push(bp);
    }

    /// SQL statements that build the schema on `e`, in order.
    pub fn statements(&self, e: Engine) -> Vec<String> {
        let mut out = Vec::new();
        let mut deferred = Vec::new();
        for bp in &self.tables {
            let has_primary = bp.primary.is_some();
            let mut cols: Vec<String> = bp.columns.iter().map(|c| c.sql(e, has_primary)).collect();
            match e {
                Engine::Sqlite => {
                    let mut sql =
                        format!("create table {} ({}", e.wrap(&bp.table), cols.join(", "));
                    for f in &bp.foreigns {
                        sql.push_str(&format!(
                            ", foreign key({}) references {}({})",
                            e.columnize(&f.columns),
                            e.wrap(&f.on),
                            e.columnize(&f.references)
                        ));
                        sql.push_str(&f.actions());
                    }
                    if let Some(pk) = &bp.primary {
                        sql.push_str(&format!(", primary key ({})", e.columnize(pk)));
                    }
                    sql.push(')');
                    out.push(sql);
                    for c in &bp.commands {
                        if let Command::Index {
                            name,
                            kind,
                            columns,
                        } = c
                        {
                            match kind {
                                IndexKind::Index => out.push(format!(
                                    "create index {} on {} ({})",
                                    e.wrap(name),
                                    e.wrap(&bp.table),
                                    e.columnize(columns)
                                )),
                                IndexKind::Unique => out.push(format!(
                                    "create unique index {} on {} ({})",
                                    e.wrap(name),
                                    e.wrap(&bp.table),
                                    e.columnize(columns)
                                )),
                                IndexKind::FullText => {}
                            }
                        }
                    }
                }
                Engine::MySql => {
                    if let Some(pk) = &bp.primary {
                        cols.push(format!("primary key ({})", e.columnize(pk)));
                    }
                    out.push(format!(
                        "create table {} ({}) default character set utf8mb4 collate 'utf8mb4_unicode_ci'",
                        e.wrap(&bp.table),
                        cols.join(", ")
                    ));
                    // MySQL creates an index named after a foreign key when no
                    // index starts with its columns, and silently drops it
                    // when a later index can serve the key. The PHP adds the
                    // constraints in command order, so replay that here and
                    // create the surviving implicit indexes explicitly.
                    let mut indexes: Vec<(Vec<String>, Option<String>)> = Vec::new();
                    if let Some(pk) = &bp.primary {
                        indexes.push((pk.clone(), None));
                    } else if let Some(c) = bp.columns.iter().find(|c| c.auto_increment) {
                        indexes.push((vec![c.name.clone()], None));
                    }
                    for c in &bp.commands {
                        match c {
                            Command::Index {
                                name,
                                kind,
                                columns,
                            } => {
                                let k = match kind {
                                    IndexKind::Index => "index",
                                    IndexKind::Unique => "unique",
                                    IndexKind::FullText => "fulltext",
                                };
                                out.push(format!(
                                    "alter table {} add {k} {}({})",
                                    e.wrap(&bp.table),
                                    e.wrap(name),
                                    e.columnize(columns)
                                ));
                                if *kind != IndexKind::FullText {
                                    indexes.retain(|(cols, implicit)| {
                                        implicit.is_none() || !columns.starts_with(cols)
                                    });
                                    indexes.push((columns.clone(), None));
                                }
                            }
                            Command::Foreign(i) => {
                                let f = &bp.foreigns[*i];
                                if !indexes.iter().any(|(cols, _)| cols.starts_with(&f.columns)) {
                                    indexes.push((f.columns.clone(), Some(f.name.clone())));
                                }
                                deferred.push(f.clone());
                            }
                        }
                    }
                    for (cols, implicit) in indexes {
                        if let Some(name) = implicit {
                            out.push(format!(
                                "alter table {} add index {}({})",
                                e.wrap(&bp.table),
                                e.wrap(&name),
                                e.columnize(&cols)
                            ));
                        }
                    }
                }
                Engine::Postgres => {
                    out.push(format!(
                        "create table {} ({})",
                        e.wrap(&bp.table),
                        cols.join(", ")
                    ));
                    if let Some(pk) = &bp.primary {
                        out.push(format!(
                            "alter table {} add primary key ({})",
                            e.wrap(&bp.table),
                            e.columnize(pk)
                        ));
                    }
                    for c in &bp.commands {
                        match c {
                            Command::Index {
                                name,
                                kind: IndexKind::Index,
                                columns,
                            } => out.push(format!(
                                "create index {} on {} ({})",
                                e.wrap(name),
                                e.wrap(&bp.table),
                                e.columnize(columns)
                            )),
                            Command::Index {
                                name,
                                kind: IndexKind::Unique,
                                columns,
                            } => out.push(format!(
                                "alter table {} add constraint {} unique ({})",
                                e.wrap(&bp.table),
                                e.wrap(name),
                                e.columnize(columns)
                            )),
                            Command::Index {
                                name,
                                kind: IndexKind::FullText,
                                columns,
                            } => out.push(format!(
                                "create index {} on {} using gin (({}))",
                                e.wrap(name),
                                e.wrap(&bp.table),
                                columns
                                    .iter()
                                    .map(|c| format!("to_tsvector('english', {})", e.wrap(c)))
                                    .collect::<Vec<_>>()
                                    .join(" || ")
                            )),
                            Command::Foreign(i) => deferred.push(bp.foreigns[*i].clone()),
                        }
                    }
                }
            }
        }
        for f in deferred {
            out.push(format!(
                "alter table {} add constraint {} {}",
                e.wrap(&f.table),
                e.wrap(&f.name),
                f.clause(e)
            ));
        }
        out
    }
}
