//! `tests/Unit/Domains/Settings/ManageRelationshipTypes/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::testing::{TestDb, factory};

/// `RelationshipGroupType::factory()` in `account_id`.
async fn relationship_group_type(db: &TestDb, account_id: Uuid) -> relationship_group_types::Model {
    relationship_group_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("relationship group type")
}

/// `RelationshipType::factory()` in `relationship_group_type_id`.
async fn relationship_type(db: &TestDb, relationship_group_type_id: Id) -> relationship_types::Model {
    relationship_types::ActiveModel {
        relationship_group_type_id: Set(relationship_group_type_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        name_reverse_relationship: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("relationship type")
}

// ---- CreateRelationshipGroupTypeTest

async fn create_relationship_group_type(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let group = CreateRelationshipGroupType {
        account_id,
        author_id: author.id,
        name: Some("type name".into()),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row = relationship_group_types::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn create_relationship_group_type_creates_a_relationship_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_relationship_group_type(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_group_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateRelationshipGroupType {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_group_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_relationship_group_type(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_group_type_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_relationship_group_type(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- CreateRelationshipTypeTest

async fn create_relationship_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group: &relationship_group_types::Model,
) -> ApiResult<()> {
    let relationship_type = CreateRelationshipType {
        account_id,
        author_id: author.id,
        relationship_group_type_id: group.id,
        name: "type name".into(),
        name_reverse_relationship: "reverse type name".into(),
        can_be_deleted: true,
        r#type: None,
    }
    .execute(&db.conn)
    .await?;

    let row = relationship_types::Entity::find_by_id(relationship_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.relationship_group_type_id, group.id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    assert_eq!(row.name_reverse_relationship.as_deref(), Some("reverse type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn create_relationship_type_creates_a_relationship_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    create_relationship_type(&db, &ross, ross.account_id, &group).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let err = CreateRelationshipType {
        account_id: ross.account_id,
        author_id: ross.id,
        relationship_group_type_id: group.id,
        name: String::new(),
        name_reverse_relationship: String::new(),
        can_be_deleted: true,
        r#type: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(create_relationship_type(&db, &ross, account.id, &group).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_relationship_type_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(create_relationship_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyRelationshipGroupTypeTest

async fn destroy_relationship_group_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group: &relationship_group_types::Model,
) -> ApiResult<()> {
    DestroyRelationshipGroupType { account_id, relationship_group_type_id: group.id, author_id: author.id }
        .execute(&db.conn)
        .await?;
    assert!(relationship_group_types::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn destroy_relationship_group_type_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    destroy_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap();
    db.drop().await;
}

// destroy_relationship_group_type_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid
// input can be built.

#[tokio::test]
async fn destroy_relationship_group_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(destroy_relationship_group_type(&db, &ross, account.id, &group).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_relationship_group_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, other.id).await;
    assert_eq!(
        destroy_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_relationship_group_type_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

/// `can_be_deleted` = false is a 409 (the PHP answers 404); not in the PHP tests.
#[tokio::test]
async fn destroy_relationship_group_type_fails_if_type_cannot_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let mut group: relationship_group_types::ActiveModel = relationship_group_type(&db, ross.account_id).await.into();
    group.can_be_deleted = Set(false);
    let group = group.update(&db.conn).await.unwrap();
    assert_eq!(
        destroy_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code,
        Code::Conflict
    );
    assert!(relationship_group_types::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---- DestroyRelationshipTypeTest

async fn destroy_relationship_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group: &relationship_group_types::Model,
    relationship_type: &relationship_types::Model,
) -> ApiResult<()> {
    DestroyRelationshipType {
        account_id,
        author_id: author.id,
        relationship_group_type_id: group.id,
        relationship_type_id: relationship_type.id,
    }
    .execute(&db.conn)
    .await?;
    assert!(relationship_types::Entity::find_by_id(relationship_type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn destroy_relationship_type_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    destroy_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap();
    db.drop().await;
}

// destroy_relationship_type_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input
// can be built.

#[tokio::test]
async fn destroy_relationship_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        destroy_relationship_type(&db, &ross, account.id, &group, &relationship_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_relationship_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, other.id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        destroy_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_relationship_type_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        destroy_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

/// `can_be_deleted` = false is a 409 (the PHP answers 404); not in the PHP tests.
#[tokio::test]
async fn destroy_relationship_type_fails_if_type_cannot_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let mut relationship_type: relationship_types::ActiveModel = relationship_type(&db, group.id).await.into();
    relationship_type.can_be_deleted = Set(false);
    let relationship_type = relationship_type.update(&db.conn).await.unwrap();
    assert_eq!(
        destroy_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap_err().code,
        Code::Conflict
    );
    assert!(relationship_types::Entity::find_by_id(relationship_type.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---- UpdateRelationshipGroupTypeTest

async fn update_relationship_group_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group: &relationship_group_types::Model,
) -> ApiResult<()> {
    let group = UpdateRelationshipGroupType {
        account_id,
        author_id: author.id,
        relationship_group_type_id: group.id,
        name: "type name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = relationship_group_types::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn update_relationship_group_type_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    update_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_group_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let err = UpdateRelationshipGroupType {
        account_id: ross.account_id,
        author_id: ross.id,
        relationship_group_type_id: group.id,
        name: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_group_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(update_relationship_group_type(&db, &ross, account.id, &group).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_group_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, other.id).await;
    assert_eq!(
        update_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_group_type_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    assert_eq!(
        update_relationship_group_type(&db, &ross, ross.account_id, &group).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- UpdateRelationshipTypeTest

async fn update_relationship_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group: &relationship_group_types::Model,
    relationship_type: &relationship_types::Model,
) -> ApiResult<()> {
    let relationship_type = UpdateRelationshipType {
        account_id,
        author_id: author.id,
        relationship_group_type_id: group.id,
        relationship_type_id: relationship_type.id,
        name: "type name".into(),
        name_reverse_relationship: "reverse type name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = relationship_types::Entity::find_by_id(relationship_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.relationship_group_type_id, group.id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    assert_eq!(row.name_reverse_relationship.as_deref(), Some("reverse type name"));
    Ok(())
}

#[tokio::test]
async fn update_relationship_type_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    update_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    let err = UpdateRelationshipType {
        account_id: ross.account_id,
        author_id: ross.id,
        relationship_group_type_id: group.id,
        relationship_type_id: relationship_type.id,
        name: String::new(),
        name_reverse_relationship: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        update_relationship_type(&db, &ross, account.id, &group, &relationship_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_type_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let group = relationship_group_type(&db, other.id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        update_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_relationship_type_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group = relationship_group_type(&db, ross.account_id).await;
    let relationship_type = relationship_type(&db, group.id).await;
    assert_eq!(
        update_relationship_type(&db, &ross, ross.account_id, &group, &relationship_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}
