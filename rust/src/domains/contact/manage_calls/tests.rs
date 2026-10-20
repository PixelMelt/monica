//! `tests/Unit/Domains/Contact/ManageCalls/Services`.

use chrono::NaiveDate;
use sea_orm::EntityTrait;

use super::*;
use crate::crypto::random_string;
use crate::entities::{call_reason_types, vaults};
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

/// `Emotion::factory()` in `account_id`.
async fn emotion(db: &TestDb, account_id: Uuid) -> emotions::Model {
    emotions::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        r#type: Set(format!("Name {}", random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("emotion")
}

/// `Call::factory()` for `contact`, with a new author and a new call reason.
async fn call(db: &TestDb, contact: &contacts::Model) -> calls::Model {
    let author = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let reason = call_reason(db, call_reason_type(db, account.id).await.id).await;
    calls::ActiveModel {
        contact_id: Set(contact.id),
        author_id: Set(Some(author.id)),
        call_reason_id: Set(Some(reason.id)),
        author_name: Set(format!("Name {}", random_string(6))),
        called_at: Set(crate::db::now()),
        duration: Set(Some(100)),
        r#type: Set("audio".into()),
        answered: Set(true),
        who_initiated: Set("me".into()),
        description: Set(Some(format!("Sentence {}", random_string(10)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("call")
}

/// `Contact::factory()->create()`: a contact in a new vault of a new account.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `$regis` with `permission` in a new vault of their account.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

fn ymd(y: i32, m: u32, d: u32) -> Timestamp {
    Timestamp(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(0, 0, 0).unwrap())
}

async fn call_row(db: &TestDb, id: Id) -> calls::Model {
    calls::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---- CreateCallTest

fn create_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    reason: &call_reasons::Model,
) -> CreateCall {
    CreateCall {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        call_reason_id: Some(reason.id),
        called_at: "1999-01-01".into(),
        duration: Some(100),
        description: None,
        r#type: "audio".into(),
        answered: Some(true),
        who_initiated: "contact".into(),
        emotion_id: None,
    }
}

async fn create_call(db: &TestDb, request: CreateCall) -> ApiResult<()> {
    let (contact_id, emotion_id) = (request.contact_id, request.emotion_id);
    let call = request.execute(&db.conn).await?;

    let row = call_row(db, call.id).await;
    assert_eq!(row.contact_id, contact_id);
    assert_eq!(row.called_at, ymd(1999, 1, 1));
    assert_eq!(row.duration, Some(100));
    assert_eq!(row.r#type, "audio");
    assert!(row.answered);
    assert_eq!(row.who_initiated, "contact");
    assert_eq!(row.emotion_id, emotion_id);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_call() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    create_call(&db, create_request(&regis, regis.account_id, &vault, &contact, &reason)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_call_with_emotion() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let emotion = emotion(&db, regis.account_id).await;
    create_call(
        &db,
        CreateCall {
            emotion_id: Some(emotion.id),
            ..create_request(&regis, regis.account_id, &vault, &contact, &reason)
        },
    )
    .await
    .unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reason = call_reason(&db, call_reason_type(&db, regis.account_id).await.id).await;
    let request = create_request(&regis, regis.account_id, &vault, &contact, &reason);
    let err = CreateCall { r#type: String::new(), ..request }.execute(&db.conn).await.unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err = create_call(&db, create_request(&regis, account.id, &vault, &contact, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err = create_call(&db, create_request(&regis, regis.account_id, &vault, &contact, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err = create_call(&db, create_request(&regis, regis.account_id, &vault, &contact, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyCallTest

async fn destroy_call(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    call: &calls::Model,
) -> ApiResult<()> {
    DestroyCall { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, call_id: call.id }
        .execute(&db.conn)
        .await?;

    assert!(calls::Entity::find_by_id(call.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    destroy_call(&db, &regis, regis.account_id, &vault, &contact, &call).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let err = destroy_call(&db, &regis, account.id, &vault, &contact, &call).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let call = call(&db, &contact).await;
    let err = destroy_call(&db, &regis, regis.account_id, &vault, &contact, &call).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let err = destroy_call(&db, &regis, regis.account_id, &vault, &contact, &call).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &other_contact(&db).await).await;
    let err = destroy_call(&db, &regis, regis.account_id, &vault, &contact, &call).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- UpdateCallTest

fn update_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    call: &calls::Model,
    reason: &call_reasons::Model,
) -> UpdateCall {
    UpdateCall {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        call_id: call.id,
        call_reason_id: Some(reason.id),
        called_at: "1999-01-01".into(),
        duration: Some(100),
        description: None,
        r#type: "audio".into(),
        answered: Some(true),
        who_initiated: "contact".into(),
        emotion_id: None,
    }
}

async fn update_call(db: &TestDb, request: UpdateCall) -> ApiResult<()> {
    let (contact_id, emotion_id) = (request.contact_id, request.emotion_id);
    let call = request.execute(&db.conn).await?;

    let row = call_row(db, call.id).await;
    assert_eq!(row.contact_id, contact_id);
    assert_eq!(row.called_at, ymd(1999, 1, 1));
    assert_eq!(row.duration, Some(100));
    assert_eq!(row.r#type, "audio");
    assert!(row.answered);
    assert_eq!(row.who_initiated, "contact");
    assert_eq!(row.emotion_id, emotion_id);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    update_call(&db, update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_updates_a_task_with_emotion() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let emotion = emotion(&db, regis.account_id).await;
    update_call(
        &db,
        UpdateCall {
            emotion_id: Some(emotion.id),
            ..update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)
        },
    )
    .await
    .unwrap();
    db.drop().await;
}

/// F8: UpdateCall saves `description`.
#[tokio::test]
async fn update_saves_the_description() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let reason = call_reason(&db, call_reason_type(&db, regis.account_id).await.id).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &call, &reason);
    UpdateCall { description: Some("we talked about the weather".into()), ..request }.execute(&db.conn).await.unwrap();
    assert_eq!(call_row(&db, call.id).await.description.as_deref(), Some("we talked about the weather"));
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let reason = call_reason(&db, call_reason_type(&db, regis.account_id).await.id).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &call, &reason);
    let err = UpdateCall { called_at: "01/01/1999".into(), who_initiated: String::new(), ..request }
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
    let call = call(&db, &contact).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err = update_call(&db, update_request(&regis, account.id, &vault, &contact, &call, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let call = call(&db, &contact).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err =
        update_call(&db, update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &contact).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err =
        update_call(&db, update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_reminder_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &other_contact(&db).await).await;
    let r#type = call_reason_type(&db, regis.account_id).await;
    let reason = call_reason(&db, r#type.id).await;
    let err =
        update_call(&db, update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_call_reason_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let call = call(&db, &other_contact(&db).await).await;
    let account = factory::account(&db.conn).await;
    let reason = call_reason(&db, call_reason_type(&db, account.id).await.id).await;
    let err =
        update_call(&db, update_request(&regis, regis.account_id, &vault, &contact, &call, &reason)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
