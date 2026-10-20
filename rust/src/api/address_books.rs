//! vCard export and address book subscriptions (the DAV client, I003):
//! the API side of `docs/rewrite/api-design.md` §11.
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::contact::dav::services::ExportVCard;
use crate::domains::contact::dav_client::jobs::SynchronizeAddressBooks;
use crate::domains::contact::dav_client::services::CreateAddressBookSubscription;
use crate::domains::contact::dav_client::{Exception, WAY_BOTH};
use crate::domains::contact::manage_reminders::format_contact_name;
use crate::entities::addressbook_subscriptions;
use crate::error::{ApiError, ApiResult};
use crate::http::access;
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::state::AppState;
use crate::validate::{Validator, clean};
use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/contacts/{contact_id}/vcard",
    operation_id = "getContactVCard",
    tag = "Contacts",
    summary = "Download the contact as a vCard",
    params(("vault_id" = uuid::Uuid, Path), ("contact_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "The vCard 4.0.", content_type = "text/vcard"), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`).")),
    extensions(("x-permission" = json!("vault:view")), ("x-inventory" = json!(["R045"])), ("x-services" = json!(["ExportVCard"])))
)]
pub async fn get_contact_v_card(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, contact_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::View).await?;
    let contact = crate::entities::contacts::Entity::live()
        .filter(crate::entities::contacts::Column::Id.eq(contact_id))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let vcard = ExportVCard {
        account_id: id.user.account_id,
        author_id: id.user.id,
        vault_id,
        contact_id: Some(contact.id),
        group_id: None,
    }
    .execute(&state.db, &state.config)
    .await?;
    let name = crate::text::slug(&format_contact_name(&id.user, &contact));

    let mut res = Response::new(Body::from(vcard.serialize()));
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/vcard"));
    if let Ok(disposition) = HeaderValue::from_str(&format!("attachment; filename=\"{name}.vcf\"")) {
        h.insert(header::CONTENT_DISPOSITION, disposition);
    }
    Ok(res)
}

/// A row of `addressbook_subscriptions`. The password is never returned.
#[derive(Serialize, ToSchema)]
pub struct AddressBookSubscription {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    /// The user the sync acts as (the creator).
    #[schema(value_type = uuid::Uuid)]
    pub user_id: Uuid,
    #[schema(value_type = uuid::Uuid)]
    pub vault_id: Uuid,
    /// The discovered address book URL.
    pub uri: String,
    pub username: String,
    pub active: bool,
    /// Bit mask; 1 push local changes, 2 get remote changes, 3 both.
    pub sync_way: i64,
    /// What the remote server supports, found at discovery.
    pub capabilities: AddressBookSubscriptionCapabilities,
    /// Minutes between automatic syncs.
    pub frequency: i64,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_synchronized_at: Option<Timestamp>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

#[derive(Serialize, ToSchema)]
pub struct AddressBookSubscriptionList {
    pub data: Vec<AddressBookSubscription>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddressBookSubscriptionCreate {
    /// The server or address book URL to discover from.
    pub base_uri: Option<String>,
    pub username: Option<String>,
    /// Stored encrypted.
    pub password: Option<String>,
    pub sync_way: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddressBookSubscriptionUpdate {
    pub username: Option<String>,
    pub password: Option<String>,
    pub active: Option<bool>,
    pub sync_way: Option<i64>,
    pub frequency: Option<i64>,
}

impl From<addressbook_subscriptions::Model> for AddressBookSubscription {
    fn from(s: addressbook_subscriptions::Model) -> Self {
        let capabilities = serde_json::from_str::<serde_json::Value>(&s.capabilities).unwrap_or_default();
        let flag = |name: &str| capabilities.get(name).and_then(serde_json::Value::as_bool);
        let address_data = capabilities.get("addressData").filter(|d| d.is_object()).map(|d| {
            let text = |name: &str| d.get(name).and_then(serde_json::Value::as_str).map(str::to_owned);
            AddressBookSubscriptionCapabilitiesAddressData {
                content_type: text("content-type"),
                version: text("version"),
            }
        });
        AddressBookSubscription {
            id: s.id,
            user_id: s.user_id,
            vault_id: s.vault_id,
            uri: s.uri,
            username: s.username,
            active: s.active,
            sync_way: s.sync_way.0,
            capabilities: AddressBookSubscriptionCapabilities {
                addressbook_multiget: flag("addressbookMultiget"),
                addressbook_query: flag("addressbookQuery"),
                sync_collection: flag("syncCollection"),
                address_data,
            },
            frequency: i64::from(s.frequency),
            last_synchronized_at: s.last_synchronized_at,
            created_at: s.created_at.unwrap_or_else(crate::db::now),
            updated_at: s.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

/// The subscription `subscription_id` of the vault.
async fn subscription(
    state: &AppState,
    vault_id: Uuid,
    subscription_id: Uuid,
) -> ApiResult<addressbook_subscriptions::Model> {
    addressbook_subscriptions::Entity::find_by_id(subscription_id)
        .filter(addressbook_subscriptions::Column::VaultId.eq(vault_id))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `sync_way`: 1, 2 or 3.
fn validate_sync_way(v: &mut Validator, sync_way: Option<i64>) {
    if sync_way.is_some_and(|w| !(1..=WAY_BOTH).contains(&w)) {
        v.fail("sync_way", "in", &[]);
    }
}

/// What the remote server supports, found at discovery.
#[derive(Serialize, ToSchema)]
pub struct AddressBookSubscriptionCapabilities {
    #[serde(rename = "addressbookMultiget")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addressbook_multiget: Option<bool>,
    #[serde(rename = "addressbookQuery")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addressbook_query: Option<bool>,
    #[serde(rename = "syncCollection")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_collection: Option<bool>,
    #[serde(rename = "addressData")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_data: Option<AddressBookSubscriptionCapabilitiesAddressData>,
}

#[derive(Serialize, ToSchema)]
pub struct AddressBookSubscriptionCapabilitiesAddressData {
    #[serde(rename = "content-type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct SyncVaultAddressBookSubscriptionBody {
    pub force: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/address-book-subscriptions",
    operation_id = "listVaultAddressBookSubscriptions",
    tag = "Vaults",
    summary = "List the vault's CardDAV address book subscriptions",
    params(("vault_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "Subscriptions, ordered by `created_at`.", body = AddressBookSubscriptionList), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`).")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn list_vault_address_book_subscriptions(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<Json<AddressBookSubscriptionList>> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    let rows = addressbook_subscriptions::Entity::find()
        .filter(addressbook_subscriptions::Column::VaultId.eq(vault_id))
        .order_by_asc(addressbook_subscriptions::Column::CreatedAt)
        .order_by_asc(addressbook_subscriptions::Column::Id)
        .all(&state.db)
        .await?;
    Ok(Json(AddressBookSubscriptionList { data: rows.into_iter().map(Into::into).collect() }))
}

#[utoipa::path(
    post,
    path = "/vaults/{vault_id}/address-book-subscriptions",
    operation_id = "createVaultAddressBookSubscription",
    tag = "Vaults",
    summary = "Subscribe the vault to a remote CardDAV address book",
    params(("vault_id" = uuid::Uuid, Path)),
    request_body = AddressBookSubscriptionCreate,
    responses((status = 201, description = "The created subscription.", body = AddressBookSubscription), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`)."), (status = 422, description = "`validation_failed` (with `errors`), `storage_limit_exceeded` or `vault_user_limit_reached`.")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["K003"])), ("x-services" = json!(["CreateAddressBookSubscription"])))
)]
pub async fn create_vault_address_book_subscription(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
    Json(body): Json<AddressBookSubscriptionCreate>,
) -> ApiResult<(StatusCode, Json<AddressBookSubscription>)> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    let base_uri = clean(body.base_uri);
    let username = clean(body.username);
    let password = body.password.filter(|p| !p.is_empty());
    let mut v = Validator::new();
    if v.required("base_uri", &base_uri) {
        v.max_len("base_uri", base_uri.as_deref(), 2096);
    }
    if v.required("username", &username) {
        v.max_len("username", username.as_deref(), 1024);
    }
    v.required("password", &password);
    validate_sync_way(&mut v, body.sync_way);
    v.finish()?;

    let created = CreateAddressBookSubscription {
        account_id: id.user.account_id,
        vault_id,
        author_id: id.user.id,
        base_uri: base_uri.unwrap_or_default(),
        username: username.unwrap_or_default(),
        password: password.unwrap_or_default(),
    }
    .execute(&state.db, &state.config)
    .await;
    let mut subscription = match created {
        Ok(subscription) => subscription,
        Err(Exception::Api(e)) => return Err(e),
        Err(e) => return Err(ApiError::field("base_uri", "discovery", e.message())),
    };

    if let Some(sync_way) = body.sync_way.filter(|w| *w != subscription.sync_way.0) {
        let mut am: addressbook_subscriptions::ActiveModel = subscription.into();
        am.sync_way = Set(sync_way.into());
        subscription = am.update(&state.db).await?;
    }

    SynchronizeAddressBooks { subscription: subscription.clone(), force: true }
        .handle(&state.db, &state.config)
        .await?;

    let subscription = self::subscription(&state, vault_id, subscription.id).await?;
    Ok((StatusCode::CREATED, Json(subscription.into())))
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/address-book-subscriptions/{subscription_id}",
    operation_id = "getVaultAddressBookSubscription",
    tag = "Vaults",
    summary = "Get an address book subscription",
    params(("vault_id" = uuid::Uuid, Path), ("subscription_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "The subscription.", body = AddressBookSubscription), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`).")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn get_vault_address_book_subscription(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, subscription_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<AddressBookSubscription>> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    Ok(Json(subscription(&state, vault_id, subscription_id).await?.into()))
}

#[utoipa::path(
    patch,
    path = "/vaults/{vault_id}/address-book-subscriptions/{subscription_id}",
    operation_id = "updateVaultAddressBookSubscription",
    tag = "Vaults",
    summary = "Change a subscription's credentials, direction, frequency or state",
    params(("vault_id" = uuid::Uuid, Path), ("subscription_id" = uuid::Uuid, Path)),
    request_body = AddressBookSubscriptionUpdate,
    responses((status = 200, description = "The updated subscription.", body = AddressBookSubscription), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`)."), (status = 422, description = "`validation_failed` (with `errors`), `storage_limit_exceeded` or `vault_user_limit_reached`.")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn update_vault_address_book_subscription(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, subscription_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AddressBookSubscriptionUpdate>,
) -> ApiResult<Json<AddressBookSubscription>> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    let current = subscription(&state, vault_id, subscription_id).await?;
    let username = clean(body.username.clone());
    let mut v = Validator::new();
    if body.username.is_some() && v.required("username", &username) {
        v.max_len("username", username.as_deref(), 1024);
    }
    if body.password.as_deref().is_some_and(str::is_empty) {
        v.fail("password", "required", &[]);
    }
    validate_sync_way(&mut v, body.sync_way);
    v.between("frequency", body.frequency, 1, 32767);
    v.finish()?;

    let mut am: addressbook_subscriptions::ActiveModel = current.clone().into();
    if let Some(username) = username {
        am.username.set_ne(username);
    }
    if let Some(password) = body.password {
        am.password = Set(crate::crypto::encrypt(&state.config.key, &password));
    }
    if let Some(active) = body.active {
        am.active.set_ne(active);
    }
    if let Some(sync_way) = body.sync_way {
        am.sync_way.set_ne(sync_way.into());
    }
    if let Some(frequency) = body.frequency {
        am.frequency.set_ne(i16::try_from(frequency).unwrap_or(i16::MAX));
    }
    let updated = if am.is_changed() { am.update(&state.db).await? } else { current };
    Ok(Json(updated.into()))
}

#[utoipa::path(
    delete,
    path = "/vaults/{vault_id}/address-book-subscriptions/{subscription_id}",
    operation_id = "deleteVaultAddressBookSubscription",
    tag = "Vaults",
    summary = "Delete a subscription",
    params(("vault_id" = uuid::Uuid, Path), ("subscription_id" = uuid::Uuid, Path)),
    responses((status = 204, description = "Done; no body."), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`).")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn delete_vault_address_book_subscription(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, subscription_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    let subscription = subscription(&state, vault_id, subscription_id).await?;
    addressbook_subscriptions::Entity::delete_by_id(subscription.id).exec(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/vaults/{vault_id}/address-book-subscriptions/{subscription_id}/sync",
    operation_id = "syncVaultAddressBookSubscription",
    tag = "Vaults",
    summary = "Queue a synchronization now",
    params(("vault_id" = uuid::Uuid, Path), ("subscription_id" = uuid::Uuid, Path)),
    request_body = SyncVaultAddressBookSubscriptionBody,
    responses((status = 202, description = "Queued; no body."), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`)."), (status = 422, description = "`validation_failed` (with `errors`), `storage_limit_exceeded` or `vault_user_limit_reached`.")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["K007"])), ("x-services" = json!([])))
)]
pub async fn sync_vault_address_book_subscription(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, subscription_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SyncVaultAddressBookSubscriptionBody>,
) -> ApiResult<StatusCode> {
    access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    let subscription = subscription(&state, vault_id, subscription_id).await?;
    SynchronizeAddressBooks { subscription, force: body.force.unwrap_or(false) }
        .handle(&state.db, &state.config)
        .await?;
    Ok(StatusCode::ACCEPTED)
}

/// The operations of this module.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_contact_v_card))
        .routes(routes!(list_vault_address_book_subscriptions, create_vault_address_book_subscription))
        .routes(routes!(
            get_vault_address_book_subscription,
            update_vault_address_book_subscription,
            delete_vault_address_book_subscription
        ))
        .routes(routes!(sync_vault_address_book_subscription))
}
