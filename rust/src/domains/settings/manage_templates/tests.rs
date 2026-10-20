//! `tests/Unit/Domains/Settings/ManageTemplates/Services`.

use sea_orm::EntityTrait;

use super::*;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `database/factories` of this domain.
mod factories {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait};

    use crate::db::id::Id;
    use crate::db::uuid::Uuid;
    use crate::entities::{modules, template_pages, templates};
    use crate::testing::factory;

    /// `Module::factory()` in `account` (a new account when `None`).
    pub async fn module<C: ConnectionTrait>(db: &C, account: Option<Uuid>) -> modules::Model {
        let account_id = match account {
            Some(a) => a,
            None => factory::account(db).await.id,
        };
        modules::ActiveModel {
            account_id: Set(account_id),
            name: Set(Some("business".into())),
            r#type: Set(Some("business".into())),
            can_be_deleted: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("module")
    }

    /// `Template::factory()` in `account` (a new account when `None`).
    pub async fn template<C: ConnectionTrait>(db: &C, account: Option<Uuid>) -> templates::Model {
        let account_id = match account {
            Some(a) => a,
            None => factory::account(db).await.id,
        };
        templates::ActiveModel { account_id: Set(account_id), name: Set(Some("business".into())), ..Default::default() }
            .insert(db)
            .await
            .expect("template")
    }

    /// `TemplatePage::factory()` in `template` (a new template when `None`) at `position`.
    pub async fn template_page<C: ConnectionTrait>(
        db: &C,
        template: Option<Id>,
        position: i32,
    ) -> template_pages::Model {
        let template_id = match template {
            Some(t) => t,
            None => self::template(db, None).await.id,
        };
        template_pages::ActiveModel {
            template_id: Set(template_id),
            name: Set(Some("business".into())),
            slug: Set("business".into()),
            position: Set(Some(position)),
            can_be_deleted: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("template page")
    }
}

/// `$templatePage->modules()->syncWithoutDetaching([$module->id => ['position' => $position]])` on a new pair.
async fn attach(db: &TestDb, template_page_id: Id, module_id: Id, position: i32) {
    module_template_page::ActiveModel {
        template_page_id: Set(template_page_id),
        module_id: Set(module_id),
        position: Set(Some(position)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("module_template_page");
}

/// The `module_template_page` row of `module_id` on `template_page_id`.
async fn pivot(db: &TestDb, template_page_id: Id, module_id: Id) -> Option<module_template_page::Model> {
    module_template_page::Entity::find()
        .filter(module_template_page::Column::TemplatePageId.eq(template_page_id))
        .filter(module_template_page::Column::ModuleId.eq(module_id))
        .one(&db.conn)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- AssociateModuleToTemplatePageTest

async fn associate_module_to_template_page(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    module: &modules::Model,
    template: &templates::Model,
    template_page: &template_pages::Model,
) -> ApiResult<()> {
    let module = AssociateModuleToTemplatePage {
        account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page.id,
        module_id: module.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(pivot(db, template_page.id, module.id).await.unwrap().position, Some(1));
    Ok(())
}

#[tokio::test]
async fn it_associates_a_module_to_a_template_page() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    associate_module_to_template_page(&db, &ross, ross.account_id, &module, &template, &template_page).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = AssociateModuleToTemplatePage {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: Id(0),
        template_page_id: Id(0),
        module_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = associate_module_to_template_page(&db, &ross, account.id, &module, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_information_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, None).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = associate_module_to_template_page(&db, &ross, ross.account_id, &module, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let template = factories::template(&db.conn, None).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = associate_module_to_template_page(&db, &ross, ross.account_id, &module, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_template_page_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, None, 1).await;
    let err = associate_module_to_template_page(&db, &ross, ross.account_id, &module, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateModuleTest

async fn create_module(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let module = CreateModule {
        account_id,
        author_id: author.id,
        name: Some("Business".into()),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: false,
        reserved_to_contact_information: None,
    }
    .execute(&db.conn)
    .await?;

    let row = modules::Entity::find_by_id(module.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("Business"));
    assert!(!row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_module() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_module(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_module_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateModule {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: false,
        reserved_to_contact_information: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_module_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_module(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateTemplateTest

async fn create_template(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let template = CreateTemplate {
        account_id,
        author_id: author.id,
        name: Some("Business".into()),
        name_translation_key: None,
        can_be_deleted: None,
    }
    .execute(&db.conn)
    .await?;

    let row = templates::Entity::find_by_id(template.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("Business"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_template(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_template_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
        can_be_deleted: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_template_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_template(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateTemplatePageTest

async fn create_template_page(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page = CreateTemplatePage {
        account_id,
        author_id: author.id,
        template_id: template.id,
        name: Some("Business awesome".into()),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(&db.conn)
    .await?;

    let row = template_pages::Entity::find_by_id(template_page.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.template_id, template.id);
    assert_eq!(row.name.as_deref(), Some("Business awesome"));
    assert_eq!(row.slug, "business-awesome");
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_template_page() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    create_template_page(&db, &ross, ross.account_id, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_template_page_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let err = CreateTemplatePage {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: template.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
        r#type: None,
        can_be_deleted: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_template_page_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    assert_eq!(create_template_page(&db, &ross, account.id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_template_page_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, None).await;
    assert_eq!(create_template_page(&db, &ross, ross.account_id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyModuleTest

async fn destroy_module(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    module: &modules::Model,
) -> ApiResult<()> {
    DestroyModule { account_id, author_id: author.id, module_id: module.id }.execute(&db.conn).await?;
    assert!(modules::Entity::find_by_id(module.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_module() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    destroy_module(&db, &ross, ross.account_id, &module).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_module_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err =
        DestroyModule { account_id: ross.account_id, author_id: ross.id, module_id: Id(0) }.execute(&db.conn).await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_module_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    assert_eq!(destroy_module(&db, &ross, account.id, &module).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_module_fails_if_module_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let module = factories::module(&db.conn, Some(account.id)).await;
    assert_eq!(destroy_module(&db, &ross, ross.account_id, &module).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_module_fails_if_module_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let mut module: modules::ActiveModel = module.into();
    module.can_be_deleted = Set(false);
    let module = module.update(&db.conn).await.unwrap();
    assert_eq!(destroy_module(&db, &ross, ross.account_id, &module).await.unwrap_err().code, Code::Conflict);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyTemplateTest

async fn destroy_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
) -> ApiResult<()> {
    DestroyTemplate { account_id, author_id: author.id, template_id: template.id }.execute(&db.conn).await?;
    assert!(templates::Entity::find_by_id(template.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    destroy_template(&db, &ross, ross.account_id, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err =
        DestroyTemplate { account_id: ross.account_id, author_id: ross.id, template_id: Id(0) }.execute(&db.conn).await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    assert_eq!(destroy_template(&db, &ross, account.id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let template = factories::template(&db.conn, Some(account.id)).await;
    assert_eq!(destroy_template(&db, &ross, ross.account_id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

/// Not in the PHP: inventory.md F18.
#[tokio::test]
async fn destroy_template_fails_if_template_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let mut template: templates::ActiveModel = template.into();
    template.can_be_deleted = Set(false);
    let template = template.update(&db.conn).await.unwrap();
    assert_eq!(destroy_template(&db, &ross, ross.account_id, &template).await.unwrap_err().code, Code::Conflict);
    assert!(templates::Entity::find_by_id(template.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyTemplatePageTest

async fn destroy_template_page(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template_page: &template_pages::Model,
) -> ApiResult<()> {
    DestroyTemplatePage {
        account_id,
        author_id: author.id,
        template_id: template_page.template_id,
        template_page_id: template_page.id,
    }
    .execute(&db.conn)
    .await?;
    assert!(template_pages::Entity::find_by_id(template_page.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

/// `createTemplatePage($account)`.
async fn create_template_page_in(db: &TestDb, account_id: Uuid) -> template_pages::Model {
    let template = factories::template(&db.conn, Some(account_id)).await;
    factories::template_page(&db.conn, Some(template.id), 1).await
}

#[tokio::test]
async fn destroy_template_page_destroys_a_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template_page = create_template_page_in(&db, ross.account_id).await;
    destroy_template_page(&db, &ross, ross.account_id, &template_page).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_page_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = DestroyTemplatePage {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: Id(0),
        template_page_id: Id(0),
    }
    .execute(&db.conn)
    .await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_page_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template_page = create_template_page_in(&db, ross.account_id).await;
    assert_eq!(destroy_template_page(&db, &ross, account.id, &template_page).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_template_page_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let template_page = create_template_page_in(&db, account.id).await;
    let err = destroy_template_page(&db, &ross, ross.account_id, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

/// Not in the PHP: inventory.md F18.
#[tokio::test]
async fn destroy_template_page_fails_if_template_page_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template_page = create_template_page_in(&db, ross.account_id).await;
    let mut template_page: template_pages::ActiveModel = template_page.into();
    template_page.can_be_deleted = Set(false);
    let template_page = template_page.update(&db.conn).await.unwrap();
    let err = destroy_template_page(&db, &ross, ross.account_id, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::Conflict);
    assert!(template_pages::Entity::find_by_id(template_page.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveModuleFromTemplatePageTest

type Association = (templates::Model, template_pages::Model, modules::Model);

async fn remove_module_from_template_page(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    (template, template_page, module): &Association,
) -> ApiResult<()> {
    RemoveModuleFromTemplatePage {
        account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page.id,
        module_id: module.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(pivot(db, template_page.id, module.id).await.is_none());
    Ok(())
}

/// `associateTemplatePageAndModule($accountA, $accountB)`.
async fn associate_template_page_and_module(db: &TestDb, account_a: Uuid, account_b: Uuid) -> Association {
    let template = factories::template(&db.conn, Some(account_a)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let module = factories::module(&db.conn, Some(account_b)).await;

    attach(db, template_page.id, module.id, 1).await;
    (template, template_page, module)
}

#[tokio::test]
async fn it_removes_a_module_from_a_template_page() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let association = associate_template_page_and_module(&db, ross.account_id, ross.account_id).await;
    remove_module_from_template_page(&db, &ross, ross.account_id, &association).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn remove_module_from_template_page_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = RemoveModuleFromTemplatePage {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: Id(0),
        template_page_id: Id(0),
        module_id: Id(0),
    }
    .execute(&db.conn)
    .await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_module_from_template_page_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let association = associate_template_page_and_module(&db, account.id, account.id).await;
    let err = remove_module_from_template_page(&db, &ross, account.id, &association).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_module_from_template_page_fails_if_information_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let association = associate_template_page_and_module(&db, ross.account_id, account.id).await;
    let err = remove_module_from_template_page(&db, &ross, ross.account_id, &association).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_module_from_template_page_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let association = associate_template_page_and_module(&db, account.id, ross.account_id).await;
    let err = remove_module_from_template_page(&db, &ross, ross.account_id, &association).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

/// Not in the PHP: inventory.md F11.
#[tokio::test]
async fn remove_module_from_template_page_fails_if_module_isnt_on_the_page() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let association = (template, template_page, module);
    let err = remove_module_from_template_page(&db, &ross, ross.account_id, &association).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    assert!(pivot(&db, association.1.id, association.2.id).await.is_none());
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateModuleTest

async fn update_module(db: &TestDb, author: &users::Model, account_id: Uuid, module: &modules::Model) -> ApiResult<()> {
    UpdateModule { account_id, author_id: author.id, module_id: module.id, name: "name".into() }
        .execute(&db.conn)
        .await?;

    let row = modules::Entity::find_by_id(module.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_module() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    update_module(&db, &ross, ross.account_id, &module).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_module_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    let err =
        UpdateModule { account_id: ross.account_id, author_id: ross.id, module_id: module.id, name: String::new() }
            .execute(&db.conn)
            .await;
    assert_eq!(err.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_module_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    assert_eq!(update_module(&db, &ross, account.id, &module).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_module_fails_if_module_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let module = factories::module(&db.conn, Some(account.id)).await;
    assert_eq!(update_module(&db, &ross, ross.account_id, &module).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateModulePositionTest

async fn update_module_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
    template_page: &template_pages::Model,
    module: &modules::Model,
) -> ApiResult<()> {
    let module1 = factories::module(&db.conn, Some(account_id)).await;
    attach(db, template_page.id, module1.id, 1).await;
    let module3 = factories::module(&db.conn, Some(account_id)).await;
    attach(db, template_page.id, module3.id, 3).await;
    let module4 = factories::module(&db.conn, Some(account_id)).await;
    attach(db, template_page.id, module4.id, 4).await;

    let request = |new_position| UpdateModulePosition {
        account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page.id,
        module_id: module.id,
        new_position,
    };

    let module = request(3).execute(&db.conn).await?;

    assert_eq!(pivot(db, template_page.id, module1.id).await.unwrap().position, Some(1));
    assert_eq!(pivot(db, template_page.id, module3.id).await.unwrap().position, Some(2));
    assert_eq!(pivot(db, template_page.id, module4.id).await.unwrap().position, Some(4));
    assert_eq!(pivot(db, template_page.id, module.id).await.unwrap().position, Some(3));

    let module = request(2).execute(&db.conn).await?;

    assert_eq!(pivot(db, template_page.id, module1.id).await.unwrap().position, Some(1));
    assert_eq!(pivot(db, template_page.id, module3.id).await.unwrap().position, Some(3));
    assert_eq!(pivot(db, template_page.id, module4.id).await.unwrap().position, Some(4));
    assert_eq!(pivot(db, template_page.id, module.id).await.unwrap().position, Some(2));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_module_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    attach(&db, template_page.id, module.id, 2).await;
    update_module_position(&db, &ross, ross.account_id, &template, &template_page, &module).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_module_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdateModulePosition {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: Id(0),
        template_page_id: Id(0),
        module_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_module_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    attach(&db, template_page.id, module.id, 2).await;
    let err = update_module_position(&db, &ross, account.id, &template, &template_page, &module).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_module_position_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, None).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    attach(&db, template_page.id, module.id, 2).await;
    let err = update_module_position(&db, &ross, ross.account_id, &template, &template_page, &module).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_module_position_fails_if_template_page_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, None, 1).await;
    let module = factories::module(&db.conn, Some(ross.account_id)).await;
    attach(&db, template_page.id, module.id, 2).await;
    let err = update_module_position(&db, &ross, ross.account_id, &template, &template_page, &module).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_module_position_fails_if_module_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, None, 1).await;
    let module = factories::module(&db.conn, None).await;
    let err = update_module_position(&db, &ross, ross.account_id, &template, &template_page, &module).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateTemplateTest

async fn update_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
) -> ApiResult<()> {
    UpdateTemplate { account_id, author_id: author.id, template_id: template.id, name: "name".into() }
        .execute(&db.conn)
        .await?;

    let row = templates::Entity::find_by_id(template.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    update_template(&db, &ross, ross.account_id, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_template_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let err = UpdateTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: template.id,
        name: String::new(),
    }
    .execute(&db.conn)
    .await;
    assert_eq!(err.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_template_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    assert_eq!(update_template(&db, &ross, account.id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let template = factories::template(&db.conn, Some(account.id)).await;
    assert_eq!(update_template(&db, &ross, ross.account_id, &template).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateTemplatePageTest

async fn update_template_page(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
    template_page: &template_pages::Model,
) -> ApiResult<()> {
    let template_page = UpdateTemplatePage {
        account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page.id,
        name: "Business".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = template_pages::Entity::find_by_id(template_page.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.template_id, template.id);
    assert_eq!(row.name.as_deref(), Some("Business"));
    assert_eq!(row.slug, "business");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_template_page() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    update_template_page(&db, &ross, ross.account_id, &template, &template_page).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = UpdateTemplatePage {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: template.id,
        template_page_id: template_page.id,
        name: String::new(),
    }
    .execute(&db.conn)
    .await;
    assert_eq!(err.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = update_template_page(&db, &ross, account.id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, None).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = update_template_page(&db, &ross, ross.account_id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_fails_if_template_page_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, None, 1).await;
    let err = update_template_page(&db, &ross, ross.account_id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateTemplatePagePositionTest

async fn update_template_page_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    template: &templates::Model,
    template_page: &template_pages::Model,
) -> ApiResult<()> {
    let template_page1 = factories::template_page(&db.conn, Some(template.id), 1).await;
    let template_page3 = factories::template_page(&db.conn, Some(template.id), 3).await;
    let template_page4 = factories::template_page(&db.conn, Some(template.id), 4).await;

    let request = |new_position| UpdateTemplatePagePosition {
        account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page.id,
        new_position,
    };
    let position = |page: Id| async move {
        let row = template_pages::Entity::find_by_id(page).one(&db.conn).await.unwrap().unwrap();
        assert_eq!(row.template_id, template.id);
        row.position
    };

    let template_page = request(3).execute(&db.conn).await?;

    assert_eq!(position(template_page1.id).await, Some(1));
    assert_eq!(position(template_page3.id).await, Some(2));
    assert_eq!(position(template_page4.id).await, Some(4));
    assert_eq!(position(template_page.id).await, Some(3));

    let template_page = request(2).execute(&db.conn).await?;

    assert_eq!(position(template_page1.id).await, Some(1));
    assert_eq!(position(template_page3.id).await, Some(3));
    assert_eq!(position(template_page4.id).await, Some(4));
    assert_eq!(position(template_page.id).await, Some(2));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_template_page_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 2).await;
    update_template_page_position(&db, &ross, ross.account_id, &template, &template_page).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_position_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = UpdateTemplatePagePosition {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: Id(0),
        template_page_id: Id(0),
        new_position: 1,
    }
    .execute(&db.conn)
    .await;
    // A missing id is a 404, not a validation error (porting.md, `exists`).
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = update_template_page_position(&db, &ross, account.id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_position_fails_if_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, None).await;
    let template_page = factories::template_page(&db.conn, Some(template.id), 1).await;
    let err = update_template_page_position(&db, &ross, ross.account_id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_template_page_position_fails_if_template_page_doesnt_belong_to_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let template = factories::template(&db.conn, Some(ross.account_id)).await;
    let template_page = factories::template_page(&db.conn, None, 1).await;
    let err = update_template_page_position(&db, &ross, ross.account_id, &template, &template_page).await;
    assert_eq!(err.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
