//! `tests/Unit/Domains/Contact/ManageContact/Dav`.

use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::super::tests::*;
use super::*;
use crate::domains::base::VaultPermission;
use crate::testing::{TestDb, factory};

/// `const defaultPropsCount = 3;` (`VERSION`, `PRODID`, `UID`).
const DEFAULT_PROPS_COUNT: usize = 3;

/// `$contact->addresses` (current and past).
async fn addresses_of(db: &TestDb, contact: &contacts::Model) -> Vec<addresses::Model> {
    addresses::Entity::find()
        .inner_join(contact_address::Entity)
        .filter(contact_address::Column::ContactId.eq(contact.id))
        .all(&db.conn)
        .await
        .unwrap()
}

/// `PHP Str::substr($text, $start, $length)` on ASCII.
fn substr(text: &str, start: usize, length: Option<usize>) -> &str {
    let end = length.map_or(text.len(), |l| (start + l).min(text.len()));
    &text[start.min(text.len())..end]
}

// ---- ExportAddressTest

#[tokio::test]
async fn it_adds_address_to_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let address = address(&db.conn, &vault).await;
    attach_address(&db.conn, &address, &contact).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportAddress.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());

    let address_type =
        address_types::Entity::find_by_id(address.address_type_id.unwrap()).one(&db.conn).await.unwrap().unwrap();
    let s = |v: &Option<String>| v.clone().unwrap_or_default();
    let text = format!(
        "ADR;TYPE={}:;{};{};{};{};{};{}",
        s(&address_type.r#type),
        s(&address.line_1),
        s(&address.line_2),
        s(&address.city),
        s(&address.province),
        s(&address.postal_code),
        s(&address.country)
    );
    assert!(v_card.serialize().contains(substr(&text, 0, Some(75))));
    assert!(v_card.serialize().contains(substr(&text, 75, None)));
    db.drop().await;
}

// ---- ExportGenderTest

#[tokio::test]
async fn export_gender_adds_gender_to_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportGender.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    let gender = genders::Entity::find_by_id(contact.gender_id.unwrap()).one(&db.conn).await.unwrap().unwrap();
    assert!(v_card.serialize().contains(&format!("GENDER:{}", gender.r#type.unwrap())));
    db.drop().await;
}

async fn export_gender_of(name: Option<&str>, r#type: Option<&str>, expected: &str) {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let gender = gender(&db.conn, user.account_id, name, r#type).await;
    let contact = contact_with(&db.conn, &vault, |c| c.gender_id = Set(Some(gender.id))).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportGender.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    assert!(v_card.serialize().contains(expected));
    db.drop().await;
}

#[tokio::test]
async fn export_gender_adds_gender_female() {
    export_gender_of(Some("Female"), Some("F"), "GENDER:F").await;
}

#[tokio::test]
async fn export_gender_adds_gender_unknown() {
    export_gender_of(Some("Name"), Some("U"), "GENDER:U").await;
}

#[tokio::test]
async fn export_gender_adds_gender_type_null() {
    export_gender_of(Some("Something"), None, "GENDER:O").await;
}

#[tokio::test]
async fn export_gender_adds_gender_type_null_male() {
    export_gender_of(Some("Male"), None, "GENDER:M").await;
}

#[tokio::test]
async fn export_gender_adds_gender_type_null_female() {
    export_gender_of(Some("Female"), None, "GENDER:F").await;
}

// ---- ExportLabelsTest

#[tokio::test]
async fn export_labels_adds_categories_to_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let label = label(&db.conn, &vault).await;
    attach_label(&db.conn, &label, &contact).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportLabels.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    assert!(v_card.serialize().contains(&format!("CATEGORIES:{}", label.name)));
    db.drop().await;
}

#[tokio::test]
async fn export_labels_adds_multiple_categories_to_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;
    let mut labels = Vec::new();
    for _ in 0..3 {
        let label = label(&db.conn, &vault).await;
        attach_label(&db.conn, &label, &contact).await;
        labels.push(label);
    }

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportLabels.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 1, v_card.children().len());
    let names: Vec<String> = labels.into_iter().map(|l| l.name).collect();
    assert!(v_card.serialize().contains(&format!("CATEGORIES:{}", names.join(","))));
    db.drop().await;
}

// ---- ExportNamesTest

#[tokio::test]
async fn export_names_adds_names_in_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let contact = contact_random(&db.conn, &vault).await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportNames.export(&user, &contact, &mut v_card).unwrap();

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
    ExportNames.export(&user, &contact, &mut v_card).unwrap();

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

// ---- ExportWorkInformationTest

#[tokio::test]
async fn export_work_information_adds_job_to_vcard() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::View).await;
    let company = company(&db.conn, &vault).await;
    let contact = contact_with(&db.conn, &vault, |c| {
        c.company_id = Set(Some(company.id));
        c.job_position = Set(Some("random".into()));
    })
    .await;

    let mut v_card = Document::vcard(vec![]).unwrap();
    ExportWorkInformation.export(&db.conn, &contact, &mut v_card).await.unwrap();

    assert_eq!(DEFAULT_PROPS_COUNT + 2, v_card.children().len());
    assert!(v_card.serialize().contains(&format!("ORG:{}", company.name)));
    assert!(v_card.serialize().contains("TITLE:random"));
    db.drop().await;
}

// ---- ImportAddressTest

fn adr() -> Vec<Option<String>> {
    ["", "line 1", "line 2", "city", "province", "postal code", "country"].into_iter().map(|s| Some(s.into())).collect()
}

fn assert_address(address: &addresses::Model) {
    assert_eq!(address.line_1.as_deref(), Some("line 1"));
    assert_eq!(address.line_2.as_deref(), Some("line 2"));
    assert_eq!(address.city.as_deref(), Some("city"));
    assert_eq!(address.province.as_deref(), Some("province"));
    assert_eq!(address.postal_code.as_deref(), Some("postal code"));
    assert_eq!(address.country.as_deref(), Some("country"));
}

#[tokio::test]
async fn import_address_imports_new_address() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let import_vcard = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let vcard = Document::vcard(vec![("ADR", adr().into())]).unwrap();

    let contact = ImportAddress
        .import(&db.conn, &import_vcard, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let addresses = addresses_of(&db, &contact).await;
    assert_eq!(1, addresses.len());
    assert_address(&addresses[0]);
    db.drop().await;
}

#[tokio::test]
async fn import_address_imports_address_type() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let import_vcard = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    address_type(&db.conn, user.account_id, "home", "home").await;

    let mut vcard = Document::vcard(vec![]).unwrap();
    vcard.add_property("ADR", adr().into(), vec![("TYPE", Some("home".into()))]).unwrap();

    let contact = ImportAddress
        .import(&db.conn, &import_vcard, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let addresses = addresses_of(&db, &contact).await;
    assert_eq!(1, addresses.len());
    let address_type =
        address_types::Entity::find_by_id(addresses[0].address_type_id.unwrap()).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(address_type.name.as_deref(), Some("home"));
    db.drop().await;
}

#[tokio::test]
async fn import_address_imports_new_address_type() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let import_vcard = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let mut vcard = Document::vcard(vec![]).unwrap();
    vcard.add_property("ADR", adr().into(), vec![("TYPE", Some("home".into()))]).unwrap();

    let contact = ImportAddress
        .import(&db.conn, &import_vcard, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let addresses = addresses_of(&db, &contact).await;
    assert_eq!(1, addresses.len());

    let created = address_types::Entity::find()
        .filter(address_types::Column::AccountId.eq(user.account_id))
        .filter(address_types::Column::Name.eq("home"))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(created.is_some());

    let address_type =
        address_types::Entity::find_by_id(addresses[0].address_type_id.unwrap()).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(address_type.name.as_deref(), Some("home"));
    db.drop().await;
}

#[tokio::test]
async fn import_address_imports_new_address_and_remove_old() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let import_vcard = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;
    for _ in 0..2 {
        let address = address(&db.conn, &vault).await;
        attach_address(&db.conn, &address, &contact).await;
    }

    let vcard = Document::vcard(vec![("ADR", adr().into())]).unwrap();

    let contact = ImportAddress
        .import(&db.conn, &import_vcard, &vcard, Some(VCardResource::Contact(Box::new(contact))))
        .await
        .unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };

    let addresses = addresses_of(&db, &contact).await;
    assert_eq!(1, addresses.len());
    assert_address(&addresses[0]);
    db.drop().await;
}

// ---- ImportContactTest

/// `new ImportContact` with a context whose author is `author`.
async fn import_contact_context(db: &TestDb, author: &users::Model) -> ImportVCardContext {
    let vault = factory::create_vault_user(&db.conn, author, VaultPermission::Edit).await;
    vcard_context(author, &vault)
}

/// `User::factory()->create(['name_order' => …])`.
async fn user_with_name_order(db: &TestDb, name_order: &str) -> users::Model {
    let user = factory::user(&db.conn, None).await;
    let mut am: users::ActiveModel = user.into();
    am.name_order = Set(name_order.into());
    sea_orm::ActiveModelTrait::update(am, &db.conn).await.unwrap()
}

#[tokio::test]
async fn import_contact_imports_names_n() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let context = import_contact_context(&db, &author).await;

    let vcard = Document::vcard(vec![(
        "N",
        vec![Some("Doe".into()), Some("John".into()), Some("Jane".into()), Some("".into()), Some("".into())].into(),
    )])
    .unwrap();
    let contact = ImportContact.import_names(&context, ContactData::default(), &vcard).unwrap();

    assert_eq!(contact.first_name.as_deref(), Some("John"));
    assert_eq!(contact.last_name.as_deref(), Some("Doe"));
    assert_eq!(contact.middle_name.as_deref(), Some("Jane"));
    db.drop().await;
}

#[tokio::test]
async fn import_contact_imports_names_nickname() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let context = import_contact_context(&db, &author).await;

    let vcard = Document::vcard(vec![("NICKNAME", "John".into())]).unwrap();
    let contact = ImportContact.import_names(&context, ContactData::default(), &vcard).unwrap();

    assert_eq!(contact.first_name.as_deref(), Some("John"));
    db.drop().await;
}

async fn import_names_fn(name_order: Option<&str>, values: Vec<(&str, &str)>, first: Option<&str>, last: Option<&str>) {
    let db = TestDb::migrated().await;
    let author = match name_order {
        Some(order) => user_with_name_order(&db, order).await,
        None => factory::user(&db.conn, None).await,
    };
    let context = import_contact_context(&db, &author).await;

    let vcard = Document::vcard(values.into_iter().map(|(k, v)| (k, v.into())).collect()).unwrap();
    let contact = ImportContact.import_names(&context, ContactData::default(), &vcard).unwrap();

    assert_eq!(contact.first_name.as_deref(), first);
    assert_eq!(contact.last_name.as_deref(), last);
    db.drop().await;
}

#[tokio::test]
async fn import_contact_imports_names_fn() {
    import_names_fn(None, vec![("FN", "John Doe")], Some("John"), Some("Doe")).await;
}

#[tokio::test]
async fn import_contact_imports_names_f_n_last() {
    import_names_fn(Some("%last_name% %first_name%"), vec![("FN", "John Doe")], Some("Doe"), Some("John")).await;
}

#[tokio::test]
async fn import_contact_imports_names_f_n_extra_space() {
    import_names_fn(None, vec![("FN", "John  Doe")], Some("John"), Some("Doe")).await;
}

#[tokio::test]
async fn import_contact_imports_name_fn() {
    import_names_fn(None, vec![("FN", "John"), ("N", "Mike;;;;")], Some("John"), None).await;
}

#[tokio::test]
async fn import_contact_imports_name_f_n_last() {
    import_names_fn(Some("%last_name% %first_name%"), vec![("FN", "John"), ("N", "Mike;;;;")], None, Some("John"))
        .await;
}

#[tokio::test]
async fn import_contact_imports_names_f_n_multiple() {
    import_names_fn(None, vec![("FN", "John Doe Marco"), ("N", "Mike;;;;")], Some("John"), Some("Doe Marco")).await;
}

#[tokio::test]
async fn import_contact_imports_uuid_default() {
    let db = TestDb::migrated().await;

    let vcard = Document::vcard(vec![("UID", "31fdc242-c974-436e-98de-6b21624d6e34".into())]).unwrap();

    let contact = ContactData::default();

    let id = import_uid(false, contact.id, get_uid(&vcard));

    assert_eq!(id.as_deref(), Some("31fdc242-c974-436e-98de-6b21624d6e34"));
    db.drop().await;
}

// ---- ImportLabelsTest

async fn import_labels(
    db: &TestDb,
    context: &ImportVCardContext,
    vcard: &Document,
    contact: contacts::Model,
) -> Vec<String> {
    let contact =
        ImportLabels.import(&db.conn, context, vcard, Some(VCardResource::Contact(Box::new(contact)))).await.unwrap();
    let Some(VCardResource::Contact(contact)) = contact else { panic!("a contact") };
    contact_labels(&db.conn, &contact).await.unwrap().into_iter().map(|l| l.name).collect()
}

#[tokio::test]
async fn import_labels_imports_one_label() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let context = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let vcard = Document::vcard(vec![("CATEGORIES", vec![Some("tag".into())].into())]).unwrap();

    let labels = import_labels(&db, &context, &vcard, contact).await;

    assert_eq!(1, labels.len());
    assert_eq!("tag", labels[0]);
    db.drop().await;
}

#[tokio::test]
async fn import_labels_imports_multiple_labels() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let context = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let vcard = Document::vcard(vec![("CATEGORIES", vec![Some("tag1".into()), Some("tag2".into())].into())]).unwrap();

    let labels = import_labels(&db, &context, &vcard, contact).await;

    assert_eq!(2, labels.len());
    assert!(labels.contains(&"tag1".to_owned()));
    assert!(labels.contains(&"tag2".to_owned()));
    db.drop().await;
}

#[tokio::test]
async fn import_labels_imports_labels_and_remove_old_labels() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let context = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let label = label(&db.conn, &vault).await;
    attach_label(&db.conn, &label, &contact).await;

    let vcard = Document::vcard(vec![("CATEGORIES", vec![Some("tag1".into()), Some("tag2".into())].into())]).unwrap();

    let labels = import_labels(&db, &context, &vcard, contact).await;

    assert_eq!(2, labels.len());
    assert!(!labels.contains(&label.name));
    assert!(labels.contains(&"tag1".to_owned()));
    assert!(labels.contains(&"tag2".to_owned()));
    db.drop().await;
}

#[tokio::test]
async fn import_labels_imports_labels_no_change() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let context = vcard_context(&user, &vault);

    let contact = factory::contact(&db.conn, &vault).await;

    let label = label(&db.conn, &vault).await;
    attach_label(&db.conn, &label, &contact).await;

    let vcard = Document::vcard(vec![("CATEGORIES", vec![Some(label.name.clone())].into())]).unwrap();

    let labels = import_labels(&db, &context, &vcard, contact).await;

    assert_eq!(1, labels.len());
    assert!(labels.contains(&label.name));
    db.drop().await;
}
