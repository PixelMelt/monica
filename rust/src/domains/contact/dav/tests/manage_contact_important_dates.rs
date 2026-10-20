//! `tests/Unit/Domains/Contact/ManageContactImportantDates/Dav`.

use sea_orm::ActiveValue::Set;

use super::super::tests::*;
use super::*;
use crate::domains::base::VaultPermission;
use crate::testing::{TestDb, factory};

/// `const defaultPropsCount = 3;` (`VERSION`, `PRODID`, `UID`).
const DEFAULT_PROPS_COUNT: usize = 3;

// ---- ExportImportantDatesTest

async fn export_bday(f: impl FnOnce(&mut contact_important_dates::ActiveModel), expected: &str) {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    important_date(&db.conn, &contact, f).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportImportantDates.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    assert!(v_card.serialize().contains(expected));
    db.drop().await;
}

#[tokio::test]
async fn it_adds_bday_to_vcard() {
    export_bday(|_| {}, "BDAY:19811029").await;
}

#[tokio::test]
async fn it_adds_bday_to_vcard2() {
    export_bday(
        |d| {
            d.year = Set(Some(1981));
            d.month = Set(None);
            d.day = Set(None);
        },
        "BDAY:1981",
    )
    .await;
}

#[tokio::test]
async fn it_adds_bday_to_vcard3() {
    export_bday(
        |d| {
            d.year = Set(None);
            d.month = Set(Some(10));
            d.day = Set(Some(27));
        },
        "BDAY:--1027",
    )
    .await;
}

// ---- ImportCalendarContactImportantDatesTest

#[tokio::test]
async fn import_calendar_contact_important_dates_imports_uuid_default() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;
    let context = vcalendar_context(&author, &vault);

    let mut vcalendar = Document::vcalendar(vec![]).unwrap();
    let vevent = vcalendar
        .create_component("VEVENT", vec![("UID", "31fdc242-c974-436e-98de-6b21624d6e34".into())], true)
        .unwrap();
    vcalendar.add(Node::Component(vevent));

    let id = ImportCalendarContactImportantDates.import_uid(&context, None, &vcalendar);

    assert_eq!(id.as_deref(), Some("31fdc242-c974-436e-98de-6b21624d6e34"));
    db.drop().await;
}

// ---- ImportImportantDatesTest

/// Imports `bday` for a factory contact (with `f` creating its existing
/// dates), and returns the contact's dates and the vault's birthdate type.
async fn import_bday(
    bday: &str,
    existing: impl AsyncFnOnce(&TestDb, &contacts::Model, &contact_important_date_types::Model),
) -> (Vec<contact_important_dates::Model>, contact_important_date_types::Model) {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let context = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = important_date_type(&db.conn, &vault).await;
    existing(&db, &contact, &r#type).await;

    let vcard = Document::vcard(vec![("BDAY", bday.into())]).unwrap();

    let contact = ImportImportantDates
        .import(&db.conn, &context, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let dates = important_dates(&db.conn, &contact).await.unwrap();
    db.drop().await;
    (dates, r#type)
}

#[tokio::test]
async fn import_important_dates_imports_bday() {
    let (dates, _) = import_bday("20251106", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(6));
    assert_eq!(dates[0].month, Some(11));
    assert_eq!(dates[0].year, Some(2025));
}

#[tokio::test]
async fn import_important_dates_imports_bday_year() {
    let (dates, _) = import_bday("2025", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, None);
    assert_eq!(dates[0].month, None);
    assert_eq!(dates[0].year, Some(2025));
}

#[tokio::test]
async fn import_important_dates_imports_bday_year_month() {
    let (dates, _) = import_bday("202510", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, None);
    assert_eq!(dates[0].month, Some(10));
    assert_eq!(dates[0].year, Some(2025));
}

#[tokio::test]
async fn import_important_dates_imports_bday_year_month2() {
    let (dates, _) = import_bday("2025-10", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, None);
    assert_eq!(dates[0].month, Some(10));
    assert_eq!(dates[0].year, Some(2025));
}

#[tokio::test]
async fn import_important_dates_imports_bday_month_day() {
    let (dates, _) = import_bday("--0415", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(15));
    assert_eq!(dates[0].month, Some(4));
    assert_eq!(dates[0].year, None);
}

#[tokio::test]
async fn import_important_dates_imports_bday_month_day2() {
    let (dates, _) = import_bday("--04-15", async |_, _, _| {}).await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(15));
    assert_eq!(dates[0].month, Some(4));
    assert_eq!(dates[0].year, None);
}

#[tokio::test]
async fn import_important_dates_imports_bday_and_remove_old_bday() {
    let (dates, r#type) = import_bday("20251106", async |db, contact, r#type| {
        important_date(&db.conn, contact, |d| d.contact_important_date_type_id = Set(Some(r#type.id))).await;
    })
    .await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(6));
    assert_eq!(dates[0].month, Some(11));
    assert_eq!(dates[0].year, Some(2025));
    assert_eq!(dates[0].contact_important_date_type_id, Some(r#type.id));
}

#[tokio::test]
async fn import_important_dates_imports_bdays_no_change() {
    let (dates, r#type) = import_bday("19811029", async |db, contact, r#type| {
        important_date(&db.conn, contact, |d| d.contact_important_date_type_id = Set(Some(r#type.id))).await;
        assert_eq!(1, important_dates(&db.conn, contact).await.unwrap().len());
    })
    .await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(29));
    assert_eq!(dates[0].month, Some(10));
    assert_eq!(dates[0].year, Some(1981));
    assert_eq!(dates[0].contact_important_date_type_id, Some(r#type.id));
}

#[tokio::test]
async fn import_important_dates_imports_bday_update_type() {
    let (dates, r#type) = import_bday("19811029", async |db, contact, _| {
        let important_date = important_date(&db.conn, contact, |d| d.contact_important_date_type_id = Set(None)).await;

        assert_eq!(important_date.contact_important_date_type_id, None);
        assert_eq!(1, important_dates(&db.conn, contact).await.unwrap().len());
    })
    .await;

    assert_eq!(1, dates.len());
    assert_eq!(dates[0].day, Some(29));
    assert_eq!(dates[0].month, Some(10));
    assert_eq!(dates[0].year, Some(1981));
    assert_eq!(dates[0].contact_important_date_type_id, Some(r#type.id));
}
