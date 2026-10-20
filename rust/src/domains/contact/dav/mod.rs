//! `app/Domains/Contact/Dav`: the vCard / iCalendar exporters and importers
//! (`Exporter`, `Importer`, `VCalendarImporter`, `VCardResource`,
//! `VCalendarResource`), the services and jobs built on them, and the
//! exporters and importers of each module (`app/Domains/Contact/*/Dav`).
//!
//! The PHP finds the exporters and importers by reflection and sorts them by
//! their `#[Order]`; [`services`] lists them in that order.

pub mod jobs;
pub mod manage_contact;
pub mod manage_contact_important_dates;
pub mod manage_contact_information;
pub mod manage_groups;
pub mod manage_tasks;
pub mod services;

use sea_orm::sea_query::IntoCondition;
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter};

use crate::config::Config;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::{contact_important_dates, contact_tasks, contacts, groups, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::vobject::{self, Document};

/// `VCardResource`: a contact or a group.
#[derive(Debug, Clone, PartialEq)]
pub enum VCardResource {
    Contact(Box<contacts::Model>),
    Group(Box<groups::Model>),
}

impl VCardResource {
    pub fn vault_id(&self) -> Uuid {
        match self {
            VCardResource::Contact(c) => c.vault_id,
            VCardResource::Group(g) => g.vault_id,
        }
    }

    pub fn vcard(&self) -> Option<&str> {
        match self {
            VCardResource::Contact(c) => c.vcard.as_deref(),
            VCardResource::Group(g) => g.vcard.as_deref(),
        }
    }

    pub fn distant_etag(&self) -> Option<&str> {
        match self {
            VCardResource::Contact(c) => c.distant_etag.as_deref(),
            VCardResource::Group(g) => g.distant_etag.as_deref(),
        }
    }
}

/// `VCalendarResource`: an important date or a task.
#[derive(Debug, Clone, PartialEq)]
pub enum VCalendarResource {
    ContactImportantDate(contact_important_dates::Model),
    ContactTask(contact_tasks::Model),
}

impl VCalendarResource {
    pub fn contact_id(&self) -> Uuid {
        match self {
            VCalendarResource::ContactImportantDate(d) => d.contact_id,
            VCalendarResource::ContactTask(t) => t.contact_id,
        }
    }

    pub fn vcalendar(&self) -> Option<&str> {
        match self {
            VCalendarResource::ContactImportantDate(d) => d.vcalendar.as_deref(),
            VCalendarResource::ContactTask(t) => t.vcalendar.as_deref(),
        }
    }

    pub fn distant_etag(&self) -> Option<&str> {
        match self {
            VCalendarResource::ContactImportantDate(d) => d.distant_etag.as_deref(),
            VCalendarResource::ContactTask(t) => t.distant_etag.as_deref(),
        }
    }
}

impl From<vobject::Error> for ApiError {
    /// A `ParseException` or `InvalidDataException` that reaches the caller.
    fn from(err: vobject::Error) -> Self {
        ApiError::internal(err)
    }
}

/// PHP's `empty()` on a string.
fn php_empty(value: &str) -> bool {
    value.is_empty() || value == "0"
}

/// PHP's `trim()`.
fn php_trim(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B'])
}

/// `Exporter::escape()`.
fn escape(value: Option<&str>) -> Option<String> {
    let value = value.unwrap_or_default();
    (!php_empty(value)).then(|| php_trim(value).to_owned())
}

/// `formatValue()` (`Exporter`, `Importer`, `VCalendarImporter` and the
/// `DAVFormat` trait have the same one).
fn format_value(value: Option<&str>) -> Option<String> {
    value.filter(|v| !php_empty(v)).map(|v| php_trim(v).replace("\\;", ";"))
}

/// `Importer::getUid()` (and the `UID` of the importers' `VEVENT` / `VTODO`).
fn get_uid(entry: &vobject::Component) -> Option<String> {
    let uuid = entry.property("UID").map(ToString::to_string).unwrap_or_default();
    (!php_empty(&uuid)).then_some(uuid)
}

/// `importUid()` of `Importer` and `VCalendarImporter`, given the importer's
/// `getUid()`: the new `$data['id']`.
fn import_uid(external: bool, id: Option<String>, uid: Option<String>) -> Option<String> {
    match uid {
        Some(uuid) if is_valid_uuid(&uuid) && !external => Some(uuid),
        _ => id,
    }
}

/// `Importer::kind()`.
fn kind(entry: &Document) -> String {
    let mut kind = entry.property("KIND");
    if kind.is_none() {
        let kinds = entry.properties("X-ADDRESSBOOKSERVER-KIND");
        if !kinds.is_empty() {
            kind = Some(kinds[0]);
        }
    }
    kind.and_then(vobject::Property::get_value).unwrap_or_else(|| "individual".to_owned())
}

/// `Ramsey\Uuid\Uuid::isValid()`.
fn is_valid_uuid(uuid: &str) -> bool {
    parse_uuid(uuid).is_some()
}

/// A `Uuid::isValid()` value as a [`Uuid`].
fn parse_uuid(uuid: &str) -> Option<Uuid> {
    let mut s = uuid.to_owned();
    for strip in ["urn:", "uuid:", "URN:", "UUID:", "{", "}"] {
        s = s.replace(strip, "");
    }
    let b = s.as_bytes();
    let hyphens = [8, 13, 18, 23];
    let valid = b.len() == 36
        && b.iter().enumerate().all(|(i, c)| if hyphens.contains(&i) { *c == b'-' } else { c.is_ascii_hexdigit() });
    if valid { s.parse().ok() } else { None }
}

/// `/** @var Contact $contact */ $contact = $result;` of the contact importers.
fn contact(result: Option<VCardResource>) -> ApiResult<contacts::Model> {
    match result {
        Some(VCardResource::Contact(contact)) => Ok(*contact),
        _ => Err(ApiError::internal("the vCard importer expects a contact")),
    }
}

/// `$contact->refresh()`.
async fn refresh<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<contacts::Model> {
    contacts::Entity::find_by_id(contact.id).one(db).await?.ok_or_else(ApiError::not_found)
}

/// `where([$column => $value])`: `= value`, or `IS NULL` for a null value.
fn eq_or_null<T: Into<sea_orm::Value>>(column: impl ColumnTrait, value: Option<T>) -> Condition {
    match value {
        Some(v) => column.eq(v).into_condition(),
        None => column.is_null().into_condition(),
    }
}

/// `SyncDAVBackend::decodeUri()`: `pathinfo(urldecode($uri), PATHINFO_FILENAME)`.
fn decode_uri(uri: &str) -> String {
    let b = uri.as_bytes();
    let mut decoded = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => decoded.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                decoded.push(u8::from_str_radix(&uri[i + 1..i + 3], 16).unwrap_or(0));
                i += 2;
            }
            c => decoded.push(c),
        }
        i += 1;
    }
    let decoded = String::from_utf8_lossy(&decoded).into_owned();
    let base = decoded.trim_end_matches('/');
    let base = base.rsplit('/').next().unwrap_or_default();
    match base.rfind('.') {
        Some(i) => base[..i].to_owned(),
        None => base.to_owned(),
    }
}

/// `User::getContactInVault($vault)`: the user's contact in the vault.
async fn get_contact_in_vault<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<Option<contacts::Model>> {
    let entry = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .one(db)
        .await?;

    let Some(entry) = entry else {
        return Ok(None);
    };

    Ok(contacts::Entity::live().filter(contacts::Column::Id.eq(entry.contact_id)).one(db).await?)
}

/// `$model->contact`; a soft-deleted contact is a 404.
async fn find_contact<C: ConnectionTrait>(db: &C, id: Uuid) -> ApiResult<contacts::Model> {
    contacts::Entity::live().filter(contacts::Column::Id.eq(id)).one(db).await?.ok_or_else(ApiError::not_found)
}

/// A date of the database as a `DateTimeInterface` value (`null` when unset).
fn timestamp(value: Option<Timestamp>) -> vobject::Input {
    value.map_or(vobject::Input::Null, |t| vobject::DateTime::utc(t.0).into())
}

/// `Carbon::parse($property->getDateTime())`: the wall-clock time (now without a date).
fn carbon_parse(date_time: Option<vobject::DateTime>) -> chrono::NaiveDateTime {
    date_time.map_or_else(|| crate::db::now().0, |d| d.local)
}

/// `Str::is($pattern, $value, $ignoreCase)`: `*` matches any run of characters.
fn str_is(pattern: Option<&str>, value: Option<&str>, ignore_case: bool) -> bool {
    let (pattern, value) = (pattern.unwrap_or_default(), value.unwrap_or_default());
    if pattern == "*" || pattern == value {
        return true;
    }
    if ignore_case && pattern.to_lowercase() == value.to_lowercase() {
        return true;
    }
    let (pattern, value) = if ignore_case {
        (pattern.to_lowercase(), value.to_lowercase())
    } else {
        (pattern.to_owned(), value.to_owned())
    };
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return false;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !value.starts_with(first) || value.len() < first.len() + last.len() || !value.ends_with(last) {
        return false;
    }
    let mut rest = &value[first.len()..value.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}

/// `route('contact.show', ['vault' => …, 'contact' => …])`.
fn route_contact_show(config: &Config, vault_id: Uuid, contact_id: Uuid) -> String {
    format!("{}/vaults/{vault_id}/contacts/{contact_id}", config.url)
}

#[cfg(test)]
mod tests;
