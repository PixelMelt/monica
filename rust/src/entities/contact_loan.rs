use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_loan")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub loan_id: Id,
    #[sea_orm(primary_key, auto_increment = false)]
    pub loaner_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub loanee_id: Uuid,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::LoaneeId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts2,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::LoanerId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts1,
    #[sea_orm(
        belongs_to = "super::loans::Entity",
        from = "Column::LoanId",
        to = "super::loans::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Loans,
}

impl Related<super::loans::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Loans.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
