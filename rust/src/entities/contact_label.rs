use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_label")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub label_id: Id,
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
    #[sea_orm(
        belongs_to = "super::labels::Entity",
        from = "Column::LabelId",
        to = "super::labels::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Labels,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::labels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Labels.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
