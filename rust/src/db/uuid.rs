//! `Uuid`: the type of the uuid keys (accounts, users, vaults, contacts,
//! address book subscriptions) and the columns that point at them.
//!
//! Laravel's `uuid()` column is `uuid` on Postgres, `char(36)` on MySQL and
//! `varchar` on SQLite, holding the hyphenated text form. sqlx binds a
//! `uuid::Uuid` as 16 raw bytes on MySQL and SQLite, which never equals the
//! stored text, and Postgres rejects a text parameter for a `uuid` column. So
//! this type binds as text, except on Postgres, and decodes either form. The
//! engine is the one the process connected to last (`db::connect`); a server
//! process talks to a single database.

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};

use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, DbBackend, DbErr, IntoActiveValue, QueryResult, TryFromU64, TryGetError, TryGetable, Value};
use serde::{Deserialize, Serialize};

static POSTGRES: AtomicU8 = AtomicU8::new(0);

/// Records the engine values are bound for. Called by `db::connect`.
pub fn set_backend(backend: DbBackend) {
    POSTGRES.store(u8::from(backend == DbBackend::Postgres), Ordering::Relaxed);
}

fn postgres() -> bool {
    POSTGRES.load(Ordering::Relaxed) == 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    /// A new time-ordered (v7) uuid, like Laravel's `Str::orderedUuid()`.
    pub fn new() -> Self {
        Uuid(uuid::Uuid::now_v7())
    }
}

impl Default for Uuid {
    fn default() -> Self {
        Uuid::new()
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
    }
}

impl FromStr for Uuid {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(Uuid)
    }
}

impl From<Uuid> for Value {
    fn from(u: Uuid) -> Self {
        if postgres() { Value::Uuid(Some(u.0)) } else { Value::String(Some(u.to_string())) }
    }
}

impl TryGetable for Uuid {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        match String::try_get_by(res, idx) {
            Ok(s) => s.parse().map_err(|e: uuid::Error| TryGetError::DbErr(DbErr::Type(e.to_string()))),
            Err(TryGetError::Null(col)) => Err(TryGetError::Null(col)),
            Err(text) => uuid::Uuid::try_get_by(res, idx).map(Uuid).map_err(|_| text),
        }
    }
}

impl ValueType for Uuid {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::Uuid(Some(u)) => Ok(Uuid(u)),
            Value::String(Some(s)) => s.parse().map_err(|_| ValueTypeErr),
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "Uuid".to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::Uuid
    }

    fn column_type() -> ColumnType {
        ColumnType::Uuid
    }
}

impl Nullable for Uuid {
    fn null() -> Value {
        if postgres() { Value::Uuid(None) } else { Value::String(None) }
    }
}

impl IntoActiveValue<Uuid> for Uuid {
    fn into_active_value(self) -> sea_orm::ActiveValue<Uuid> {
        sea_orm::ActiveValue::Set(self)
    }
}

impl TryFromU64 for Uuid {
    fn try_from_u64(_: u64) -> Result<Self, DbErr> {
        Err(DbErr::ConvertFromU64("Uuid"))
    }
}
