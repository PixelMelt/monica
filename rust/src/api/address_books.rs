//! vCard export and address book subscriptions (the DAV client, I003):
//! the API side of `docs/rewrite/api-design.md` §11.
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::error::ApiResult;
use crate::http::access;
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::state::AppState;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::View).await?;
    todo!("getContactVCard")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("listVaultAddressBookSubscriptions")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("createVaultAddressBookSubscription")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("getVaultAddressBookSubscription")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("updateVaultAddressBookSubscription")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("deleteVaultAddressBookSubscription")
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
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::Manage).await?;
    todo!("syncVaultAddressBookSubscription")
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
