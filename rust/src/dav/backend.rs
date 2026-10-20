//! `app/Domains/Contact/Dav/Web`: the links between sabre/dav and Monica
//! (`Auth/AuthBackend`, `DAVACL/PrincipalBackend`, `Backend/GetVaults`,
//! `Backend/SyncDAVBackend`, `Backend/CardDAV/CardDAVBackend`,
//! `Backend/CalDAV/{CalDAVBackend,AbstractCalDAVBackend,CalDAVDates,CalDAVTasks}`).
//! `WithUser` is the [`Ctx`] every function takes; `IDAVBackend` and
//! `ICalDAVBackend` are the [`SyncBackend`] and [`CalBackend`] enums.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

use super::props::PropValue;
use super::server::{DavError, DavResult};
use super::xml::{NS_CALDAV, NS_CALENDARSERVER, NS_CARDDAV, NS_SABREDAV};
use crate::config::Config;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::domains::contact::dav::jobs::{UpdateVCalendar, UpdateVCard};
use crate::domains::contact::dav::manage_contact::card_dav_get_object;
use crate::domains::contact::dav::manage_contact_important_dates::cal_dav_dates_get_object;
use crate::domains::contact::dav::manage_tasks::cal_dav_tasks_get_object;
use crate::domains::contact::dav::services::{ExportVCalendar, ExportVCard, GetEtag, ReadVObject};
use crate::domains::contact::dav::{VCalendarResource, VCardResource};
use crate::domains::contact::manage_contact::DestroyContact;
use crate::domains::contact::manage_contact_important_dates::DestroyContactImportantDate;
use crate::domains::contact::manage_groups::DestroyGroup;
use crate::domains::contact::manage_tasks::DestroyContactTask;
use crate::entities::{
    contact_important_dates, contact_tasks, contacts, groups, sync_tokens, user_vault, users, vaults,
};

/// `Sabre\DAV\Sync\Plugin::SYNCTOKEN_PREFIX`.
pub const SYNCTOKEN_PREFIX: &str = "http://sabre.io/ns/sync/";

/// What the backends work with: the database, the configuration, and the
/// user of `withUser($user)` (`Auth::user()`).
pub struct Ctx<'a> {
    pub db: &'a DatabaseConnection,
    pub config: &'a Config,
    pub user: users::Model,
}

/// PHP's `empty()` on a string.
fn php_empty(value: &str) -> bool {
    value.is_empty() || value == "0"
}

/// PHP's `(int)` on a string: its leading integer.
fn php_intval(value: &str) -> i64 {
    let s = value.trim_start();
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, s.strip_prefix('+').unwrap_or(s)),
    };
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    digits[..end].parse::<i64>().map(|n| sign * n).unwrap_or(0)
}

/// `Carbon::parse($value)` of a vCard `REV` or an iCalendar `LAST-MODIFIED`
/// (`null` is now), in UTC.
fn carbon_parse(value: Option<String>) -> DavResult<NaiveDateTime> {
    let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
        return Ok(crate::db::now().0);
    };
    let upper = value.trim().to_uppercase();
    let v = upper.as_str();
    let utc = |s: &str, f: &str| NaiveDateTime::parse_from_str(s, f).ok();
    let parsed = utc(v, "%Y%m%dT%H%M%SZ")
        .or_else(|| utc(v, "%Y%m%dT%H%M%S"))
        .or_else(|| utc(v, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|| utc(v, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|| utc(v, "%Y-%m-%d %H:%M:%S"))
        .or_else(|| chrono::DateTime::parse_from_rfc3339(v).ok().map(|d| d.naive_utc()))
        .or_else(|| chrono::DateTime::parse_from_str(v, "%Y%m%dT%H%M%S%z").ok().map(|d| d.naive_utc()))
        .or_else(|| NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(|d| d.and_time(NaiveTime::MIN)))
        .or_else(|| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok().map(|d| d.and_time(NaiveTime::MIN)));
    parsed.ok_or_else(|| {
        DavError::internal(
            "Carbon\\Exceptions\\InvalidFormatException",
            format!("Could not parse '{value}': Failed to parse time string ({value})"),
        )
    })
}

/// `Str::of($value)->after($search)`.
fn str_after<'a>(value: &'a str, search: &str) -> &'a str {
    match value.find(search) {
        Some(i) if !search.is_empty() => &value[i + search.len()..],
        _ => value,
    }
}

// ---------------------------------------------------------------- AuthBackend

/// `AuthBackend::check()`: the user the middleware authenticated, as a principal.
pub fn auth_check(ctx: &Ctx) -> (bool, String) {
    (true, get_principal_user(&ctx.user))
}

// ----------------------------------------------------------- PrincipalBackend

/// `PrincipalBackend::PRINCIPAL_PREFIX`.
pub const PRINCIPAL_PREFIX: &str = "principals/";

/// A principal of `getPrincipals()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Principal {
    pub uri: String,
    pub displayname: String,
    pub email_address: String,
}

impl Principal {
    /// `$principal[$key]`.
    pub fn get(&self, key: &str) -> Option<&str> {
        match key {
            "uri" => Some(&self.uri),
            "{DAV:}displayname" => Some(&self.displayname),
            k if k == format!("{{{NS_SABREDAV}}}email-address") => Some(&self.email_address),
            _ => None,
        }
    }
}

/// `PrincipalBackend::getPrincipalUser($user)`.
pub fn get_principal_user(user: &users::Model) -> String {
    format!("{PRINCIPAL_PREFIX}{}", user.email)
}

/// `$user->name`.
fn user_name(user: &users::Model) -> String {
    format!("{} {}", user.first_name.as_deref().unwrap_or_default(), user.last_name.as_deref().unwrap_or_default())
}

fn get_principals(ctx: &Ctx) -> Vec<Principal> {
    vec![Principal {
        uri: get_principal_user(&ctx.user),
        displayname: user_name(&ctx.user),
        email_address: ctx.user.email.clone(),
    }]
}

/// `PrincipalBackend::getPrincipalsByPrefix($prefixPath)`.
pub fn get_principals_by_prefix(ctx: &Ctx, prefix_path: &str) -> Vec<Principal> {
    let prefix_path = if prefix_path.ends_with('/') { prefix_path.to_owned() } else { format!("{prefix_path}/") };

    get_principals(ctx).into_iter().filter(|principal| principal.uri.starts_with(&prefix_path)).collect()
}

/// `PrincipalBackend::getPrincipalByPath($path)`.
pub fn get_principal_by_path(ctx: &Ctx, path: &str) -> Option<Principal> {
    get_principals_by_prefix(ctx, PRINCIPAL_PREFIX).into_iter().find(|principal| principal.uri == path)
}

/// `PrincipalBackend::searchPrincipals($prefixPath, $searchProperties, $test)`.
pub fn search_principals(
    ctx: &Ctx,
    prefix_path: &str,
    search_properties: &[(String, Option<String>)],
    test: &str,
) -> DavResult<Vec<String>> {
    let mut result = Vec::new();
    let principals = get_principals_by_prefix(ctx, prefix_path);
    if principals.is_empty() {
        return Ok(result);
    }

    for principal in principals {
        let mut ok = false;
        for (key, value) in search_properties {
            let Some(found) = principal.get(key) else {
                return Err(DavError::internal("ErrorException", format!("Undefined array key \"{key}\"")));
            };
            // `==` against a value that isn't text (`None`) is false.
            if Some(found) == value.as_deref() {
                ok = true;
            } else if test == "allof" {
                ok = false;
                break;
            }
        }
        if ok {
            result.push(principal.uri);
        }
    }

    Ok(result)
}

/// `PrincipalBackend::getGroupMemberSet($principal)`.
pub fn get_group_member_set(ctx: &Ctx, principal: &str) -> Vec<String> {
    match get_principal_by_path(ctx, principal) {
        Some(principal) => vec![principal.uri],
        None => Vec::new(),
    }
}

/// `PrincipalBackend::getGroupMembership($principal)`.
pub fn get_group_membership(ctx: &Ctx, principal: &str) -> Vec<String> {
    get_group_member_set(ctx, principal)
}

// ------------------------------------------------------------------ GetVaults

/// `GetVaults::vaults($collectionId, $permission)`.
pub async fn vaults(
    ctx: &Ctx<'_>,
    collection_id: Option<&str>,
    permission: VaultPermission,
) -> DavResult<Vec<vaults::Model>> {
    let mut query = vaults::Entity::find()
        .inner_join(user_vault::Entity)
        .filter(user_vault::Column::UserId.eq(ctx.user.id))
        .filter(user_vault::Column::Permission.lte(permission as i32));

    if let Some(collection_id) = collection_id {
        match collection_id.parse::<Uuid>() {
            Ok(id) => query = query.filter(vaults::Column::Id.eq(id)),
            Err(_) => return Ok(Vec::new()),
        }
    }

    Ok(query.all(ctx.db).await?)
}

// ------------------------------------------------------------- SyncDAVBackend

/// An `IDavResource`: a contact, a group, an important date or a task.
#[derive(Debug, Clone, PartialEq)]
pub enum DavResource {
    Contact(contacts::Model),
    Group(groups::Model),
    ContactImportantDate(contact_important_dates::Model),
    ContactTask(contact_tasks::Model),
}

impl DavResource {
    fn created_at(&self) -> Option<Timestamp> {
        match self {
            DavResource::Contact(m) => m.created_at,
            DavResource::Group(m) => m.created_at,
            DavResource::ContactImportantDate(m) => m.created_at,
            DavResource::ContactTask(m) => m.created_at,
        }
    }

    fn updated_at(&self) -> Option<Timestamp> {
        match self {
            DavResource::Contact(m) => m.updated_at,
            DavResource::Group(m) => m.updated_at,
            DavResource::ContactImportantDate(m) => m.updated_at,
            DavResource::ContactTask(m) => m.updated_at,
        }
    }

    fn deleted_at(&self) -> Option<Timestamp> {
        match self {
            DavResource::Contact(m) => m.deleted_at,
            DavResource::Group(m) => m.deleted_at,
            DavResource::ContactImportantDate(m) => m.deleted_at,
            DavResource::ContactTask(m) => m.deleted_at,
        }
    }

    fn distant_uuid(&self) -> Option<&str> {
        match self {
            DavResource::Contact(m) => m.distant_uuid.as_deref(),
            DavResource::Group(m) => m.distant_uuid.as_deref(),
            DavResource::ContactImportantDate(m) => m.distant_uuid.as_deref(),
            DavResource::ContactTask(m) => m.distant_uuid.as_deref(),
        }
    }

    /// `$obj->uuid` (contacts have none).
    fn uuid(&self) -> Option<Uuid> {
        match self {
            DavResource::Contact(_) => None,
            DavResource::Group(m) => m.uuid,
            DavResource::ContactImportantDate(m) => m.uuid,
            DavResource::ContactTask(m) => m.uuid,
        }
    }

    fn id(&self) -> String {
        match self {
            DavResource::Contact(m) => m.id.to_string(),
            DavResource::Group(m) => m.id.to_string(),
            DavResource::ContactImportantDate(m) => m.id.to_string(),
            DavResource::ContactTask(m) => m.id.to_string(),
        }
    }
}

/// The calendar backends of `CalDAVBackend::getBackends()`, with their vault
/// (`AbstractCalDAVBackend::withVault()`).
#[derive(Debug, Clone, PartialEq)]
pub enum CalBackend {
    /// `CalDAVDates`.
    Dates(vaults::Model),
    /// `CalDAVTasks`.
    Tasks(vaults::Model),
}

impl CalBackend {
    fn vault(&self) -> &vaults::Model {
        match self {
            CalBackend::Dates(vault) | CalBackend::Tasks(vault) => vault,
        }
    }

    /// `backendId()`.
    pub fn backend_id(&self) -> String {
        match self {
            CalBackend::Dates(vault) => format!("dates-{}", vault.id),
            CalBackend::Tasks(vault) => format!("tasks-{}", vault.id),
        }
    }

    /// `backendUri()`.
    pub fn backend_uri(&self) -> String {
        match self {
            CalBackend::Dates(vault) => format!("dates-{}", vault.name),
            CalBackend::Tasks(vault) => format!("tasks-{}", vault.name),
        }
    }
}

/// A backend with `SyncDAVBackend`.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncBackend {
    /// `CardDAVBackend`.
    Card,
    Cal(CalBackend),
}

impl SyncBackend {
    /// `backendId($collectionId)`.
    fn backend_id(&self, collection_id: Option<&str>) -> String {
        match self {
            SyncBackend::Card => format!("contacts-{}", collection_id.unwrap_or_default()),
            SyncBackend::Cal(cal) => cal.backend_id(),
        }
    }

    /// `getExtension()`.
    fn get_extension(&self) -> &'static str {
        match self {
            SyncBackend::Card => ".vcf",
            SyncBackend::Cal(_) => ".ics",
        }
    }

    /// `getObjects($collectionId)`.
    async fn get_objects(&self, ctx: &Ctx<'_>, collection_id: Option<&str>) -> DavResult<Vec<DavResource>> {
        match self {
            SyncBackend::Card => card_get_objects(ctx, collection_id).await,
            SyncBackend::Cal(cal) => cal_get_objects(ctx, cal).await,
        }
    }

    /// `getDeletedObjects($collectionId)`.
    async fn get_deleted_objects(&self, ctx: &Ctx<'_>, collection_id: Option<&str>) -> DavResult<Vec<DavResource>> {
        match self {
            SyncBackend::Card => card_get_deleted_objects(ctx, collection_id).await,
            SyncBackend::Cal(cal) => cal_get_deleted_objects(ctx, cal).await,
        }
    }

    /// `refreshObject($obj)`.
    async fn refresh_object(&self, ctx: &Ctx<'_>, obj: &DavResource) -> DavResult<String> {
        match self {
            SyncBackend::Card => card_refresh_object(ctx, obj).await,
            SyncBackend::Cal(cal) => cal_refresh_object(ctx, cal, obj).await,
        }
    }
}

/// `getCurrentSyncToken($collectionId)`.
pub async fn get_current_sync_token(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
) -> DavResult<Option<sync_tokens::Model>> {
    Ok(sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(ctx.user.account_id))
        .filter(sync_tokens::Column::UserId.eq(ctx.user.id))
        .filter(sync_tokens::Column::Name.eq(backend.backend_id(collection_id)))
        .order_by_desc(sync_tokens::Column::CreatedAt)
        .one(ctx.db)
        .await?)
}

/// `refreshSyncToken($collectionId)`: create or refresh the token if a change happened.
pub async fn refresh_sync_token(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
) -> DavResult<sync_tokens::Model> {
    let mut token = get_current_sync_token(ctx, backend, collection_id).await?;

    let refresh = match &token {
        None => true,
        Some(token) => get_last_modified(ctx, backend, collection_id).await?.is_some_and(|last| token.timestamp < last),
    };
    if refresh {
        token = Some(create_sync_token_now(ctx, backend, collection_id).await?);
    }

    Ok(token.expect("set above"))
}

/// `getSyncToken($collectionId, $syncTokenId)`.
async fn get_sync_token(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
    sync_token_id: i64,
) -> DavResult<Option<sync_tokens::Model>> {
    Ok(sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(ctx.user.account_id))
        .filter(sync_tokens::Column::UserId.eq(ctx.user.id))
        .filter(sync_tokens::Column::Name.eq(backend.backend_id(collection_id)))
        .filter(sync_tokens::Column::Id.eq(Id(sync_token_id)))
        .one(ctx.db)
        .await?)
}

/// `createSyncTokenNow($collectionId)`.
async fn create_sync_token_now(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
) -> DavResult<sync_tokens::Model> {
    Ok(sync_tokens::ActiveModel {
        account_id: Set(ctx.user.account_id),
        user_id: Set(ctx.user.id),
        name: Set(backend.backend_id(collection_id)),
        timestamp: Set(crate::db::now()),
        ..Default::default()
    }
    .insert(ctx.db)
    .await?)
}

/// `getLastModified($collectionId)`.
pub async fn get_last_modified(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
) -> DavResult<Option<Timestamp>> {
    Ok(backend.get_objects(ctx, collection_id).await?.iter().filter_map(DavResource::updated_at).max())
}

/// The result of `getChanges()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Changes {
    pub sync_token: Id,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// `getChanges($collectionId, $syncToken)`: `None` for an unknown token.
pub async fn get_changes(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
    sync_token: Option<&str>,
) -> DavResult<Option<Changes>> {
    let mut timestamp = None;
    if let Some(sync_token) = sync_token.filter(|t| !t.is_empty()) {
        let token = get_sync_token(ctx, backend, collection_id, php_intval(sync_token)).await?;

        let Some(token) = token else {
            // syncToken is not recognized
            return Ok(None);
        };

        timestamp = Some(token.timestamp);
    }

    let objs = backend.get_objects(ctx, collection_id).await?;

    Ok(Some(Changes {
        sync_token: refresh_sync_token(ctx, backend, collection_id).await?.id,
        added: get_added(backend, &objs, timestamp),
        modified: get_modified(ctx, backend, &objs, timestamp).await?,
        deleted: get_deleted(ctx, backend, collection_id, timestamp).await?,
    }))
}

/// `getAdded($objs, $timestamp)`.
fn get_added(backend: &SyncBackend, objs: &[DavResource], timestamp: Option<Timestamp>) -> Vec<String> {
    objs.iter()
        .filter(|obj| timestamp.is_none_or(|t| obj.created_at().is_some_and(|c| c >= t)))
        .map(|obj| encode_uri(backend, obj))
        .collect()
}

/// `getModified($objs, $timestamp)`.
async fn get_modified(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    objs: &[DavResource],
    timestamp: Option<Timestamp>,
) -> DavResult<Vec<String>> {
    let mut modified = Vec::new();
    for obj in objs {
        let Some(t) = timestamp else { continue };
        if obj.updated_at().is_some_and(|u| u > t) && obj.created_at().is_none_or(|c| c < t) {
            backend.refresh_object(ctx, obj).await?;

            modified.push(encode_uri(backend, obj));
        }
    }
    Ok(modified)
}

/// `getDeleted($collectionId, $timestamp)`.
async fn get_deleted(
    ctx: &Ctx<'_>,
    backend: &SyncBackend,
    collection_id: Option<&str>,
    timestamp: Option<Timestamp>,
) -> DavResult<Vec<String>> {
    Ok(backend
        .get_deleted_objects(ctx, collection_id)
        .await?
        .iter()
        .filter(|obj| timestamp.is_none_or(|t| obj.deleted_at().is_some_and(|d| d >= t)))
        .map(|obj| encode_uri(backend, obj))
        .collect())
}

/// `encodeUri($obj)`.
fn encode_uri(backend: &SyncBackend, obj: &DavResource) -> String {
    let id = match (obj.distant_uuid(), obj.uuid()) {
        (Some(distant_uuid), _) => str_after(distant_uuid, "urn:uuid:").to_owned(),
        (None, Some(uuid)) => uuid.to_string(),
        (None, None) => obj.id(),
    };

    super::http::urlencode(&format!("{id}{}", backend.get_extension()))
}

// ------------------------------------------------------------- CardDAVBackend

/// The CardDAV `SyncDAVBackend`.
pub const CARD: SyncBackend = SyncBackend::Card;

/// An address book of `getAddressBooksForUser()`.
#[derive(Debug, Clone, PartialEq)]
pub struct AddressBookInfo {
    /// `id`: the vault id.
    pub id: String,
    /// `uri`: the vault name.
    pub uri: String,
    pub principaluri: String,
    /// The `{…}` properties, in order.
    pub properties: Vec<(String, PropValue)>,
}

impl AddressBookInfo {
    /// `$this->addressBookInfo[$name]`.
    pub fn get(&self, name: &str) -> Option<&PropValue> {
        self.properties.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

/// `CardDAVBackend::getAddressBooksForUser($principalUri)`.
pub async fn get_address_books_for_user(ctx: &Ctx<'_>, _principal_uri: &str) -> DavResult<Vec<AddressBookInfo>> {
    let mut result = Vec::new();
    for vault in vaults(ctx, None, VaultPermission::View).await? {
        result.push(get_address_book_details(ctx, &vault).await?);
    }
    Ok(result)
}

async fn get_address_book_details(ctx: &Ctx<'_>, vault: &vaults::Model) -> DavResult<AddressBookInfo> {
    let token = get_current_sync_token(ctx, &CARD, Some(&vault.id.to_string())).await?;

    let mut properties = vec![("{DAV:}displayname".to_owned(), PropValue::Text(vault.name.clone()))];
    if let Some(description) = vault.description.as_ref().filter(|d| !d.is_empty()) {
        properties.push((format!("{{{NS_CARDDAV}}}addressbook-description"), PropValue::Text(description.clone())));
    }
    if let Some(token) = token {
        properties.push(("{DAV:}sync-token".to_owned(), PropValue::Text(token.id.to_string())));
        properties.push((format!("{{{NS_SABREDAV}}}sync-token"), PropValue::Text(token.id.to_string())));
        properties.push((
            format!("{{{NS_CALENDARSERVER}}}getctag"),
            PropValue::Text(format!("{SYNCTOKEN_PREFIX}{}", token.id)),
        ));
    }

    Ok(AddressBookInfo {
        id: vault.id.to_string(),
        uri: vault.name.clone(),
        principaluri: get_principal_user(&ctx.user),
        properties,
    })
}

/// `CardDAVBackend::getChangesForAddressBook($addressBookId, $syncToken, …)`.
pub async fn get_changes_for_address_book(
    ctx: &Ctx<'_>,
    address_book_id: &str,
    sync_token: Option<&str>,
) -> DavResult<Option<Changes>> {
    get_changes(ctx, &CARD, Some(address_book_id), sync_token).await
}

/// A card of `getCards()` / `getCard()` (`prepareCard()`).
#[derive(Debug, Clone, PartialEq)]
pub struct CardData {
    pub contact_id: String,
    pub uri: String,
    pub carddata: String,
    pub etag: String,
    pub distant_etag: Option<String>,
    pub lastmodified: i64,
}

fn vcard_resource(resource: &DavResource) -> VCardResource {
    match resource {
        DavResource::Group(g) => VCardResource::Group(Box::new(g.clone())),
        DavResource::Contact(c) => VCardResource::Contact(Box::new(c.clone())),
        DavResource::ContactImportantDate(_) | DavResource::ContactTask(_) => {
            unreachable!("the CardDAV backend only reads contacts and groups")
        }
    }
}

/// `$resource->refresh()`.
async fn refresh(ctx: &Ctx<'_>, resource: &DavResource) -> DavResult<DavResource> {
    let not_found =
        || DavError::internal("Illuminate\\Database\\Eloquent\\ModelNotFoundException", "No query results for model.");
    Ok(match resource {
        DavResource::Contact(m) => {
            DavResource::Contact(contacts::Entity::find_by_id(m.id).one(ctx.db).await?.ok_or_else(not_found)?)
        }
        DavResource::Group(m) => {
            DavResource::Group(groups::Entity::find_by_id(m.id).one(ctx.db).await?.ok_or_else(not_found)?)
        }
        DavResource::ContactImportantDate(m) => DavResource::ContactImportantDate(
            contact_important_dates::Entity::find_by_id(m.id).one(ctx.db).await?.ok_or_else(not_found)?,
        ),
        DavResource::ContactTask(m) => {
            DavResource::ContactTask(contact_tasks::Entity::find_by_id(m.id).one(ctx.db).await?.ok_or_else(not_found)?)
        }
    })
}

/// `$resource->updated_at->timestamp`.
fn timestamp(resource: &DavResource) -> DavResult<i64> {
    resource
        .updated_at()
        .map(|t| t.timestamp())
        .ok_or_else(|| DavError::internal("ErrorException", "Attempt to read property \"timestamp\" on null"))
}

/// `CardDAVBackend::prepareCard($resource)`.
pub async fn prepare_card(ctx: &Ctx<'_>, resource: &DavResource) -> DavResult<CardData> {
    let vcard = match resource {
        DavResource::Contact(c) => c.vcard.clone(),
        DavResource::Group(g) => g.vcard.clone(),
        _ => None,
    };
    let result = async {
        let mut carddata = vcard.clone();
        let mut timestamp_ = None;
        if let Some(data) = carddata.as_deref().filter(|d| !php_empty(d)) {
            timestamp_ = rev(data)?;
        }

        let stale = match (&carddata, timestamp_) {
            (None, _) | (_, None) => true,
            (Some(data), Some(t)) => php_empty(data) || resource.updated_at().is_some_and(|u| t < u.0),
        };
        if stale {
            carddata = Some(card_refresh_object(ctx, resource).await?);
        }

        let etag = GetEtag {
            account_id: ctx.user.account_id,
            author_id: ctx.user.id,
            vault_id: vcard_resource(resource).vault_id(),
            vcard: Some(vcard_resource(&refresh(ctx, resource).await?)),
            vcalendar: None,
        }
        .execute(ctx.db)
        .await?;

        Ok(CardData {
            contact_id: resource.id(),
            uri: encode_uri(&CARD, resource),
            carddata: carddata.unwrap_or_default(),
            etag,
            distant_etag: vcard_resource(resource).distant_etag().map(str::to_owned),
            lastmodified: timestamp(resource)?,
        })
    }
    .await;

    result.inspect_err(|e: &DavError| {
        tracing::error!(carddata = vcard, id = resource.id(), "CardDAVBackend prepareCard: {}", e.message);
    })
}

/// `CardDAVBackend::rev($entry)`.
fn rev(entry: &str) -> DavResult<Option<NaiveDateTime>> {
    let vcard = ReadVObject { entry: entry.to_owned() }.execute()?;

    let Some(vcard) = vcard else {
        return Ok(None);
    };

    Ok(Some(carbon_parse(vcard.property("REV").map(ToString::to_string))?))
}

/// `CardDAVBackend::refreshObject($obj)`.
async fn card_refresh_object(ctx: &Ctx<'_>, obj: &DavResource) -> DavResult<String> {
    let (vault_id, contact_id, group_id) = match obj {
        DavResource::Contact(c) => (c.vault_id, Some(c.id), None),
        DavResource::Group(g) => (g.vault_id, None, Some(g.id)),
        _ => unreachable!("the CardDAV backend only reads contacts and groups"),
    };

    let vcard = ExportVCard { account_id: ctx.user.account_id, author_id: ctx.user.id, vault_id, contact_id, group_id }
        .execute(ctx.db, ctx.config)
        .await?;

    Ok(vcard.serialize())
}

/// `CardDAVBackend::getObject($collectionId, $uri)`.
pub async fn card_get_object(ctx: &Ctx<'_>, collection_id: &str, uri: &str) -> DavResult<Option<DavResource>> {
    let Ok(collection_id) = collection_id.parse::<Uuid>() else {
        return Ok(None);
    };
    Ok(card_dav_get_object(ctx.db, &ctx.user, collection_id, uri).await.ok().flatten().map(|r| match r {
        VCardResource::Contact(c) => DavResource::Contact(*c),
        VCardResource::Group(g) => DavResource::Group(*g),
    }))
}

/// `CardDAVBackend::getObjects($collectionId)`: the active contacts, then the groups.
async fn card_get_objects(ctx: &Ctx<'_>, collection_id: Option<&str>) -> DavResult<Vec<DavResource>> {
    let vaults = vaults(ctx, collection_id, VaultPermission::View).await?;

    let mut result = Vec::new();
    for vault in &vaults {
        let contacts = contacts::Entity::live()
            .filter(contacts::Column::VaultId.eq(vault.id))
            .filter(contacts::Column::Listed.eq(true))
            .all(ctx.db)
            .await?;
        result.extend(contacts.into_iter().map(DavResource::Contact));
    }

    for vault in &vaults {
        let groups = groups::Entity::live().filter(groups::Column::VaultId.eq(vault.id)).all(ctx.db).await?;
        result.extend(groups.into_iter().map(DavResource::Group));
    }

    Ok(result)
}

/// `CardDAVBackend::getDeletedObjects($collectionId)`.
async fn card_get_deleted_objects(ctx: &Ctx<'_>, collection_id: Option<&str>) -> DavResult<Vec<DavResource>> {
    let vaults = vaults(ctx, collection_id, VaultPermission::View).await?;

    let mut result = Vec::new();
    for vault in &vaults {
        let contacts = contacts::Entity::find()
            .filter(contacts::Column::VaultId.eq(vault.id))
            .filter(contacts::Column::DeletedAt.is_not_null())
            .all(ctx.db)
            .await?;
        result.extend(contacts.into_iter().map(DavResource::Contact));
    }

    for vault in &vaults {
        let groups = groups::Entity::find()
            .filter(groups::Column::VaultId.eq(vault.id))
            .filter(groups::Column::DeletedAt.is_not_null())
            .all(ctx.db)
            .await?;
        result.extend(groups.into_iter().map(DavResource::Group));
    }

    Ok(result)
}

/// `CardDAVBackend::getCards($addressbookId)`.
pub async fn get_cards(ctx: &Ctx<'_>, addressbook_id: &str) -> DavResult<Vec<CardData>> {
    let mut cards = Vec::new();
    for card in card_get_objects(ctx, Some(addressbook_id)).await? {
        cards.push(prepare_card(ctx, &card).await?);
    }
    Ok(cards)
}

/// `CardDAVBackend::getCard($addressBookId, $cardUri)`: `None` for `false`.
pub async fn get_card(ctx: &Ctx<'_>, address_book_id: &str, card_uri: &str) -> DavResult<Option<CardData>> {
    let card = card_get_object(ctx, address_book_id, card_uri).await?;

    match card {
        None => Ok(None),
        Some(card) => Ok(Some(prepare_card(ctx, &card).await?)),
    }
}

/// `CardDAVBackend::createCard($addressBookId, $cardUri, $cardData)`.
pub async fn create_card(
    ctx: &Ctx<'_>,
    address_book_id: &str,
    card_uri: &str,
    card_data: &str,
) -> DavResult<Option<String>> {
    update_card(ctx, address_book_id, card_uri, card_data).await
}

/// `CardDAVBackend::updateCard($addressBookId, $cardUri, $cardData)`.
pub async fn update_card(
    ctx: &Ctx<'_>,
    address_book_id: &str,
    card_uri: &str,
    card_data: &str,
) -> DavResult<Option<String>> {
    let vault =
        vaults(ctx, Some(address_book_id), VaultPermission::Edit).await?.into_iter().next().ok_or_else(|| {
            DavError::internal(
                "Illuminate\\Database\\Eloquent\\ModelNotFoundException",
                "No query results for model [App\\Models\\Vault].",
            )
        })?;

    UpdateVCard {
        account_id: ctx.user.account_id,
        author_id: ctx.user.id,
        vault_id: vault.id,
        uri: card_uri.to_owned(),
        etag: None,
        external: None,
        card: card_data.to_owned(),
    }
    .execute(ctx.db)
    .await?;

    Ok(None)
}

/// `CardDAVBackend::deleteCard($addressBookId, $cardUri)`.
pub async fn delete_card(ctx: &Ctx<'_>, address_book_id: &str, card_uri: &str) -> DavResult<bool> {
    let obj = card_get_object(ctx, address_book_id, card_uri).await?;

    match obj {
        Some(DavResource::Contact(contact)) => {
            DestroyContact {
                account_id: ctx.user.account_id,
                author_id: ctx.user.id,
                vault_id: contact.vault_id,
                contact_id: contact.id,
            }
            .execute(ctx.db)
            .await?;

            Ok(true)
        }
        Some(DavResource::Group(group)) => {
            DestroyGroup {
                account_id: ctx.user.account_id,
                author_id: ctx.user.id,
                vault_id: group.vault_id,
                group_id: group.id,
            }
            .execute(ctx.db)
            .await?;

            Ok(true)
        }
        _ => Ok(false),
    }
}

// -------------------------------------------------------------- CalDAVBackend

/// `CalDAVBackend::getBackends()`: the dates and the tasks of each vault.
async fn get_backends(ctx: &Ctx<'_>) -> DavResult<Vec<CalBackend>> {
    let mut backends = Vec::new();
    for vault in vaults(ctx, None, VaultPermission::View).await? {
        backends.push(CalBackend::Dates(vault.clone()));
        backends.push(CalBackend::Tasks(vault));
    }
    Ok(backends)
}

/// `CalDAVBackend::getBackend($id)`.
async fn get_backend(ctx: &Ctx<'_>, id: &str) -> DavResult<Option<CalBackend>> {
    Ok(get_backends(ctx).await?.into_iter().find(|backend| backend.backend_id() == id))
}

/// A calendar of `getCalendarsForUser()` (`getDescription()`).
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarInfo {
    pub id: String,
    pub uri: String,
    pub principaluri: String,
    /// The `{…}` properties, in order.
    pub properties: Vec<(String, PropValue)>,
}

impl CalendarInfo {
    /// `$this->calendarInfo[$name]`.
    pub fn get(&self, name: &str) -> Option<&PropValue> {
        self.properties.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

/// `CalDAVBackend::getCalendarsForUser($principalUri)`.
pub async fn get_calendars_for_user(ctx: &Ctx<'_>, _principal_uri: &str) -> DavResult<Vec<CalendarInfo>> {
    let mut calendars = Vec::new();
    for backend in get_backends(ctx).await? {
        calendars.push(get_description(ctx, &backend).await?);
    }
    Ok(calendars)
}

/// `CalDAVBackend::getChangesForCalendar($calendarId, $syncToken, …)`:
/// `[]` (no changes array) when the backend or the token is unknown.
pub async fn get_changes_for_calendar(
    ctx: &Ctx<'_>,
    calendar_id: &str,
    sync_token: Option<&str>,
) -> DavResult<Option<Changes>> {
    let Some(backend) = get_backend(ctx, calendar_id).await? else {
        return Ok(None);
    };
    get_changes(ctx, &SyncBackend::Cal(backend), Some(calendar_id), sync_token).await
}

/// `CalDAVBackend::getCalendarObjects($calendarId)`.
pub async fn get_calendar_objects(ctx: &Ctx<'_>, calendar_id: &str) -> DavResult<Vec<ObjectData>> {
    let Some(backend) = get_backend(ctx, calendar_id).await? else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for date in cal_get_objects(ctx, &backend).await? {
        if let Some(data) = prepare_data(ctx, &backend, &date).await? {
            result.push(data);
        }
    }
    Ok(result)
}

/// `CalDAVBackend::getCalendarObject($calendarId, $objectUri)`: `None` for `[]`.
pub async fn get_calendar_object(ctx: &Ctx<'_>, calendar_id: &str, object_uri: &str) -> DavResult<Option<ObjectData>> {
    let Some(backend) = get_backend(ctx, calendar_id).await? else {
        return Ok(None);
    };

    match cal_get_object(ctx, &backend, object_uri).await? {
        Some(obj) => prepare_data(ctx, &backend, &obj).await,
        None => Ok(None),
    }
}

/// `CalDAVBackend::createCalendarObject($calendarId, $objectUri, $calendarData)`.
pub async fn create_calendar_object(
    ctx: &Ctx<'_>,
    calendar_id: &str,
    object_uri: &str,
    calendar_data: &str,
) -> DavResult<Option<String>> {
    update_calendar_object(ctx, calendar_id, object_uri, calendar_data).await
}

/// `CalDAVBackend::updateCalendarObject($calendarId, $objectUri, $calendarData)`.
pub async fn update_calendar_object(
    ctx: &Ctx<'_>,
    calendar_id: &str,
    object_uri: &str,
    calendar_data: &str,
) -> DavResult<Option<String>> {
    match get_backend(ctx, calendar_id).await? {
        Some(backend) => update_or_create_calendar_object(ctx, &backend, object_uri, calendar_data).await,
        None => Ok(None),
    }
}

/// `CalDAVBackend::deleteCalendarObject($calendarId, $objectUri)`.
pub async fn delete_calendar_object(ctx: &Ctx<'_>, calendar_id: &str, object_uri: &str) -> DavResult<()> {
    if let Some(backend) = get_backend(ctx, calendar_id).await? {
        cal_delete_calendar_object(ctx, &backend, object_uri).await?;
    }
    Ok(())
}

// ------------------------------------------------------ AbstractCalDAVBackend

/// `getDescription()` of `CalDAVDates` / `CalDAVTasks` (with the parent's).
async fn get_description(ctx: &Ctx<'_>, backend: &CalBackend) -> DavResult<CalendarInfo> {
    let sync = SyncBackend::Cal(backend.clone());
    let token = format!("{SYNCTOKEN_PREFIX}{}", refresh_sync_token(ctx, &sync, Some(&backend.backend_id())).await?.id);

    let mut properties = vec![
        ("{DAV:}sync-token".to_owned(), PropValue::Text(token.clone())),
        (format!("{{{NS_SABREDAV}}}sync-token"), PropValue::Text(token.clone())),
        (format!("{{{NS_CALENDARSERVER}}}getctag"), PropValue::Text(token)),
    ];

    let vault = backend.vault();
    let locale = crate::i18n::current();
    let (displayname, description, component) = match backend {
        CalBackend::Dates(_) => {
            let name = crate::i18n::t(&locale, "Contact important dates of :name", &[("name", &vault.name)]);
            (name.clone(), name, "VEVENT")
        }
        CalBackend::Tasks(_) => {
            (crate::i18n::t(&locale, "Tasks of :name", &[("name", &vault.name)]), vault.name.clone(), "VTODO")
        }
    };
    properties.push(("{DAV:}displayname".to_owned(), PropValue::Text(displayname)));
    properties.push((format!("{{{NS_CALDAV}}}calendar-description"), PropValue::Text(description)));
    if let Some(timezone) = &ctx.user.timezone {
        properties.push((format!("{{{NS_CALDAV}}}calendar-timezone"), PropValue::Text(timezone.clone())));
    }
    properties.push((
        format!("{{{NS_CALDAV}}}supported-calendar-component-set"),
        PropValue::SupportedCalendarComponentSet(vec![component.to_owned()]),
    ));
    properties.push((
        format!("{{{NS_CALDAV}}}schedule-calendar-transp"),
        PropValue::ScheduleCalendarTransp("transparent".to_owned()),
    ));

    Ok(CalendarInfo {
        id: backend.backend_id(),
        uri: backend.backend_uri(),
        principaluri: get_principal_user(&ctx.user),
        properties,
    })
}

/// A calendar object of `getCalendarObjects()` / `getCalendarObject()` (`exportData()`).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectData {
    pub id: Option<String>,
    pub uri: String,
    pub calendardata: String,
    pub etag: String,
    pub distant_etag: Option<String>,
    pub lastmodified: i64,
}

fn vcalendar_resource(resource: &DavResource) -> VCalendarResource {
    match resource {
        DavResource::ContactImportantDate(d) => VCalendarResource::ContactImportantDate(d.clone()),
        DavResource::ContactTask(t) => VCalendarResource::ContactTask(t.clone()),
        DavResource::Contact(_) | DavResource::Group(_) => {
            unreachable!("the CalDAV backends only read important dates and tasks")
        }
    }
}

/// `prepareData($obj)` of `CalDAVDates` / `CalDAVTasks`: `None` for `[]`.
async fn prepare_data(ctx: &Ctx<'_>, backend: &CalBackend, obj: &DavResource) -> DavResult<Option<ObjectData>> {
    match (backend, obj) {
        (CalBackend::Dates(_), DavResource::ContactImportantDate(_))
        | (CalBackend::Tasks(_), DavResource::ContactTask(_)) => Ok(Some(export_data(ctx, backend, obj).await?)),
        _ => Ok(None),
    }
}

/// `AbstractCalDAVBackend::exportData($resource)`.
async fn export_data(ctx: &Ctx<'_>, backend: &CalBackend, resource: &DavResource) -> DavResult<ObjectData> {
    let vcalendar = vcalendar_resource(resource).vcalendar().map(str::to_owned);
    let result = async {
        let mut calendardata = vcalendar.clone();
        let mut timestamp_ = None;
        if let Some(data) = calendardata.as_deref().filter(|d| !php_empty(d)) {
            timestamp_ = last_modified(backend, data)?;
        }

        let stale = match (&calendardata, timestamp_) {
            (None, _) | (_, None) => true,
            (Some(data), Some(t)) => php_empty(data) || resource.updated_at().is_some_and(|u| t < u.0),
        };
        if stale {
            calendardata = Some(cal_refresh_object(ctx, backend, resource).await?);
        }

        let etag = GetEtag {
            account_id: ctx.user.account_id,
            author_id: ctx.user.id,
            vault_id: backend.vault().id,
            vcard: None,
            vcalendar: Some(vcalendar_resource(&refresh(ctx, resource).await?)),
        }
        .execute(ctx.db)
        .await?;

        Ok(ObjectData {
            id: resource.uuid().map(|u| u.to_string()),
            uri: encode_uri(&SyncBackend::Cal(backend.clone()), resource),
            calendardata: calendardata.unwrap_or_default(),
            etag,
            distant_etag: vcalendar_resource(resource).distant_etag().map(str::to_owned),
            lastmodified: timestamp(resource)?,
        })
    }
    .await;

    result.inspect_err(|e: &DavError| {
        tracing::error!(
            calendardata = vcalendar,
            id = resource.id(),
            "AbstractCalDAVBackend exportData: {}",
            e.message
        );
    })
}

/// `AbstractCalDAVBackend::updateOrCreateCalendarObject($calendarId, $objectUri, $calendarData)`.
async fn update_or_create_calendar_object(
    ctx: &Ctx<'_>,
    backend: &CalBackend,
    object_uri: &str,
    calendar_data: &str,
) -> DavResult<Option<String>> {
    Ok(UpdateVCalendar {
        account_id: ctx.user.account_id,
        author_id: ctx.user.id,
        vault_id: backend.vault().id,
        uri: object_uri.to_owned(),
        etag: None,
        external: None,
        calendar: calendar_data.to_owned(),
    }
    .execute(ctx.db)
    .await?)
}

/// `refreshObject($obj)` of `CalDAVDates` / `CalDAVTasks`.
async fn cal_refresh_object(ctx: &Ctx<'_>, backend: &CalBackend, obj: &DavResource) -> DavResult<String> {
    let (contact_important_date_id, contact_task_id) = match obj {
        DavResource::ContactImportantDate(d) => (Some(d.id), None),
        DavResource::ContactTask(t) => (None, Some(t.id)),
        _ => unreachable!("the CalDAV backends only read important dates and tasks"),
    };

    let vcal = ExportVCalendar {
        account_id: ctx.user.account_id,
        author_id: ctx.user.id,
        vault_id: backend.vault().id,
        contact_important_date_id,
        contact_task_id,
    }
    .execute(ctx.db, ctx.config)
    .await?;

    Ok(vcal.serialize())
}

/// `getObject($collectionId, $uri)` of `CalDAVDates` / `CalDAVTasks`.
async fn cal_get_object(ctx: &Ctx<'_>, backend: &CalBackend, uri: &str) -> DavResult<Option<DavResource>> {
    Ok(match backend {
        CalBackend::Dates(vault) => {
            cal_dav_dates_get_object(ctx.db, vault, uri).await.ok().flatten().map(DavResource::ContactImportantDate)
        }
        CalBackend::Tasks(vault) => {
            cal_dav_tasks_get_object(ctx.db, vault, uri).await.ok().flatten().map(DavResource::ContactTask)
        }
    })
}

/// `$this->vault->contacts`.
async fn vault_contacts(ctx: &Ctx<'_>, vault: &vaults::Model) -> DavResult<Vec<contacts::Model>> {
    Ok(contacts::Entity::live().filter(contacts::Column::VaultId.eq(vault.id)).all(ctx.db).await?)
}

/// `getObjects($collectionId)` of `CalDAVDates` / `CalDAVTasks`.
async fn cal_get_objects(ctx: &Ctx<'_>, backend: &CalBackend) -> DavResult<Vec<DavResource>> {
    let mut result = Vec::new();
    for contact in vault_contacts(ctx, backend.vault()).await? {
        match backend {
            CalBackend::Dates(_) => {
                let dates = contact_important_dates::Entity::live()
                    .filter(contact_important_dates::Column::ContactId.eq(contact.id))
                    .all(ctx.db)
                    .await?;
                result.extend(dates.into_iter().map(DavResource::ContactImportantDate));
            }
            CalBackend::Tasks(_) => {
                let tasks = contact_tasks::Entity::live()
                    .filter(contact_tasks::Column::ContactId.eq(contact.id))
                    .all(ctx.db)
                    .await?;
                result.extend(tasks.into_iter().map(DavResource::ContactTask));
            }
        }
    }
    Ok(result)
}

/// `getDeletedObjects($collectionId)` of `CalDAVDates` / `CalDAVTasks`.
async fn cal_get_deleted_objects(ctx: &Ctx<'_>, backend: &CalBackend) -> DavResult<Vec<DavResource>> {
    let mut result = Vec::new();
    for contact in vault_contacts(ctx, backend.vault()).await? {
        match backend {
            CalBackend::Dates(_) => {
                let dates = contact_important_dates::Entity::find()
                    .filter(contact_important_dates::Column::ContactId.eq(contact.id))
                    .filter(contact_important_dates::Column::DeletedAt.is_not_null())
                    .all(ctx.db)
                    .await?;
                result.extend(dates.into_iter().map(DavResource::ContactImportantDate));
            }
            CalBackend::Tasks(_) => {
                let tasks = contact_tasks::Entity::find()
                    .filter(contact_tasks::Column::ContactId.eq(contact.id))
                    .filter(contact_tasks::Column::DeletedAt.is_not_null())
                    .all(ctx.db)
                    .await?;
                result.extend(tasks.into_iter().map(DavResource::ContactTask));
            }
        }
    }
    Ok(result)
}

/// `deleteCalendarObject($objectUri)` of `CalDAVDates` / `CalDAVTasks`.
async fn cal_delete_calendar_object(ctx: &Ctx<'_>, backend: &CalBackend, object_uri: &str) -> DavResult<()> {
    let obj = cal_get_object(ctx, backend, object_uri).await?;

    match obj {
        Some(DavResource::ContactImportantDate(important_date)) => {
            DestroyContactImportantDate {
                account_id: ctx.user.account_id,
                author_id: ctx.user.id,
                vault_id: backend.vault().id,
                contact_id: important_date.contact_id,
                contact_important_date_id: important_date.id,
            }
            .execute(ctx.db)
            .await?;
        }
        Some(DavResource::ContactTask(task)) => {
            DestroyContactTask {
                account_id: ctx.user.account_id,
                author_id: ctx.user.id,
                vault_id: backend.vault().id,
                contact_id: task.contact_id,
                contact_task_id: task.id,
            }
            .execute(ctx.db)
            .await?;
        }
        _ => {}
    }
    Ok(())
}

/// `lastModified($entry)` of `CalDAVDates` (`VEVENT`) / `CalDAVTasks` (`VTODO`).
fn last_modified(backend: &CalBackend, entry: &str) -> DavResult<Option<NaiveDateTime>> {
    let vcalendar = ReadVObject { entry: entry.to_owned() }.execute()?;

    let Some(vcalendar) = vcalendar else {
        return Ok(None);
    };

    let component = match backend {
        CalBackend::Dates(_) => "VEVENT",
        CalBackend::Tasks(_) => "VTODO",
    };
    let value = vcalendar.component(component).and_then(|c| c.property("LAST-MODIFIED")).map(ToString::to_string);

    Ok(Some(carbon_parse(value)?))
}
