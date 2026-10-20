//! `tests/Unit/Domains/Contact/ManageContact/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryOrder};

use super::*;
use crate::crypto::random_string;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Template::factory()` in `account_id`.
async fn template(db: &TestDb, account_id: Uuid) -> templates::Model {
    templates::ActiveModel { account_id: Set(account_id), name: Set(Some("business".into())), ..Default::default() }
        .insert(&db.conn)
        .await
        .expect("template")
}

/// `Pronoun::factory()` in `account_id`.
async fn pronoun(db: &TestDb, account_id: Uuid) -> pronouns::Model {
    pronouns::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("pronoun")
}

/// `File::factory()` in `vault_id`.
async fn file(db: &TestDb, vault_id: Uuid) -> files::Model {
    files::ActiveModel {
        vault_id: Set(vault_id),
        uuid: Set(Uuid::new().to_string()),
        original_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        cdn_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        name: Set(format!("Name {}", random_string(6))),
        mime_type: Set("avatar".into()),
        r#type: Set("avatar".into()),
        size: Set(rand::random_range(200 * 1024 * 1024..=500 * 1024 * 1024)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("file")
}

/// `Company::factory()` in `vault_id`.
async fn company(db: &TestDb, vault_id: Uuid) -> companies::Model {
    companies::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(format!("Name {}", random_string(6))),
        r#type: Set(Some("company".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("company")
}

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_for(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

/// `Vault::factory()->create()`.
async fn vault_elsewhere(db: &TestDb) -> vaults::Model {
    let account = factory::account(&db.conn).await;
    factory::vault(&db.conn, account.id).await
}

async fn contact_row(db: &TestDb, id: Uuid) -> contacts::Model {
    contacts::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

async fn set_company(db: &TestDb, contact: &contacts::Model, company_id: Id) {
    let mut c: contacts::ActiveModel = contact.clone().into();
    c.company_id = Set(Some(company_id));
    c.update(&db.conn).await.unwrap();
}

// ---------------------------------------------------------------- CopyContactToAnotherVaultTest

async fn copy_contact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    new_vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let new_contact = CopyContactToAnotherVault {
        account_id,
        vault_id: vault.id,
        other_vault_id: new_vault.id,
        author_id: author.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(contact_row(db, new_contact.id).await.vault_id, new_vault.id);
    assert_eq!(contact_row(db, contact.id).await.vault_id, vault.id);
    Ok(())
}

#[tokio::test]
async fn it_copies_a_contact_to_another_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    copy_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CopyContactToAnotherVault {
        account_id: regis.account_id,
        vault_id: vault.id,
        other_vault_id: Uuid::new(),
        author_id: regis.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = copy_contact(&db, &regis, account.id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let err = copy_contact(&db, &regis, regis.account_id, &other_vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = copy_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_destination_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = copy_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateContactTest

async fn create_contact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    gender: Option<&genders::Model>,
    pronoun: Option<&pronouns::Model>,
    template: Option<&templates::Model>,
) -> ApiResult<()> {
    let contact = CreateContact {
        account_id,
        id: None,
        vault_id: vault.id,
        author_id: author.id,
        first_name: Some("Ross".into()),
        last_name: None,
        middle_name: None,
        nickname: None,
        maiden_name: None,
        prefix: None,
        suffix: None,
        gender_id: gender.map(|g| g.id),
        pronoun_id: pronoun.map(|p| p.id),
        template_id: template.map(|t| t.id),
        listed: false,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_row(db, contact.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.first_name.as_deref(), Some("Ross"));
    assert_eq!(row.gender_id, gender.map(|g| g.id));
    assert_eq!(row.pronoun_id, pronoun.map(|p| p.id));
    assert_eq!(row.template_id, template.map(|t| t.id));

    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .filter(contact_feed_items::Column::Action.eq("contact_created"))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(feed.is_some());
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    create_contact(&db, &regis, regis.account_id, &vault, None, None, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_contact_with_gender() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let gender = factory::gender(&db.conn, regis.account_id).await;
    create_contact(&db, &regis, regis.account_id, &vault, Some(&gender), None, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_contact_with_pronoun() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    create_contact(&db, &regis, regis.account_id, &vault, None, Some(&pronoun), None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_contact_with_template() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let template = template(&db, regis.account_id).await;
    create_contact(&db, &regis, regis.account_id, &vault, None, None, Some(&template)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let err = CreateContact {
        account_id: regis.account_id,
        id: None,
        vault_id: vault.id,
        author_id: regis.id,
        first_name: Some("x".repeat(256)),
        last_name: None,
        middle_name: None,
        nickname: None,
        maiden_name: None,
        prefix: None,
        suffix: None,
        gender_id: None,
        pronoun_id: None,
        template_id: None,
        listed: false,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::vault(&db.conn, regis.account_id).await;
    let err = create_contact(&db, &regis, account.id, &vault, None, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let account = factory::create_account(&db.conn).await;
    let regis = factory::user(&db.conn, Some(account.id)).await;
    let vault = vault_elsewhere(&db).await;
    let err = create_contact(&db, &regis, account.id, &vault, None, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let err = create_contact(&db, &regis, regis.account_id, &vault, None, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyContactTest

async fn destroy_contact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    // Carbon::setTestNow(2018-01-01) has no equivalent: `deleted_at` is
    // checked against the time around the call.
    let before = crate::db::now();
    DestroyContact { account_id, author_id: author.id, vault_id: vault.id, contact_id: contact.id }
        .execute(&db.conn)
        .await?;
    let after = crate::db::now();

    let row = contact_row(db, contact.id).await;
    assert!(row.deleted_at.is_some_and(|t| before <= t && t <= after));

    let files = files::Entity::find().filter(files::Column::UfileableId.eq(contact.id)).all(&db.conn).await.unwrap();
    assert!(files.is_empty());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let file = file(&db, vault.id).await;
    let mut file: files::ActiveModel = file.into();
    file.fileable_type = Set(Some("App\\Models\\Contact".into()));
    file.ufileable_id = Set(Some(contact.id));
    file.update(&db.conn).await.unwrap();

    destroy_contact(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no
// invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = destroy_contact(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let err = destroy_contact(&db, &regis, regis.account_id, &other_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = destroy_contact(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let mut c: contacts::ActiveModel = contact.into();
    c.can_be_deleted = Set(false);
    let contact = c.update(&db.conn).await.unwrap();
    let err = destroy_contact(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Conflict);
    db.drop().await;
}

// ---------------------------------------------------------------- MoveContactToAnotherVaultTest

async fn move_contact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    new_vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let contact = MoveContactToAnotherVault {
        account_id,
        vault_id: vault.id,
        other_vault_id: new_vault.id,
        author_id: author.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(contact_row(db, contact.id).await.vault_id, new_vault.id);
    Ok(())
}

#[tokio::test]
async fn it_moves_a_contact_to_another_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    move_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_moves_a_contact_to_another_vault_and_copy_the_company_information_if_there_are_multiple_contacts_in_it() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault.id).await;
    for _ in 0..2 {
        let other = factory::contact(&db.conn, &vault).await;
        set_company(&db, &other, company.id).await;
    }
    set_company(&db, &contact, company.id).await;

    move_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap();

    assert!(companies::Entity::find_by_id(company.id).one(&db.conn).await.unwrap().is_some());
    assert_ne!(contact_row(&db, contact.id).await.company_id, Some(company.id));
    db.drop().await;
}

#[tokio::test]
async fn it_moves_a_contact_to_another_vault_and_move_the_company_information_if_there_are_no_other_contacts_in_it() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault.id).await;
    set_company(&db, &contact, company.id).await;

    move_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap();

    let row = companies::Entity::find_by_id(company.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, new_vault.id);
    db.drop().await;
}

#[tokio::test]
async fn move_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = MoveContactToAnotherVault {
        account_id: regis.account_id,
        vault_id: vault.id,
        other_vault_id: Uuid::new(),
        author_id: regis.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn move_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = move_contact(&db, &regis, account.id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn move_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let err = move_contact(&db, &regis, regis.account_id, &other_vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn move_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = move_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn move_fails_if_user_doesnt_have_right_permission_in_destination_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let new_vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = move_contact(&db, &regis, regis.account_id, &vault, &new_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleArchiveContactTest

async fn toggle_archive(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let service =
        || ToggleArchiveContact { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id };

    let contact = service().execute(&db.conn).await?;
    let row = contact_row(db, contact.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert!(!row.listed);

    let contact = service().execute(&db.conn).await?;
    let row = contact_row(db, contact.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert!(row.listed);
    Ok(())
}

#[tokio::test]
async fn it_toggles_a_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    toggle_archive(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// archive_fails_if_wrong_parameters_are_given: every field is typed, no
// invalid input can be built.

#[tokio::test]
async fn archive_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_archive(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn archive_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let err = toggle_archive(&db, &regis, regis.account_id, &other_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn archive_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_archive(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleFavoriteContactTest

async fn toggle_favorite(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let service =
        || ToggleFavoriteContact { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id };
    let pivot = || contact_vault_user::Entity::find_by_id((contact.id, vault.id, author.id)).one(&db.conn);

    service().execute(&db.conn).await?;
    assert!(pivot().await.unwrap().unwrap().is_favorite);

    service().execute(&db.conn).await?;
    assert!(!pivot().await.unwrap().unwrap().is_favorite);

    // F6: the first favorite writes `favorited` (the PHP wrote `unfavorited`).
    let actions: Vec<String> = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .order_by_asc(contact_feed_items::Column::Id)
        .all(&db.conn)
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.action)
        .collect();
    assert_eq!(actions, ["favorited", "unfavorited"]);
    Ok(())
}

#[tokio::test]
async fn it_toggles_the_favorite_state_of_a_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    toggle_favorite(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// favorite_fails_if_wrong_parameters_are_given: every field is typed, no
// invalid input can be built.

#[tokio::test]
async fn favorite_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_favorite(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn favorite_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let err = toggle_favorite(&db, &regis, regis.account_id, &other_vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn favorite_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_favorite(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactSortOrderTest

async fn update_sort_order(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    UpdateContactSortOrder { account_id, vault_id: vault.id, author_id: author.id, sort_order: "desc".into() }
        .execute(&db.conn)
        .await?;

    let row = users::Entity::find_by_id(author.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_sort_order, "desc");
    Ok(())
}

#[tokio::test]
async fn it_updates_the_way_contact_are_sorted() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    update_sort_order(&db, &regis, regis.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn sort_order_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let err = UpdateContactSortOrder {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        sort_order: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn sort_order_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let err = update_sort_order(&db, &regis, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactTemplateTest

async fn update_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let contact = UpdateContactTemplate {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        template_id: template.id,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_row(db, contact.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.template_id, Some(template.id));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_contact_template() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, regis.account_id).await;
    update_template(&db, &regis, regis.account_id, &vault, &contact, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn template_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = UpdateContactTemplate {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        template_id: Id(i64::MAX),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn template_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, regis.account_id).await;
    let err = update_template(&db, &regis, account.id, &vault, &contact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn template_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let template = template(&db, regis.account_id).await;
    let err = update_template(&db, &regis, regis.account_id, &other_vault, &contact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn template_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, regis.account_id).await;
    let err = update_template(&db, &regis, regis.account_id, &vault, &contact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn template_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, factory::account(&db.conn).await.id).await;
    let err = update_template(&db, &regis, regis.account_id, &vault, &contact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactTest

async fn update_contact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    gender: Option<&genders::Model>,
    pronoun: Option<&pronouns::Model>,
) -> ApiResult<()> {
    let contact = UpdateContact {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        first_name: "Ross".into(),
        last_name: None,
        middle_name: None,
        nickname: None,
        maiden_name: None,
        gender_id: gender.map(|g| g.id),
        pronoun_id: pronoun.map(|p| p.id),
        suffix: None,
        prefix: None,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_row(db, contact.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.first_name.as_deref(), Some("Ross"));
    assert_eq!(row.gender_id, gender.map(|g| g.id));
    assert_eq!(row.pronoun_id, pronoun.map(|p| p.id));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    update_contact(&db, &regis, regis.account_id, &vault, &contact, None, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_updates_a_contact_with_pronoun() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    update_contact(&db, &regis, regis.account_id, &vault, &contact, None, Some(&pronoun)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_updates_a_contact_with_gender() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let gender = factory::gender(&db.conn, regis.account_id).await;
    update_contact(&db, &regis, regis.account_id, &vault, &contact, Some(&gender), None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = UpdateContact {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        first_name: String::new(),
        last_name: None,
        middle_name: None,
        nickname: None,
        maiden_name: None,
        gender_id: None,
        pronoun_id: None,
        suffix: None,
        prefix: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = update_contact(&db, &regis, account.id, &vault, &contact, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_elsewhere(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_vault = vault_elsewhere(&db).await;
    let err = update_contact(&db, &regis, regis.account_id, &other_vault, &contact, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = update_contact(&db, &regis, regis.account_id, &vault, &contact, None, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactViewTest

async fn update_view(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    UpdateContactView { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id }
        .execute(&db.conn)
        .await?;

    let row =
        contact_vault_user::Entity::find_by_id((contact.id, vault.id, author.id)).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.number_of_views, 1);
    Ok(())
}

#[tokio::test]
async fn it_updates_the_contact_view() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    update_view(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// view_fails_if_wrong_parameters_are_given: every field is typed, no
// invalid input can be built.

#[tokio::test]
async fn view_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = update_view(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn view_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault_elsewhere(&db).await).await;
    let err = update_view(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn view_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = update_view(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
