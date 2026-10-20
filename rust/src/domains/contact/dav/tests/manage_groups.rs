//! `tests/Unit/Domains/Contact/ManageGroups/Dav`.

use sea_orm::{ActiveValue::Set, EntityTrait};

use super::super::manage_contact;
use super::super::services::ImportVCardContext;
use super::super::tests::*;
use super::*;
use crate::domains::base::VaultPermission;
use crate::entities::vaults;
use crate::testing::{TestDb, factory};

/// `const defaultPropsCount = 3;` (`VERSION`, `PRODID`, `UID`).
const DEFAULT_PROPS_COUNT: usize = 3;

/// `createVaultUser(User::factory()->create(), Vault::PERMISSION_MANAGE)`, a
/// group and a contact in it.
async fn vault_group_contact(db: &TestDb) -> (vaults::Model, groups::Model, contacts::Model) {
    let user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let group = group(&db.conn, &vault).await;
    let contact = factory::contact(&db.conn, &vault).await;
    (vault, group, contact)
}

fn members(vcard: &Document) -> Vec<String> {
    vcard.properties("MEMBER").into_iter().map(ToString::to_string).collect()
}

// ---- ExportMembersTest

#[tokio::test]
async fn it_exports_a_member() {
    let db = TestDb::migrated().await;
    let (_, group, contact) = vault_group_contact(&db).await;
    add_to_group(&db.conn, &group, &contact).await;

    let mut vcard = Document::vcard(vec![("KIND", "group".into())]).unwrap();

    ExportMembers.export(&db.conn, &group, &mut vcard).await.unwrap();

    let members = members(&vcard);

    assert_eq!(1, members.len());
    assert!(members.contains(&contact.id.to_string()));
    db.drop().await;
}

#[tokio::test]
async fn it_exports_removing_members() {
    let db = TestDb::migrated().await;
    let (_, group, contact) = vault_group_contact(&db).await;
    add_to_group(&db.conn, &group, &contact).await;

    let mut vcard = Document::vcard(vec![("KIND", "group".into()), ("MEMBER", "false".into())]).unwrap();

    ExportMembers.export(&db.conn, &group, &mut vcard).await.unwrap();

    let members = members(&vcard);

    assert_eq!(1, members.len());
    assert!(members.contains(&contact.id.to_string()));
    assert!(!members.contains(&"false".to_owned()));
    db.drop().await;
}

#[tokio::test]
async fn it_exports_keeping_members() {
    let db = TestDb::migrated().await;
    let (_, group, contact) = vault_group_contact(&db).await;
    add_to_group(&db.conn, &group, &contact).await;

    let mut vcard = Document::vcard(vec![("KIND", "group".into()), ("MEMBER", contact.id.to_string().into())]).unwrap();

    ExportMembers.export(&db.conn, &group, &mut vcard).await.unwrap();

    let members = members(&vcard);

    assert_eq!(1, members.len());
    assert!(members.contains(&contact.id.to_string()));
    db.drop().await;
}

// ---- ExportNamesTest (of `ManageContact\Dav\ExportNames`)

#[tokio::test]
async fn export_names_adds_names_in_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    manage_contact::ExportNames.export(&user, &contact, &mut v_card).unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 2, v_card.children().len());
    assert!(v_card.serialize().contains(&format!("FN:{}", factory_name(&contact))));
    assert!(v_card.serialize().contains(&format!(
        "N:{};{};;;",
        contact.last_name.as_deref().unwrap(),
        contact.first_name.as_deref().unwrap()
    )));
    db.drop().await;
}

#[tokio::test]
async fn export_names_adds_nickname_in_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_with(&db.conn, &vault, |c| c.nickname = Set(Some("Nick".into()))).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    manage_contact::ExportNames.export(&user, &contact, &mut v_card).unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 3, v_card.children().len());
    assert!(v_card.serialize().contains(&format!("FN:{}", factory_name(&contact))));
    assert!(v_card.serialize().contains(&format!(
        "N:{};{};;;",
        contact.last_name.as_deref().unwrap(),
        contact.first_name.as_deref().unwrap()
    )));
    assert!(v_card.serialize().contains("NICKNAME:Nick"));
    db.drop().await;
}

// ---- ImportGroupTest

async fn context(db: &TestDb) -> ImportVCardContext {
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Manage).await;
    vcard_context(&author, &vault)
}

#[tokio::test]
async fn import_group_imports_names_n() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![(
        "N",
        vec![Some("NameGroup".into()), Some("".into()), Some("".into()), Some("".into()), Some("".into())].into(),
    )])
    .unwrap();
    let group = ImportGroup.import_names(GroupData::default(), &vcard).unwrap();

    assert_eq!(group.name.as_deref(), Some("NameGroup"));
    db.drop().await;
}

#[tokio::test]
async fn import_group_imports_names_fn() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![("FN", "Name Group".into())]).unwrap();
    let group = ImportGroup.import_names(GroupData::default(), &vcard).unwrap();

    assert_eq!(group.name.as_deref(), Some("Name Group"));
    db.drop().await;
}

#[tokio::test]
async fn import_group_imports_name_fn() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![("FN", "Other Name".into()), ("N", "Name Group".into())]).unwrap();
    let group = ImportGroup.import_names(GroupData::default(), &vcard).unwrap();

    assert_eq!(group.name.as_deref(), Some("Name Group"));
    db.drop().await;
}

#[tokio::test]
async fn import_group_imports_uuid_default() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![("UID", "31fdc242-c974-436e-98de-6b21624d6e34".into())]).unwrap();

    let group = GroupData::default();

    let id = import_uid(false, group.id, get_uid(&vcard));

    assert_eq!(id.as_deref(), Some("31fdc242-c974-436e-98de-6b21624d6e34"));
    db.drop().await;
}

#[tokio::test]
async fn import_group_updates_name() {
    let db = TestDb::migrated().await;
    let context = context(&db).await;

    let group = group(&db.conn, &context.vault).await;
    let mut am: groups::ActiveModel = group.into();
    am.distant_uuid = Set(Some("31fdc242-c974-436e-98de-6b21624d6e34".into()));
    let group = sea_orm::ActiveModelTrait::update(am, &db.conn).await.unwrap();

    let vcard = Document::vcard(vec![
        ("UID", "31fdc242-c974-436e-98de-6b21624d6e34".into()),
        ("KIND", "group".into()),
        ("FN", "New Name".into()),
    ])
    .unwrap();

    ImportGroup.import(&db.conn, &context, &vcard, Some(VCardResource::Group(Box::new(group.clone())))).await.unwrap();

    let group = groups::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();

    assert_eq!("New Name", group.name);
    db.drop().await;
}

// ---- ImportMembersTest

async fn group_contacts_of(db: &TestDb, group: &groups::Model) -> Vec<contacts::Model> {
    group_contacts(&db.conn, group).await.unwrap()
}

#[tokio::test]
async fn import_members_imports_a_member() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![("MEMBER", "31fdc242-c974-436e-98de-6b21624d6e34".into())]).unwrap();

    let members = ImportMembers.import_members(&vcard);

    assert!(members.contains(&"31fdc242-c974-436e-98de-6b21624d6e34".to_owned()));
    db.drop().await;
}

#[tokio::test]
async fn import_members_imports_multiple_members() {
    let db = TestDb::migrated().await;

    let mut vcard = Document::vcard(vec![]).unwrap();
    vcard.add_property("MEMBER", "31fdc242-c974-436e-98de-6b21624d6e34".into(), vec![]).unwrap();
    vcard.add_property("MEMBER", "61fdc242-c974-436e-98de-6b21624d6e34".into(), vec![]).unwrap();

    let members = ImportMembers.import_members(&vcard);

    assert!(members.contains(&"31fdc242-c974-436e-98de-6b21624d6e34".to_owned()));
    assert!(members.contains(&"61fdc242-c974-436e-98de-6b21624d6e34".to_owned()));
    db.drop().await;
}

#[tokio::test]
async fn import_members_updates_members() {
    let db = TestDb::migrated().await;
    let context = context(&db).await;

    let group = group(&db.conn, &context.vault).await;
    let contact = factory::contact(&db.conn, &context.vault).await;

    let members = vec![contact.id.to_string()];

    ImportMembers.update_group_members(&db.conn, &context, &group, &members).await.unwrap();

    let c = &group_contacts_of(&db, &group).await[0];

    assert_eq!(contact.id, c.id);
    db.drop().await;
}

#[tokio::test]
async fn import_members_keeps_existing_members() {
    let db = TestDb::migrated().await;
    let context = context(&db).await;

    let group = group(&db.conn, &context.vault).await;
    let contact = factory::contact(&db.conn, &context.vault).await;
    add_to_group(&db.conn, &group, &contact).await;

    let members = vec![contact.id.to_string()];

    ImportMembers.update_group_members(&db.conn, &context, &group, &members).await.unwrap();

    let c = &group_contacts_of(&db, &group).await[0];

    assert_eq!(contact.id, c.id);
    db.drop().await;
}

#[tokio::test]
async fn import_members_keeps_existing_members_and_add_new() {
    let db = TestDb::migrated().await;
    let context = context(&db).await;

    let group = group(&db.conn, &context.vault).await;
    let contact1 = factory::contact(&db.conn, &context.vault).await;
    add_to_group(&db.conn, &group, &contact1).await;
    let contact2 = factory::contact(&db.conn, &context.vault).await;

    let members = vec![contact1.id.to_string(), contact2.id.to_string()];

    ImportMembers.update_group_members(&db.conn, &context, &group, &members).await.unwrap();

    for contact in group_contacts_of(&db, &group).await {
        assert!(members.contains(&contact.id.to_string()));
    }
    db.drop().await;
}

#[tokio::test]
async fn import_members_removes_old_members() {
    let db = TestDb::migrated().await;
    let context = context(&db).await;

    let group = group(&db.conn, &context.vault).await;
    let contact1 = factory::contact(&db.conn, &context.vault).await;
    add_to_group(&db.conn, &group, &contact1).await;
    let contact2 = factory::contact(&db.conn, &context.vault).await;
    add_to_group(&db.conn, &group, &contact2).await;

    let members = vec![contact2.id.to_string()];

    ImportMembers.update_group_members(&db.conn, &context, &group, &members).await.unwrap();

    for contact in group_contacts_of(&db, &group).await {
        assert!(members.contains(&contact.id.to_string()));
    }
    db.drop().await;
}
