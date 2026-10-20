use crate::db::id::{Id, Int};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub queue: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub attempts: Int,
    pub reserved_at: Option<Int>,
    pub available_at: Int,
    pub created_at: Int,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
