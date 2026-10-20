use crate::db::id::Int;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "crons")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Int,
    #[sea_orm(unique)]
    pub command: String,
    pub last_run_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

timestamps!();
