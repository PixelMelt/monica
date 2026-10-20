//! `tests/Unit/Domains/Settings/ManagePronouns/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

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

// ---- CreatePronounTest

async fn create_pronoun(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let pronoun = CreatePronoun {
        account_id,
        author_id: author.id,
        name: Some("pronoun name".into()),
        name_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = pronouns::Entity::find_by_id(pronoun.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("pronoun name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_pronoun() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_pronoun(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreatePronoun {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
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
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_pronoun(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_pronoun(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyPronounTest

async fn destroy_pronoun(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    pronoun: &pronouns::Model,
) -> ApiResult<()> {
    DestroyPronoun { account_id, author_id: author.id, pronoun_id: pronoun.id }.execute(&db.conn).await?;
    assert!(pronouns::Entity::find_by_id(pronoun.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_pronoun() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    destroy_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    assert_eq!(destroy_pronoun(&db, &ross, account.id, &pronoun).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_pronoun_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let pronoun = pronoun(&db, other.id).await;
    assert_eq!(destroy_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    assert_eq!(destroy_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- UpdatePronounTest

async fn update_pronoun(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    pronoun: &pronouns::Model,
) -> ApiResult<()> {
    let pronoun =
        UpdatePronoun { account_id, author_id: author.id, pronoun_id: pronoun.id, name: "pronoun name".into() }
            .execute(&db.conn)
            .await?;

    let row = pronouns::Entity::find_by_id(pronoun.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("pronoun name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_pronoun() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    update_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    let err =
        UpdatePronoun { account_id: ross.account_id, author_id: ross.id, pronoun_id: pronoun.id, name: String::new() }
            .execute(&db.conn)
            .await
            .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    assert_eq!(update_pronoun(&db, &ross, account.id, &pronoun).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_pronoun_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let pronoun = pronoun(&db, other.id).await;
    assert_eq!(update_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let pronoun = pronoun(&db, ross.account_id).await;
    assert_eq!(update_pronoun(&db, &ross, ross.account_id, &pronoun).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
