//! `tests/Unit/Domains/Settings/ManageUsers/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::entities::contacts;
use crate::error::Code;
use crate::testing::{TestDb, factory};

fn mail() -> (Mailer, Config) {
    let config = Config::for_tests("", std::env::temp_dir());
    (Mailer::new(&config).expect("mailer"), config)
}

async fn row(db: &TestDb, user: &users::Model) -> Option<users::Model> {
    users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap()
}

/// `User::factory()->create(['account_id' => $account_id, 'is_account_administrator' => true])`.
async fn administrator_in(db: &TestDb, account_id: Uuid) -> users::Model {
    let mut user: users::ActiveModel = factory::user(&db.conn, Some(account_id)).await.into();
    user.is_account_administrator = Set(true);
    user.update(&db.conn).await.unwrap()
}

// ---- AcceptInvitationTest

/// `User::factory()->create(['invitation_code' => (string) Str::uuid()])`,
/// accepted when `accepted`.
async fn invited_user(db: &TestDb, accepted: bool) -> users::Model {
    let mut user: users::ActiveModel = factory::user(&db.conn, None).await.into();
    user.invitation_code = Set(Some(uuid::Uuid::new_v4().to_string()));
    if accepted {
        user.invitation_accepted_at = Set(Some(crate::db::now()));
    }
    user.update(&db.conn).await.unwrap()
}

async fn accept_invitation(db: &TestDb, user: &users::Model) -> ApiResult<()> {
    let (mailer, config) = mail();
    let before = crate::db::now();
    let user = AcceptInvitation {
        invitation_code: user.invitation_code.as_deref().unwrap().parse().unwrap(),
        first_name: "James".into(),
        last_name: "Bond".into(),
        password: "password".into(),
    }
    .execute(&db.conn, &mailer, &config)
    .await?;
    let after = crate::db::now();

    let row = row(db, &user).await.unwrap();
    assert!(!row.is_account_administrator);
    assert_eq!(row.first_name.as_deref(), Some("James"));
    assert_eq!(row.last_name.as_deref(), Some("Bond"));
    for at in [row.invitation_accepted_at.unwrap(), row.email_verified_at.unwrap()] {
        assert!(before <= at && at <= after);
    }

    let channel = user_notification_channels::Entity::find()
        .filter(user_notification_channels::Column::UserId.eq(user.id))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(channel.label.as_deref(), Some("Email address"));
    assert_eq!(channel.r#type, "email");
    assert_eq!(channel.content, row.email);
    assert!(channel.active);
    Ok(())
}

#[tokio::test]
async fn it_accepts_the_invitation_from_another_user() {
    let db = TestDb::migrated().await;
    let user = invited_user(&db, false).await;
    accept_invitation(&db, &user).await.unwrap();
    db.drop().await;
}

// accept_fails_if_user_doesnt_have_a_valid_invitation_code: `invitation_code`
// is a `Uuid`, so the code "123" can't be passed.

#[tokio::test]
async fn accept_fails_if_user_has_already_accepted_invitation() {
    let db = TestDb::migrated().await;
    let user = invited_user(&db, true).await;
    assert_eq!(accept_invitation(&db, &user).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn accept_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (mailer, config) = mail();
    let err = AcceptInvitation {
        invitation_code: Uuid::new(),
        first_name: String::new(),
        last_name: "Bond".into(),
        password: "password".into(),
    }
    .execute(&db.conn, &mailer, &config)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
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

// ---- InviteUserTest

async fn invite(db: &TestDb, account_id: Uuid, author: &users::Model) -> ApiResult<()> {
    let (mailer, config) = mail();
    let new_user =
        InviteUser { account_id, author_id: author.id, email: "admin@admin.com".into(), is_administrator: true }
            .execute(&db.conn, &mailer, &config)
            .await?;

    let row = row(db, &new_user).await.unwrap();
    assert_eq!(row.email, "admin@admin.com");
    assert!(row.is_account_administrator);

    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "admin@admin.com");
    assert!(sent[0].text.contains(&format!("http://localhost/invitation/{}", row.invitation_code.unwrap())));
    Ok(())
}

#[tokio::test]
async fn it_invites_another_user() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    invite(&db, author.account_id, &author).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn invite_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    assert_eq!(invite(&db, account.id, &author).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn invite_fails_if_email_is_already_taken() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let mut taken: users::ActiveModel = factory::create_user(&db.conn).await.into();
    taken.email = Set("admin@admin.com".into());
    taken.update(&db.conn).await.unwrap();
    assert_eq!(invite(&db, author.account_id, &author).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn invite_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let author = factory::create_user(&db.conn).await;
    assert_eq!(invite(&db, author.account_id, &author).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn invite_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let author = factory::create_administrator(&db.conn).await;
    let (mailer, config) = mail();
    let err = InviteUser {
        account_id: author.account_id,
        author_id: author.id,
        email: String::new(),
        is_administrator: true,
    }
    .execute(&db.conn, &mailer, &config)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
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
