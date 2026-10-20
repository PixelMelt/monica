//! `tests/Unit/Domains/Contact/DAV/Services`: `ExportVCalendarTest`,
//! `ExportVCardTest`, `GetEtagTest`, `ImportVCardTest`. (`CardDAVTest`,
//! `CalDAVTest`, `CalDAVDatesTest`, `CalDAVTasksTest`, `DAVServerTest` and
//! `VCardContactTest` go through the DAV server.)

use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::super::tests::*;
use super::*;
use crate::testing::{TestDb, factory};
use crate::vobject::{self, Input};

// ---- ExportVCalendarTest

#[tokio::test]
async fn it_exports_a_task() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let task = task(&db.conn, &contact).await;

    let v_card = ExportVCalendar {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        contact_important_date_id: None,
        contact_task_id: Some(task.id),
    }
    .execute(&db.conn, &config())
    .await
    .unwrap();

    assert_eq!(v_card.kind, Kind::VCalendar);
    assert_eq!(get_task(&db.conn, &task).await, v_card.serialize());
    db.drop().await;
}

#[tokio::test]
async fn it_exports_a_date() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let date = important_date(&db.conn, &contact, |_| {}).await;

    let v_card = ExportVCalendar {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        contact_important_date_id: Some(date.id),
        contact_task_id: None,
    }
    .execute(&db.conn, &config())
    .await
    .unwrap();

    assert_eq!(v_card.kind, Kind::VCalendar);
    assert_eq!(get_date(&db.conn, &date).await, v_card.serialize());
    db.drop().await;
}

// ---- ExportVCardTest

#[tokio::test]
async fn export_vcard_exports_a_contact() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;

    let v_card = ExportVCard {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        contact_id: Some(contact.id),
        group_id: None,
    }
    .execute(&db.conn, &config())
    .await
    .unwrap();

    assert_eq!(v_card.kind, Kind::VCard);
    assert_vobject_equals_vobject(&get_card(&db.conn, &contact, false).await, &v_card.serialize());
    db.drop().await;
}

#[tokio::test]
async fn export_vcard_exports_a_contact_with_vcard() {
    let db = TestDb::migrated().await;
    let source = "BEGIN:VCARD
VERSION:4.0
PRODID:-//Sabre//Sabre VObject 4.3.0//EN
SOURCE:**ANY**
UID:**ANY**
FN:Dr. John Doe III
N:Doe;John;;;
GENDER:M
REV:**ANY**
END:VCARD";

    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;

    let gender = gender(&db.conn, vault.account_id, Some("Name"), Some("M")).await;
    let contact = contact_with(&db.conn, &vault, |c| {
        c.vcard = Set(Some(source.into()));
        c.first_name = Set(Some("John".into()));
        c.last_name = Set(Some("Doe".into()));
        c.gender_id = Set(Some(gender.id));
    })
    .await;

    let v_card = ExportVCard {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        contact_id: Some(contact.id),
        group_id: None,
    }
    .execute(&db.conn, &config())
    .await
    .unwrap();

    assert_eq!(v_card.kind, Kind::VCard);
    assert_vobject_equals_vobject(source, &v_card.serialize());
    db.drop().await;
}

#[tokio::test]
async fn export_vcard_exports_a_group_with_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;

    let gender = gender(&db.conn, vault.account_id, Some("Name"), Some("M")).await;

    let vcard = "BEGIN:VCARD
VERSION:4.0
PRODID:-//Sabre//Sabre VObject 4.3.0//EN
SOURCE:**ANY**
UID:**ANY**
FN:Dr. John Doe III
N:Doe;John;;;
GENDER:M
REV:**ANY**
END:VCARD";

    let contact = contact_with(&db.conn, &vault, |c| {
        c.vcard = Set(Some(vcard.into()));
        c.first_name = Set(Some("John".into()));
        c.last_name = Set(Some("Doe".into()));
        c.gender_id = Set(Some(gender.id));
    })
    .await;

    let source = format!(
        "BEGIN:VCARD
VERSION:4.0
PRODID:-//Sabre//Sabre VObject 4.3.0//EN
SOURCE:**ANY**
MEMBER:{}
UID:**ANY**
KIND:group
FN:Colleagues
N:Colleagues;;;;
REV:**ANY**
END:VCARD",
        contact.id
    );

    let group = group(&db.conn, &vault).await;
    let mut am: groups::ActiveModel = group.into();
    am.name = Set("Colleagues".into());
    am.vcard = Set(Some(source.clone()));
    let group = sea_orm::ActiveModelTrait::update(am, &db.conn).await.unwrap();
    add_to_group(&db.conn, &group, &contact).await;

    let v_card = ExportVCard {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        contact_id: None,
        group_id: Some(group.id),
    }
    .execute(&db.conn, &config())
    .await
    .unwrap();

    assert_eq!(v_card.kind, Kind::VCard);
    assert_vobject_equals_vobject(&source, &v_card.serialize());
    db.drop().await;
}

// ---- GetEtagTest

#[tokio::test]
async fn get_etag_get_etag_local_contact() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_with(&db.conn, &vault, |c| c.vcard = Set(Some("test".into()))).await;

    let etag = GetEtag {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        vcard: Some(VCardResource::Contact(Box::new(contact))),
        vcalendar: None,
    }
    .execute(&db.conn)
    .await
    .unwrap();

    assert_eq!("\"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"", etag);
    db.drop().await;
}

#[tokio::test]
async fn get_etag_get_etag_distant_contact() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_with(&db.conn, &vault, |c| {
        c.vcard = Set(Some("test".into()));
        c.distant_etag = Set(Some("\"test\"".into()));
    })
    .await;

    let etag = GetEtag {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        vcard: Some(VCardResource::Contact(Box::new(contact))),
        vcalendar: None,
    }
    .execute(&db.conn)
    .await
    .unwrap();

    assert_eq!("\"test\"", etag);
    db.drop().await;
}

// ---- ImportVCardTest

#[tokio::test]
async fn import_vcard_can_not_import_because_no_firstname_or_nickname_in_vcard() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![]).unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_not_import_because_no_firstname_in_vcard() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![(
        "N",
        vec![Some("John".into()), Some("".into()), Some("".into()), Some("".into()), Some("".into())].into(),
    )])
    .unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_not_import_because_empty_firstname_in_vcard() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![("N", ";;;;".into())]).unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_not_import_vcard() {
    let db = TestDb::migrated().await;
    let vcard = vobject::read(
        "
BEGIN:VCARD
VERSION:3.0
N:;;;;
FN:
ORG:;
EMAIL;TYPE=home;TYPE=pref:mail@example.org
NOTE:
NICKNAME:
TITLE:
REV:20210900T000102Z
END:VCARD",
        vobject::OPTION_FORGIVING + vobject::OPTION_IGNORE_INVALID_LINES,
    )
    .unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_not_import_because_empty_nickname_in_vcard() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![("NICKNAME", "".into())]).unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_not_import_because_empty_fullname_in_vcard() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![("FN", "".into())]).unwrap();

    assert!(!can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_import_firstname() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![(
        "N",
        vec![Some("".into()), Some("John".into()), Some("".into()), Some("".into()), Some("".into())].into(),
    )])
    .unwrap();

    assert!(can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_import_nickname() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![("NICKNAME", "John".into())]).unwrap();

    assert!(can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_can_import_fullname() {
    let db = TestDb::migrated().await;
    let vcard = Document::vcard(vec![("FN", "John Doe".into())]).unwrap();

    assert!(can_import_current_entry(&vcard));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_formats_value() {
    let db = TestDb::migrated().await;
    let result = super::super::format_value(Some(""));
    assert_eq!(result, None);

    let result = super::super::format_value(Some("This is a value"));
    assert_eq!(result.as_deref(), Some("This is a value"));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_creates_a_contact() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;
    let import_vcard = vcard_context(&author, &vault);

    let vcard = Document::vcard(vec![
        ("N", vec![Some("John".into()), Some("Doe".into()), Some("".into()), Some("".into()), Some("".into())].into()),
        ("EMAIL", "john@doe.com".into()),
    ])
    .unwrap();

    let contact = import_entry(&db.conn, &import_vcard, &vcard).await.unwrap();

    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };
    assert!(contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_creates_a_contact_full() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;

    let vcard = "BEGIN:VCARD
VERSION:3.0
UID:31fdc242-c974-436e-98de-6b21624d6e34
N:John;Doe;;;
FN:John Doe
REV:20210900T000102Z
END:VCARD";

    ImportVCard {
        account_id: author.account_id,
        author_id: author.id,
        vault_id: vault.id,
        contact_id: None,
        entry: Entry::String(vcard.into()),
        behaviour: "behaviour_add".into(),
        etag: None,
        uri: None,
        external: Some(true),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let row = contacts::Entity::find()
        .filter(contacts::Column::DistantUuid.eq("31fdc242-c974-436e-98de-6b21624d6e34"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.first_name.as_deref(), Some("Doe"));
    assert_eq!(row.last_name.as_deref(), Some("John"));
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_creates_a_group_full() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;

    let contact = factory::contact(&db.conn, &vault).await;

    let vcard = format!(
        "BEGIN:VCARD
VERSION:3.0
UID:61fdc242-c974-436e-98de-6b21624d6e35
KIND:group
N:Colleagues;;;;
FN:Colleagues
MEMBER:{}
REV:20210900T000102Z
END:VCARD",
        contact.id
    );

    ImportVCard {
        account_id: author.account_id,
        author_id: author.id,
        vault_id: vault.id,
        contact_id: None,
        entry: Entry::String(vcard),
        behaviour: "behaviour_add".into(),
        etag: None,
        uri: None,
        external: Some(true),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let group = groups::Entity::find()
        .filter(groups::Column::DistantUuid.eq("61fdc242-c974-436e-98de-6b21624d6e35"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.vault_id, vault.id);
    assert_eq!(group.name, "Colleagues");

    let pivot = crate::entities::contact_group::Entity::find()
        .filter(crate::entities::contact_group::Column::GroupId.eq(group.id))
        .filter(crate::entities::contact_group::Column::ContactId.eq(contact.id))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(pivot.is_some());
    db.drop().await;
}

#[tokio::test]
async fn import_vcard_imports_uuid_contact() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;
    let import_vcard = vcard_context(&author, &vault);

    let vcard = Document::vcard(vec![
        ("FN", "John Doe".into()),
        ("UID", Input::Text("31fdc242-c974-436e-98de-6b21624d6e34".into())),
    ])
    .unwrap();

    let contact = import_entry(&db.conn, &import_vcard, &vcard).await.unwrap();

    let id: Uuid = "31fdc242-c974-436e-98de-6b21624d6e34".parse().unwrap();
    assert!(contacts::Entity::find_by_id(id).one(&db.conn).await.unwrap().is_some());
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };
    assert_eq!("31fdc242-c974-436e-98de-6b21624d6e34", contact.id.to_string());
    db.drop().await;
}
