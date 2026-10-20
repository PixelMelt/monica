//! `tests/Unit/Domains/Settings/ManageGroupTypes/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `GroupType::factory()` in `account_id`.
fn group_type_factory(account_id: Uuid) -> group_types::ActiveModel {
    group_types::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(1),
        ..Default::default()
    }
}

/// `GroupTypeRole::factory()` in `group_type_id`.
fn group_type_role_factory(group_type_id: Id) -> group_type_roles::ActiveModel {
    group_type_roles::ActiveModel {
        group_type_id: Set(group_type_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(Some(1)),
        ..Default::default()
    }
}

/// `GroupType::factory()->create()`: in a new account.
async fn group_type_elsewhere(db: &TestDb) -> group_types::Model {
    let account = factory::account(&db.conn).await;
    group_type_factory(account.id).insert(&db.conn).await.unwrap()
}

/// `assertDatabaseHas('group_types', [id, account_id, position])`.
async fn assert_group_type(db: &TestDb, id: Id, account_id: Uuid, position: i32) {
    let row = group_types::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!((row.account_id, row.position), (account_id, position));
}

/// `assertDatabaseHas('group_type_roles', [id, group_type_id, position])`.
async fn assert_group_type_role(db: &TestDb, id: Id, group_type_id: Id, position: i32) {
    let row = group_type_roles::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!((row.group_type_id, row.position), (group_type_id, Some(position)));
}

// ---------------------------------------------------------------- CreateGroupTypeTest

async fn create_group_type(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let group_type = CreateGroupType {
        account_id,
        author_id: author.id,
        label: Some("type name".into()),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = group_types::Entity::find_by_id(group_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert_eq!(row.position, 1);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_group_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_group_type(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateGroupType {
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
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_group_type(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_group_type(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateGroupTypeRoleTest

async fn create_group_type_role(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    let group_type_role = CreateGroupTypeRole {
        account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        label: Some("type name".into()),
        label_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = group_type_roles::Entity::find_by_id(group_type_role.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.group_type_id, group_type.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert_eq!(row.position, Some(1));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_group_type_role() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    create_group_type_role(&db, &ross, ross.account_id, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_role_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateGroupTypeRole {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: Id(0),
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
async fn create_role_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(create_group_type_role(&db, &ross, account.id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_role_fails_if_group_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group_type = group_type_factory(account.id).insert(&db.conn).await.unwrap();
    assert_eq!(create_group_type_role(&db, &ross, account.id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_role_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(
        create_group_type_role(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGroupTypeTest

async fn destroy_group_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    DestroyGroupType { account_id, author_id: author.id, group_type_id: group_type.id }.execute(&db.conn).await?;
    assert!(group_types::Entity::find_by_id(group_type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_group_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    destroy_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = DestroyGroupType { account_id: ross.account_id, author_id: ross.id, group_type_id: Id(0) }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(destroy_group_type(&db, &ross, account.id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    assert_eq!(destroy_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(destroy_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGroupTypeRoleTest

async fn destroy_group_type_role(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
    group_type_role: &group_type_roles::Model,
) -> ApiResult<()> {
    DestroyGroupTypeRole {
        account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        group_type_role_id: group_type_role.id,
    }
    .execute(&db.conn)
    .await?;
    assert!(group_type_roles::Entity::find_by_id(group_type_role.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_group_type_role() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    destroy_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_role_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = DestroyGroupTypeRole {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: Id(0),
        group_type_role_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn destroy_role_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_group_type_role(&db, &ross, account.id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_role_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_role_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGroupTypeTest

async fn update_group_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    let group_type =
        UpdateGroupType { account_id, author_id: author.id, group_type_id: group_type.id, label: "type label".into() }
            .execute(&db.conn)
            .await?;

    let row = group_types::Entity::find_by_id(group_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("type label"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_group_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    update_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let err = UpdateGroupType {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: group_type.id,
        label: String::new(),
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
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(update_group_type(&db, &ross, account.id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    assert_eq!(update_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(update_group_type(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGroupTypePositionTest

async fn update_group_type_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    let group_type1 =
        group_types::ActiveModel { position: Set(1), ..group_type_factory(account_id) }.insert(&db.conn).await.unwrap();
    let group_type3 =
        group_types::ActiveModel { position: Set(3), ..group_type_factory(account_id) }.insert(&db.conn).await.unwrap();
    let group_type4 =
        group_types::ActiveModel { position: Set(4), ..group_type_factory(account_id) }.insert(&db.conn).await.unwrap();

    let request = |new_position| UpdateGroupTypePosition {
        account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        new_position,
    };

    let group_type = request(3).execute(&db.conn).await?;

    assert_group_type(db, group_type1.id, account_id, 1).await;
    assert_group_type(db, group_type3.id, account_id, 2).await;
    assert_group_type(db, group_type4.id, account_id, 4).await;
    assert_group_type(db, group_type.id, account_id, 3).await;

    let group_type = request(2).execute(&db.conn).await?;

    assert_group_type(db, group_type1.id, account_id, 1).await;
    assert_group_type(db, group_type3.id, account_id, 3).await;
    assert_group_type(db, group_type4.id, account_id, 4).await;
    assert_group_type(db, group_type.id, account_id, 2).await;
    Ok(())
}

#[tokio::test]
async fn update_position_updates_a_group_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    update_group_type_position(&db, &ross, ross.account_id, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdateGroupTypePosition {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(update_group_type_position(&db, &ross, account.id, &group_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_gift_stage_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    assert_eq!(
        update_group_type_position(&db, &ross, ross.account_id, &group_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGroupTypeRoleTest

async fn update_group_type_role(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
    group_type_role: &group_type_roles::Model,
) -> ApiResult<()> {
    let group_type_role = UpdateGroupTypeRole {
        account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        group_type_role_id: group_type_role.id,
        label: "type label".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = group_type_roles::Entity::find_by_id(group_type_role.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.group_type_id, group_type.id);
    assert_eq!(row.label.as_deref(), Some("type label"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_group_type_role() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    update_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_role_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    let err = UpdateGroupTypeRole {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: group_type.id,
        group_type_role_id: group_type_role.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_role_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_group_type_role(&db, &ross, account.id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_role_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_role_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_group_type_role(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGroupTypeRolePositionTest

async fn update_group_type_role_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    group_type: &group_types::Model,
    group_type_role: &group_type_roles::Model,
) -> ApiResult<()> {
    let role1 = group_type_roles::ActiveModel { position: Set(Some(1)), ..group_type_role_factory(group_type.id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let role3 = group_type_roles::ActiveModel { position: Set(Some(3)), ..group_type_role_factory(group_type.id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let role4 = group_type_roles::ActiveModel { position: Set(Some(4)), ..group_type_role_factory(group_type.id) }
        .insert(&db.conn)
        .await
        .unwrap();

    let request = |new_position| UpdateGroupTypeRolePosition {
        account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        group_type_role_id: group_type_role.id,
        new_position,
    };

    let group_type_role = request(3).execute(&db.conn).await?;

    assert_group_type_role(db, role1.id, group_type.id, 1).await;
    assert_group_type_role(db, role3.id, group_type.id, 2).await;
    assert_group_type_role(db, role4.id, group_type.id, 4).await;
    assert_group_type_role(db, group_type_role.id, group_type.id, 3).await;

    let group_type_role = request(2).execute(&db.conn).await?;

    assert_group_type_role(db, role1.id, group_type.id, 1).await;
    assert_group_type_role(db, role3.id, group_type.id, 3).await;
    assert_group_type_role(db, role4.id, group_type.id, 4).await;
    assert_group_type_role(db, group_type_role.id, group_type.id, 2).await;
    Ok(())
}

#[tokio::test]
async fn update_role_position_updates_a_group_type_role() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    update_group_type_role_position(&db, &ross, ross.account_id, &group_type, &group_type_role).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_role_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdateGroupTypeRolePosition {
        account_id: ross.account_id,
        author_id: ross.id,
        group_type_id: Id(0),
        group_type_role_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_role_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let group_type = group_type_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_group_type_role_position(&db, &ross, account.id, &group_type, &group_type_role).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_role_position_fails_if_gift_stage_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let group_type = group_type_elsewhere(&db).await;
    let group_type_role = group_type_role_factory(group_type.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_group_type_role_position(&db, &ross, ross.account_id, &group_type, &group_type_role)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}
