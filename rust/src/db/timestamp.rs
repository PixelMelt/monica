//! `Timestamp`: the type of every `timestamp`/`datetime` column, in UTC.
//!
//! Laravel's `timestamp()` is `timestamp` on MySQL and `timestamp(0) without
//! time zone` on Postgres; `datetime()` is `datetime` on MySQL. sqlx decodes
//! MySQL's `timestamp` only into `DateTime<Utc>` and Postgres' only into
//! `NaiveDateTime`, so this type decodes either and holds the naive UTC
//! value Laravel stores. Serialized as RFC 3339 with a `Z` (api-design.md §4).

use std::fmt;

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, IntoActiveValue, QueryResult, TryGetError, TryGetable, Value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub NaiveDateTime);

impl Timestamp {
    pub fn utc(&self) -> DateTime<Utc> {
        self.0.and_utc()
    }

    pub fn timestamp(&self) -> i64 {
        self.utc().timestamp()
    }
}

impl From<NaiveDateTime> for Timestamp {
    fn from(v: NaiveDateTime) -> Self {
        Timestamp(v)
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(v: DateTime<Utc>) -> Self {
        Timestamp(v.naive_utc())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.utc().to_rfc3339_opts(SecondsFormat::Secs, true))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        DateTime::parse_from_rfc3339(&s).map(|t| Timestamp(t.naive_utc())).map_err(serde::de::Error::custom)
    }
}

impl From<Timestamp> for Value {
    fn from(t: Timestamp) -> Self {
        Value::ChronoDateTime(Some(t.0))
    }
}

impl TryGetable for Timestamp {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        match NaiveDateTime::try_get_by(res, idx) {
            Ok(v) => Ok(Timestamp(v)),
            Err(TryGetError::Null(col)) => Err(TryGetError::Null(col)),
            Err(naive) => DateTime::<Utc>::try_get_by(res, idx).map(Timestamp::from).map_err(|_| naive),
        }
    }
}

impl ValueType for Timestamp {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::ChronoDateTime(Some(v)) => Ok(Timestamp(v)),
            Value::ChronoDateTimeUtc(Some(v)) => Ok(Timestamp::from(v)),
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "Timestamp".to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::ChronoDateTime
    }

    fn column_type() -> ColumnType {
        ColumnType::DateTime
    }
}

impl Nullable for Timestamp {
    fn null() -> Value {
        Value::ChronoDateTime(None)
    }
}

impl IntoActiveValue<Timestamp> for Timestamp {
    fn into_active_value(self) -> sea_orm::ActiveValue<Timestamp> {
        sea_orm::ActiveValue::Set(self)
    }
}
