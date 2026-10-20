use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "quick_facts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_quick_facts_template_id: Id,
    pub contact_id: Uuid,
    pub content: String,
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
        belongs_to = "super::vault_quick_facts_templates::Entity",
        from = "Column::VaultQuickFactsTemplateId",
        to = "super::vault_quick_facts_templates::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    VaultQuickFactsTemplates,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::vault_quick_facts_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::VaultQuickFactsTemplates.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
