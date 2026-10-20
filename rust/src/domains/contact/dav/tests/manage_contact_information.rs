//! `tests/Unit/Domains/Contact/ManageContactInformation/Dav`.

use sea_orm::ActiveValue::Set;

use super::super::tests::*;
use super::*;
use crate::domains::base::VaultPermission;
use crate::entities::vaults;
use crate::testing::{TestDb, factory};

/// `const defaultPropsCount = 3;` (`VERSION`, `PRODID`, `UID`).
const DEFAULT_PROPS_COUNT: usize = 3;

// ---- ExportContactInformationTest

async fn export_information(
    f: impl FnOnce(&mut contact_information_types::ActiveModel),
    data: &str,
    expected: impl FnOnce(&str) -> String,
) {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let r#type = contact_information_type(&db.conn, vault.account_id, f).await;
    let info = contact_information(&db.conn, &contact, r#type.id, data).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportContactInformation.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    assert!(v_card.serialize().contains(&expected(&info.data)));
    db.drop().await;
}

#[tokio::test]
async fn it_adds_email_to_vcard() {
    export_information(
        |t| t.name = Set(Some("Email address".into())),
        "fake@email.com",
        |d| format!("EMAIL;PREF=1:{d}"),
    )
    .await;
}

#[tokio::test]
async fn it_adds_tel_to_vcard() {
    export_information(
        |t| {
            t.name = Set(Some("Phone".into()));
            t.r#type = Set(Some("phone".into()));
            t.protocol = Set(Some("tel:".into()));
        },
        "1234567890",
        |d| format!("TEL;PREF=1:{d}"),
    )
    .await;
}

#[tokio::test]
async fn it_adds_social_profile_to_vcard() {
    export_information(
        |t| {
            t.name = Set(None);
            t.name_translation_key = Set(Some("GitHub".into()));
            t.r#type = Set(Some("x-social-profile".into()));
        },
        "github",
        |d| format!("X-SOCIAL-PROFILE;TYPE=GitHub;X-USER={d}:"),
    )
    .await;
}

#[tokio::test]
async fn it_adds_social_profile_to_vcard_name() {
    export_information(
        |t| {
            t.name = Set(Some("GitHub".into()));
            t.name_translation_key = Set(Some("Facebook".into()));
            t.r#type = Set(Some("x-social-profile".into()));
        },
        "github",
        |d| format!("X-SOCIAL-PROFILE;TYPE=GitHub;X-USER={d}:"),
    )
    .await;
}

#[tokio::test]
async fn it_adds_impp_to_vcard() {
    export_information(
        |t| {
            t.name = Set(None);
            t.name_translation_key = Set(Some("Telegram".into()));
            t.r#type = Set(Some("impp".into()));
        },
        "yo",
        |d| format!("IMPP;X-SERVICE-TYPE=Telegram:{d}"),
    )
    .await;
}

// ---- ImportContactInformationTest

/// Imports `vcard` into a factory contact; `setup` creates what the test
/// needs first. Returns the contact's information with their types.
async fn import_information(
    administrator: bool,
    setup: impl AsyncFnOnce(&TestDb, &vaults::Model, &contacts::Model),
    vcard: Document,
) -> Vec<(contact_information::Model, contact_information_types::Model)> {
    let db = TestDb::migrated().await;
    let user = if administrator {
        factory::create_administrator(&db.conn).await
    } else {
        factory::create_user(&db.conn).await
    };
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    setup(&db, &vault, &contact).await;
    let context = vcard_context(&user, &vault);

    let contact = ImportContactInformation
        .import(&db.conn, &context, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let mut result = Vec::new();
    for info in contact_informations(&db.conn, &contact).await.unwrap() {
        let r#type = information_type(&db.conn, &info).await.unwrap();
        result.push((info, r#type));
    }
    db.drop().await;
    result
}

fn vcard_with(name: &str, value: &str, parameters: Vec<(&str, Option<String>)>) -> Document {
    let mut vcard = Document::vcard(vec![]).unwrap();
    vcard.add_property(name, value.into(), parameters).unwrap();
    vcard
}

#[tokio::test]
async fn import_contact_information_imports_new_email() {
    let infos = import_information(
        false,
        async |db, vault, _| {
            contact_information_type(&db.conn, vault.account_id, |_| {}).await;
        },
        Document::vcard(vec![("EMAIL", "test@test.com".into())]).unwrap(),
    )
    .await;

    assert_eq!(1, infos.len());
    assert_eq!("test@test.com", infos[0].0.data);
    assert_eq!(infos[0].1.r#type.as_deref(), Some("email"));
}

#[tokio::test]
async fn import_contact_information_imports_new_email_type() {
    let infos = import_information(
        false,
        async |db, vault, _| {
            contact_information_type(&db.conn, vault.account_id, |_| {}).await;
        },
        vcard_with("EMAIL", "test@test.com", vec![("TYPE", Some("home".into()))]),
    )
    .await;

    assert_eq!(1, infos.len());
    assert_eq!("test@test.com", infos[0].0.data);
    assert_eq!(infos[0].1.r#type.as_deref(), Some("email"));
    assert_eq!(infos[0].0.kind.as_deref(), Some("home"));
}

#[tokio::test]
async fn import_contact_information_imports_new_email_with_type_creation() {
    let infos = import_information(
        true,
        async |_, _, _| {},
        vcard_with("EMAIL", "test@test.com", vec![("TYPE", Some("home".into()))]),
    )
    .await;

    assert_eq!(1, infos.len());
    assert_eq!("test@test.com", infos[0].0.data);
    assert_eq!(infos[0].1.r#type.as_deref(), Some("EMAIL"));
    assert_eq!(infos[0].0.kind.as_deref(), Some("home"));
}

async fn phone_type(db: &TestDb, vault: &vaults::Model) {
    contact_information_type(&db.conn, vault.account_id, |t| {
        t.r#type = Set(Some("phone".into()));
        t.protocol = Set(Some("tel:".into()));
    })
    .await;
}

#[tokio::test]
async fn import_contact_information_imports_new_tel() {
    let infos = import_information(
        false,
        async |db, vault, _| phone_type(db, vault).await,
        Document::vcard(vec![("TEL", "1234567890".into())]).unwrap(),
    )
    .await;

    assert_eq!(1, infos.len());
    assert_eq!("1234567890", infos[0].0.data);
    assert_eq!(infos[0].1.r#type.as_deref(), Some("phone"));
}

#[tokio::test]
async fn import_contact_information_imports_tel_with_type() {
    let infos = import_information(
        false,
        async |db, vault, _| phone_type(db, vault).await,
        vcard_with("TEL", "1234567890", vec![("TYPE", Some("cell".into()))]),
    )
    .await;

    assert_eq!(1, infos.len());
    assert_eq!("1234567890", infos[0].0.data);
    assert_eq!(infos[0].1.r#type.as_deref(), Some("phone"));
    assert_eq!(infos[0].0.kind.as_deref(), Some("cell"));
}

#[tokio::test]
async fn import_contact_information_imports_social_profile() {
    // `social_profiles()`.
    for (r#type, value) in [("Facebook", "test"), ("Whatsapp", "test"), ("Instagram", "test")] {
        let infos = import_information(
            false,
            async |db, vault, _| {
                contact_information_type(&db.conn, vault.account_id, |t| {
                    t.r#type = Set(Some("X-SOCIAL-PROFILE".into()));
                    t.name_translation_key = Set(Some(r#type.into()));
                })
                .await;
            },
            vcard_with("X-SOCIAL-PROFILE", "", vec![("TYPE", Some(r#type.into())), ("X-USER", Some(value.into()))]),
        )
        .await;

        assert_eq!(1, infos.len());
        assert_eq!(value, infos[0].0.data);
        assert_eq!(infos[0].1.r#type.as_deref(), Some("X-SOCIAL-PROFILE"));
        assert_eq!(infos[0].1.name_translation_key.as_deref(), Some(r#type));
    }
}

#[tokio::test]
async fn import_contact_information_imports_impp() {
    // `impps()`.
    for (r#type, value) in [("Telegram", "test"), ("Hangouts", "test")] {
        let infos = import_information(
            false,
            async |db, vault, _| {
                contact_information_type(&db.conn, vault.account_id, |t| {
                    t.r#type = Set(Some("IMPP".into()));
                    t.name_translation_key = Set(Some(r#type.into()));
                })
                .await;
            },
            vcard_with("IMPP", value, vec![("X-SERVICE-TYPE", Some(r#type.into()))]),
        )
        .await;

        assert_eq!(1, infos.len());
        assert_eq!(value, infos[0].0.data);
        assert_eq!(infos[0].1.r#type.as_deref(), Some("IMPP"));
        assert_eq!(infos[0].1.name_translation_key.as_deref(), Some(r#type));
    }
}

#[tokio::test]
async fn import_contact_information_updates_email_address() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = contact_information_type(&db.conn, vault.account_id, |_| {}).await;
    let info = contact_information(&db.conn, &contact, r#type.id, "old@email.com").await;
    let context = vcard_context(&user, &vault);

    let vcard = Document::vcard(vec![("EMAIL", "test@test.com".into())]).unwrap();

    let contact = ImportContactInformation
        .import(&db.conn, &context, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let info = contact_information::Entity::find_by_id(info.id).one(&db.conn).await.unwrap().unwrap();

    assert_eq!(1, contact_informations(&db.conn, &contact).await.unwrap().len());
    assert_eq!("test@test.com", info.data);
    assert_eq!(information_type(&db.conn, &info).await.unwrap().r#type.as_deref(), Some("email"));
    db.drop().await;
}

#[tokio::test]
async fn import_contact_information_destroys_email_address() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = contact_information_type(&db.conn, vault.account_id, |_| {}).await;
    let info = contact_information(&db.conn, &contact, r#type.id, "old@email.com").await;
    contact_information_type(&db.conn, vault.account_id, |t| {
        t.r#type = Set(Some("phone".into()));
        t.protocol = Set(Some("tel:".into()));
    })
    .await;
    let context = vcard_context(&user, &vault);

    let vcard = vcard_with("TEL", "1234567890", vec![("TYPE", Some("cell".into()))]);

    ImportContactInformation
        .import(&db.conn, &context, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();

    // `$contactInformation->refresh()` throws ModelNotFoundException.
    assert!(contact_information::Entity::find_by_id(info.id).one(&db.conn).await.unwrap().is_none());
    db.drop().await;
}
