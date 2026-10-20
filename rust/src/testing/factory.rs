//! Model factories (`database/factories`) and the `tests/TestCase.php`
//! helpers built on them. Values that the PHP draws from Faker are random
//! here too.

use std::sync::OnceLock;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait};

use crate::crypto::random_string;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::entities::{accounts, contacts, genders, user_vault, users, vaults};

/// The password of factory users.
pub const PASSWORD: &str = "password";

fn password_hash() -> String {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| crate::password::hash(PASSWORD)).clone()
}

fn name() -> String {
    format!("Name {}", random_string(6))
}

/// `Account::factory()`.
pub async fn account<C: ConnectionTrait>(db: &C) -> accounts::Model {
    accounts::ActiveModel { id: Set(Uuid::new()), storage_limit_in_mb: Set(10), ..Default::default() }
        .insert(db)
        .await
        .expect("account")
}

/// `User::factory()` in `account` (a new account when `None`).
pub async fn user<C: ConnectionTrait>(db: &C, account: Option<Uuid>) -> users::Model {
    let account_id = match account {
        Some(a) => a,
        None => self::account(db).await.id,
    };
    users::ActiveModel {
        id: Set(Uuid::new()),
        account_id: Set(account_id),
        first_name: Set(Some(name())),
        last_name: Set(Some(name())),
        email: Set(format!("{}@example.com", random_string(12).to_lowercase())),
        email_verified_at: Set(Some(crate::db::now())),
        password: Set(Some(password_hash())),
        remember_token: Set(Some(random_string(10))),
        is_account_administrator: Set(false),
        name_order: Set("%first_name% %last_name%".into()),
        date_format: Set("MMM DD, YYYY".into()),
        default_map_site: Set("google_maps".into()),
        help_shown: Set(true),
        timezone: Set(Some("UTC".into())),
        number_format: Set("locale".into()),
        distance_format: Set("mi".into()),
        locale: Set("en".into()),
        contact_sort_order: Set("last_updated".into()),
        is_instance_administrator: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("user")
}

/// `createUser()`.
pub async fn create_user<C: ConnectionTrait>(db: &C) -> users::Model {
    user(db, None).await
}

/// `createAdministrator()`: `User::factory()->administrator()`.
pub async fn create_administrator<C: ConnectionTrait>(db: &C) -> users::Model {
    let u = user(db, None).await;
    let mut a: users::ActiveModel = u.into();
    a.is_account_administrator = Set(true);
    a.update(db).await.expect("administrator")
}

/// `createAccount()`.
pub async fn create_account<C: ConnectionTrait>(db: &C) -> accounts::Model {
    account(db).await
}

/// `Vault::factory()` in `account_id`.
pub async fn vault<C: ConnectionTrait>(db: &C, account_id: Uuid) -> vaults::Model {
    vaults::ActiveModel {
        id: Set(Uuid::new()),
        account_id: Set(account_id),
        r#type: Set("personal".into()),
        name: Set(name()),
        description: Set(Some(format!("Description {}", random_string(8)))),
        default_activity_tab: Set("activity".into()),
        show_group_tab: Set(true),
        show_tasks_tab: Set(true),
        show_files_tab: Set(true),
        show_journal_tab: Set(true),
        show_companies_tab: Set(true),
        show_reports_tab: Set(true),
        show_calendar_tab: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("vault")
}

/// `createVault($account)`.
pub async fn create_vault<C: ConnectionTrait>(db: &C, account_id: Uuid) -> vaults::Model {
    vault(db, account_id).await
}

/// `Gender::factory()` in `account_id`.
pub async fn gender<C: ConnectionTrait>(db: &C, account_id: Uuid) -> genders::Model {
    const LIST: [&str; 5] = ["M", "F", "O", "U", "N"];
    genders::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(name())),
        r#type: Set(Some(LIST[rand::random_range(0..LIST.len())].into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("gender")
}

/// `Contact::factory()` in `vault` (with a gender of the vault's account).
pub async fn contact<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> contacts::Model {
    let g = gender(db, vault.account_id).await;
    contacts::ActiveModel {
        id: Set(Uuid::new()),
        vault_id: Set(vault.id),
        gender_id: Set(Some(g.id)),
        first_name: Set(Some("Regis".into())),
        last_name: Set(Some("Troyat".into())),
        can_be_deleted: Set(true),
        listed: Set(true),
        prefix: Set(Some("Dr.".into())),
        suffix: Set(Some("III".into())),
        show_quick_facts: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact")
}

/// `setPermissionInVault($user, $permission, $vault)`: membership with a
/// new contact for the user.
pub async fn set_permission_in_vault<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    permission: VaultPermission,
    vault: &vaults::Model,
) {
    let c = contact(db, vault).await;
    user_vault::ActiveModel {
        vault_id: Set(vault.id),
        user_id: Set(user.id),
        contact_id: Set(c.id),
        permission: Set(permission as i32),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("user_vault");
}

/// `createVaultUser($user, $permission)`.
pub async fn create_vault_user<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    permission: VaultPermission,
) -> vaults::Model {
    let v = vault(db, user.account_id).await;
    let mut a: vaults::ActiveModel = v.into();
    a.description = Set(Some(format!("Contacts for {}", user.email)));
    let v = a.update(db).await.expect("vault");
    set_permission_in_vault(db, user, permission, &v).await;
    v
}
