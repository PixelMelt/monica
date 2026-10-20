//! `tests/Unit/Domains/Settings/ManageGiftOccasions/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `GiftOccasion::factory()` in `account_id` (a new account when `None`) at `position`.
async fn gift_occasion(db: &TestDb, account_id: Option<Uuid>, position: i32) -> gift_occasions::Model {
    let account_id = match account_id {
        Some(a) => a,
        None => factory::account(&db.conn).await.id,
    };
    gift_occasions::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(Some(position)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("gift occasion")
}

async fn row(db: &TestDb, id: Id) -> Option<gift_occasions::Model> {
    gift_occasions::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

// ---------------------------------------------------------------- CreateGiftOccasionTest

async fn create(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let occasion =
        CreateGiftOccasion { account_id, author_id: author.id, label: "type name".into() }.execute(&db.conn).await?;

    let row = row(db, occasion.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert_eq!(row.position, Some(1));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_gift_occasion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateGiftOccasion { account_id: ross.account_id, author_id: ross.id, label: String::new() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGiftOccasionTest

async fn destroy(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    gift_occasion: &gift_occasions::Model,
) -> ApiResult<()> {
    DestroyGiftOccasion { account_id, gift_occasion_id: gift_occasion.id, author_id: author.id }
        .execute(&db.conn)
        .await?;
    assert!(row(db, gift_occasion.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_gift_occasion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    destroy(&db, &ross, ross.account_id, &gift_occasion).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    assert_eq!(destroy(&db, &ross, account.id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, None, 1).await;
    assert_eq!(destroy(&db, &ross, ross.account_id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    assert_eq!(destroy(&db, &ross, ross.account_id, &gift_occasion).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGiftOccasionTest

async fn update(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    gift_occasion: &gift_occasions::Model,
) -> ApiResult<()> {
    let gift_occasion = UpdateGiftOccasion {
        account_id,
        author_id: author.id,
        gift_occasion_id: gift_occasion.id,
        label: "type label".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = row(db, gift_occasion.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type label"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_gift_occasion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    update(&db, &ross, ross.account_id, &gift_occasion).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    let err = UpdateGiftOccasion {
        account_id: ross.account_id,
        author_id: ross.id,
        gift_occasion_id: gift_occasion.id,
        label: "x".repeat(256),
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
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update(&db, &ross, account.id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, None, 1).await;
    assert_eq!(update(&db, &ross, ross.account_id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update(&db, &ross, ross.account_id, &gift_occasion).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGiftOccasionPositionTest

async fn update_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    occasion: &gift_occasions::Model,
) -> ApiResult<()> {
    let gift_occasion1 = gift_occasion(db, Some(account_id), 1).await;
    let gift_occasion3 = gift_occasion(db, Some(account_id), 3).await;
    let gift_occasion4 = gift_occasion(db, Some(account_id), 4).await;
    let request = |new_position| UpdateGiftOccasionPosition {
        account_id,
        author_id: author.id,
        gift_occasion_id: occasion.id,
        new_position,
    };
    let positions = async || {
        let mut positions = Vec::new();
        for id in [gift_occasion1.id, gift_occasion3.id, gift_occasion4.id, occasion.id] {
            let row = row(db, id).await.unwrap();
            assert_eq!(row.account_id, account_id);
            positions.push(row.position.unwrap());
        }
        positions
    };

    request(3).execute(&db.conn).await?;
    assert_eq!(positions().await, [1, 2, 4, 3]);

    request(2).execute(&db.conn).await?;
    assert_eq!(positions().await, [1, 3, 4, 2]);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_template_page_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    update_position(&db, &ross, ross.account_id, &gift_occasion).await.unwrap();
    db.drop().await;
}

// update_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let gift_occasion = gift_occasion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update_position(&db, &ross, account.id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_gift_occasion_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gift_occasion = gift_occasion(&db, None, 1).await;
    assert_eq!(update_position(&db, &ross, ross.account_id, &gift_occasion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
