//! `Id`: the type of every integer primary and foreign key.
//!
//! Laravel's `id()` and `foreignId()` are `bigint unsigned` on MySQL and
//! `bigint` elsewhere. sqlx decodes MySQL's unsigned column only into `u64`
//! and Postgres has no `u64`, so a plain `i64` field fails on one engine or
//! the other. `Id` holds an `i64` and decodes either.

use std::fmt;

use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
use sea_orm::{
    ColIdx, DbErr, IntoActiveValue, QueryResult, TryFromU64, TryGetError, TryGetable, Value,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub i64);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<i64> for Id {
    fn from(v: i64) -> Self {
        Id(v)
    }
}

impl From<Id> for Value {
    fn from(id: Id) -> Self {
        Value::BigInt(Some(id.0))
    }
}

impl TryGetable for Id {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        match i64::try_get_by(res, idx) {
            Ok(v) => Ok(Id(v)),
            Err(TryGetError::Null(col)) => Err(TryGetError::Null(col)),
            Err(signed) => match u64::try_get_by(res, idx) {
                Ok(v) => <i64 as std::convert::TryFrom<u64>>::try_from(v)
                    .map(Id)
                    .map_err(|_| signed),
                Err(_) => Err(signed),
            },
        }
    }
}

impl ValueType for Id {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::BigInt(Some(v)) => Ok(Id(v)),
            Value::BigUnsigned(Some(v)) => <i64 as std::convert::TryFrom<u64>>::try_from(v)
                .map(Id)
                .map_err(|_| ValueTypeErr),
            Value::Int(Some(v)) => Ok(Id(v.into())),
            Value::Unsigned(Some(v)) => Ok(Id(v.into())),
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "Id".to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::BigInt
    }

    fn column_type() -> ColumnType {
        ColumnType::BigInteger
    }
}

impl Nullable for Id {
    fn null() -> Value {
        Value::BigInt(None)
    }
}

impl IntoActiveValue<Id> for Id {
    fn into_active_value(self) -> sea_orm::ActiveValue<Id> {
        sea_orm::ActiveValue::Set(self)
    }
}

impl TryFromU64 for Id {
    fn try_from_u64(n: u64) -> Result<Self, DbErr> {
        <i64 as std::convert::TryFrom<u64>>::try_from(n)
            .map(Id)
            .map_err(|e| DbErr::TryIntoErr {
                from: "u64",
                into: "Id",
                source: std::sync::Arc::new(e),
            })
    }
}
