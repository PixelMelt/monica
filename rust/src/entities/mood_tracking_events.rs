use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mood_tracking_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub mood_tracking_parameter_id: Id,
    pub rated_at: DateTime,
    #[sea_orm(column_type = "Text", nullable)]
    pub note: Option<String>,
    pub number_of_hours_slept: Option<i32>,
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
        belongs_to = "super::mood_tracking_parameters::Entity",
        from = "Column::MoodTrackingParameterId",
        to = "super::mood_tracking_parameters::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    MoodTrackingParameters,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::mood_tracking_parameters::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MoodTrackingParameters.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
