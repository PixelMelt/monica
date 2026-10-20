//! `app/Domains/Contact/DavClient`: the CardDAV client that synchronizes a
//! vault with a remote address book (I003), the model helpers of
//! `app/Models/AddressBookSubscription.php`, and the console commands
//! `NewAddressBookSubscription` (K003) and `UpdateAddressBookSubscription`
//! (K007). Queued jobs and batches run synchronously.
//!
//! `Log::channel('database')` goes to `tracing`, as in
//! [`crate::domains::contact::dav`]; `Log::shareContext()` is a span with the
//! `addressbook_subscription_id` field.
//!
//! The client calls the CardDAV backend of the DAV server
//! (`CardDAVBackend`); the parts it needs are here, as private stand-ins.

pub mod dav;
pub mod jobs;
pub mod services;
pub mod utils;

use std::io::Write as _;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use self::dav::DavClient;
use self::jobs::SynchronizeAddressBooks;
use self::services::CreateAddressBookSubscription;
use crate::config::Config;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::domains::contact::dav::VCardResource;
use crate::domains::contact::dav::services::{ExportVCard, GetEtag, ReadVObject};
use crate::domains::contact::manage_contact::DestroyContact;
use crate::domains::contact::manage_groups::DestroyGroup;
use crate::entities::{addressbook_subscriptions, contacts, groups, sync_tokens, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::http::state::AppState;

/// `AddressBookSubscription::WAY_PUSH`: allow push changes to the distant server.
pub const WAY_PUSH: i64 = 0x1;

/// `AddressBookSubscription::WAY_GET`: allow get changes from the distant server.
pub const WAY_GET: i64 = 0x2;

/// `AddressBookSubscription::WAY_BOTH`: allow both push and get changes.
pub const WAY_BOTH: i64 = WAY_GET | WAY_PUSH;

/// The exceptions the client code throws and catches.
#[derive(Debug)]
pub enum Exception {
    /// `Illuminate\Http\Client\RequestException`: a 4xx or 5xx answer.
    Request(Box<dav::Response>),
    /// `Illuminate\Http\Client\ConnectionException`.
    Connection(String),
    /// `DavClientException`.
    DavClient(String),
    /// `DavServerNotCompliantException` (a `DavClientException`).
    DavServerNotCompliant(String),
    /// Any other exception or error of the client code (unreadable XML, a
    /// PHP `TypeError`).
    Error(String),
    /// An exception of a service or of the database.
    Api(ApiError),
}

impl Exception {
    /// `$e->getMessage()`.
    pub fn message(&self) -> String {
        match self {
            Exception::Request(response) => {
                let message = format!("HTTP request returned status code {}", response.status);
                let summary: String = response.body.chars().take(120).collect();
                if summary.is_empty() {
                    message
                } else if response.body.chars().count() > 120 {
                    format!("{message}:\n{summary} (truncated...)\n")
                } else {
                    format!("{message}:\n{summary}\n")
                }
            }
            Exception::Connection(m) | Exception::DavClient(m) | Exception::DavServerNotCompliant(m) => m.clone(),
            Exception::Error(m) => m.clone(),
            Exception::Api(e) => e
                .errors
                .first()
                .map(|f| f.message.clone())
                .or_else(|| e.detail.clone())
                .unwrap_or_else(|| e.title.clone()),
        }
    }
}

impl From<ApiError> for Exception {
    fn from(err: ApiError) -> Self {
        Exception::Api(err)
    }
}

impl From<sea_orm::DbErr> for Exception {
    fn from(err: sea_orm::DbErr) -> Self {
        Exception::Api(err.into())
    }
}

/// `$subscription->isWayPush`.
fn is_way_push(subscription: &addressbook_subscriptions::Model) -> bool {
    (subscription.sync_way.0 & WAY_PUSH) == WAY_PUSH
}

/// `$subscription->isWayGet`.
fn is_way_get(subscription: &addressbook_subscriptions::Model) -> bool {
    (subscription.sync_way.0 & WAY_GET) == WAY_GET
}

/// `$subscription->password`: `decrypt($value, true)`.
fn password(config: &Config, subscription: &addressbook_subscriptions::Model) -> Result<String, Exception> {
    crate::crypto::decrypt(&config.key, &subscription.password)
        .ok_or_else(|| Exception::Error("The payload is invalid.".to_owned()))
}

/// `$subscription->getClient()`: a new client.
fn get_client(config: &Config, subscription: &addressbook_subscriptions::Model) -> Result<DavClient, Exception> {
    let mut client = DavClient::new(config);
    client.set_base_uri(&subscription.uri).set_credentials(&subscription.username, &password(config, subscription)?);
    Ok(client)
}

/// `HasCapability::hasCapability()`.
fn has_capability(subscription: &addressbook_subscriptions::Model, capability: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(&subscription.capabilities)
        .ok()
        .and_then(|c| c.get(capability).and_then(serde_json::Value::as_bool))
        .unwrap_or(false)
}

/// `$subscription->user`.
async fn user<C: ConnectionTrait>(db: &C, subscription: &addressbook_subscriptions::Model) -> ApiResult<users::Model> {
    users::Entity::find_by_id(subscription.user_id).one(db).await?.ok_or_else(ApiError::not_found)
}

/// `Log::shareContext(['addressbook_subscription_id' => …])`.
fn share_context(subscription: &addressbook_subscriptions::Model) -> tracing::Span {
    tracing::info_span!("addressbook_subscription", addressbook_subscription_id = %subscription.id)
}

/// PHP's `empty()` on a string.
fn php_empty(value: &str) -> bool {
    value.is_empty() || value == "0"
}

// ---- CardDAVBackend

/// What `CardDAVBackend::prepareCard()` returns.
#[derive(Debug, Clone, PartialEq)]
struct Card {
    contact_id: String,
    uri: String,
    carddata: String,
    etag: String,
    distant_etag: Option<String>,
}

/// What `CardDAVBackend::getChangesForAddressBook()` returns.
#[derive(Debug, Clone, Default, PartialEq)]
struct Changes {
    added: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
}

/// Stand-in for CardDAVBackend::backendId() until the server port lands.
fn backend_id(collection_id: Uuid) -> String {
    format!("contacts-{collection_id}")
}

/// Stand-in for CardDAVBackend::vaults() (`GetVaults`) until the server port lands.
async fn get_vaults<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Option<Uuid>,
    permission: VaultPermission,
) -> ApiResult<Vec<vaults::Model>> {
    let mut vaults = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::Permission.lte(permission as i32));

    if let Some(collection_id) = collection_id {
        vaults = vaults.filter(user_vault::Column::VaultId.eq(collection_id));
    }

    Ok(vaults.find_also_related(vaults::Entity).all(db).await?.into_iter().filter_map(|(_, v)| v).collect())
}

/// Stand-in for CardDAVBackend::getCurrentSyncToken() until the server port lands.
async fn get_current_sync_token<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<Option<sync_tokens::Model>> {
    Ok(sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(user.account_id))
        .filter(sync_tokens::Column::UserId.eq(user.id))
        .filter(sync_tokens::Column::Name.eq(backend_id(collection_id)))
        .order_by_desc(sync_tokens::Column::CreatedAt)
        .one(db)
        .await?)
}

/// Stand-in for CardDAVBackend::refreshSyncToken() until the server port lands.
async fn refresh_sync_token<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<sync_tokens::Model> {
    let token = get_current_sync_token(db, user, collection_id).await?;

    if let Some(token) = token
        && get_last_modified(db, user, collection_id).await?.is_none_or(|last| token.timestamp >= last)
    {
        return Ok(token);
    }

    create_sync_token_now(db, user, collection_id).await
}

/// Stand-in for CardDAVBackend::getSyncToken() until the server port lands.
async fn get_sync_token<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
    sync_token_id: i64,
) -> ApiResult<Option<sync_tokens::Model>> {
    Ok(sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(user.account_id))
        .filter(sync_tokens::Column::UserId.eq(user.id))
        .filter(sync_tokens::Column::Name.eq(backend_id(collection_id)))
        .filter(sync_tokens::Column::Id.eq(Id(sync_token_id)))
        .one(db)
        .await?)
}

/// Stand-in for CardDAVBackend::createSyncTokenNow() until the server port lands.
async fn create_sync_token_now<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<sync_tokens::Model> {
    Ok(sync_tokens::ActiveModel {
        account_id: Set(user.account_id),
        user_id: Set(user.id),
        name: Set(backend_id(collection_id)),
        timestamp: Set(crate::db::now()),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Stand-in for CardDAVBackend::getLastModified() until the server port lands.
async fn get_last_modified<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<Option<Timestamp>> {
    Ok(get_objects(db, user, collection_id).await?.iter().filter_map(updated_at).max())
}

/// Stand-in for CardDAVBackend::getChangesForAddressBook() (`getChanges()`) until the server port lands.
async fn get_changes_for_address_book<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    user: &users::Model,
    address_book_id: Uuid,
    sync_token: &str,
) -> ApiResult<Option<Changes>> {
    let mut timestamp = None;
    if !sync_token.is_empty() {
        let token = get_sync_token(db, user, address_book_id, sync_token.trim().parse::<i64>().unwrap_or(0)).await?;

        let Some(token) = token else {
            // syncToken is not recognized
            return Ok(None);
        };

        timestamp = Some(token.timestamp);
    }

    let objs = get_objects(db, user, address_book_id).await?;

    refresh_sync_token(db, user, address_book_id).await?;
    let added = get_added(&objs, timestamp);
    let modified = get_modified(db, config, user, &objs, timestamp).await?;
    let deleted = get_deleted(db, user, address_book_id, timestamp).await?;

    Ok(Some(Changes { added, modified, deleted }))
}

/// Stand-in for CardDAVBackend::getAdded() until the server port lands.
fn get_added(objs: &[VCardResource], timestamp: Option<Timestamp>) -> Vec<String> {
    objs.iter()
        .filter(|obj| timestamp.is_none() || created_at(obj).is_some_and(|c| Some(c) >= timestamp))
        .map(encode_uri)
        .collect()
}

/// Stand-in for CardDAVBackend::getModified() until the server port lands.
async fn get_modified<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    user: &users::Model,
    objs: &[VCardResource],
    timestamp: Option<Timestamp>,
) -> ApiResult<Vec<String>> {
    let mut modified = Vec::new();
    for obj in objs {
        let Some(timestamp) = timestamp else { continue };
        if updated_at(obj).is_some_and(|u| u > timestamp) && created_at(obj).is_none_or(|c| c < timestamp) {
            refresh_object(db, config, user, obj).await?;

            modified.push(encode_uri(obj));
        }
    }
    Ok(modified)
}

/// Stand-in for CardDAVBackend::getDeleted() until the server port lands.
async fn get_deleted<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
    timestamp: Option<Timestamp>,
) -> ApiResult<Vec<String>> {
    Ok(get_deleted_objects(db, user, collection_id)
        .await?
        .iter()
        .filter(|obj| timestamp.is_none() || deleted_at(obj).is_some_and(|d| Some(d) >= timestamp))
        .map(encode_uri)
        .collect())
}

/// Stand-in for CardDAVBackend::encodeUri() until the server port lands.
fn encode_uri(obj: &VCardResource) -> String {
    let id = match obj {
        VCardResource::Contact(c) => c.distant_uuid.as_deref().map(str_after_urn).unwrap_or_else(|| c.id.to_string()),
        VCardResource::Group(g) => g
            .distant_uuid
            .as_deref()
            .map(str_after_urn)
            .or_else(|| g.uuid.map(|u| u.to_string()))
            .unwrap_or_else(|| g.id.to_string()),
    };

    urlencode(&format!("{id}.vcf"))
}

/// `Str::of($value)->after('urn:uuid:')`.
fn str_after_urn(value: &str) -> String {
    value.split_once("urn:uuid:").map_or(value, |(_, after)| after).to_owned()
}

/// PHP's `urlencode()`.
fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "+".to_owned(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Stand-in for CardDAVBackend::getUuid() (`decodeUri()`) until the server port lands.
fn get_uuid(uri: &str) -> String {
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

/// Stand-in for CardDAVBackend::getObject() until the server port lands.
async fn get_object<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
    uri: &str,
) -> Option<VCardResource> {
    // Object not found
    get_object_uuid(db, user, collection_id, &get_uuid(uri)).await.ok().flatten()
}

/// Stand-in for CardDAVBackend::getObjectUuid() until the server port lands.
async fn get_object_uuid<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
    uuid: &str,
) -> ApiResult<Option<VCardResource>> {
    let vault = get_vaults(db, user, Some(collection_id), VaultPermission::View).await?.into_iter().next();

    let Some(vault) = vault else {
        return Err(ApiError::forbidden());
    };

    if let Some(contact) = contacts::Entity::live()
        .filter(contacts::Column::VaultId.eq(vault.id))
        .filter(contacts::Column::DistantUuid.eq(uuid))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Contact(Box::new(contact))));
    }
    if let Some(group) = groups::Entity::live()
        .filter(groups::Column::VaultId.eq(vault.id))
        .filter(groups::Column::DistantUuid.eq(uuid))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Group(Box::new(group))));
    }
    let Ok(id) = uuid.parse::<Uuid>() else {
        return Ok(None);
    };
    if let Some(contact) = contacts::Entity::live()
        .filter(contacts::Column::VaultId.eq(vault.id))
        .filter(contacts::Column::Id.eq(id))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Contact(Box::new(contact))));
    }
    Ok(groups::Entity::live()
        .filter(groups::Column::VaultId.eq(vault.id))
        .filter(groups::Column::Uuid.eq(id))
        .one(db)
        .await?
        .map(|g| VCardResource::Group(Box::new(g))))
}

/// Stand-in for CardDAVBackend::getObjects() until the server port lands.
async fn get_objects<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<Vec<VCardResource>> {
    let vaults = get_vaults(db, user, Some(collection_id), VaultPermission::View).await?;

    let mut result = Vec::new();
    for vault in &vaults {
        for contact in contacts::Entity::live()
            .filter(contacts::Column::VaultId.eq(vault.id))
            .filter(contacts::Column::Listed.eq(true))
            .all(db)
            .await?
        {
            result.push(VCardResource::Contact(Box::new(contact)));
        }
    }

    for vault in &vaults {
        for group in groups::Entity::live().filter(groups::Column::VaultId.eq(vault.id)).all(db).await? {
            result.push(VCardResource::Group(Box::new(group)));
        }
    }

    Ok(result)
}

/// Stand-in for CardDAVBackend::getDeletedObjects() until the server port lands.
async fn get_deleted_objects<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    collection_id: Uuid,
) -> ApiResult<Vec<VCardResource>> {
    let vaults = get_vaults(db, user, Some(collection_id), VaultPermission::View).await?;

    let mut result = Vec::new();
    for vault in &vaults {
        for contact in contacts::Entity::find()
            .filter(contacts::Column::VaultId.eq(vault.id))
            .filter(contacts::Column::DeletedAt.is_not_null())
            .all(db)
            .await?
        {
            result.push(VCardResource::Contact(Box::new(contact)));
        }
    }

    for vault in &vaults {
        for group in groups::Entity::find()
            .filter(groups::Column::VaultId.eq(vault.id))
            .filter(groups::Column::DeletedAt.is_not_null())
            .all(db)
            .await?
        {
            result.push(VCardResource::Group(Box::new(group)));
        }
    }

    Ok(result)
}

/// Stand-in for CardDAVBackend::prepareCard() until the server port lands.
async fn prepare_card<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    user: &users::Model,
    resource: &VCardResource,
) -> ApiResult<Card> {
    let mut carddata = resource.vcard().map(str::to_owned);
    let result = async {
        let mut timestamp = None;
        if let Some(data) = carddata.as_deref().filter(|c| !php_empty(c)) {
            timestamp = rev(data)?;
        }

        let stale = match (timestamp, updated_at(resource)) {
            (None, _) => true,
            (Some(timestamp), Some(updated_at)) => timestamp < updated_at.0,
            (Some(_), None) => false,
        };
        if carddata.as_deref().is_none_or(php_empty) || stale {
            carddata = Some(refresh_object(db, config, user, resource).await?);
        }

        let resource = refresh(db, resource).await?;
        let etag = GetEtag {
            account_id: user.account_id,
            author_id: user.id,
            vault_id: resource.vault_id(),
            vcard: Some(resource.clone()),
            vcalendar: None,
        }
        .execute(db)
        .await?;

        Ok(Card {
            contact_id: match &resource {
                VCardResource::Contact(c) => c.id.to_string(),
                VCardResource::Group(g) => g.id.to_string(),
            },
            uri: encode_uri(&resource),
            carddata: carddata.clone().unwrap_or_default(),
            etag,
            distant_etag: resource.distant_etag().map(str::to_owned),
        })
    }
    .await;

    result.inspect_err(|e: &ApiError| {
        tracing::error!(carddata, "CardDAVBackend prepareCard: {}", e.title);
    })
}

/// Stand-in for CardDAVBackend::rev() until the server port lands.
fn rev(entry: &str) -> ApiResult<Option<chrono::NaiveDateTime>> {
    let vcard = ReadVObject { entry: entry.to_owned() }.execute()?;

    let Some(vcard) = vcard else {
        return Ok(None);
    };

    carbon_parse(vcard.property("REV").map(ToString::to_string)).map(Some)
}

/// `Carbon::parse($value)` for the `REV` of a vCard (now when there is none).
fn carbon_parse(value: Option<String>) -> ApiResult<chrono::NaiveDateTime> {
    let Some(value) = value else {
        return Ok(crate::db::now().0);
    };
    chrono::NaiveDateTime::parse_from_str(&value, "%Y%m%dT%H%M%SZ")
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&value).map(|d| d.naive_utc()))
        .map_err(|_| ApiError::internal(format!("Could not parse '{value}': Failed to parse time string")))
}

/// Stand-in for CardDAVBackend::refreshObject() until the server port lands.
async fn refresh_object<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    user: &users::Model,
    obj: &VCardResource,
) -> ApiResult<String> {
    let (contact_id, group_id) = match obj {
        VCardResource::Contact(c) => (Some(c.id), None),
        VCardResource::Group(g) => (None, Some(g.id)),
    };

    let vcard =
        ExportVCard { account_id: user.account_id, author_id: user.id, vault_id: obj.vault_id(), contact_id, group_id }
            .execute(db, config)
            .await?;

    Ok(vcard.serialize())
}

/// Stand-in for CardDAVBackend::getCard() until the server port lands.
async fn get_card<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    user: &users::Model,
    address_book_id: Uuid,
    card_uri: &str,
) -> ApiResult<Option<Card>> {
    let card = get_object(db, user, address_book_id, card_uri).await;

    match card {
        None => Ok(None),
        Some(card) => prepare_card(db, config, user, &card).await.map(Some),
    }
}

/// Stand-in for CardDAVBackend::deleteCard() until the server port lands.
async fn delete_card<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    address_book_id: Uuid,
    card_uri: &str,
) -> ApiResult<bool> {
    let obj = get_object(db, user, address_book_id, card_uri).await;

    match obj {
        Some(VCardResource::Contact(obj)) => {
            DestroyContact {
                account_id: user.account_id,
                author_id: user.id,
                vault_id: obj.vault_id,
                contact_id: obj.id,
            }
            .execute(db)
            .await?;

            Ok(true)
        }
        Some(VCardResource::Group(obj)) => {
            DestroyGroup { account_id: user.account_id, author_id: user.id, vault_id: obj.vault_id, group_id: obj.id }
                .execute(db)
                .await?;

            Ok(true)
        }
        None => Ok(false),
    }
}

/// `$resource->refresh()`.
async fn refresh<C: ConnectionTrait>(db: &C, resource: &VCardResource) -> ApiResult<VCardResource> {
    Ok(match resource {
        VCardResource::Contact(c) => VCardResource::Contact(Box::new(
            contacts::Entity::find_by_id(c.id).one(db).await?.ok_or_else(ApiError::not_found)?,
        )),
        VCardResource::Group(g) => VCardResource::Group(Box::new(
            groups::Entity::find_by_id(g.id).one(db).await?.ok_or_else(ApiError::not_found)?,
        )),
    })
}

/// `$obj->created_at`.
fn created_at(obj: &VCardResource) -> Option<Timestamp> {
    match obj {
        VCardResource::Contact(c) => c.created_at,
        VCardResource::Group(g) => g.created_at,
    }
}

/// `$obj->updated_at`.
fn updated_at(obj: &VCardResource) -> Option<Timestamp> {
    match obj {
        VCardResource::Contact(c) => c.updated_at,
        VCardResource::Group(g) => g.updated_at,
    }
}

/// `$obj->deleted_at`.
fn deleted_at(obj: &VCardResource) -> Option<Timestamp> {
    match obj {
        VCardResource::Contact(c) => c.deleted_at,
        VCardResource::Group(g) => g.deleted_at,
    }
}

// ---- Console commands

/// The options of `monica:newaddressbooksubscription`.
#[derive(Debug, Clone, Default)]
pub struct NewAddressBookSubscriptionOptions {
    /// `--email`: Monica account to add subscription to.
    pub email: Option<String>,
    /// `--vaultId`: Id of the vault to add subscription to.
    pub vault_id: Option<String>,
    /// `--url`: CardDAV url of the address book.
    pub url: Option<String>,
    /// `--login`: Login.
    pub login: Option<String>,
    /// `--password`: Password of the account.
    pub password: Option<String>,
    /// `--pushonly`: Set only push way.
    pub pushonly: bool,
    /// `--getonly`: Set only get way.
    pub getonly: bool,
}

/// `NewAddressBookSubscription::handle()` (K003, `monica:newaddressbooksubscription`):
/// add a new dav subscription. Returns the exit code.
pub async fn new_address_book_subscription(state: &AppState, options: NewAddressBookSubscriptionOptions) -> i32 {
    let db = &state.db;
    let config = &state.config;
    let Some(user) = command_user(db, options.email.as_deref()).await else {
        return 1;
    };
    let Some(vault) = command_vault(db, options.vault_id.as_deref()).await else {
        return 2;
    };

    if user.account_id != vault.account_id {
        eprintln!("Vault does not belong to this account");

        return 3;
    }

    let pushonly = options.pushonly;
    let getonly = options.getonly;
    if pushonly && getonly {
        eprintln!("Cannot set both pushonly and getonly");

        return 4;
    }

    let url = options.url.unwrap_or_else(|| ask("CardDAV url of the address book"));
    let login = options.login.unwrap_or_else(|| ask("Login name"));
    let password = options.password.unwrap_or_else(|| ask("User password"));

    let result = async {
        let subscription = CreateAddressBookSubscription {
            account_id: user.account_id,
            vault_id: vault.id,
            author_id: user.id,
            base_uri: url,
            username: login,
            password,
        }
        .execute(db, config)
        .await?;

        if !pushonly && !getonly {
            return Ok(subscription);
        }
        let mut am: addressbook_subscriptions::ActiveModel = subscription.into();
        am.sync_way = Set(if pushonly { WAY_PUSH } else { WAY_GET }.into());
        Ok::<_, Exception>(am.update(db).await?)
    }
    .await;
    let subscription = match result {
        Ok(subscription) => subscription,
        Err(e) => {
            eprintln!("Could not add subscription");
            eprintln!("{}", e.message());

            return -1;
        }
    };

    println!("Subscription added: {}", subscription.id);
    if let Err(e) = (SynchronizeAddressBooks { subscription, force: true }).handle(db, config).await {
        tracing::error!("SynchronizeAddressBooks: {e}");
    }

    0
}

/// `NewAddressBookSubscription::user()`.
async fn command_user<C: ConnectionTrait>(db: &C, email: Option<&str>) -> Option<users::Model> {
    let Some(email) = email else {
        eprintln!("Please provide an email address");

        return None;
    };

    match users::Entity::find().filter(users::Column::Email.eq(email)).one(db).await {
        Ok(Some(user)) => Some(user),
        _ => {
            eprintln!("Could not find user");

            None
        }
    }
}

/// `NewAddressBookSubscription::vault()`.
async fn command_vault<C: ConnectionTrait>(db: &C, vault_id: Option<&str>) -> Option<vaults::Model> {
    let Some(vault_id) = vault_id else {
        eprintln!("Please provide an vaultId");

        return None;
    };

    let vault = match vault_id.parse::<Uuid>() {
        Ok(id) => vaults::Entity::find_by_id(id).one(db).await.ok().flatten(),
        Err(_) => None,
    };
    if vault.is_none() {
        eprintln!("Could not find vault");
    }
    vault
}

/// `$this->ask($question)` / `$this->secret($question)`: a line of the standard input.
fn ask(question: &str) -> String {
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    answer.trim_end_matches(['\r', '\n']).to_owned()
}

/// `UpdateAddressBookSubscription::handle()` (K007, `monica:updateaddressbooksubscription`):
/// synchronize the subscription `--subscriptionId`, fully with `--force`.
pub async fn update_address_book_subscription(state: &AppState, subscription_id: &str, force: bool) -> ApiResult<()> {
    let id = subscription_id.parse::<Uuid>().map_err(|_| ApiError::not_found())?;
    let subscription =
        addressbook_subscriptions::Entity::find_by_id(id).one(&state.db).await?.ok_or_else(ApiError::not_found)?;

    SynchronizeAddressBooks { subscription, force }.handle(&state.db, &state.config).await
}

/// `UpdateAddressBooks` (J002, hourly): synchronize the subscriptions that are due.
pub async fn update_address_books(state: &AppState) -> anyhow::Result<()> {
    jobs::UpdateAddressBooks.handle(&state.db, &state.config).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
