use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "job_batches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub total_jobs: i32,
    pub pending_jobs: i32,
    pub failed_jobs: i32,
    #[sea_orm(column_type = "Text")]
    pub failed_job_ids: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub options: Option<String>,
    pub cancelled_at: Option<i32>,
    pub created_at: i32,
    pub finished_at: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::addressbook_subscriptions::Entity")]
    AddressbookSubscriptions,
}

impl Related<super::addressbook_subscriptions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AddressbookSubscriptions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
