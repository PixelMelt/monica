use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "streaks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub goal_id: Id,
    pub happened_at: DateTime,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
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

impl ActiveModelBehavior for ActiveModel {}
