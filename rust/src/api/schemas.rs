//! Response objects shared by several areas.

use serde::Serialize;
use utoipa::ToSchema;

use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::{contacts, users};

/// The logged-in user's display and format preferences.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserPreferences {
    /// How contact names are built, e.g. `%first_name% %last_name%`.
    pub name_order: String,
    /// `last_updated`, `asc` or `desc`.
    pub contact_sort_order: String,
    pub date_format: String,
    pub timezone: Option<String>,
    pub number_format: String,
    /// `km` or `mi`.
    pub distance_format: String,
    /// `google_maps` or `open_street_maps`.
    pub default_map_site: String,
    pub locale: String,
}

/// The logged-in user.
#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    #[schema(value_type = uuid::Uuid)]
    pub account_id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: String,
    pub is_account_administrator: bool,
    pub is_instance_administrator: bool,
    /// A secret exists and is confirmed.
    pub two_factor_enabled: bool,
    pub preferences: UserPreferences,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl From<users::Model> for CurrentUser {
    fn from(u: users::Model) -> Self {
        CurrentUser {
            two_factor_enabled: crate::auth::two_factor::enabled(&u),
            id: u.id,
            account_id: u.account_id,
            first_name: u.first_name,
            last_name: u.last_name,
            username: u.username,
            is_account_administrator: u.is_account_administrator,
            is_instance_administrator: u.is_instance_administrator,
            preferences: UserPreferences {
                name_order: u.name_order,
                contact_sort_order: u.contact_sort_order,
                date_format: u.date_format,
                timezone: u.timezone,
                number_format: u.number_format,
                distance_format: u.distance_format,
                default_map_site: u.default_map_site,
                locale: u.locale,
            },
            created_at: u.created_at.unwrap_or_else(crate::db::now),
            updated_at: u.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

/// Enough to show a contact anywhere; the client builds the display name
/// from the user's `name_order` and fetches the avatar separately.
#[derive(Debug, Serialize, ToSchema)]
pub struct ContactSummary {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    #[schema(value_type = uuid::Uuid)]
    pub vault_id: Uuid,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: Option<String>,
    pub nickname: Option<String>,
    pub maiden_name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    #[schema(value_type = Option<i64>)]
    pub avatar_file_id: Option<crate::db::id::Id>,
    /// False for contacts hidden from lists (e.g. created via a relationship).
    pub listed: bool,
}

impl From<contacts::Model> for ContactSummary {
    fn from(c: contacts::Model) -> Self {
        ContactSummary {
            id: c.id,
            vault_id: c.vault_id,
            first_name: c.first_name,
            middle_name: c.middle_name,
            last_name: c.last_name,
            nickname: c.nickname,
            maiden_name: c.maiden_name,
            prefix: c.prefix,
            suffix: c.suffix,
            avatar_file_id: c.file_id,
            listed: c.listed,
        }
    }
}

/// A user as other users see them; `contact` is the user's own contact in
/// the vault of the response, when there is one.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserSummary {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub contact: Option<ContactSummary>,
}

impl UserSummary {
    pub fn new(u: &users::Model, contact: Option<contacts::Model>) -> Self {
        UserSummary {
            id: u.id,
            first_name: u.first_name.clone(),
            last_name: u.last_name.clone(),
            contact: contact.map(Into::into),
        }
    }
}
