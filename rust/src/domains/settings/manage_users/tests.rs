//! `tests/Unit/Domains/Settings/ManageUsers/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::entities::contacts;
use crate::error::Code;
use crate::testing::{TestDb, factory};

async fn row(db: &TestDb, user: &users::Model) -> Option<users::Model> {
    users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap()
}

/// `User::factory()->create(['account_id' => $account_id, 'is_account_administrator' => true])`.
async fn administrator_in(db: &TestDb, account_id: Uuid) -> users::Model {
    let mut user: users::ActiveModel = factory::user(&db.conn, Some(account_id)).await.into();
    user.is_account_administrator = Set(true);
    user.update(&db.conn).await.unwrap()
}

// ---- DestroyUserTest

/// `Contact::factory()->create()`: a contact in a new vault of a new account.
async fn any_contact(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `$vault->users()->save($user, ['permission' => $permission, 'contact_id' => Contact::factory()->create()->id])`.
async fn save_in_vault(db: &TestDb, vault: &vaults::Model, user: &users::Model, permission: VaultPermission) {
    let contact = any_contact(db).await;
    user_vault::ActiveModel {
        vault_id: Set(vault.id),
        user_id: Set(user.id),
        contact_id: Set(contact.id),
        permission: Set(permission as i32),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap();
}

async fn destroy_user(
    db: &TestDb,
    account_id: Uuid,
    author: &users::Model,
    user: &users::Model,
    vaults: Option<[&vaults::Model; 3]>,
) -> ApiResult<()> {
    DestroyUser { account_id, author_id: author.id, user_id: user.id }.execute(&db.conn).await?;

    assert!(row(db, user).await.is_none());
    let [vault_manager, vault_editor, vault_viewer] = vaults.unwrap();
    let vault = |v: &vaults::Model| vaults::Entity::find_by_id(v.id).one(&db.conn);
    assert!(vault(vault_manager).await.unwrap().is_none());
    assert!(vault(vault_editor).await.unwrap().is_some());
    assert!(vault(vault_viewer).await.unwrap().is_some());

    for v in [vault_editor, vault_viewer] {
        let member =
            user_vault::Entity::find().filter(user_vault::Column::VaultId.eq(v.id)).one(&db.conn).await.unwrap();
        assert!(member.is_some());
    }
    Ok(())
}

#[tokio::test]
async fn it_deletes_another_user() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let user = factory::user(&db.conn, Some(author.account_id)).await;

    let vault_manager = factory::vault(&db.conn, author.account_id).await;
    save_in_vault(&db, &vault_manager, &user, VaultPermission::Manage).await;
    let vault_editor = factory::vault(&db.conn, author.account_id).await;
    save_in_vault(&db, &vault_editor, &user, VaultPermission::Edit).await;
    let other = factory::create_user(&db.conn).await;
    save_in_vault(&db, &vault_editor, &other, VaultPermission::Manage).await;
    let vault_viewer = factory::vault(&db.conn, author.account_id).await;
    save_in_vault(&db, &vault_viewer, &user, VaultPermission::View).await;
    let other = factory::create_user(&db.conn).await;
    save_in_vault(&db, &vault_viewer, &other, VaultPermission::Manage).await;

    destroy_user(&db, author.account_id, &author, &user, Some([&vault_manager, &vault_editor, &vault_viewer]))
        .await
        .unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_author_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let user = factory::user(&db.conn, Some(author.account_id)).await;
    assert_eq!(destroy_user(&db, account.id, &author, &user, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let author = factory::create_user(&db.conn).await;
    let user = factory::user(&db.conn, Some(author.account_id)).await;
    assert_eq!(destroy_user(&db, author.account_id, &author, &user, None).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_is_not_in_the_same_account_as_the_other() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let user = factory::create_user(&db.conn).await;
    assert_eq!(destroy_user(&db, author.account_id, &author, &user, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- GiveAdministratorPrivilegeTest

async fn give(db: &TestDb, account_id: Uuid, author: &users::Model, another_user: &users::Model) -> ApiResult<()> {
    GiveAdministratorPrivilege { account_id, author_id: author.id, user_id: another_user.id }.execute(&db.conn).await?;

    assert!(row(db, another_user).await.unwrap().is_account_administrator);
    Ok(())
}

#[tokio::test]
async fn it_gives_the_administrator_privilege_to_another_user() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(author.account_id)).await;
    give(&db, author.account_id, &author, &another_user).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn give_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(author.account_id)).await;
    assert_eq!(give(&db, account.id, &author, &another_user).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn give_fails_if_the_other_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let another_user = factory::create_user(&db.conn).await;
    assert_eq!(give(&db, author.account_id, &author, &another_user).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn give_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let author = factory::create_user(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(author.account_id)).await;
    assert_eq!(give(&db, author.account_id, &author, &another_user).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// give_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

// ---- CreateUser (in place of InviteUserTest)

async fn create(db: &TestDb, account_id: Uuid, author: &users::Model, username: &str) -> ApiResult<users::Model> {
    CreateUser {
        account_id,
        author_id: author.id,
        first_name: "Ann".into(),
        last_name: "Smith".into(),
        username: username.into(),
        password: "abcd".into(),
        is_administrator: true,
    }
    .execute(&db.conn)
    .await
}

#[tokio::test]
async fn it_creates_another_user() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let new_user = create(&db, author.account_id, &author, " Ann.Smith ").await.unwrap();

    let row = row(&db, &new_user).await.unwrap();
    assert_eq!(row.username, "ann.smith");
    assert_eq!(row.account_id, author.account_id);
    assert!(row.is_account_administrator);
    assert!(crate::password::verify("abcd", row.password.as_deref().unwrap()));
    assert!(!row.is_instance_administrator);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    assert_eq!(create(&db, account.id, &author, "ann").await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let author = factory::create_user(&db.conn).await;
    assert_eq!(create(&db, author.account_id, &author, "ann").await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

/// Usernames: unique ignoring case, no whitespace or `:`, at most 255
/// characters, required.
#[tokio::test]
async fn create_fails_for_a_taken_or_malformed_username() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let failure = |e: crate::error::ApiError| (e.code, e.errors[0].field.clone(), e.errors[0].code.clone());
    let taken = author.username.to_uppercase();
    for (username, rule) in [(taken.as_str(), "unique"), ("ann smith", "regex"), ("ann:x", "regex"), ("", "required")] {
        let err = create(&db, author.account_id, &author, username).await.unwrap_err();
        assert_eq!(failure(err), (Code::ValidationFailed, "username".into(), rule.into()), "{username:?}");
    }
    let long = "a".repeat(256);
    assert_eq!(failure(create(&db, author.account_id, &author, &long).await.unwrap_err()).2, "max");
    db.drop().await;
}

// ---- RemoveAdministratorPrivilegeTest

async fn remove(db: &TestDb, account_id: Uuid, author: &users::Model, another_user: &users::Model) -> ApiResult<()> {
    RemoveAdministratorPrivilege { account_id, author_id: author.id, user_id: another_user.id }
        .execute(&db.conn)
        .await?;

    assert!(!row(db, another_user).await.unwrap().is_account_administrator);
    Ok(())
}

#[tokio::test]
async fn it_removes_the_administrator_privilege_from_another_user() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let another_user = administrator_in(&db, author.account_id).await;
    remove(&db, author.account_id, &author, &another_user).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let another_user = administrator_in(&db, author.account_id).await;
    assert_eq!(remove(&db, account.id, &author, &another_user).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_the_other_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let another_user = factory::create_user(&db.conn).await;
    assert_eq!(remove(&db, author.account_id, &author, &another_user).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let author = factory::create_user(&db.conn).await;
    let another_user = administrator_in(&db, author.account_id).await;
    assert_eq!(remove(&db, author.account_id, &author, &another_user).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_the_two_users_are_the_same() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    assert_eq!(remove(&db, author.account_id, &author, &author).await.unwrap_err().code, Code::Conflict);
    db.drop().await;
}

// remove_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.
