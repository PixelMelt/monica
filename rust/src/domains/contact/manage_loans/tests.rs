//! `tests/Unit/Domains/Contact/ManageLoans/Services`.

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::entities::{contact_feed_items, currencies, users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Loan::TYPE_DEBT`.
const TYPE_DEBT: &str = "debt";

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `Loan::factory()` in `vault_id` (a new vault when `None`).
async fn loan(db: &TestDb, vault_id: Option<Uuid>) -> loans::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => factory::create_vault(&db.conn, factory::create_account(&db.conn).await.id).await.id,
    };
    loans::ActiveModel {
        vault_id: Set(vault_id),
        r#type: Set(TYPE_DEBT.into()),
        name: Set(format!("word{}", random_string(6))),
        description: Set(Some(format!("Sentence {}.", random_string(8)))),
        amount_lent: Set(Some(rand::random_range(0..100_000))),
        loaned_at: Set(Some(Timestamp(NaiveDate::from_ymd_opt(2004, 6, 15).unwrap().and_hms_opt(8, 30, 0).unwrap()))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("loan")
}

/// `Currency::factory()`.
async fn currency(db: &TestDb) -> currencies::Model {
    currencies::ActiveModel { code: Set("CAD".into()), ..Default::default() }.insert(&db.conn).await.expect("currency")
}

/// `ContactFeedItem::factory()` for `contact_id`, about the loan `feedable_id`.
async fn feed_item(db: &TestDb, contact_id: Uuid, feedable_id: Id) {
    let author = factory::create_user(&db.conn).await;
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact_id),
        action: Set("note_created".into()),
        description: Set(Some(random_string(6))),
        feedable_id: Set(Some(feedable_id)),
        feedable_type: Set(Some("App\\Models\\Loan".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("feed item");
}

/// `createUser()`, `createVault()` and `setPermissionInVault()` with `permission`.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

fn date(y: i32, m: u32, d: u32) -> Timestamp {
    Timestamp(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(0, 0, 0).unwrap())
}

async fn assert_link(db: &TestDb, loan_id: Id, loaner: &contacts::Model, loanee: &contacts::Model) {
    assert!(
        contact_loan::Entity::find()
            .filter(contact_loan::Column::LoanId.eq(loan_id))
            .filter(contact_loan::Column::LoanerId.eq(loaner.id))
            .filter(contact_loan::Column::LoaneeId.eq(loanee.id))
            .one(&db.conn)
            .await
            .unwrap()
            .is_some()
    );
}

// ---------------------------------------------------------------- CreateLoanTest

async fn create_loan(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    loaner: &contacts::Model,
    loanee: &contacts::Model,
) -> ApiResult<()> {
    let currency = currency(db).await;

    let loan = CreateLoan {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        currency_id: Some(currency.id),
        r#type: TYPE_DEBT.into(),
        name: "Orgy".into(),
        description: Some("This is incredible".into()),
        loaner_ids: vec![loaner.id],
        loanee_ids: vec![loanee.id],
        amount_lent: Some(123),
        loaned_at: Some("2020-01-01".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = loans::Entity::find_by_id(loan.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.r#type, TYPE_DEBT);
    assert_eq!(row.name, "Orgy");
    assert_eq!(row.description.as_deref(), Some("This is incredible"));
    assert_eq!(row.amount_lent, Some(123));
    assert_eq!(row.loaned_at, Some(date(2020, 1, 1)));

    assert_link(db, loan.id, loaner, loanee).await;
    Ok(())
}

#[tokio::test]
async fn it_creates_a_loan() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    create_loan(&db, &regis, regis.account_id, &vault, &contact, &loaner, &loanee).await.unwrap();
    db.drop().await;
}

/// F22: every loaner and loanee pair is stored, so a loan keeps all its
/// loaners and loanees.
#[tokio::test]
async fn it_keeps_every_loaner_and_loanee() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let (a, b, c, d) = (
        factory::contact(&db.conn, &vault).await,
        factory::contact(&db.conn, &vault).await,
        factory::contact(&db.conn, &vault).await,
        factory::contact(&db.conn, &vault).await,
    );
    let currency = currency(&db).await;
    let loan = CreateLoan {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        currency_id: Some(currency.id),
        r#type: TYPE_DEBT.into(),
        name: "Dinner".into(),
        description: None,
        loaner_ids: vec![a.id, b.id],
        loanee_ids: vec![c.id, d.id],
        amount_lent: Some(100),
        loaned_at: None,
    }
    .execute(&db.conn)
    .await
    .unwrap();
    for loaner in [&a, &b] {
        for loanee in [&c, &d] {
            assert_link(&db, loan.id, loaner, loanee).await;
        }
    }
    let rows =
        contact_loan::Entity::find().filter(contact_loan::Column::LoanId.eq(loan.id)).all(&db.conn).await.unwrap();
    assert_eq!(rows.len(), 4);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateLoan {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        currency_id: None,
        r#type: TYPE_DEBT.into(),
        name: String::new(),
        description: None,
        loaner_ids: vec![contact.id],
        loanee_ids: vec![contact.id],
        amount_lent: None,
        loaned_at: None,
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let err = create_loan(&db, &regis, account.id, &vault, &contact, &loaner, &loanee).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let err = create_loan(&db, &regis, regis.account_id, &vault, &contact, &loaner, &loanee).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_loaner_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let loaner = other_contact(&db).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let err = create_loan(&db, &regis, regis.account_id, &vault, &contact, &loaner, &loanee).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_loanee_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = other_contact(&db).await;
    let err = create_loan(&db, &regis, regis.account_id, &vault, &contact, &loaner, &loanee).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let err = create_loan(&db, &regis, regis.account_id, &vault, &contact, &loaner, &loanee).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyLoanTest

async fn destroy_loan(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    loan: &loans::Model,
) -> ApiResult<()> {
    DestroyLoan { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, loan_id: loan.id }
        .execute(&db.conn)
        .await?;

    assert!(loans::Entity::find_by_id(loan.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_loan() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    feed_item(&db, contact.id, loan.id).await;
    destroy_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = destroy_loan(&db, &regis, account.id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = destroy_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_loan_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, None).await;
    let err = destroy_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleLoanTest

async fn toggle_loan(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    loan: &loans::Model,
) -> ApiResult<loans::Model> {
    ToggleLoan { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, loan_id: loan.id }
        .execute(&db.conn)
        .await
}

#[tokio::test]
async fn it_toggles_a_loan() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;

    // Carbon::setTestNow(2018-01-01) has no equivalent: `settled_at` is
    // checked against the time around the call.
    let before = crate::db::now();
    let loan = toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap();
    let after = crate::db::now();

    let row = loans::Entity::find_by_id(loan.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.r#type, TYPE_DEBT);
    assert!(row.settled);
    let settled_at = row.settled_at.unwrap();
    assert!(before <= settled_at && settled_at <= after);
    db.drop().await;
}

// toggle_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn toggle_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = toggle_loan(&db, &regis, account.id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_loan_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, None).await;
    let err = toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

/// Q6: toggling a settled loan back clears `settled_at`.
#[tokio::test]
async fn toggle_clears_settled_at_when_unsettling() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap();
    toggle_loan(&db, &regis, regis.account_id, &vault, &contact, &loan).await.unwrap();

    let row = loans::Entity::find_by_id(loan.id).one(&db.conn).await.unwrap().unwrap();
    assert!(!row.settled);
    assert_eq!(row.settled_at, None);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLoanTest

async fn update_loan(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    loan: &loans::Model,
    (loaner, loanee): (&contacts::Model, &contacts::Model),
) -> ApiResult<()> {
    let loan = UpdateLoan {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        loan_id: loan.id,
        currency_id: None,
        r#type: TYPE_DEBT.into(),
        name: "resto".into(),
        description: Some("This is incredible".into()),
        loaner_ids: vec![loaner.id],
        loanee_ids: vec![loanee.id],
        amount_lent: Some(123),
        loaned_at: Some("2020-01-01".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = loans::Entity::find_by_id(loan.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.r#type, TYPE_DEBT);
    assert_eq!(row.name, "resto");
    assert_eq!(row.description.as_deref(), Some("This is incredible"));
    assert_eq!(row.amount_lent, Some(123));
    assert_eq!(row.loaned_at, Some(date(2020, 1, 1)));

    assert_link(db, loan.id, loaner, loanee).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_loan() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = UpdateLoan {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        loan_id: loan.id,
        currency_id: None,
        r#type: TYPE_DEBT.into(),
        name: String::new(),
        description: None,
        loaner_ids: vec![contact.id],
        loanee_ids: vec![contact.id],
        amount_lent: None,
        loaned_at: None,
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err = update_loan(&db, &regis, account.id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err =
        update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, Some(vault.id)).await;
    let err =
        update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_loan_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, None).await;
    let err =
        update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_loaner_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = other_contact(&db).await;
    let loanee = factory::contact(&db.conn, &vault).await;
    let loan = loan(&db, None).await;
    let err =
        update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_loanee_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let loaner = factory::contact(&db.conn, &vault).await;
    let loanee = other_contact(&db).await;
    let loan = loan(&db, None).await;
    let err =
        update_loan(&db, &regis, regis.account_id, &vault, &contact, &loan, (&loaner, &loanee)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
