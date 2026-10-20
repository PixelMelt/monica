//! `tests/Unit/Domains/Contact/ManageContactImportantDates/Services`.

use super::*;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `ContactImportantDateType::factory()` in `vault_id`.
async fn contact_important_date_type(db: &TestDb, vault_id: Uuid) -> contact_important_date_types::Model {
    contact_important_date_types::ActiveModel {
        vault_id: Set(vault_id),
        label: Set("Birthdate".into()),
        internal_type: Set(Some("birthdate".into())),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("contact important date type")
}

/// `ContactImportantDate::factory()` for `contact` (a contact in a new
/// vault when `None`), with `(day, month, year)`.
async fn contact_important_date(
    db: &TestDb,
    contact: Option<&contacts::Model>,
    (day, month, year): (Option<i32>, Option<i32>, Option<i32>),
) -> contact_important_dates::Model {
    let contact = match contact {
        Some(c) => c.clone(),
        None => contact_elsewhere(db).await,
    };
    let r#type = contact_important_date_type(db, contact.vault_id).await;
    contact_important_dates::ActiveModel {
        contact_id: Set(contact.id),
        label: Set("birthdate".into()),
        day: Set(day),
        month: Set(month),
        year: Set(year),
        contact_important_date_type_id: Set(Some(r#type.id)),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("contact important date")
}

const BIRTHDATE: (Option<i32>, Option<i32>, Option<i32>) = (Some(29), Some(10), Some(1981));

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_for(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

/// `Contact::factory()->create()`: in a vault of another account.
async fn contact_elsewhere(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    factory::contact(&db.conn, &factory::vault(&db.conn, account.id).await).await
}

async fn assert_date(db: &TestDb, contact: &contacts::Model, r#type: Option<&contact_important_date_types::Model>) {
    let row = contact_important_dates::Entity::find()
        .filter(contact_important_dates::Column::ContactId.eq(contact.id))
        .filter(contact_important_dates::Column::Label.eq("birthdate"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((row.day, row.month, row.year), BIRTHDATE);
    assert_eq!(row.contact_important_date_type_id, r#type.map(|t| t.id));
}

// ---------------------------------------------------------------- CreateContactImportantDateTest

async fn create_date(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    r#type: Option<&contact_important_date_types::Model>,
) -> ApiResult<()> {
    CreateContactImportantDate {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_important_date_type_id: r#type.map(|t| t.id),
        label: "birthdate".into(),
        day: Some(29),
        month: Some(10),
        year: Some(1981),
    }
    .execute(&db.conn)
    .await?;

    assert_date(db, contact, r#type).await;

    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .filter(contact_feed_items::Column::Action.eq("important_date_created"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(feed.description.as_deref(), Some("birthdate Oct 29, 1981"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact_date() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    create_date(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_contact_date_with_type() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = contact_important_date_type(&db, vault.id).await;
    create_date(&db, &regis, regis.account_id, &vault, &contact, Some(&r#type)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateContactImportantDate {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_important_date_type_id: None,
        label: String::new(),
        day: None,
        month: None,
        year: None,
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
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_date(&db, &regis, account.id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let err = create_date(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_date(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyContactImportantDateTest

async fn destroy_date(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    date: &contact_important_dates::Model,
) -> ApiResult<()> {
    // Carbon::setTestNow(now()) has no equivalent: `deleted_at` is checked
    // against the time around the call.
    let before = crate::db::now();
    DestroyContactImportantDate {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_important_date_id: date.id,
    }
    .execute(&db.conn)
    .await?;
    let after = crate::db::now();

    let row = contact_important_dates::Entity::find_by_id(date.id).one(&db.conn).await.unwrap().unwrap();
    assert!(row.deleted_at.is_some_and(|t| before <= t && t <= after));
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_contact_date() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    destroy_date(&db, &regis, regis.account_id, &vault, &contact, &date).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no
// invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = destroy_date(&db, &regis, account.id, &vault, &contact, &date).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = destroy_date(&db, &regis, regis.account_id, &vault, &contact, &date).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = destroy_date(&db, &regis, regis.account_id, &vault, &contact, &date).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_date_does_not_exist() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, None, BIRTHDATE).await;
    let err = destroy_date(&db, &regis, regis.account_id, &vault, &contact, &date).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactImportantDateTest

async fn update_date(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    date: &contact_important_dates::Model,
    r#type: Option<&contact_important_date_types::Model>,
) -> ApiResult<()> {
    UpdateContactImportantDate {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_important_date_id: date.id,
        contact_important_date_type_id: r#type.map(|t| t.id),
        label: "birthdate".into(),
        day: Some(29),
        month: Some(10),
        year: Some(1981),
    }
    .execute(&db.conn)
    .await?;

    assert_date(db, contact, r#type).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_contact_date() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    update_date(&db, &regis, regis.account_id, &vault, &contact, &date, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_updates_a_contact_date_with_type() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let r#type = contact_important_date_type(&db, vault.id).await;
    update_date(&db, &regis, regis.account_id, &vault, &contact, &date, Some(&r#type)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = UpdateContactImportantDate {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_important_date_id: date.id,
        contact_important_date_type_id: None,
        label: String::new(),
        day: None,
        month: None,
        year: None,
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
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = update_date(&db, &regis, account.id, &vault, &contact, &date, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = update_date(&db, &regis, regis.account_id, &vault, &contact, &date, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, Some(&contact), BIRTHDATE).await;
    let err = update_date(&db, &regis, regis.account_id, &vault, &contact, &date, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_date_does_not_exist() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let date = contact_important_date(&db, None, BIRTHDATE).await;
    let err = update_date(&db, &regis, regis.account_id, &vault, &contact, &date, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ImportantDateHelperTest (formatDate)

async fn format(db: &TestDb, date: (Option<i32>, Option<i32>, Option<i32>)) -> Option<String> {
    let user = factory::create_user(&db.conn).await;
    let date = contact_important_date(db, None, date).await;
    format_date(&date, &user).unwrap()
}

#[tokio::test]
async fn format_date_gets_the_date_based_on_a_complete_date() {
    let db = TestDb::migrated().await;
    assert_eq!(format(&db, (Some(29), Some(10), Some(1981))).await.as_deref(), Some("Oct 29, 1981"));
    db.drop().await;
}

#[tokio::test]
async fn format_date_gets_the_date_based_on_a_year() {
    let db = TestDb::migrated().await;
    assert_eq!(format(&db, (None, None, Some(1970))).await.as_deref(), Some("1970"));
    db.drop().await;
}

#[tokio::test]
async fn format_date_gets_the_date_based_on_a_month_or_day() {
    let db = TestDb::migrated().await;
    assert_eq!(format(&db, (Some(29), Some(10), None)).await.as_deref(), Some("Oct 29"));
    db.drop().await;
}

#[tokio::test]
async fn format_date_cant_get_the_date_if_the_date_is_not_set() {
    let db = TestDb::migrated().await;
    assert_eq!(format(&db, (None, None, None)).await, None);
    db.drop().await;
}
