//! `tests/Unit/Domains/Settings/ManageReligion/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Religion::factory()` in `account_id` (a new account when `None`) at `position`.
async fn religion(db: &TestDb, account_id: Option<Uuid>, position: i32) -> religions::Model {
    let account_id = match account_id {
        Some(a) => a,
        None => factory::account(&db.conn).await.id,
    };
    religions::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        translation_key: Set(Some(random_string(6).to_lowercase())),
        position: Set(Some(position)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("religion")
}

async fn row(db: &TestDb, id: Id) -> Option<religions::Model> {
    religions::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

// ---------------------------------------------------------------- CreateReligionTest

async fn create(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let religion =
        CreateReligion { account_id, author_id: author.id, name: "type name".into() }.execute(&db.conn).await?;

    let row = row(db, religion.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    assert_eq!(row.position, Some(1));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_religion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateReligion { account_id: ross.account_id, author_id: ross.id, name: String::new() }
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

// ---------------------------------------------------------------- DestroyReligionTest

async fn destroy(db: &TestDb, author: &users::Model, account_id: Uuid, religion: &religions::Model) -> ApiResult<()> {
    DestroyReligion { account_id, religion_id: religion.id, author_id: author.id }.execute(&db.conn).await?;
    assert!(row(db, religion.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_religion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    destroy(&db, &ross, ross.account_id, &religion).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    assert_eq!(destroy(&db, &ross, account.id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_religion_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, None, 1).await;
    assert_eq!(destroy(&db, &ross, ross.account_id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    assert_eq!(destroy(&db, &ross, ross.account_id, &religion).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateReligionTest

async fn update(db: &TestDb, author: &users::Model, account_id: Uuid, religion: &religions::Model) -> ApiResult<()> {
    let religion =
        UpdateReligion { account_id, author_id: author.id, religion_id: religion.id, name: "type name".into() }
            .execute(&db.conn)
            .await?;

    let row = row(db, religion.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_religion() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    update(&db, &ross, ross.account_id, &religion).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    let err = UpdateReligion {
        account_id: ross.account_id,
        author_id: ross.id,
        religion_id: religion.id,
        name: "x".repeat(256),
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
    let religion = religion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update(&db, &ross, account.id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_religion_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, None, 1).await;
    assert_eq!(update(&db, &ross, ross.account_id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update(&db, &ross, ross.account_id, &religion).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateReligionPositionTest

async fn update_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    target: &religions::Model,
) -> ApiResult<()> {
    let religion1 = religion(db, Some(account_id), 1).await;
    let religion3 = religion(db, Some(account_id), 3).await;
    let religion4 = religion(db, Some(account_id), 4).await;
    let request = |new_position| UpdateReligionPosition {
        account_id,
        author_id: author.id,
        religion_id: target.id,
        new_position,
    };
    let positions = async || {
        let mut positions = Vec::new();
        for id in [religion1.id, religion3.id, religion4.id, target.id] {
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
async fn it_updates_a_religion_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    update_position(&db, &ross, ross.account_id, &religion).await.unwrap();
    db.drop().await;
}

// update_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let religion = religion(&db, Some(ross.account_id), 1).await;
    assert_eq!(update_position(&db, &ross, account.id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_religion_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let religion = religion(&db, None, 1).await;
    assert_eq!(update_position(&db, &ross, ross.account_id, &religion).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
