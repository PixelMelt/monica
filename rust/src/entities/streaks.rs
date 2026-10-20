use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "streaks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub goal_id: Id,
    pub happened_at: Timestamp,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::goals::Entity",
        from = "Column::GoalId",
        to = "super::goals::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Goals,
}

impl Related<super::goals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Goals.def()
    }
}

timestamps!();
