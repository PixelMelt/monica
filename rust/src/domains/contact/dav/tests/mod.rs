//! What the `dav` tests share: the factories of `database/factories` they
//! need, `tests/Unit/Domains/Contact/DAV/CardEtag.php`, and
//! `Sabre\VObject\PHPUnitAssertions::assertVObjectEqualsVObject()`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, EntityTrait};

use super::services::{ImportVCalendarContext, ImportVCardContext};
use crate::config::Config;
use crate::crypto::random_string;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::entities::{
    address_types, addresses, companies, contact_important_date_types, contact_important_dates, contact_information,
    contact_information_types, contact_tasks, contacts, group_types, groups, labels, users, vaults,
};
use crate::testing::factory;
use crate::vobject::{self, Kind};

/// The configuration the exports build their URLs from (`APP_URL`).
pub fn config() -> Config {
    Config::for_tests("sqlite::memory:", std::env::temp_dir())
}

/// `new ImportVCard; $importVCard->author = …; $importVCard->vault = …;`.
pub fn vcard_context(author: &users::Model, vault: &vaults::Model) -> ImportVCardContext {
    ImportVCardContext { author: author.clone(), vault: vault.clone(), external: false, uri: None, etag: None }
}

/// `new ImportVCalendar; …->author = …; …->vault = …;`.
pub fn vcalendar_context(author: &users::Model, vault: &vaults::Model) -> ImportVCalendarContext {
    ImportVCalendarContext { author: author.clone(), vault: vault.clone(), external: false, uri: None, etag: None }
}

/// `Contact::factory()->random()` in `vault`.
pub async fn contact_random<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> contacts::Model {
    let contact = factory::contact(db, vault).await;
    let mut am: contacts::ActiveModel = contact.into();
    am.first_name = Set(Some(format!("First{}", random_string(6))));
    am.last_name = Set(Some(format!("Last{}", random_string(6))));
    am.update(db).await.expect("contact")
}

/// `Contact::factory()->create($attributes)`: the factory contact, then `f`.
pub async fn contact_with<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    f: impl FnOnce(&mut contacts::ActiveModel),
) -> contacts::Model {
    let mut am: contacts::ActiveModel = contact_random(db, vault).await.into();
    f(&mut am);
    am.update(db).await.expect("contact")
}

/// The `name` of a factory contact for a user with the default name order
/// (`NameHelper::formatContactName`): `Dr. {first} {last} III`.
pub fn factory_name(contact: &contacts::Model) -> String {
    format!(
        "Dr. {} {} III",
        contact.first_name.as_deref().unwrap_or_default(),
        contact.last_name.as_deref().unwrap_or_default()
    )
}

/// `Gender::factory()->create($attributes)`.
pub async fn gender<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    name: Option<&str>,
    r#type: Option<&str>,
) -> crate::entities::genders::Model {
    crate::entities::genders::ActiveModel {
        account_id: Set(account_id),
        name: Set(name.map(str::to_owned)),
        r#type: Set(r#type.map(str::to_owned)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("gender")
}

/// `GroupType::factory()` in `account_id`.
async fn group_type<C: ConnectionTrait>(db: &C, account_id: Uuid) -> group_types::Model {
    group_types::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(format!("Label {}", random_string(6)))),
        position: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("group type")
}

/// `Group::factory()->create(['vault_id' => …])`.
pub async fn group<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> groups::Model {
    let group_type = group_type(db, vault.account_id).await;
    groups::ActiveModel {
        uuid: Set(Some(Uuid::new())),
        vault_id: Set(vault.id),
        group_type_id: Set(Some(group_type.id)),
        name: Set(format!("Name {}", random_string(6))),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("group")
}

/// `$group->contacts()->syncWithoutDetaching([$contact->id => ['group_type_role_id' => null]])`.
pub async fn add_to_group<C: ConnectionTrait>(db: &C, group: &groups::Model, contact: &contacts::Model) {
    crate::entities::contact_group::ActiveModel {
        group_id: Set(group.id),
        contact_id: Set(contact.id),
        group_type_role_id: Set(None),
        created_at: Set(None),
        updated_at: Set(None),
    }
    .insert(db)
    .await
    .expect("contact_group");
}

/// `Label::factory()->create(['vault_id' => …])`.
pub async fn label<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> labels::Model {
    let word = format!("word{}", random_string(6).to_lowercase());
    labels::ActiveModel {
        vault_id: Set(vault.id),
        name: Set(word.clone()),
        slug: Set(word),
        description: Set(Some(format!("Sentence {}.", random_string(8)))),
        bg_color: Set("bg-zinc-200".into()),
        text_color: Set("text-zinc-700".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("label")
}

/// `$contact->labels()->syncWithoutDetaching($label)`.
pub async fn attach_label<C: ConnectionTrait>(db: &C, label: &labels::Model, contact: &contacts::Model) {
    crate::entities::contact_label::ActiveModel {
        label_id: Set(label.id),
        contact_id: Set(contact.id),
        created_at: Set(None),
        updated_at: Set(None),
    }
    .insert(db)
    .await
    .expect("contact_label");
}

/// `AddressType::factory()->create($attributes)`.
pub async fn address_type<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    name: &str,
    r#type: &str,
) -> address_types::Model {
    address_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(name.to_owned())),
        r#type: Set(Some(r#type.to_owned())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("address type")
}

/// `Address::factory()->create(['vault_id' => …])`.
pub async fn address<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> addresses::Model {
    let account = factory::account(db).await;
    let address_type = address_type(db, account.id, &format!("Name {}", random_string(6)), "home").await;
    addresses::ActiveModel {
        vault_id: Set(vault.id),
        address_type_id: Set(Some(address_type.id)),
        line_1: Set(Some(format!("{} Main Street", rand::random_range(1..999)))),
        line_2: Set(Some(format!("{} Side Street", rand::random_range(1..999)))),
        city: Set(Some(format!("City{}", random_string(5)))),
        province: Set(Some(format!("Province {}", random_string(5)))),
        postal_code: Set(Some(format!("{}", rand::random_range(10000..99999)))),
        country: Set(Some(rand::random_range(1..=3).to_string())),
        latitude: Set(Some(45.5)),
        longitude: Set(Some(-73.5)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("address")
}

/// `$contact->addresses()->attach($address, ['is_past_address' => false])`.
pub async fn attach_address<C: ConnectionTrait>(db: &C, address: &addresses::Model, contact: &contacts::Model) {
    crate::entities::contact_address::ActiveModel {
        contact_id: Set(contact.id),
        address_id: Set(address.id),
        is_past_address: Set(false),
        created_at: Set(Some(crate::db::now())),
        updated_at: Set(Some(crate::db::now())),
    }
    .insert(db)
    .await
    .expect("contact_address");
}

/// `Company::factory()->create(['vault_id' => …])`.
pub async fn company<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> companies::Model {
    companies::ActiveModel {
        vault_id: Set(vault.id),
        name: Set(format!("Name {}", random_string(6))),
        r#type: Set(Some("company".into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("company")
}

/// `ContactInformationType::factory()->create($attributes)` (type `email`,
/// protocol `mailto:` and a name unless given).
pub async fn contact_information_type<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    f: impl FnOnce(&mut contact_information_types::ActiveModel),
) -> contact_information_types::Model {
    let mut am = contact_information_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        protocol: Set(Some("mailto:".into())),
        can_be_deleted: Set(false),
        r#type: Set(Some("email".into())),
        ..Default::default()
    };
    f(&mut am);
    am.insert(db).await.expect("contact information type")
}

/// `ContactInformation::factory()->create([...])`.
pub async fn contact_information<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    type_id: Id,
    data: &str,
) -> contact_information::Model {
    contact_information::ActiveModel {
        contact_id: Set(contact.id),
        type_id: Set(type_id),
        data: Set(data.to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact information")
}

/// `ContactImportantDateType::factory()->create(['vault_id' => …])`.
pub async fn important_date_type<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
) -> contact_important_date_types::Model {
    contact_important_date_types::ActiveModel {
        vault_id: Set(vault.id),
        label: Set("Birthdate".into()),
        internal_type: Set(Some("birthdate".into())),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact important date type")
}

/// `ContactImportantDate::factory()->create($attributes)`: 1981-10-29,
/// `birthdate`, of a new birthdate type unless given.
pub async fn important_date<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    f: impl FnOnce(&mut contact_important_dates::ActiveModel),
) -> contact_important_dates::Model {
    let mut am = contact_important_dates::ActiveModel {
        contact_id: Set(contact.id),
        label: Set("birthdate".into()),
        day: Set(Some(29)),
        month: Set(Some(10)),
        year: Set(Some(1981)),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    };
    f(&mut am);
    if am.contact_important_date_type_id.is_not_set() {
        let vault = vaults::Entity::find_by_id(contact.vault_id).one(db).await.unwrap().unwrap();
        am.contact_important_date_type_id = Set(Some(important_date_type(db, &vault).await.id));
    }
    am.insert(db).await.expect("contact important date")
}

/// `ContactTask::factory()->create(['contact_id' => …])`.
pub async fn task<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> contact_tasks::Model {
    let author = factory::user(db, None).await;
    let due = chrono::NaiveDate::from_ymd_opt(2003, 4, 5).unwrap().and_hms_opt(6, 7, 8).unwrap();
    contact_tasks::ActiveModel {
        contact_id: Set(contact.id),
        author_id: Set(Some(author.id)),
        author_name: Set(format!("Name {}", random_string(6))),
        label: Set(format!("Sentence {} {}.", random_string(5), random_string(7))),
        completed: Set(false),
        due_at: Set(Some(crate::db::timestamp::Timestamp(due))),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact task")
}

/// `CardEtag::append()`: a line, folded at 75 characters.
pub fn append(content: &str, data: &str) -> String {
    let mut data = data.to_owned();
    let mut content: Vec<char> = content.chars().collect();
    let mut tab = String::new();
    loop {
        let line: Vec<char> = tab.chars().chain(content.iter().copied()).collect();
        if line.len() <= 75 {
            data.push_str(&line.iter().collect::<String>());
            data.push('\n');
            return data;
        }
        data.push_str(&line[..75].iter().collect::<String>());
        data.push('\n');
        content = line[75..].to_vec();
        tab = " ".to_owned();
    }
}

fn ymdhis(t: Option<crate::db::timestamp::Timestamp>) -> String {
    t.unwrap().0.format("%Y%m%dT%H%M%SZ").to_string()
}

/// `CardEtag::getCard($contact, $realFormat)`.
pub async fn get_card<C: ConnectionTrait>(db: &C, contact: &contacts::Model, real_format: bool) -> String {
    let contact = contacts::Entity::find_by_id(contact.id).one(db).await.unwrap().unwrap();
    let url = format!("{}/vaults/{}/contacts/{}", config().url, contact.vault_id, contact.id);
    let timestamp = ymdhis(contact.updated_at);
    let s = |v: &Option<String>| v.clone().unwrap_or_default();

    let mut data = append("BEGIN:VCARD", "");
    data = append("VERSION:4.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", vobject::VERSION), &data);
    data = append(&format!("UID:{}", contact.id), &data);
    data = append(&format!("SOURCE:{url}"), &data);
    data = append(&format!("FN:{}", factory_name(&contact)), &data);
    data =
        append(&format!("N:{};{};{};;", s(&contact.last_name), s(&contact.first_name), s(&contact.middle_name)), &data);

    if let Some(nickname) = &contact.nickname {
        data = append(&format!("NICKNAME:{nickname}"), &data);
    }

    if let Some(gender_id) = contact.gender_id {
        let gender = crate::entities::genders::Entity::find_by_id(gender_id).one(db).await.unwrap().unwrap();
        data = append(&format!("GENDER:{}", s(&gender.r#type)), &data);
    }

    data = append(&format!("REV:{timestamp}"), &data);
    data = append("END:VCARD", &data);

    if real_format { data.replace('\n', "\r\n") } else { data }
}

/// `CardEtag::getTask($task)`.
pub async fn get_task<C: ConnectionTrait>(db: &C, task: &contact_tasks::Model) -> String {
    let task = contact_tasks::Entity::find_by_id(task.id).one(db).await.unwrap().unwrap();
    let contact = contacts::Entity::find_by_id(task.contact_id).one(db).await.unwrap().unwrap();
    let url = format!("{}/vaults/{}/contacts/{}", config().url, contact.vault_id, contact.id);

    let mut data = append("BEGIN:VCALENDAR", "");
    data = append("VERSION:2.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", vobject::VERSION), &data);
    data = append("CALSCALE:GREGORIAN", &data);
    data = append(&format!("SOURCE:{url}"), &data);

    data = append("BEGIN:VTIMEZONE", &data);
    data = append("TZID:UTC", &data);
    data = append("END:VTIMEZONE", &data);

    data = append("BEGIN:VTODO", &data);
    data = append(&format!("UID:{}", task.uuid.unwrap()), &data);
    data = append(&format!("SUMMARY:{}", task.label), &data);
    data = append(&format!("DTSTAMP:{}", ymdhis(task.created_at)), &data);
    data = append(&format!("CREATED:{}", ymdhis(task.created_at)), &data);
    data = append(&format!("LAST-MODIFIED:{}", ymdhis(task.updated_at)), &data);
    if task.due_at.is_some() {
        data = append(&format!("DUE:{}", ymdhis(task.due_at)), &data);
    }

    data = append(&format!("ATTACH:{url}"), &data);
    data = append("END:VTODO", &data);
    data = append("END:VCALENDAR", &data);

    data.replace('\n', "\r\n")
}

/// `CardEtag::getDate($date)`.
pub async fn get_date<C: ConnectionTrait>(db: &C, date: &contact_important_dates::Model) -> String {
    let date = contact_important_dates::Entity::find_by_id(date.id).one(db).await.unwrap().unwrap();
    let contact = contacts::Entity::find_by_id(date.contact_id).one(db).await.unwrap().unwrap();
    let url = format!("{}/vaults/{}/contacts/{}", config().url, contact.vault_id, contact.id);
    let (year, month, day) = (date.year.unwrap(), date.month.unwrap(), date.day.unwrap());

    let mut data = append("BEGIN:VCALENDAR", "");
    data = append("VERSION:2.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", vobject::VERSION), &data);
    data = append("CALSCALE:GREGORIAN", &data);
    data = append(&format!("SOURCE:{url}"), &data);

    data = append("BEGIN:VTIMEZONE", &data);
    data = append("TZID:UTC", &data);
    data = append("END:VTIMEZONE", &data);

    data = append("BEGIN:VEVENT", &data);
    data = append(&format!("UID:{}", date.uuid.unwrap()), &data);
    data = append(&format!("SUMMARY:{}", date.label), &data);
    data = append(&format!("DTSTART;VALUE=DATE:{year}{month}{day}"), &data);
    let next = day + 1;
    data = append(&format!("DTEND;VALUE=DATE:{year}{month}{next}"), &data);
    data = append(&format!("RRULE:FREQ=YEARLY;BYMONTH={month};BYMONTHDAY={day}"), &data);

    data = append(&format!("DTSTAMP:{}", ymdhis(date.created_at)), &data);
    data = append(&format!("CREATED:{}", ymdhis(date.created_at)), &data);
    data = append(&format!("LAST-MODIFIED:{}", ymdhis(date.updated_at)), &data);

    data = append(&format!("ATTACH:{url}"), &data);
    data = append(&format!("DESCRIPTION:See {} profile {url}", factory_name(&contact)), &data);
    data = append("END:VEVENT", &data);
    data = append("END:VCALENDAR", &data);

    data.replace('\n', "\r\n")
}

/// `assertVObjectEqualsVObject($expected, $actual)`: both read, without
/// `PRODID` (and a `GREGORIAN` `CALSCALE`); an expected `NAME:**ANY**` line
/// matches any value.
pub fn assert_vobject_equals_vobject(expected: &str, actual: &str) {
    let get_obj = |input: &str| {
        let mut input = vobject::read(input, 0).expect("Input must be a string, stream or VObject component");
        input.remove("PRODID");
        if input.kind == Kind::VCalendar
            && input.property("CALSCALE").map(ToString::to_string).as_deref() == Some("GREGORIAN")
        {
            input.remove("CALSCALE");
        }
        input.serialize()
    };

    let expected = get_obj(expected);
    let mut actual = get_obj(actual);

    // Finding wildcards in expected.
    for line in expected.split('\n') {
        let Some(name) = line.strip_suffix(":**ANY**\r") else { continue };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_uppercase()) {
            continue;
        }
        actual = actual
            .split('\n')
            .map(|l| {
                if l.starts_with(&format!("{name}:")) && l.ends_with('\r') {
                    format!("{name}:**ANY**\r")
                } else {
                    l.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    assert_eq!(expected, actual);
}
