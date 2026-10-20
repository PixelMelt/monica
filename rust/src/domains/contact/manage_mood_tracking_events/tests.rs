//! `tests/Unit/Domains/Contact/ManageMoodTrackingEvents/Services`.

use chrono::NaiveDate;
use sea_orm::EntityTrait;

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `MoodTrackingParameter::factory()` in `vault_id`.
async fn mood_tracking_parameter(db: &TestDb, vault_id: Uuid) -> mood_tracking_parameters::Model {
    mood_tracking_parameters::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        hex_color: Set(format!("Name {}", random_string(6))),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("mood tracking parameter")
}

/// `MoodTrackingEvent::factory()` for `contact` with `parameter`.
async fn mood_tracking_event(
    db: &TestDb,
    contact: &contacts::Model,
    parameter: &mood_tracking_parameters::Model,
) -> mood_tracking_events::Model {
    mood_tracking_events::ActiveModel {
        contact_id: Set(contact.id),
        mood_tracking_parameter_id: Set(parameter.id),
        rated_at: Set(crate::db::now()),
        note: Set(Some(format!("Name {}", random_string(6)))),
        number_of_hours_slept: Set(Some(rand::random_range(0..100_000))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("mood tracking event")
}

/// `Vault::factory()->create()`: a vault of a new account.
async fn other_vault(db: &TestDb) -> vaults::Model {
    let account = factory::account(&db.conn).await;
    factory::vault(&db.conn, account.id).await
}

/// `Contact::factory()->create()`: a contact in a new vault of a new account.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let vault = other_vault(db).await;
    factory::contact(&db.conn, &vault).await
}

/// `$regis` with `permission` in a new vault of their account.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

/// The event row has the values the PHP tests send.
async fn assert_event(db: &TestDb, id: Id, contact: &contacts::Model, parameter: &mood_tracking_parameters::Model) {
    let row = mood_tracking_events::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.mood_tracking_parameter_id, parameter.id);
    assert_eq!(row.rated_at, Timestamp(NaiveDate::from_ymd_opt(1982, 2, 3).unwrap().and_hms_opt(0, 0, 0).unwrap()));
    assert_eq!(row.note.as_deref(), Some("fou"));
    assert_eq!(row.number_of_hours_slept, Some(3));
}

async fn assert_feed_item(db: &TestDb, contact: &contacts::Model, action: &str) {
    let item = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .filter(contact_feed_items::Column::Action.eq(action))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(item.is_some());
}

// ---- CreateMoodTrackingEventTest

fn create_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    parameter: &mood_tracking_parameters::Model,
) -> CreateMoodTrackingEvent {
    CreateMoodTrackingEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        mood_tracking_parameter_id: parameter.id,
        rated_at: "1982-02-03".into(),
        note: Some("fou".into()),
        number_of_hours_slept: Some(3),
    }
}

async fn create_event(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    parameter: &mood_tracking_parameters::Model,
) -> ApiResult<()> {
    let event = create_request(author, account_id, vault, contact, parameter).execute(&db.conn).await?;

    assert_event(db, event.id, contact, parameter).await;
    assert_feed_item(db, contact, ACTION_MOOD_TRACKING_EVENT_CREATED).await;
    Ok(())
}

#[tokio::test]
async fn it_creates_a_mood_tracking_event() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    create_event(&db, &regis, regis.account_id, &vault, &contact, &parameter).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let request = create_request(&regis, regis.account_id, &vault, &contact, &parameter);
    let err = CreateMoodTrackingEvent { rated_at: String::new(), ..request }.execute(&db.conn).await.unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let err = create_event(&db, &regis, account.id, &vault, &contact, &parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let err = create_event(&db, &regis, regis.account_id, &vault, &contact, &parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let err = create_event(&db, &regis, regis.account_id, &vault, &contact, &parameter).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let err = create_event(&db, &regis, regis.account_id, &vault, &contact, &parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- DestroyMoodTrackingEventTest

async fn destroy_event(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    event: &mood_tracking_events::Model,
) -> ApiResult<()> {
    DestroyMoodTrackingEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        mood_tracking_event_id: event.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(mood_tracking_events::Entity::find_by_id(event.id).one(&db.conn).await.unwrap().is_none());
    assert_feed_item(db, contact, ACTION_MOOD_TRACKING_EVENT_DESTROYED).await;
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_mood_tracking_event() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    destroy_event(&db, &regis, regis.account_id, &vault, &contact, &event).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let err = destroy_event(&db, &regis, account.id, &vault, &contact, &event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let err = destroy_event(&db, &regis, regis.account_id, &vault, &contact, &event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let err = destroy_event(&db, &regis, regis.account_id, &vault, &contact, &event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_event_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &other_contact(&db).await, &parameter).await;
    let err = destroy_event(&db, &regis, regis.account_id, &vault, &contact, &event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- UpdateMoodTrackingEventTest

fn update_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    event: &mood_tracking_events::Model,
    parameter: &mood_tracking_parameters::Model,
) -> UpdateMoodTrackingEvent {
    UpdateMoodTrackingEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        mood_tracking_parameter_id: parameter.id,
        mood_tracking_event_id: event.id,
        rated_at: "1982-02-03".into(),
        note: Some("fou".into()),
        number_of_hours_slept: Some(3),
    }
}

async fn update_event(
    db: &TestDb,
    request: UpdateMoodTrackingEvent,
    contact: &contacts::Model,
    parameter: &mood_tracking_parameters::Model,
) -> ApiResult<()> {
    let event = request.execute(&db.conn).await?;

    assert_event(db, event.id, contact, parameter).await;
    assert_feed_item(db, contact, ACTION_MOOD_TRACKING_EVENT_UPDATED).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_mood_tracking_event() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    update_event(&db, request, &contact, &parameter).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    let err = UpdateMoodTrackingEvent { rated_at: "1982-2-3".into(), ..request }.execute(&db.conn).await.unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, account.id, &vault, &contact, &event, &parameter);
    assert_eq!(update_event(&db, request, &contact, &parameter).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    assert_eq!(update_event(&db, request, &contact, &parameter).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    assert_eq!(update_event(&db, request, &contact, &parameter).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_event_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, vault.id).await;
    let event = mood_tracking_event(&db, &other_contact(&db).await, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    assert_eq!(update_event(&db, request, &contact, &parameter).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let parameter = mood_tracking_parameter(&db, other_vault(&db).await.id).await;
    let event = mood_tracking_event(&db, &contact, &parameter).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &event, &parameter);
    assert_eq!(update_event(&db, request, &contact, &parameter).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
