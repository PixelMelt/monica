//! `tests/Unit/Domains/Settings/ManageCallReasons/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `CallReasonType::factory()` in `account_id`.
async fn call_reason_type(db: &TestDb, account_id: Uuid) -> call_reason_types::Model {
    call_reason_types::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("call reason type")
}

/// `CallReason::factory()` in `call_reason_type_id`.
async fn call_reason(db: &TestDb, call_reason_type_id: Id) -> call_reasons::Model {
    call_reasons::ActiveModel {
        call_reason_type_id: Set(call_reason_type_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("call reason")
}

// ---- CreateCallReasonTest

async fn create_call_reason(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    reason_type: &call_reason_types::Model,
) -> ApiResult<()> {
    let reason = CreateCallReason {
        account_id,
        author_id: author.id,
        call_reason_type_id: reason_type.id,
        label: Some("type name".into()),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = call_reasons::Entity::find_by_id(reason.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.call_reason_type_id, reason_type.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_call_reason() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    create_call_reason(&db, &ross, ross.account_id, &reason_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let err = CreateCallReason {
        account_id: ross.account_id,
        author_id: ross.id,
        call_reason_type_id: reason_type.id,
        label: Some("x".repeat(256)),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(create_call_reason(&db, &ross, account.id, &reason_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(create_call_reason(&db, &ross, ross.account_id, &reason_type).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- CreateCallReasonTypeTest

async fn create_call_reason_type(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let reason_type = CreateCallReasonType {
        account_id,
        author_id: author.id,
        label: Some("type name".into()),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = call_reason_types::Entity::find_by_id(reason_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_call_reason_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_call_reason_type(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateCallReasonType {
        account_id: ross.account_id,
        author_id: ross.id,
        label: Some("x".repeat(256)),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_call_reason_type(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_call_reason_type_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_call_reason_type(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyCallReasonTest

async fn destroy_call_reason(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    reason_type: &call_reason_types::Model,
    reason: &call_reasons::Model,
) -> ApiResult<()> {
    DestroyCallReason {
        account_id,
        author_id: author.id,
        call_reason_type_id: reason_type.id,
        call_reason_id: reason.id,
    }
    .execute(&db.conn)
    .await?;
    assert!(call_reasons::Entity::find_by_id(reason.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn destroy_call_reason_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    destroy_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap();
    db.drop().await;
}

// destroy_call_reason_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_call_reason_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        destroy_call_reason(&db, &ross, account.id, &reason_type, &reason).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_call_reason_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, other.id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        destroy_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_call_reason_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        destroy_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- DestroyCallReasonTypeTest

async fn destroy_call_reason_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    reason_type: &call_reason_types::Model,
) -> ApiResult<()> {
    DestroyCallReasonType { account_id, call_reason_type_id: reason_type.id, author_id: author.id }
        .execute(&db.conn)
        .await?;
    assert!(call_reason_types::Entity::find_by_id(reason_type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn destroy_call_reason_type_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    destroy_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap();
    db.drop().await;
}

// destroy_call_reason_type_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_call_reason_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(destroy_call_reason_type(&db, &ross, account.id, &reason_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_call_reason_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, other.id).await;
    assert_eq!(
        destroy_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_call_reason_type_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- UpdateCallReasonTest

async fn update_call_reason(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    reason_type: &call_reason_types::Model,
    reason: &call_reasons::Model,
) -> ApiResult<()> {
    let reason = UpdateCallReason {
        account_id,
        author_id: author.id,
        call_reason_type_id: reason_type.id,
        call_reason_id: reason.id,
        label: "type name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = call_reasons::Entity::find_by_id(reason.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.call_reason_type_id, reason_type.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_call_reason() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    update_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    let err = UpdateCallReason {
        account_id: ross.account_id,
        author_id: ross.id,
        call_reason_type_id: reason_type.id,
        call_reason_id: reason.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        update_call_reason(&db, &ross, account.id, &reason_type, &reason).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, other.id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        update_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let reason = call_reason(&db, reason_type.id).await;
    assert_eq!(
        update_call_reason(&db, &ross, ross.account_id, &reason_type, &reason).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- UpdateCallReasonTypeTest

async fn update_call_reason_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    reason_type: &call_reason_types::Model,
) -> ApiResult<()> {
    let reason_type = UpdateCallReasonType {
        account_id,
        author_id: author.id,
        call_reason_type_id: reason_type.id,
        label: "type name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = call_reason_types::Entity::find_by_id(reason_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    update_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    let err = UpdateCallReasonType {
        account_id: ross.account_id,
        author_id: ross.id,
        call_reason_type_id: reason_type.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(update_call_reason_type(&db, &ross, account.id, &reason_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let reason_type = call_reason_type(&db, other.id).await;
    assert_eq!(
        update_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_call_reason_type_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let reason_type = call_reason_type(&db, ross.account_id).await;
    assert_eq!(
        update_call_reason_type(&db, &ross, ross.account_id, &reason_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}
