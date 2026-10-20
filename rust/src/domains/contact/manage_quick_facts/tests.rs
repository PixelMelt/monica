//! `tests/Unit/Domains/Contact/ManageQuickFacts/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `VaultQuickFactsTemplate::factory()` in `vault_id` (a new vault when `None`).
async fn template(db: &TestDb, vault_id: Option<Uuid>) -> vault_quick_facts_templates::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => factory::create_vault(&db.conn, factory::create_account(&db.conn).await.id).await.id,
    };
    vault_quick_facts_templates::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(1),
        label_translation_key: Set(Some("vault_quick_fact_template.label_translation_key".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("vault quick facts template")
}

/// `QuickFact::factory()`: a new template and contact unless given.
async fn quick_fact(db: &TestDb, contact_id: Option<Uuid>, template_id: Option<Id>) -> quick_facts::Model {
    let template_id = match template_id {
        Some(id) => id,
        None => template(db, None).await.id,
    };
    let contact_id = match contact_id {
        Some(id) => id,
        None => other_contact(db).await.id,
    };
    quick_facts::ActiveModel {
        vault_quick_facts_template_id: Set(template_id),
        contact_id: Set(contact_id),
        content: Set("birthdate".into()),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("quick fact")
}

/// `createUser()`, `createVault()` and `setPermissionInVault()` with `permission`.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

// ---------------------------------------------------------------- CreateQuickFactTest

async fn create_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    contact: &contacts::Model,
    vault: &vaults::Model,
    template: &vault_quick_facts_templates::Model,
) -> ApiResult<()> {
    let quick_fact = CreateQuickFact {
        account_id,
        author_id: author.id,
        contact_id: contact.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: template.id,
        content: "test".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = quick_facts::Entity::find_by_id(quick_fact.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_quick_facts_template_id, template.id);
    assert_eq!(row.content, "test");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_quick_fact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    create_quick_fact(&db, &regis, regis.account_id, &contact, &vault, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    let err = CreateQuickFact {
        account_id: regis.account_id,
        author_id: regis.id,
        contact_id: contact.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: template.id,
        content: String::new(),
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
    let template = template(&db, Some(vault.id)).await;
    let err = create_quick_fact(&db, &regis, account.id, &contact, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    let err = create_quick_fact(&db, &regis, regis.account_id, &contact, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_template_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, None).await;
    let err = create_quick_fact(&db, &regis, regis.account_id, &contact, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let template = template(&db, Some(vault.id)).await;
    let err = create_quick_fact(&db, &regis, regis.account_id, &contact, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyQuickFactTest

async fn destroy_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    quick_fact: &quick_facts::Model,
) -> ApiResult<()> {
    DestroyQuickFact {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        quick_fact_id: quick_fact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(quick_facts::Entity::find_by_id(quick_fact.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_deletes_a_quick_fact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let quick_fact = quick_fact(&db, Some(contact.id), None).await;
    destroy_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let quick_fact = quick_fact(&db, Some(contact.id), None).await;
    let err = destroy_quick_fact(&db, &regis, account.id, &vault, &contact, &quick_fact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let quick_fact = quick_fact(&db, Some(contact.id), None).await;
    let err = destroy_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let quick_fact = quick_fact(&db, Some(contact.id), None).await;
    let err = destroy_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_quick_fact_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let quick_fact = quick_fact(&db, None, None).await;
    let err = destroy_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleQuickFactModuleTest

async fn toggle_module(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let request =
        || ToggleQuickFactModule { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id };

    request().execute(&db.conn).await?;
    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert!(row.show_quick_facts);

    request().execute(&db.conn).await?;
    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert!(!row.show_quick_facts);
    Ok(())
}

#[tokio::test]
async fn it_toggles_the_module() {
    let db = TestDb::migrated().await;
    let (user, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    toggle_module(&db, &user, user.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// toggle_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn toggle_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_module(&db, &user, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Edit, &vault).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_module(&db, &user, user.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = toggle_module(&db, &user, user.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateQuickFactTest

async fn update_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    quick_fact: &quick_facts::Model,
    template: &vault_quick_facts_templates::Model,
) -> ApiResult<()> {
    let quick_fact = UpdateQuickFact {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        quick_fact_id: quick_fact.id,
        content: "love".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = quick_facts::Entity::find_by_id(quick_fact.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_quick_facts_template_id, template.id);
    assert_eq!(row.content, "love");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_quick_fact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, Some(contact.id), Some(template.id)).await;
    update_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, Some(contact.id), Some(template.id)).await;
    let err = UpdateQuickFact {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        quick_fact_id: quick_fact.id,
        content: String::new(),
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
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, Some(contact.id), Some(template.id)).await;
    let err = update_quick_fact(&db, &regis, account.id, &vault, &contact, &quick_fact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, Some(contact.id), Some(template.id)).await;
    let err =
        update_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, Some(contact.id), Some(template.id)).await;
    let err =
        update_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_quick_fact_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let template = template(&db, Some(vault.id)).await;
    let quick_fact = quick_fact(&db, None, Some(template.id)).await;
    let err =
        update_quick_fact(&db, &regis, regis.account_id, &vault, &contact, &quick_fact, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
