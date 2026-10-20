//! `tests/Unit/Domains/Settings/ManagePostTemplates/Services`.

use super::*;
use crate::entities::users;
use crate::testing::{TestDb, factory};

/// `PostTemplate::factory()` in `account_id`.
fn post_template_factory(account_id: Uuid) -> post_templates::ActiveModel {
    post_templates::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some("business".into())),
        position: Set(1),
        can_be_deleted: Set(false),
        ..Default::default()
    }
}

/// `PostTemplateSection::factory()` in `post_template_id`.
fn post_template_section_factory(post_template_id: Id) -> post_template_sections::ActiveModel {
    post_template_sections::ActiveModel {
        post_template_id: Set(post_template_id),
        label: Set(Some("business".into())),
        position: Set(1),
        ..Default::default()
    }
}

/// `PostTemplate::factory()->create()`: in a new account.
async fn post_template_elsewhere(db: &TestDb) -> post_templates::Model {
    let account = factory::account(&db.conn).await;
    post_template_factory(account.id).insert(&db.conn).await.unwrap()
}

/// `PostTemplateSection::factory()->create()`: in a new post template.
async fn post_template_section_elsewhere(db: &TestDb) -> post_template_sections::Model {
    let post_template = post_template_elsewhere(db).await;
    post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap()
}

/// `assertDatabaseHas('post_templates', [id, account_id, position])`.
async fn assert_post_template(db: &TestDb, id: Id, account_id: Uuid, position: i32) {
    let row = post_templates::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!((row.account_id, row.position), (account_id, position));
}

/// `assertDatabaseHas('post_template_sections', [id, post_template_id, position])`.
async fn assert_post_template_section(db: &TestDb, id: Id, post_template_id: Id, position: i32) {
    let row = post_template_sections::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!((row.post_template_id, row.position), (post_template_id, position));
}

// ---------------------------------------------------------------- CreatePostTemplateTest

async fn create_post_template(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let post_template = CreatePostTemplate {
        account_id,
        author_id: author.id,
        label: Some("Business".into()),
        label_translation_key: None,
        can_be_deleted: false,
    }
    .execute(&db.conn)
    .await?;

    let row = post_templates::Entity::find_by_id(post_template.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("Business"));
    assert_eq!(row.position, 1);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_post_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_post_template(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreatePostTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        label: Some("x".repeat(256)),
        label_translation_key: None,
        can_be_deleted: false,
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
    assert_eq!(create_post_template(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreatePostTemplateSectionTest

async fn create_post_template_section(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
) -> ApiResult<()> {
    let post_template_section = CreatePostTemplateSection {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        label: Some("Business awesome".into()),
        label_translation_key: None,
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row =
        post_template_sections::Entity::find_by_id(post_template_section.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.post_template_id, post_template.id);
    assert_eq!(row.label.as_deref(), Some("Business awesome"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_post_template_section() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    create_post_template_section(&db, &ross, ross.account_id, &post_template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_section_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreatePostTemplateSection {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: Id(0),
        label: Some("x".repeat(256)),
        label_translation_key: None,
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_section_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(
        create_post_template_section(&db, &ross, account.id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn create_section_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    assert_eq!(
        create_post_template_section(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyPostTemplateTest

async fn destroy_post_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
) -> ApiResult<()> {
    DestroyPostTemplate { account_id, author_id: author.id, post_template_id: post_template.id }
        .execute(&db.conn)
        .await?;
    assert!(post_templates::Entity::find_by_id(post_template.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_post_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template =
        post_templates::ActiveModel { can_be_deleted: Set(true), ..post_template_factory(ross.account_id) }
            .insert(&db.conn)
            .await
            .unwrap();
    destroy_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = DestroyPostTemplate { account_id: ross.account_id, author_id: ross.id, post_template_id: Id(0) }
        .execute(&db.conn)
        .await
        .unwrap_err();
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let post_template = post_template_factory(account.id).insert(&db.conn).await.unwrap();
    assert_eq!(destroy_post_template(&db, &ross, account.id, &post_template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    assert_eq!(
        destroy_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_post_template_cannot_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::Conflict
    );
    assert!(post_templates::Entity::find_by_id(post_template.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyPostTemplateSectionTest

async fn destroy_post_template_section(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
    post_template_section: &post_template_sections::Model,
) -> ApiResult<()> {
    DestroyPostTemplateSection {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        post_template_section_id: post_template_section.id,
    }
    .execute(&db.conn)
    .await?;
    assert!(
        post_template_sections::Entity::find_by_id(post_template_section.id).one(&db.conn).await.unwrap().is_none()
    );
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_post_template_section() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    destroy_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_section_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = DestroyPostTemplateSection {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: Id(0),
        post_template_section_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_section_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_post_template_section(&db, &ross, account.id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_section_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        destroy_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_section_fails_if_post_template_section_doesnt_belong_to_post_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_elsewhere(&db).await;
    assert_eq!(
        destroy_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_section_fails_if_post_template_section_cannot_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_sections::ActiveModel {
        can_be_deleted: Set(false),
        ..post_template_section_factory(post_template.id)
    }
    .insert(&db.conn)
    .await
    .unwrap();
    assert_eq!(
        destroy_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::Conflict
    );
    assert!(
        post_template_sections::Entity::find_by_id(post_template_section.id).one(&db.conn).await.unwrap().is_some()
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostTemplateTest

async fn update_post_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
) -> ApiResult<()> {
    let post_template = UpdatePostTemplate {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        label: "Business".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = post_templates::Entity::find_by_id(post_template.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.label.as_deref(), Some("Business"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    update_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let err = UpdatePostTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: post_template.id,
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
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(update_post_template(&db, &ross, account.id, &post_template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    assert_eq!(
        update_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_post_template_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    assert_eq!(
        update_post_template(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostTemplatePositionTest

async fn update_post_template_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
) -> ApiResult<()> {
    let post_template1 = post_templates::ActiveModel { position: Set(1), ..post_template_factory(account_id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let post_template3 = post_templates::ActiveModel { position: Set(3), ..post_template_factory(account_id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let post_template4 = post_templates::ActiveModel { position: Set(4), ..post_template_factory(account_id) }
        .insert(&db.conn)
        .await
        .unwrap();

    let request = |new_position| UpdatePostTemplatePosition {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        new_position,
    };

    let post_template = request(3).execute(&db.conn).await?;

    assert_post_template(db, post_template1.id, account_id, 1).await;
    assert_post_template(db, post_template3.id, account_id, 2).await;
    assert_post_template(db, post_template4.id, account_id, 4).await;
    assert_post_template(db, post_template.id, account_id, 3).await;

    let post_template = request(2).execute(&db.conn).await?;

    assert_post_template(db, post_template1.id, account_id, 1).await;
    assert_post_template(db, post_template3.id, account_id, 3).await;
    assert_post_template(db, post_template4.id, account_id, 4).await;
    assert_post_template(db, post_template.id, account_id, 2).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post_template_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_templates::ActiveModel { position: Set(2), ..post_template_factory(ross.account_id) }
        .insert(&db.conn)
        .await
        .unwrap();
    update_post_template_position(&db, &ross, ross.account_id, &post_template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdatePostTemplatePosition {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_post_template_position(&db, &ross, account.id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_position_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    assert_eq!(
        update_post_template_position(&db, &ross, ross.account_id, &post_template).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostTemplateSectionTest

async fn update_post_template_section(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
    post_template_section: &post_template_sections::Model,
) -> ApiResult<()> {
    UpdatePostTemplateSection {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        post_template_section_id: post_template_section.id,
        label: "name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row =
        post_template_sections::Entity::find_by_id(post_template_section.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.post_template_id, post_template.id);
    assert_eq!(row.label.as_deref(), Some("name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post_template_section() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    update_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_section_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    let err = UpdatePostTemplateSection {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: post_template.id,
        post_template_section_id: post_template_section.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_section_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(account.id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_post_template_section(&db, &ross, account.id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_section_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_section_fails_if_post_template_section_doesnt_belong_to_post_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let post_template = post_template_factory(account.id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_elsewhere(&db).await;
    assert_eq!(
        update_post_template_section(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostTemplateSectionPositionTest

async fn update_post_template_section_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    post_template: &post_templates::Model,
    post_template_section: &post_template_sections::Model,
) -> ApiResult<()> {
    let section1 =
        post_template_sections::ActiveModel { position: Set(1), ..post_template_section_factory(post_template.id) }
            .insert(&db.conn)
            .await
            .unwrap();
    let section3 =
        post_template_sections::ActiveModel { position: Set(3), ..post_template_section_factory(post_template.id) }
            .insert(&db.conn)
            .await
            .unwrap();
    let section4 =
        post_template_sections::ActiveModel { position: Set(4), ..post_template_section_factory(post_template.id) }
            .insert(&db.conn)
            .await
            .unwrap();

    let request = |new_position| UpdatePostTemplateSectionPosition {
        account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        post_template_section_id: post_template_section.id,
        new_position,
    };

    let post_template_section = request(3).execute(&db.conn).await?;

    assert_post_template_section(db, section1.id, post_template.id, 1).await;
    assert_post_template_section(db, section3.id, post_template.id, 2).await;
    assert_post_template_section(db, section4.id, post_template.id, 4).await;
    assert_post_template_section(db, post_template_section.id, post_template.id, 3).await;

    let post_template_section = request(2).execute(&db.conn).await?;

    assert_post_template_section(db, section1.id, post_template.id, 1).await;
    assert_post_template_section(db, section3.id, post_template.id, 3).await;
    assert_post_template_section(db, section4.id, post_template.id, 4).await;
    assert_post_template_section(db, post_template_section.id, post_template.id, 2).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post_template_section_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section =
        post_template_sections::ActiveModel { position: Set(2), ..post_template_section_factory(post_template.id) }
            .insert(&db.conn)
            .await
            .unwrap();
    update_post_template_section_position(&db, &ross, ross.account_id, &post_template, &post_template_section)
        .await
        .unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_section_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdatePostTemplateSectionPosition {
        account_id: ross.account_id,
        author_id: ross.id,
        post_template_id: Id(0),
        post_template_section_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_section_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_post_template_section_position(&db, &ross, account.id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_section_position_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_elsewhere(&db).await;
    let post_template_section = post_template_section_factory(post_template.id).insert(&db.conn).await.unwrap();
    assert_eq!(
        update_post_template_section_position(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_section_position_fails_if_template_page_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let post_template = post_template_factory(ross.account_id).insert(&db.conn).await.unwrap();
    let post_template_section = post_template_section_elsewhere(&db).await;
    assert_eq!(
        update_post_template_section_position(&db, &ross, ross.account_id, &post_template, &post_template_section)
            .await
            .unwrap_err()
            .code,
        Code::NotFound
    );
    db.drop().await;
}
