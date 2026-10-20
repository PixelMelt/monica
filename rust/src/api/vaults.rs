//! `/vaults`: vaults and their members.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use rand::seq::SliceRandom;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::schemas::{ContactSummary, UserSummary};
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::domains::vault::manage_vault::{
    CreateVault, DestroyVault, UpdateVault, UpdateVaultDashboardDefaultTab, UpdateVaultTabVisibility,
};
use crate::domains::vault::manage_vault_settings::{
    ChangeVaultAccess, GrantVaultAccessToUser, RemoveVaultAccess, UpdateVaultDefaultTemplate,
};
use crate::entities::{contacts, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::http::access;
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::state::AppState;
use crate::validate::{Validator, clean};

/// A vault, with the caller's permission and own contact in it.
#[derive(Serialize, ToSchema)]
pub struct Vault {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    #[schema(value_type = uuid::Uuid)]
    pub account_id: Uuid,
    /// `personal`, `family` or `community`.
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
    #[schema(value_type = Option<i64>)]
    pub default_template_id: Option<Id>,
    /// `activity`, `life_events` or `life_metrics`.
    pub default_activity_tab: String,
    pub show_group_tab: bool,
    pub show_tasks_tab: bool,
    pub show_files_tab: bool,
    pub show_journal_tab: bool,
    pub show_companies_tab: bool,
    pub show_reports_tab: bool,
    pub show_calendar_tab: bool,
    /// The caller's permission: 100 manager, 200 editor, 300 viewer.
    pub permission: i32,
    /// The caller's own contact in the vault.
    #[schema(value_type = uuid::Uuid)]
    pub user_contact_id: Uuid,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl Vault {
    fn new(v: vaults::Model, membership: &user_vault::Model) -> Self {
        Vault {
            id: v.id,
            account_id: v.account_id,
            r#type: v.r#type,
            name: v.name,
            description: v.description,
            default_template_id: v.default_template_id,
            default_activity_tab: v.default_activity_tab,
            show_group_tab: v.show_group_tab,
            show_tasks_tab: v.show_tasks_tab,
            show_files_tab: v.show_files_tab,
            show_journal_tab: v.show_journal_tab,
            show_companies_tab: v.show_companies_tab,
            show_reports_tab: v.show_reports_tab,
            show_calendar_tab: v.show_calendar_tab,
            permission: membership.permission,
            user_contact_id: membership.contact_id,
            created_at: v.created_at.unwrap_or_else(crate::db::now),
            updated_at: v.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct VaultListItem {
    #[serde(flatten)]
    pub vault: Vault,
    /// All contacts of the vault, unlisted ones included.
    pub contacts_count: u64,
    /// Up to 5 random contacts of the vault.
    pub contacts_sample: Vec<ContactSummary>,
}

#[derive(Serialize, ToSchema)]
pub struct VaultList {
    pub data: Vec<VaultListItem>,
}

async fn membership(state: &AppState, user: Uuid, vault: Uuid) -> ApiResult<user_vault::Model> {
    user_vault::Entity::find_by_id((vault, user)).one(&state.db).await?.ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    get,
    path = "/vaults",
    operation_id = "listVaults",
    tag = "Vaults",
    summary = "The caller's vaults",
    responses((status = 200, description = "The vaults, by name.", body = VaultList)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R012", "R332"])), ("x-services" = json!([])))
)]
pub async fn list_vaults(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<VaultList>> {
    let rows = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(id.user.id))
        .find_also_related(vaults::Entity)
        .all(&state.db)
        .await?;
    let mut data = Vec::new();
    for (m, v) in rows {
        let Some(v) = v.filter(|v| v.account_id == id.user.account_id) else { continue };
        let mut contacts = contacts::Entity::live().filter(contacts::Column::VaultId.eq(v.id)).all(&state.db).await?;
        let contacts_count = contacts.len() as u64;
        contacts.shuffle(&mut rand::rng());
        contacts.truncate(5);
        data.push(VaultListItem {
            vault: Vault::new(v, &m),
            contacts_count,
            contacts_sample: contacts.into_iter().map(Into::into).collect(),
        });
    }
    data.sort_by_key(|i| i.vault.name.to_lowercase());
    Ok(Json(VaultList { data }))
}

#[derive(Deserialize, ToSchema)]
pub struct VaultCreate {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[utoipa::path(
    post,
    path = "/vaults",
    operation_id = "createVault",
    tag = "Vaults",
    summary = "Create a vault",
    request_body = VaultCreate,
    responses((status = 201, description = "The vault.", body = Vault), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R014", "R333"])), ("x-services" = json!(["CreateVault", "CreateContactImportantDateType"])))
)]
pub async fn create_vault(
    State(state): State<AppState>,
    Auth(id): Auth,
    Json(body): Json<VaultCreate>,
) -> ApiResult<(StatusCode, Json<Vault>)> {
    let name = clean(body.name);
    let mut v = Validator::new();
    v.required("name", &name);
    v.finish()?;
    let tx = state.db.begin().await?;
    let vault = CreateVault {
        account_id: id.user.account_id,
        author_id: id.user.id,
        template_id: None,
        r#type: "personal".into(),
        name: name.unwrap(),
        description: clean(body.description),
    }
    .execute(&tx)
    .await?;
    tx.commit().await?;
    let m = membership(&state, id.user.id, vault.id).await?;
    Ok((StatusCode::CREATED, Json(Vault::new(vault, &m))))
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}",
    operation_id = "getVault",
    tag = "Vaults",
    summary = "A vault",
    params(("vault_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "The vault.", body = Vault), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("vault:view")), ("x-inventory" = json!(["R016", "R334"])), ("x-services" = json!([])))
)]
pub async fn get_vault(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<Json<Vault>> {
    let a = access::vault(&state, &id, vault_id, VaultPermission::View).await?;
    Ok(Json(Vault::new(a.vault, &a.membership)))
}

#[derive(Deserialize, ToSchema)]
pub struct VaultUpdate {
    pub name: Option<String>,
    #[serde(default, with = "::serde_with::rust::double_option")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    #[schema(value_type = Option<i64>)]
    pub default_template_id: Option<Id>,
    pub default_activity_tab: Option<String>,
    pub show_group_tab: Option<bool>,
    pub show_tasks_tab: Option<bool>,
    pub show_files_tab: Option<bool>,
    pub show_journal_tab: Option<bool>,
    pub show_companies_tab: Option<bool>,
    pub show_reports_tab: Option<bool>,
    pub show_calendar_tab: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/vaults/{vault_id}",
    operation_id = "updateVault",
    tag = "Vaults",
    summary = "Change a vault",
    params(("vault_id" = uuid::Uuid, Path)),
    request_body = VaultUpdate,
    responses((status = 200, description = "The vault.", body = Vault), (status = 422, description = "Validation failed.")),
    extensions(
        ("x-permission" = json!("vault:manage")),
        ("x-inventory" = json!(["R017", "R019", "R175", "R190", "R335"])),
        ("x-services" = json!(["UpdateVault", "UpdateVaultDefaultTemplate", "UpdateVaultTabVisibility", "UpdateVaultDashboardDefaultTab"])),
        ("x-field-permissions" = json!({"default_activity_tab": "vault:view"}))
    )
)]
pub async fn update_vault(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
    Json(body): Json<VaultUpdate>,
) -> ApiResult<Json<Vault>> {
    let tabs = [
        body.show_group_tab,
        body.show_tasks_tab,
        body.show_files_tab,
        body.show_journal_tab,
        body.show_companies_tab,
        body.show_reports_tab,
        body.show_calendar_tab,
    ];
    let manage_fields = body.name.is_some()
        || body.description.is_some()
        || body.default_template_id.is_some()
        || tabs.iter().any(Option::is_some);
    let needed = if manage_fields { VaultPermission::Manage } else { VaultPermission::View };
    let a = access::vault(&state, &id, vault_id, needed).await?;
    let (account_id, author_id) = (id.user.account_id, id.user.id);

    let tx = state.db.begin().await?;
    let mut vault = a.vault;
    if body.name.is_some() || body.description.is_some() {
        let name = match body.name {
            Some(n) => n.trim().to_owned(),
            None => vault.name.clone(),
        };
        let description = match body.description {
            Some(d) => clean(d),
            None => vault.description.clone(),
        };
        vault = UpdateVault { account_id, author_id, vault_id, name, description }.execute(&tx).await?;
    }
    if let Some(template_id) = body.default_template_id {
        vault = UpdateVaultDefaultTemplate { account_id, author_id, vault_id, template_id }.execute(&tx).await?;
    }
    if tabs.iter().any(Option::is_some) {
        vault = UpdateVaultTabVisibility {
            account_id,
            author_id,
            vault_id,
            show_group_tab: body.show_group_tab.unwrap_or(vault.show_group_tab),
            show_tasks_tab: body.show_tasks_tab.unwrap_or(vault.show_tasks_tab),
            show_files_tab: body.show_files_tab.unwrap_or(vault.show_files_tab),
            show_journal_tab: body.show_journal_tab.unwrap_or(vault.show_journal_tab),
            show_companies_tab: body.show_companies_tab.unwrap_or(vault.show_companies_tab),
            show_reports_tab: body.show_reports_tab.unwrap_or(vault.show_reports_tab),
            show_calendar_tab: body.show_calendar_tab.unwrap_or(vault.show_calendar_tab),
        }
        .execute(&tx)
        .await?;
    }
    if let Some(tab) = body.default_activity_tab {
        let mut v = Validator::new();
        v.one_of("default_activity_tab", Some(&tab), &["activity", "life_events", "life_metrics"]);
        v.finish()?;
        vault = UpdateVaultDashboardDefaultTab { account_id, author_id, vault_id, default_activity_tab: tab }
            .execute(&tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(Vault::new(vault, &a.membership)))
}

#[utoipa::path(
    delete,
    path = "/vaults/{vault_id}",
    operation_id = "deleteVault",
    tag = "Vaults",
    summary = "Delete a vault",
    params(("vault_id" = uuid::Uuid, Path)),
    responses((status = 204, description = "Deleted."), (status = 403, description = "Not a manager."), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R018", "R336"])), ("x-services" = json!(["DestroyVault"])))
)]
pub async fn delete_vault(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    let tx = state.db.begin().await?;
    DestroyVault { account_id: id.user.account_id, author_id: id.user.id, vault_id }.execute(&tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- members

#[derive(Serialize, ToSchema)]
pub struct VaultMember {
    pub user: UserSummary,
    /// 100 manager, 200 editor, 300 viewer.
    pub permission: i32,
}

#[derive(Serialize, ToSchema)]
pub struct VaultMemberList {
    pub data: Vec<VaultMember>,
}

#[derive(Serialize, ToSchema)]
pub struct VaultMemberCandidateList {
    pub data: Vec<UserSummary>,
}

async fn member(state: &AppState, vault_id: Uuid, user: &users::Model) -> ApiResult<VaultMember> {
    let m = membership(state, user.id, vault_id).await?;
    let contact = contacts::Entity::find_by_id(m.contact_id).one(&state.db).await?;
    Ok(VaultMember { user: UserSummary::new(user, contact), permission: m.permission })
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/members",
    operation_id = "listVaultMembers",
    tag = "Vaults",
    summary = "The vault's members",
    params(("vault_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "The members.", body = VaultMemberList)),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R173"])), ("x-services" = json!([])))
)]
pub async fn list_vault_members(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<Json<VaultMemberList>> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    let rows = user_vault::Entity::find()
        .filter(user_vault::Column::VaultId.eq(vault_id))
        .find_also_related(users::Entity)
        .all(&state.db)
        .await?;
    let mut data = Vec::new();
    for (m, u) in rows {
        let Some(u) = u else { continue };
        let contact = contacts::Entity::find_by_id(m.contact_id).one(&state.db).await?;
        data.push(VaultMember { user: UserSummary::new(&u, contact), permission: m.permission });
    }
    data.sort_by_key(|m| (m.user.first_name.clone().unwrap_or_default().to_lowercase(), m.user.id));
    Ok(Json(VaultMemberList { data }))
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/member-candidates",
    operation_id = "listVaultMemberCandidates",
    tag = "Vaults",
    summary = "Account users who could be added",
    params(("vault_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "Verified users of the account who aren't members.", body = VaultMemberCandidateList)),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R173"])), ("x-services" = json!([])))
)]
pub async fn list_vault_member_candidates(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
) -> ApiResult<Json<VaultMemberCandidateList>> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    let members: Vec<Uuid> = user_vault::Entity::find()
        .filter(user_vault::Column::VaultId.eq(vault_id))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|m| m.user_id)
        .collect();
    let users = users::Entity::find()
        .filter(users::Column::AccountId.eq(id.user.account_id))
        .filter(users::Column::EmailVerifiedAt.is_not_null())
        .all(&state.db)
        .await?;
    let data = users.iter().filter(|u| !members.contains(&u.id)).map(|u| UserSummary::new(u, None)).collect();
    Ok(Json(VaultMemberCandidateList { data }))
}

#[derive(Deserialize, ToSchema)]
pub struct VaultMemberCreate {
    #[schema(value_type = Option<uuid::Uuid>)]
    pub user_id: Option<Uuid>,
    pub permission: Option<i32>,
}

fn check_permission(v: &mut Validator, p: Option<i32>) {
    if let Some(p) = p
        && VaultPermission::from_value(p).is_none()
    {
        v.fail("permission", "in", &[]);
    }
}

#[utoipa::path(
    post,
    path = "/vaults/{vault_id}/members",
    operation_id = "addVaultMember",
    tag = "Vaults",
    summary = "Give an account user access to the vault",
    params(("vault_id" = uuid::Uuid, Path)),
    request_body = VaultMemberCreate,
    responses(
        (status = 201, description = "The new member.", body = VaultMember),
        (status = 409, description = "Already a member, or yourself."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R176"])), ("x-services" = json!(["GrantVaultAccessToUser", "ScheduleContactReminderForUser"])))
)]
pub async fn add_vault_member(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(vault_id): Path<Uuid>,
    Json(body): Json<VaultMemberCreate>,
) -> ApiResult<(StatusCode, Json<VaultMember>)> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    let mut v = Validator::new();
    v.required("user_id", &body.user_id);
    if v.required("permission", &body.permission) {
        check_permission(&mut v, body.permission);
    }
    v.finish()?;
    let tx = state.db.begin().await?;
    let user = GrantVaultAccessToUser {
        account_id: id.user.account_id,
        author_id: id.user.id,
        vault_id,
        user_id: body.user_id.unwrap(),
        permission: body.permission.unwrap(),
    }
    .execute(&tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(member(&state, vault_id, &user).await?)))
}

#[derive(Deserialize, ToSchema)]
pub struct VaultMemberUpdate {
    pub permission: Option<i32>,
}

#[utoipa::path(
    patch,
    path = "/vaults/{vault_id}/members/{user_id}",
    operation_id = "updateVaultMember",
    tag = "Vaults",
    summary = "Change a member's permission",
    params(("vault_id" = uuid::Uuid, Path), ("user_id" = uuid::Uuid, Path)),
    request_body = VaultMemberUpdate,
    responses(
        (status = 200, description = "The member.", body = VaultMember),
        (status = 409, description = "Your own permission."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R177"])), ("x-services" = json!(["ChangeVaultAccess"])))
)]
pub async fn update_vault_member(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<VaultMemberUpdate>,
) -> ApiResult<Json<VaultMember>> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    membership(&state, user_id, vault_id).await?;
    let mut v = Validator::new();
    if v.required("permission", &body.permission) {
        check_permission(&mut v, body.permission);
    }
    v.finish()?;
    let user = ChangeVaultAccess {
        account_id: id.user.account_id,
        author_id: id.user.id,
        vault_id,
        user_id,
        permission: body.permission.unwrap(),
    }
    .execute(&state.db)
    .await?;
    Ok(Json(member(&state, vault_id, &user).await?))
}

#[utoipa::path(
    delete,
    path = "/vaults/{vault_id}/members/{user_id}",
    operation_id = "removeVaultMember",
    tag = "Vaults",
    summary = "Remove a member",
    params(("vault_id" = uuid::Uuid, Path), ("user_id" = uuid::Uuid, Path)),
    responses((status = 204, description = "Removed."), (status = 409, description = "Yourself.")),
    extensions(("x-permission" = json!("vault:manage")), ("x-inventory" = json!(["R178"])), ("x-services" = json!(["RemoveVaultAccess"])))
)]
pub async fn remove_vault_member(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    access::vault(&state, &id, vault_id, VaultPermission::Manage).await?;
    membership(&state, user_id, vault_id).await?;
    let tx = state.db.begin().await?;
    RemoveVaultAccess { account_id: id.user.account_id, author_id: id.user.id, vault_id, user_id }.execute(&tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
