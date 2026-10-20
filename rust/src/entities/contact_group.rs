use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_group")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub group_id: Id,
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    pub group_type_role_id: Option<Id>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
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
        belongs_to = "super::group_type_roles::Entity",
        from = "Column::GroupTypeRoleId",
        to = "super::group_type_roles::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    GroupTypeRoles,
    #[sea_orm(
        belongs_to = "super::groups::Entity",
        from = "Column::GroupId",
        to = "super::groups::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Groups,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::group_type_roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GroupTypeRoles.def()
    }
}

impl Related<super::groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Groups.def()
    }
}

timestamps!();
