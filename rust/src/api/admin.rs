//! `/admin/accounts`: the instance administrator's dashboard. Each account
//! is separate (its own users, vaults and settings). Sign-up closes after
//! the first run, so other accounts are created here, each with its first
//! user.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::auth::PasswordConfirmation;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::settings::create_account::CreateAccount;
use crate::domains::settings::manage_users::{normalize_username, validate_username};
use crate::entities::{accounts, contacts, files, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::http::access;
use crate::http::auth::{Auth, require_password_confirmation};
use crate::http::json::Json;
use crate::http::session::Session;
use crate::http::state::AppState;
use crate::validate::{Validator, clean};

#[derive(Serialize, ToSchema)]
pub struct AdminAccountUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_account_administrator: bool,
    pub is_instance_administrator: bool,
}

#[derive(Serialize, ToSchema)]
pub struct AdminAccount {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    /// By creation date.
    pub users: Vec<AdminAccountUser>,
    pub vault_count: u64,
    /// Contacts that aren't deleted, in all the account's vaults.
    pub contact_count: u64,
    /// Bytes of the files (photos, documents, avatars) of the account's vaults.
    pub file_size: u64,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub created_at: Option<Timestamp>,
}

#[derive(Serialize, ToSchema)]
pub struct AdminAccountList {
    pub data: Vec<AdminAccount>,
}

/// The accounts with `ids` (every account when `None`), oldest first.
async fn load(state: &AppState, ids: Option<Vec<Uuid>>) -> ApiResult<Vec<AdminAccount>> {
    let db = &state.db;
    let mut select =
        accounts::Entity::find().order_by_asc(accounts::Column::CreatedAt).order_by_asc(accounts::Column::Id);
    if let Some(ids) = &ids {
        select = select.filter(accounts::Column::Id.is_in(ids.clone()));
    }
    let accounts = select.all(db).await?;
    let account_ids: Vec<Uuid> = accounts.iter().map(|a| a.id).collect();

    let mut users_by_account: HashMap<Uuid, Vec<AdminAccountUser>> = HashMap::new();
    let users = users::Entity::find()
        .filter(users::Column::AccountId.is_in(account_ids.clone()))
        .order_by_asc(users::Column::CreatedAt)
        .order_by_asc(users::Column::Username)
        .all(db)
        .await?;
    for u in users {
        users_by_account.entry(u.account_id).or_default().push(AdminAccountUser {
            id: u.id,
            username: u.username,
            first_name: u.first_name,
            last_name: u.last_name,
            is_account_administrator: u.is_account_administrator,
            is_instance_administrator: u.is_instance_administrator,
        });
    }

    let vaults: Vec<(Uuid, Uuid)> = vaults::Entity::find()
        .select_only()
        .column(vaults::Column::Id)
        .column(vaults::Column::AccountId)
        .filter(vaults::Column::AccountId.is_in(account_ids))
        .into_tuple()
        .all(db)
        .await?;
    let account_of: HashMap<Uuid, Uuid> = vaults.iter().copied().collect();
    let vault_ids: Vec<Uuid> = account_of.keys().copied().collect();

    let mut contact_counts: HashMap<Uuid, u64> = HashMap::new();
    let per_vault: Vec<(Uuid, i64)> = contacts::Entity::find()
        .select_only()
        .column(contacts::Column::VaultId)
        .column_as(contacts::Column::Id.count(), "n")
        .filter(contacts::Column::VaultId.is_in(vault_ids.clone()))
        .filter(contacts::Column::DeletedAt.is_null())
        .group_by(contacts::Column::VaultId)
        .into_tuple()
        .all(db)
        .await?;
    for (vault, n) in per_vault {
        *contact_counts.entry(account_of[&vault]).or_default() += u64::try_from(n).unwrap_or(0);
    }

    let mut file_sizes: HashMap<Uuid, u64> = HashMap::new();
    let sizes: Vec<(Uuid, i32)> = files::Entity::find()
        .select_only()
        .column(files::Column::VaultId)
        .column(files::Column::Size)
        .filter(files::Column::VaultId.is_in(vault_ids))
        .into_tuple()
        .all(db)
        .await?;
    for (vault, size) in sizes {
        *file_sizes.entry(account_of[&vault]).or_default() += u64::try_from(size).unwrap_or(0);
    }

    Ok(accounts
        .into_iter()
        .map(|a| AdminAccount {
            users: users_by_account.remove(&a.id).unwrap_or_default(),
            vault_count: vaults.iter().filter(|(_, account)| *account == a.id).count() as u64,
            contact_count: contact_counts.get(&a.id).copied().unwrap_or(0),
            file_size: file_sizes.get(&a.id).copied().unwrap_or(0),
            created_at: a.created_at,
            id: a.id,
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/admin/accounts",
    operation_id = "listAdminAccounts",
    tag = "Admin",
    summary = "The accounts of this install",
    responses((status = 200, description = "Oldest first.", body = AdminAccountList), (status = 403, description = "Not an instance administrator.")),
    extensions(("x-permission" = json!("instance:admin")), ("x-inventory" = json!(["X010"])), ("x-services" = json!([])))
)]
pub async fn list_admin_accounts(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<AdminAccountList>> {
    access::instance_administrator(&id)?;
    Ok(Json(AdminAccountList { data: load(&state, None).await? }))
}

#[derive(Deserialize, ToSchema)]
pub struct AdminAccountCreate {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    /// The first user's password; they can change it in their profile.
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
}

#[utoipa::path(
    post,
    path = "/admin/accounts",
    operation_id = "createAdminAccount",
    tag = "Admin",
    summary = "Create an account and its first user",
    description = "The first user administers the new account; they are not an instance administrator.",
    request_body = AdminAccountCreate,
    responses(
        (status = 201, description = "The new account.", body = AdminAccount),
        (status = 403, description = "Not an instance administrator."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("instance:admin")), ("x-inventory" = json!(["X010"])), ("x-services" = json!(["CreateAccount", "SetupAccount"])))
)]
pub async fn create_admin_account(
    State(state): State<AppState>,
    Auth(id): Auth,
    Json(body): Json<AdminAccountCreate>,
) -> ApiResult<(StatusCode, Json<AdminAccount>)> {
    access::instance_administrator(&id)?;
    let first_name = clean(body.first_name);
    let last_name = clean(body.last_name);
    let username = normalize_username(body.username.as_deref().unwrap_or_default());
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.required("last_name", &last_name);
    validate_username(&state.db, &mut v, &username, None).await?;
    if v.required("password", &body.password) {
        crate::password::validate(
            &mut v,
            "password",
            body.password.as_deref().unwrap_or_default(),
            state.config.is_production(),
            Some(body.password_confirmation.as_deref()),
        );
    }
    v.finish()?;

    let tx = state.db.begin().await?;
    let user = CreateAccount {
        first_name: first_name.unwrap(),
        last_name: last_name.unwrap(),
        username,
        password: body.password,
        is_instance_administrator: false,
    }
    .execute(&tx)
    .await?;
    tx.commit().await?;
    let account = load(&state, Some(vec![user.account_id])).await?.pop().ok_or_else(ApiError::not_found)?;
    Ok((StatusCode::CREATED, Json(account)))
}

#[utoipa::path(
    delete,
    path = "/admin/accounts/{account_id}",
    operation_id = "deleteAdminAccount",
    tag = "Admin",
    summary = "Delete an account with its users, vaults and files",
    params(("account_id" = uuid::Uuid, Path)),
    request_body = PasswordConfirmation,
    responses(
        (status = 204, description = "Deleted."),
        (status = 403, description = "Not an instance administrator, or `password_confirmation_required`."),
        (status = 404, description = "Not found."),
        (status = 409, description = "The caller's own account."),
        (status = 422, description = "Wrong password."),
    ),
    extensions(("x-permission" = json!("instance:admin")), ("x-inventory" = json!(["X010"])), ("x-services" = json!([])))
)]
pub async fn delete_admin_account(
    State(state): State<AppState>,
    Auth(id): Auth,
    session: Session,
    Path(account_id): Path<Uuid>,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    access::instance_administrator(&id)?;
    let account = accounts::Entity::find_by_id(account_id).one(&state.db).await?.ok_or_else(ApiError::not_found)?;
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    if account.id == id.user.account_id {
        return Err(ApiError::conflict("You can't delete your own account."));
    }
    let vault_ids: Vec<Uuid> = vaults::Entity::find()
        .filter(vaults::Column::AccountId.eq(account.id))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|v| v.id)
        .collect();
    accounts::Entity::delete_by_id(account.id).exec(&state.db).await?;
    for vault in vault_ids {
        state.storage.delete_vault(vault).await;
    }
    Ok(StatusCode::NO_CONTENT)
}
