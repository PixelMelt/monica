//! Response objects shared by several areas.

use serde::Serialize;
use utoipa::ToSchema;

use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::users;

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
    pub help_shown: bool,
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
    pub email: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub email_verified_at: Option<Timestamp>,
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
            email: u.email,
            email_verified_at: u.email_verified_at,
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
                help_shown: u.help_shown,
            },
            created_at: u.created_at.unwrap_or_else(crate::db::now),
            updated_at: u.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}
