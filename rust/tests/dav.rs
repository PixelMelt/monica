//! The CardDAV / CalDAV server (`/dav`): the PHP tests
//! `tests/Unit/Domains/Contact/DAV/Services/{DAVServerTest,CardDAVTest,
//! CalDAVTest,CalDAVDatesTest,CalDAVTasksTest,VCardContactTest}.php` (one
//! module each, the fixtures of `CardEtag`), then HTTP tests of discovery,
//! sync, writes, exports and authentication.

mod common;

use base64::Engine;
use chrono::NaiveDate;
use common::{TestApp, TestResponse};
use monica::db::timestamp::Timestamp;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::entities::{
    contact_group, contact_important_date_types, contact_important_dates, contact_tasks, contacts, genders,
    group_types, groups, sync_tokens, users, vaults,
};
use monica::testing::factory;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

// ---------------------------------------------------------------- harness

/// A user authenticated like `createUser()` (`Sanctum::actingAs($user,
/// ['read', 'write'])`): Basic with the email and a read/write token.
struct Dav {
    app: TestApp,
    user: users::Model,
    token: String,
}

impl Dav {
    async fn new() -> Dav {
        let app = TestApp::new().await;
        let user = factory::create_user(&app.state.db).await;
        let token = token(&app, &user, &["read", "write"]).await;
        Dav { app, user, token }
    }

    fn db(&self) -> &DatabaseConnection {
        &self.app.state.db
    }

    /// `$this->call($method, $uri, …, $server, $content)`.
    async fn call(&mut self, method: &str, uri: &str, body: &str, headers: &[(&str, &str)]) -> TestResponse {
        let auth = basic(&self.user.email, &self.token);
        let mut all = vec![("authorization", auth.as_str())];
        all.extend_from_slice(headers);
        self.app.raw(method, uri, body, &all).await
    }

    async fn finish(self) {
        self.app.finish().await;
    }
}

/// A personal access token of `user` with these abilities.
async fn token(app: &TestApp, user: &users::Model, abilities: &[&str]) -> String {
    let abilities: Vec<String> = abilities.iter().map(|a| (*a).to_owned()).collect();
    monica::http::tokens::create(&app.state, user.id, "dav", &abilities).await.expect("token").1
}

fn basic(user: &str, password: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}")))
}

fn body(res: &TestResponse) -> String {
    String::from_utf8_lossy(&res.body).into_owned()
}

fn header(res: &TestResponse, name: &str) -> Option<String> {
    res.headers.get(name).map(|v| v.to_str().unwrap().to_owned())
}

/// `$response->assertStatus($status)`.
fn assert_status(res: &TestResponse, status: u16) {
    assert_eq!(res.status.as_u16(), status, "body: {}", body(res));
}

/// `$response->assertSee($value, false)`.
#[track_caller]
fn assert_see(res: &TestResponse, value: &str) {
    let body = body(res);
    assert!(body.contains(value), "expected to see:\n{value}\n\nin:\n{body}");
}

/// `$response->assertHeader('X-Sabre-Version')`.
#[track_caller]
fn assert_sabre(res: &TestResponse) {
    assert_eq!(header(res, "x-sabre-version").as_deref(), Some("4.7.0"));
}

/// `rawurlencode($value)`.
fn rawurlencode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The multistatus root element of the server.
const MULTISTATUS: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:card="urn:ietf:params:xml:ns:carddav" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/">"#;

fn at(y: i32, m: u32, d: u32, h: u32) -> Timestamp {
    Timestamp(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap())
}

// --------------------------------------------------------------- factories

/// `createVaultUser($user, $permission)`.
async fn vault_user(db: &DatabaseConnection, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    factory::create_vault_user(db, user, permission).await
}

/// `Contact::factory()->random()->create(['vault_id' => …])`.
async fn contact_random(db: &DatabaseConnection, vault: &vaults::Model) -> contacts::Model {
    let contact = factory::contact(db, vault).await;
    let mut am: contacts::ActiveModel = contact.into();
    am.first_name = Set(Some(format!("First{}", monica::crypto::random_string(6))));
    am.last_name = Set(Some(format!("Last{}", monica::crypto::random_string(6))));
    am.update(db).await.expect("contact")
}

/// Sets the timestamps of every contact of the vault (`Carbon::setTestNow()`
/// around their creation).
async fn contacts_at(db: &DatabaseConnection, vault: &vaults::Model, time: Timestamp) {
    for contact in contacts::Entity::find().filter(contacts::Column::VaultId.eq(vault.id)).all(db).await.unwrap() {
        let mut am: contacts::ActiveModel = contact.into();
        am.created_at = Set(Some(time));
        am.updated_at = Set(Some(time));
        am.update(db).await.unwrap();
    }
}

/// `ContactImportantDateType::factory()->create(['vault_id' => …])`.
async fn important_date_type(db: &DatabaseConnection, vault: &vaults::Model) -> contact_important_date_types::Model {
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

/// `ContactImportantDate::factory()->create(['contact_id' => …] + $label)`: 1981-10-29.
async fn important_date(
    db: &DatabaseConnection,
    contact: &contacts::Model,
    label: &str,
) -> contact_important_dates::Model {
    let vault = vaults::Entity::find_by_id(contact.vault_id).one(db).await.unwrap().unwrap();
    let date_type = important_date_type(db, &vault).await;
    contact_important_dates::ActiveModel {
        contact_id: Set(contact.id),
        contact_important_date_type_id: Set(Some(date_type.id)),
        label: Set(label.to_owned()),
        day: Set(Some(29)),
        month: Set(Some(10)),
        year: Set(Some(1981)),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact important date")
}

/// `ContactTask::factory()->create(['contact_id' => …] + $label)`.
async fn task(db: &DatabaseConnection, contact: &contacts::Model, label: &str) -> contact_tasks::Model {
    let author = factory::user(db, None).await;
    let due = NaiveDate::from_ymd_opt(2003, 4, 5).unwrap().and_hms_opt(6, 7, 8).unwrap();
    contact_tasks::ActiveModel {
        contact_id: Set(contact.id),
        author_id: Set(Some(author.id)),
        author_name: Set(format!("Name {}", monica::crypto::random_string(6))),
        label: Set(label.to_owned()),
        completed: Set(false),
        due_at: Set(Some(Timestamp(due))),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("contact task")
}

/// `SyncToken::factory()->create([...])` at `time`.
async fn sync_token(db: &DatabaseConnection, user: &users::Model, name: &str, time: Timestamp) -> sync_tokens::Model {
    sync_tokens::ActiveModel {
        account_id: Set(user.account_id),
        user_id: Set(user.id),
        name: Set(name.to_owned()),
        timestamp: Set(time),
        created_at: Set(Some(time)),
        updated_at: Set(Some(time)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("sync token")
}

/// `SyncToken::where([...])->orderBy('created_at')->get()`.
async fn tokens_named(db: &DatabaseConnection, user: &users::Model, name: &str) -> Vec<sync_tokens::Model> {
    sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(user.account_id))
        .filter(sync_tokens::Column::UserId.eq(user.id))
        .filter(sync_tokens::Column::Name.eq(name))
        .order_by_asc(sync_tokens::Column::CreatedAt)
        .order_by_asc(sync_tokens::Column::Id)
        .all(db)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- CardEtag

/// `CardEtag::append()`: a line, folded at 75 characters.
fn append(content: &str, data: &str) -> String {
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

fn ymdhis(t: Option<Timestamp>) -> String {
    t.unwrap().0.format("%Y%m%dT%H%M%SZ").to_string()
}

/// `route('contact.show', …)`.
fn contact_url(contact: &contacts::Model) -> String {
    format!("http://localhost/vaults/{}/contacts/{}", contact.vault_id, contact.id)
}

/// The name the export gives a factory contact (`Dr. {first} {last} III`).
fn factory_name(contact: &contacts::Model) -> String {
    format!(
        "Dr. {} {} III",
        contact.first_name.as_deref().unwrap_or_default(),
        contact.last_name.as_deref().unwrap_or_default()
    )
}

/// `CardEtag::getCard($contact, $realFormat)`.
async fn get_card(db: &DatabaseConnection, contact: &contacts::Model, real_format: bool) -> String {
    let contact = contacts::Entity::find_by_id(contact.id).one(db).await.unwrap().unwrap();
    let s = |v: &Option<String>| v.clone().unwrap_or_default();

    let mut data = append("BEGIN:VCARD", "");
    data = append("VERSION:4.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", monica::vobject::VERSION), &data);
    data = append(&format!("UID:{}", contact.id), &data);
    data = append(&format!("SOURCE:{}", contact_url(&contact)), &data);
    data = append(&format!("FN:{}", factory_name(&contact)), &data);
    data =
        append(&format!("N:{};{};{};;", s(&contact.last_name), s(&contact.first_name), s(&contact.middle_name)), &data);
    if let Some(nickname) = &contact.nickname {
        data = append(&format!("NICKNAME:{nickname}"), &data);
    }
    if let Some(gender_id) = contact.gender_id {
        let gender = genders::Entity::find_by_id(gender_id).one(db).await.unwrap().unwrap();
        data = append(&format!("GENDER:{}", s(&gender.r#type)), &data);
    }
    data = append(&format!("REV:{}", ymdhis(contact.updated_at)), &data);
    data = append("END:VCARD", &data);

    if real_format { data.replace('\n', "\r\n") } else { data }
}

/// `CardEtag::getTask($task)`.
async fn get_task(db: &DatabaseConnection, task: &contact_tasks::Model) -> String {
    let task = contact_tasks::Entity::find_by_id(task.id).one(db).await.unwrap().unwrap();
    let contact = contacts::Entity::find_by_id(task.contact_id).one(db).await.unwrap().unwrap();
    let url = contact_url(&contact);

    let mut data = append("BEGIN:VCALENDAR", "");
    data = append("VERSION:2.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", monica::vobject::VERSION), &data);
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
async fn get_date(db: &DatabaseConnection, date: &contact_important_dates::Model) -> String {
    let date = contact_important_dates::Entity::find_by_id(date.id).one(db).await.unwrap().unwrap();
    let contact = contacts::Entity::find_by_id(date.contact_id).one(db).await.unwrap().unwrap();
    let url = contact_url(&contact);
    let (year, month, day) = (date.year.unwrap(), date.month.unwrap(), date.day.unwrap());

    let mut data = append("BEGIN:VCALENDAR", "");
    data = append("VERSION:2.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", monica::vobject::VERSION), &data);
    data = append("CALSCALE:GREGORIAN", &data);
    data = append(&format!("SOURCE:{url}"), &data);
    data = append("BEGIN:VTIMEZONE", &data);
    data = append("TZID:UTC", &data);
    data = append("END:VTIMEZONE", &data);
    data = append("BEGIN:VEVENT", &data);
    data = append(&format!("UID:{}", date.uuid.unwrap()), &data);
    data = append(&format!("SUMMARY:{}", date.label), &data);
    data = append(&format!("DTSTART;VALUE=DATE:{year}{month}{day}"), &data);
    data = append(&format!("DTEND;VALUE=DATE:{year}{month}{}", day + 1), &data);
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

/// `hash('sha256', $data)`.
fn sha256(data: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data.as_bytes()))
}

/// `CardEtag::getEtag($contact)`.
async fn card_etag(db: &DatabaseConnection, contact: &contacts::Model) -> String {
    sha256(&get_card(db, contact, true).await)
}

/// `assertVObjectEqualsVObject($expected, $actual)`: both read, without `PRODID`.
fn assert_vobject_equals_vobject(expected: &str, actual: &str) {
    let get_obj = |input: &str| {
        let mut input = monica::vobject::read(input, 0).expect("a vobject");
        input.remove("PRODID");
        input.serialize()
    };
    assert_eq!(get_obj(expected), get_obj(actual));
}

/// The body of a `sync-collection` report.
fn sync_collection(token: &str) -> String {
    format!(
        "<sync-collection xmlns='DAV:'>
                <sync-token>{token}</sync-token>
                <sync-level>1</sync-level>
                <prop>
                    <getetag />
                </prop>
            </sync-collection>"
    )
}

/// The getctag / sync-token PROPFIND body.
const GETCTAG: &str = "<propfind xmlns='DAV:' xmlns:cs='http://calendarserver.org/ns/'>
                <prop>
                    <cs:getctag />
                    <sync-token />
                </prop>
            </propfind>";

const XML: (&str, &str) = ("content-type", "application/xml; charset=utf-8");

// ---------------------------------------------------------- DAVServerTest

mod dav_server_test {
    use super::*;

    #[tokio::test]
    async fn test_dav_propfind_base() {
        let mut t = Dav::new().await;

        let response = t.call("PROPFIND", "/dav", "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, "<d:response><d:href>/dav/</d:href>");
        assert_see(&response, "<d:response><d:href>/dav/principals/</d:href>");
        assert_see(&response, "<d:response><d:href>/dav/addressbooks/</d:href>");
        t.finish().await;
    }

    #[tokio::test]
    async fn test_dav_propfind_principals() {
        let mut t = Dav::new().await;

        let response = t.call("PROPFIND", "/dav/principals", "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, "<d:response><d:href>/dav/principals/</d:href>");
        assert_see(&response, &format!("<d:response><d:href>/dav/principals/{}/</d:href>", t.user.email));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_dav_propfind_principals_user() {
        let mut t = Dav::new().await;
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/principals/{email}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/principals/{email}/</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_dav_ensure_browser_plugin_not_enabled() {
        let mut t = Dav::new().await;

        let response = t.call("GET", "/dav", "", &[]).await;

        assert_status(&response, 302);
        assert_sabre(&response);
        assert_eq!(header(&response, "location").as_deref(), Some("http://localhost"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_groupmemberset() {
        let mut t = Dav::new().await;
        let email = t.user.email.clone();

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/principals/{email}/"),
                r#"<propfind xmlns="DAV:"
                xmlns:CAL="urn:ietf:params:xml:ns:caldav"
                xmlns:CARD="urn:ietf:params:xml:ns:carddav">
                <prop>
                    <CARD:addressbook-home-set />
                    <group-member-set />
                </prop>
            </propfind>"#,
                &[XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(
            &response,
            &format!(
                "{MULTISTATUS}<d:response><d:href>/dav/principals/{email}/</d:href><d:propstat><d:prop><card:addressbook-home-set><d:href>/dav/addressbooks/{email}/</d:href></card:addressbook-home-set><d:group-member-set><d:href>/dav/principals/{email}/</d:href></d:group-member-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_report_propertysearch() {
        let mut t = Dav::new().await;
        let email = t.user.email.clone();
        let name = format!(
            "{} {}",
            t.user.first_name.clone().unwrap_or_default(),
            t.user.last_name.clone().unwrap_or_default()
        );

        let response = t
            .call(
                "REPORT",
                "/dav/principals/",
                &format!(
                    r#"<principal-property-search xmlns="DAV:">
                <property-search>
                    <match>{name}</match>
                    <prop>
                        <displayname/>
                    </prop>
                </property-search>
                <prop>
                    <displayname/>
                </prop>
            </principal-property-search>"#
                ),
                &[("depth", "0"), XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(
            &response,
            &format!(
                "{MULTISTATUS}<d:response><d:href>/dav/principals/{email}/</d:href><d:propstat><d:prop><d:displayname>{name}</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"
            ),
        );
        t.finish().await;
    }
}

// ------------------------------------------------------------ CardDAVTest

mod card_dav_test {
    use super::*;

    #[tokio::test]
    async fn test_carddav_propfind_addressbooks() {
        let mut t = Dav::new().await;

        let response = t.call("PROPFIND", "/dav/addressbooks", "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, "<d:response><d:href>/dav/addressbooks/</d:href>");
        assert_see(&response, &format!("<d:response><d:href>/dav/addressbooks/{}/</d:href>", t.user.email));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_addressbooks_user() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/addressbooks/{email}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/addressbooks/{email}/</d:href>"));
        assert_see(&response, &format!("<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_contacts() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/addressbooks/{email}/{vaultname}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href>"));
        assert_see(
            &response,
            &format!("<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>", contact.id),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_contacts_with_props() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                r#"<d:propfind xmlns:d="DAV:">
               <d:prop>
                 <d:displayname />
               </d:prop>
             </d:propfind>"#,
                &[("depth", "0")],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href><d:propstat><d:prop><d:displayname>{}</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                vault.name
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_one_contact() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id);
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_propfind_one_contact_without_extension() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/addressbooks/{email}/{vaultname}/{}", contact.id);
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_getctag() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response =
            t.call("PROPFIND", &format!("/dav/addressbooks/{email}/"), GETCTAG, &[("depth", "1"), XML]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_getctag_contacts() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call("PROPFIND", &format!("/dav/addressbooks/{email}/{vaultname}/"), GETCTAG, &[("depth", "0"), XML])
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_sync_collection_with_token() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        contacts_at(t.db(), &vault, at(2019, 1, 1, 9)).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let token = sync_token(t.db(), &t.user, &format!("contacts-{}", vault.id), at(2018, 1, 1, 8)).await;

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                &sync_collection(&format!("http://sabre.io/ns/sync/{}", token.id)),
                &[XML],
            )
            .await;

        assert_status(&response, 207);

        let token = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await.pop().unwrap();
        let etag = card_etag(t.db(), &contact).await;

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                contact.id, token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_sync_collection_init() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                "<sync-collection xmlns='DAV:'>
                <sync-token />
                <sync-level>1</sync-level>
                <prop>
                    <getetag />
                </prop>
            </sync-collection>",
                &[XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();
        let etag = card_etag(t.db(), &contact).await;

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                contact.id, token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_sync_collection_deleted_contact() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        contacts_at(t.db(), &vault, at(2019, 1, 1, 9)).await;
        let mut am: contacts::ActiveModel =
            contacts::Entity::find_by_id(contact.id).one(t.db()).await.unwrap().unwrap().into();
        am.deleted_at = Set(Some(at(2019, 3, 1, 9)));
        am.updated_at = Set(Some(at(2019, 1, 1, 9)));
        am.update(t.db()).await.unwrap();
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let token = sync_token(t.db(), &t.user, &format!("contacts-{}", vault.id), at(2019, 2, 1, 9)).await;

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                &sync_collection(&format!("http://sabre.io/ns/sync/{}", token.id)),
                &[XML],
            )
            .await;

        assert_status(&response, 207);

        let token = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await.pop().unwrap();

        assert_see(
            &response,
            &format!(
                "{MULTISTATUS}
 <d:response>
  <d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>
  <d:status>HTTP/1.1 404 Not Found</d:status>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>
</d:multistatus>",
                contact.id, token.id
            ),
        );
        t.finish().await;
    }
}

// ------------------------------------------------------------- CalDAVTest

mod cal_dav_test {
    use super::*;

    #[tokio::test]
    async fn test_caldav_propfind_calendars() {
        let mut t = Dav::new().await;

        let response = t.call("PROPFIND", "/dav/calendars", "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, "<d:response><d:href>/dav/calendars/</d:href>");
        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{}/</d:href>", t.user.email));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_calendars_user() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/calendars/{email}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{email}/</d:href>"));
        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href>"));
        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/</d:href>"));
        t.finish().await;
    }
}

// --------------------------------------------------------- CalDAVDatesTest

mod cal_dav_dates_test {
    use super::*;

    #[tokio::test]
    async fn test_caldav_propfind_dates() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/calendars/{email}/dates-{vaultname}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href>"));
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/{}.ics</d:href>",
                date.uuid.unwrap()
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_date_with_props() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/calendars/{email}/dates-{vaultname}/"),
                r#"<d:propfind xmlns:d="DAV:">
               <d:prop>
                 <d:displayname />
               </d:prop>
             </d:propfind>"#,
                &[("depth", "0")],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href><d:propstat><d:prop><d:displayname>Contact important dates of {}</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                vault.name
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_one_date() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/calendars/{email}/dates-{vaultname}/{}.ics", date.uuid.unwrap());
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_one_date_without_extension() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/calendars/{email}/dates-{vaultname}/{}", date.uuid.unwrap());
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_getctag() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/calendars/{email}/"), GETCTAG, &[("depth", "1"), XML]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("dates-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_getctag_dates() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call("PROPFIND", &format!("/dav/calendars/{email}/dates-{vaultname}/"), GETCTAG, &[("depth", "0"), XML])
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("dates-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_sync_collection_with_token() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let mut am: contact_important_dates::ActiveModel = date.clone().into();
        am.created_at = Set(Some(at(2019, 1, 1, 9)));
        am.updated_at = Set(Some(at(2019, 1, 1, 9)));
        let date = am.update(t.db()).await.unwrap();
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let token = sync_token(t.db(), &t.user, &format!("dates-{}", vault.id), at(2018, 1, 1, 8)).await;

        let response = t
            .call(
                "REPORT",
                &format!("/dav/calendars/{email}/dates-{vaultname}/"),
                &sync_collection(&format!("http://sabre.io/ns/sync/{}", token.id)),
                &[XML],
            )
            .await;

        assert_status(&response, 207);

        let token = tokens_named(t.db(), &t.user, &format!("dates-{}", vault.id)).await.pop().unwrap();
        let etag = sha256(&get_date(t.db(), &date).await);

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/calendars/{email}/dates-{vaultname}/{}.ics</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                date.uuid.unwrap(),
                token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_sync_collection_init() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/calendars/{email}/dates-{vaultname}/"),
                "<sync-collection xmlns='DAV:'>
                <sync-token />
                <sync-level>1</sync-level>
                <prop>
                    <getetag />
                </prop>
            </sync-collection>",
                &[XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("dates-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();
        let etag = sha256(&get_date(t.db(), &date).await);

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/calendars/{email}/dates-{vaultname}/{}.ics</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                date.uuid.unwrap(),
                token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_update_one_date() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "Old Label").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let data = get_date(t.db(), &date).await.replace("Old Label", "New Label");

        let response = t
            .call("PUT", &format!("/dav/calendars/{email}/dates-{vaultname}/{}.ics", date.uuid.unwrap()), &data, &[XML])
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);

        let date = contact_important_dates::Entity::find_by_id(date.id).one(t.db()).await.unwrap().unwrap();
        assert_eq!(date.label, "New Label");
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_put_one_date() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        important_date_type(t.db(), &vault).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let data = "BEGIN:VCALENDAR
PRODID:-//Sabre//Sabre VObject 4.5.3//EN
VERSION:2.0
BEGIN:VEVENT
CREATED:20250814T000000Z
LAST-MODIFIED:20250814T000000Z
DTSTAMP:20250814T000000Z
UID:36ee6e82-5262-404f-aea1-98859c631892
SUMMARY:Réunion importante
DTSTART;VALUE=DATE:20250813
DTEND;VALUE=DATE:20250814
TRANSP:TRANSPARENT
END:VEVENT
END:VCALENDAR
";

        let response = t
            .call(
                "PUT",
                &format!("/dav/calendars/{email}/dates-{vaultname}/36ee6e82-5262-404f-aea1-98859c631892.ics"),
                data,
                &[XML],
            )
            .await;

        assert_status(&response, 201);
        assert_sabre(&response);

        let uuid: Uuid = "36ee6e82-5262-404f-aea1-98859c631892".parse().unwrap();
        let date = contact_important_dates::Entity::find()
            .filter(contact_important_dates::Column::Uuid.eq(uuid))
            .one(t.db())
            .await
            .unwrap()
            .expect("the date");
        assert_eq!(date.label, "Réunion importante");
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_delete_one_date() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call("DELETE", &format!("/dav/calendars/{email}/dates-{vaultname}/{}.ics", date.uuid.unwrap()), "", &[])
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);

        let date = contact_important_dates::Entity::find_by_id(date.id).one(t.db()).await.unwrap().unwrap();
        assert!(date.deleted_at.is_some());
        t.finish().await;
    }
}

// --------------------------------------------------------- CalDAVTasksTest

mod cal_dav_tasks_test {
    use super::*;

    #[tokio::test]
    async fn test_caldav_propfind_tasks() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/calendars/{email}/tasks-{vaultname}"), "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/</d:href>"));
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/{}.ics</d:href>",
                task.uuid.unwrap()
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_tasks_with_props() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/calendars/{email}/tasks-{vaultname}/"),
                r#"<d:propfind xmlns:d="DAV:">
               <d:prop>
                 <d:displayname />
               </d:prop>
             </d:propfind>"#,
                &[("depth", "0")],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/</d:href><d:propstat><d:prop><d:displayname>Tasks of {}</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                vault.name
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_one_task() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/calendars/{email}/tasks-{vaultname}/{}.ics", task.uuid.unwrap());
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_propfind_one_date_without_extension() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let path = format!("/dav/calendars/{email}/tasks-{vaultname}/{}", task.uuid.unwrap());
        let response = t.call("PROPFIND", &path, "", &[]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        assert_see(&response, &format!("<d:response><d:href>{path}</d:href>"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_getctag() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.call("PROPFIND", &format!("/dav/calendars/{email}/"), GETCTAG, &[("depth", "1"), XML]).await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("tasks-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_getctag_tasks() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call("PROPFIND", &format!("/dav/calendars/{email}/tasks-{vaultname}/"), GETCTAG, &[("depth", "0"), XML])
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("tasks-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();

        assert_see(&response, MULTISTATUS);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/tasks-{vaultname}/</d:href><d:propstat><d:prop><cs:getctag>http://sabre.io/ns/sync/{id}</cs:getctag><d:sync-token>http://sabre.io/ns/sync/{id}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                id = token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_sync_collection_with_token() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let mut am: contact_tasks::ActiveModel = task.clone().into();
        am.created_at = Set(Some(at(2019, 1, 1, 9)));
        am.updated_at = Set(Some(at(2019, 1, 1, 9)));
        let task = am.update(t.db()).await.unwrap();
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let token = sync_token(t.db(), &t.user, &format!("tasks-{}", vault.id), at(2018, 1, 1, 8)).await;

        let response = t
            .call(
                "REPORT",
                &format!("/dav/calendars/{email}/tasks-{vaultname}/"),
                &sync_collection(&format!("http://sabre.io/ns/sync/{}", token.id)),
                &[XML],
            )
            .await;

        assert_status(&response, 207);

        let token = tokens_named(t.db(), &t.user, &format!("tasks-{}", vault.id)).await.pop().unwrap();
        let etag = sha256(&get_task(t.db(), &task).await);

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/calendars/{email}/tasks-{vaultname}/{}.ics</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                task.uuid.unwrap(),
                token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_sync_collection_init() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/calendars/{email}/tasks-{vaultname}/"),
                "<sync-collection xmlns='DAV:'>
                <sync-token />
                <sync-level>1</sync-level>
                <prop>
                    <getetag />
                </prop>
            </sync-collection>",
                &[XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let tokens = tokens_named(t.db(), &t.user, &format!("tasks-{}", vault.id)).await;
        assert!(!tokens.is_empty());
        let token = tokens.last().unwrap();
        let etag = sha256(&get_task(t.db(), &task).await);

        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>/dav/calendars/{email}/tasks-{vaultname}/{}.ics</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>",
                task.uuid.unwrap(),
                token.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_update_one_task() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Old Label").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let data = get_task(t.db(), &task).await.replace("Old Label", "New Label");

        let response = t
            .call("PUT", &format!("/dav/calendars/{email}/tasks-{vaultname}/{}.ics", task.uuid.unwrap()), &data, &[XML])
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);

        let task = contact_tasks::Entity::find_by_id(task.id).one(t.db()).await.unwrap().unwrap();
        assert_eq!(task.label, "New Label");
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_put_one_task() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let data = "BEGIN:VCALENDAR
PRODID:-//Sabre//Sabre VObject 4.5.3//EN
VERSION:2.0
BEGIN:VTODO
CREATED:20250729T000000Z
LAST-MODIFIED:20250814T000000Z
DTSTAMP:20250814T000000Z
UID:3a7baf23-50b6-43dd-b441-7d70362f6356
SUMMARY:My Task
END:VTODO
END:VCALENDAR
";

        let response = t
            .call(
                "PUT",
                &format!("/dav/calendars/{email}/tasks-{vaultname}/3a7baf23-50b6-43dd-b441-7d70362f6356.ics"),
                data,
                &[XML],
            )
            .await;

        assert_status(&response, 201);
        assert_sabre(&response);

        let uuid: Uuid = "3a7baf23-50b6-43dd-b441-7d70362f6356".parse().unwrap();
        let task = contact_tasks::Entity::find()
            .filter(contact_tasks::Column::Uuid.eq(uuid))
            .one(t.db())
            .await
            .unwrap()
            .expect("the task");
        assert_eq!(task.label, "My Task");
        t.finish().await;
    }

    #[tokio::test]
    async fn test_caldav_delete_one_task() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Manage).await;
        let contact = contact_random(t.db(), &vault).await;
        let task = task(t.db(), &contact, "Sentence.").await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call("DELETE", &format!("/dav/calendars/{email}/tasks-{vaultname}/{}.ics", task.uuid.unwrap()), "", &[])
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);

        let task = contact_tasks::Entity::find_by_id(task.id).one(t.db()).await.unwrap().unwrap();
        assert!(task.deleted_at.is_some());
        t.finish().await;
    }
}

// ------------------------------------------------------- VCardContactTest

mod vcard_contact_test {
    use super::*;

    /// `$contact->updated_at->addDays($days)->toRfc7231String()`.
    fn rfc7231(contact: &contacts::Model, days: i64) -> String {
        (contact.updated_at.unwrap().0 + chrono::Duration::days(days)).format("%a, %d %b %Y %H:%M:%S GMT").to_string()
    }

    const DOEX: &str = "BEGIN:VCARD\nVERSION:4.0\nFN:John Doex\nN:Doex;John;;;\nEND:VCARD";

    async fn assert_doex(db: &DatabaseConnection, vault: &vaults::Model) {
        let found = contacts::Entity::find()
            .filter(contacts::Column::VaultId.eq(vault.id))
            .filter(contacts::Column::FirstName.eq("John"))
            .filter(contacts::Column::LastName.eq("Doex"))
            .one(db)
            .await
            .unwrap();
        assert!(found.is_some(), "no John Doex in the vault");
    }

    #[tokio::test]
    async fn test_carddav_get_one_contact() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response = t
            .call(
                "GET",
                &format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id),
                "",
                &[("accept", "text/vcard; version=4.0")],
            )
            .await;

        assert_status(&response, 200);
        assert_sabre(&response);

        assert_vobject_equals_vobject(&get_card(t.db(), &contact, true).await, &body(&response));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_put_one_contact() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let uuid = Uuid::new();
        let email = t.user.email.clone();

        let response = t
            .call(
                "PUT",
                &format!("/dav/addressbooks/{email}/{vaultname}/{uuid}.vcf"),
                &format!("BEGIN:VCARD\nVERSION:4.0\nUID:{uuid}\nFN:John Doe\nN:Doe;John;;;\nEND:VCARD"),
                &[XML],
            )
            .await;

        assert_status(&response, 201);
        assert_sabre(&response);
        assert!(header(&response, "etag").is_none());

        let contact = contacts::Entity::find_by_id(uuid).one(t.db()).await.unwrap().expect("the contact");
        assert_eq!(contact.vault_id, vault.id);
        assert_eq!(contact.first_name.as_deref(), Some("John"));
        assert_eq!(contact.last_name.as_deref(), Some("Doe"));
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_base() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response =
            t.call("PUT", &format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id), DOEX, &[XML]).await;

        assert_status(&response, 204);
        assert_sabre(&response);
        assert!(header(&response, "etag").is_none());

        assert_doex(t.db(), &vault).await;
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_if_modified() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let filename = monica::dav::http::urlencode(&format!("{}.vcf", contact.id));
        let email = t.user.email.clone();

        let response = t
            .call(
                "PUT",
                &format!("/dav/addressbooks/{email}/{vaultname}/{filename}"),
                DOEX,
                &[("if-modified-since", &rfc7231(&contact, -1)), XML],
            )
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);
        assert!(header(&response, "etag").is_none());

        assert_doex(t.db(), &vault).await;
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_if_modified_not_modified() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let filename = monica::dav::http::urlencode(&format!("{}.vcf", contact.id));
        let email = t.user.email.clone();

        let response = t
            .call(
                "PUT",
                &format!("/dav/addressbooks/{email}/{vaultname}/{filename}"),
                DOEX,
                &[("if-modified-since", &rfc7231(&contact, 1)), XML],
            )
            .await;

        // Not modified
        assert_status(&response, 304);
        assert_sabre(&response);
        // The PHP test asserts Last-Modified is missing; Server::checkPreconditions() sets it on this 304.
        assert_eq!(header(&response, "last-modified"), Some(rfc7231(&contact, 0)));
        assert!(header(&response, "etag").is_none());
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_if_unmodified() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let filename = monica::dav::http::urlencode(&format!("{}.vcf", contact.id));
        let email = t.user.email.clone();

        let response = t
            .call(
                "PUT",
                &format!("/dav/addressbooks/{email}/{vaultname}/{filename}"),
                DOEX,
                &[("if-unmodified-since", &rfc7231(&contact, 1)), XML],
            )
            .await;

        assert_status(&response, 204);
        assert_sabre(&response);
        assert!(header(&response, "etag").is_none());

        assert_doex(t.db(), &vault).await;
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_if_unmodified_error() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let filename = monica::dav::http::urlencode(&format!("{}.vcf", contact.id));
        let email = t.user.email.clone();

        let response = t
            .call(
                "PUT",
                &format!("/dav/addressbooks/{email}/{vaultname}/{filename}"),
                DOEX,
                &[("if-unmodified-since", &rfc7231(&contact, -1)), XML],
            )
            .await;

        // PRECONDITION FAILED
        assert_status(&response, 412);
        assert_sabre(&response);

        assert_see(
            &response,
            "<d:error xmlns:d=\"DAV:\" xmlns:s=\"http://sabredav.org/ns\">
  <s:sabredav-version>4.7.0</s:sabredav-version>
  <s:exception>Sabre\\DAV\\Exception\\PreconditionFailed</s:exception>
  <s:message>An If-Unmodified-Since header was specified, but the entity has been changed since the specified date.</s:message>",
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_update_existing_contact_no_modify() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let filename = monica::dav::http::urlencode(&format!("{}.vcf", contact.id));
        let email = t.user.email.clone();
        let path = format!("/dav/addressbooks/{email}/{vaultname}/{filename}");

        let response = t.call("GET", &path, "", &[]).await;
        let data = body(&response);

        let response = t.call("PUT", &path, &data, &[XML]).await;

        assert_status(&response, 204);
        assert_sabre(&response);
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_contacts_report_version4() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                r#"<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
               <d:prop>
                 <d:getetag />
                 <card:address-data content-type="text/vcard" version="4.0" />
               </d:prop>
             </card:addressbook-query>"#,
                &[("depth", "1"), XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let vcard = get_card(t.db(), &contact, false).await.replace('\n', "&#13;\n");
        let etag = card_etag(t.db(), &contact).await;

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href><d:propstat><d:prop><d:getetag>&quot;{etag}&quot;</d:getetag><card:address-data>{vcard}</card:address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                contact.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_contacts_report_version3() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                r#"<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
               <d:prop>
                 <d:getetag />
                 <card:address-data content-type="text/vcard" version="3.0" />
               </d:prop>
             </card:addressbook-query>"#,
                &[("depth", "1"), XML],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let vcard =
            get_card(t.db(), &contact, false).await.replace("VERSION:4.0", "VERSION:3.0").replace('\n', "&#13;\n");
        let etag = card_etag(t.db(), &contact).await;

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href><d:propstat><d:prop><d:getetag>&quot;{etag}&quot;</d:getetag><card:address-data>{vcard}</card:address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                contact.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn test_carddav_contacts_report_multiget() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact1 = contact_random(t.db(), &vault).await;
        let contact2 = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response = t
            .call(
                "REPORT",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                &format!(
                    r#"<card:addressbook-multiget xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
               <d:prop>
                 <d:getetag />
                 <card:address-data content-type="text/vcard" version="4.0" />
               </d:prop>
               <d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>
               <d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href>
             </card:addressbook-multiget>"#,
                    contact1.id, contact2.id
                ),
                &[("depth", "1")],
            )
            .await;

        assert_status(&response, 207);
        assert_sabre(&response);

        let vcard1 = get_card(t.db(), &contact1, false).await.replace('\n', "&#13;\n");
        let vcard2 = get_card(t.db(), &contact2, false).await.replace('\n', "&#13;\n");
        let etag1 = card_etag(t.db(), &contact1).await;
        let etag2 = card_etag(t.db(), &contact2).await;

        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href><d:propstat><d:prop><d:getetag>&quot;{etag1}&quot;</d:getetag><card:address-data>{vcard1}</card:address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                contact1.id
            ),
        );
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/{}.vcf</d:href><d:propstat><d:prop><d:getetag>&quot;{etag2}&quot;</d:getetag><card:address-data>{vcard2}</card:address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
                contact2.id
            ),
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn carddav_delete_one_contact() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();

        let response =
            t.call("DELETE", &format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id), "", &[]).await;

        assert_status(&response, 204);
        assert_sabre(&response);
        assert!(header(&response, "etag").is_none());

        let contact = contacts::Entity::find_by_id(contact.id).one(t.db()).await.unwrap().unwrap();
        assert!(contact.deleted_at.is_some());
        t.finish().await;
    }
}

// ------------------------------------------------------------- HTTP tests

mod http {
    use super::*;

    /// A group of the vault with these members (`Group::factory()`).
    async fn group(db: &DatabaseConnection, vault: &vaults::Model, members: &[&contacts::Model]) -> groups::Model {
        let group_type = group_types::ActiveModel {
            account_id: Set(vault.account_id),
            label: Set(Some("Family".into())),
            position: Set(1),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        let group = groups::ActiveModel {
            uuid: Set(Some(Uuid::new())),
            vault_id: Set(vault.id),
            group_type_id: Set(Some(group_type.id)),
            name: Set("The family".into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        for member in members {
            contact_group::ActiveModel {
                group_id: Set(group.id),
                contact_id: Set(member.id),
                group_type_role_id: Set(None),
                created_at: Set(None),
                updated_at: Set(None),
            }
            .insert(db)
            .await
            .unwrap();
        }
        group
    }

    /// The `<d:href>` values of a body, in order.
    fn hrefs(body: &str) -> Vec<String> {
        body.split("<d:href>").skip(1).filter_map(|s| s.split("</d:href>").next()).map(str::to_owned).collect()
    }

    #[tokio::test]
    async fn discovery_from_well_known_to_the_homes() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t.app.raw("GET", "/.well-known/carddav", "", &[]).await;
        assert!(response.status.is_redirection());
        assert_eq!(header(&response, "location").as_deref(), Some("/dav"));

        let response = t
            .call(
                "PROPFIND",
                "/dav/",
                r#"<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#,
                &[("depth", "0")],
            )
            .await;
        assert_status(&response, 207);
        assert_see(
            &response,
            &format!("<d:current-user-principal><d:href>/dav/principals/{email}/</d:href></d:current-user-principal>"),
        );

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/principals/{email}/"),
                r#"<d:propfind xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:prop><card:addressbook-home-set/><cal:calendar-home-set/><d:displayname/></d:prop></d:propfind>"#,
                &[("depth", "0")],
            )
            .await;
        assert_status(&response, 207);
        assert_see(
            &response,
            &format!(
                "<card:addressbook-home-set><d:href>/dav/addressbooks/{email}/</d:href></card:addressbook-home-set>"
            ),
        );
        assert_see(
            &response,
            &format!("<cal:calendar-home-set><d:href>/dav/calendars/{email}/</d:href></cal:calendar-home-set>"),
        );

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/addressbooks/{email}/"),
                r#"<d:propfind xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:prop><d:resourcetype/><d:displayname/><card:supported-address-data/></d:prop></d:propfind>"#,
                &[("depth", "1")],
            )
            .await;
        assert_status(&response, 207);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/><card:addressbook/></d:resourcetype><d:displayname>{}</d:displayname><card:supported-address-data><card:address-data-type content-type=\"text/vcard\" version=\"3.0\"/><card:address-data-type content-type=\"text/vcard\" version=\"4.0\"/><card:address-data-type content-type=\"application/vcard+json\" version=\"4.0\"/></card:supported-address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                vault.name
            ),
        );

        let response = t
            .call(
                "PROPFIND",
                &format!("/dav/calendars/{email}/"),
                r#"<d:propfind xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:prop><d:resourcetype/><cal:supported-calendar-component-set/></d:prop></d:propfind>"#,
                &[("depth", "1")],
            )
            .await;
        assert_status(&response, 207);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/calendars/{email}/dates-{vaultname}/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/><cal:calendar/></d:resourcetype><cal:supported-calendar-component-set><cal:comp name=\"VEVENT\"/></cal:supported-calendar-component-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
            ),
        );
        assert_see(&response, "<cal:comp name=\"VTODO\"/>");
        t.finish().await;
    }

    #[tokio::test]
    async fn options_and_propfind_of_a_card() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();
        let path = format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id);

        let response = t.call("OPTIONS", &path, "", &[]).await;
        assert_status(&response, 200);
        assert_eq!(
            header(&response, "dav").as_deref(),
            Some(
                "1, 3, extended-mkcol, addressbook, calendar-access, calendar-proxy, access-control, calendarserver-principal-property-search"
            )
        );
        assert_eq!(
            header(&response, "allow").as_deref(),
            Some("OPTIONS, GET, HEAD, DELETE, PROPFIND, PUT, PROPPATCH, COPY, MOVE, REPORT")
        );

        // allprop
        let response = t.call("PROPFIND", &path, "", &[("depth", "0")]).await;
        assert_status(&response, 207);
        let card = get_card(t.db(), &contact, true).await;
        let etag = sha256(&card);
        let updated = contacts::Entity::find_by_id(contact.id).one(t.db()).await.unwrap().unwrap().updated_at.unwrap();
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>{path}</d:href><d:propstat><d:prop><d:getlastmodified>{}</d:getlastmodified><d:getcontentlength>{}</d:getcontentlength><d:resourcetype/><d:getetag>&quot;{etag}&quot;</d:getetag><d:getcontenttype>text/vcard; charset=utf-8</d:getcontenttype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
                updated.0.format("%a, %d %b %Y %H:%M:%S GMT"),
                card.len()
            ),
        );

        // A missing card is a 404 error document.
        let response = t.call("PROPFIND", &format!("/dav/addressbooks/{email}/{vaultname}/nope.vcf"), "", &[]).await;
        assert_status(&response, 404);
        assert_see(
            &response,
            "<s:exception>Sabre\\DAV\\Exception\\NotFound</s:exception>\n  <s:message>Card not found</s:message>",
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn sync_collection_initial_then_after_a_change_and_a_delete() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let changed = contact_random(t.db(), &vault).await;
        let deleted = contact_random(t.db(), &vault).await;
        contacts_at(t.db(), &vault, at(2020, 1, 1, 9)).await;
        let email = t.user.email.clone();
        let collection = format!("/dav/addressbooks/{email}/{vaultname}/");

        // Initial sync: every card, and a token.
        let response = t.call("REPORT", &collection, &sync_collection(""), &[XML]).await;
        assert_status(&response, 207);
        let body1 = body(&response);
        assert!(body1.contains(&format!("{collection}{}.vcf", changed.id)));
        assert!(body1.contains(&format!("{collection}{}.vcf", deleted.id)));
        let first = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await.pop().unwrap();
        assert_see(&response, &format!("<d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>", first.id));

        // The token was taken a while ago; then a contact changes and another one is deleted.
        let mut am: sync_tokens::ActiveModel = first.clone().into();
        am.timestamp = Set(at(2021, 1, 1, 9));
        am.update(t.db()).await.unwrap();
        let mut am: contacts::ActiveModel = changed.clone().into();
        am.nickname = Set(Some("Changed".into()));
        am.update(t.db()).await.unwrap();
        let response = t.call("DELETE", &format!("{collection}{}.vcf", deleted.id), "", &[]).await;
        assert_status(&response, 204);

        let response = t
            .call("REPORT", &collection, &sync_collection(&format!("http://sabre.io/ns/sync/{}", first.id)), &[XML])
            .await;
        assert_status(&response, 207);
        let etag = card_etag(t.db(), &changed).await;
        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>{collection}{}.vcf</d:href>
  <d:propstat>
   <d:prop>
    <d:getetag>&quot;{etag}&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>",
                changed.id
            ),
        );
        assert_see(
            &response,
            &format!(
                "<d:response>
  <d:href>{collection}{}.vcf</d:href>
  <d:status>HTTP/1.1 404 Not Found</d:status>
 </d:response>",
                deleted.id
            ),
        );
        let last = tokens_named(t.db(), &t.user, &format!("contacts-{}", vault.id)).await.pop().unwrap();
        assert_ne!(last.id, first.id);
        assert_see(&response, &format!("<d:sync-token>http://sabre.io/ns/sync/{}</d:sync-token>", last.id));

        // An unknown token.
        let response = t.call("REPORT", &collection, &sync_collection("http://sabre.io/ns/sync/999999"), &[XML]).await;
        assert_status(&response, 403);
        assert_see(&response, "<d:valid-sync-token/>");
        t.finish().await;
    }

    #[tokio::test]
    async fn put_a_vcard_3_then_read_it_as_vcard_4() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let uuid = Uuid::new();
        let email = t.user.email.clone();
        let path = format!("/dav/addressbooks/{email}/{vaultname}/{uuid}.vcf");

        let card = format!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uuid}\r\nFN:Jane Roe\r\nN:Roe;Jane;;;\r\nTEL;TYPE=CELL,VOICE:+1 555 0100\r\nEND:VCARD\r\n"
        );
        let response = t.call("PUT", &path, &card, &[("if-none-match", "*"), ("content-type", "text/vcard")]).await;
        assert_status(&response, 201);

        let contact = contacts::Entity::find_by_id(uuid).one(t.db()).await.unwrap().expect("the contact");
        assert_eq!(contact.first_name.as_deref(), Some("Jane"));

        // A second create of the same uri: the If-None-Match fails.
        let response = t.call("PUT", &path, &card, &[("if-none-match", "*")]).await;
        assert_status(&response, 412);

        let response = t.call("GET", &path, "", &[("accept", "text/vcard; version=4.0")]).await;
        assert_status(&response, 200);
        assert_eq!(header(&response, "content-type").as_deref(), Some("text/vcard; version=4.0; charset=utf-8"));
        let card = body(&response);
        assert!(card.contains("VERSION:4.0\r\n"), "{card}");
        assert!(card.contains("TEL;TYPE=CELL,VOICE:+1 555 0100\r\n"), "{card}");

        // A client that asks for nothing gets vCard 3.
        let response = t.call("GET", &path, "", &[]).await;
        assert_eq!(header(&response, "content-type").as_deref(), Some("text/vcard; charset=utf-8"));
        assert!(body(&response).contains("VERSION:3.0\r\n"));
        t.finish().await;
    }

    #[tokio::test]
    async fn put_with_a_stale_if_match_and_invalid_data() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();
        let path = format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", contact.id);

        // The etag of the card, as a client reads it.
        let response = t.call("GET", &path, "", &[]).await;
        let etag = header(&response, "etag").expect("an etag");

        let response = t.call("PUT", &path, DOEX_WITH_UID, &[("if-match", "\"stale\"")]).await;
        assert_status(&response, 412);
        assert_eq!(header(&response, "etag"), Some(etag.clone()));
        assert_see(&response, "<s:header>If-Match</s:header>");

        let response = t.call("PUT", &path, DOEX_WITH_UID, &[("if-match", &etag)]).await;
        assert_status(&response, 204);

        // Not a vCard.
        let response = t.call("PUT", &path, "hello", &[]).await;
        assert_status(&response, 415);
        assert_see(&response, "<s:exception>Sabre\\DAV\\Exception\\UnsupportedMediaType</s:exception>");

        // A view-only vault can't be written.
        let other = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let path = format!("/dav/addressbooks/{email}/{}/{}.vcf", rawurlencode(&other.name), Uuid::new());
        let response = t.call("PUT", &path, DOEX_WITH_UID, &[]).await;
        assert_status(&response, 500);
        t.finish().await;
    }

    const DOEX_WITH_UID: &str = "BEGIN:VCARD\nVERSION:4.0\nUID:x\nFN:John Doex\nN:Doex;John;;;\nEND:VCARD";

    #[tokio::test]
    async fn groups_are_cards_converted_for_vcard_3_clients() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let group = group(t.db(), &vault, &[&contact]).await;
        let email = t.user.email.clone();
        let path = format!("/dav/addressbooks/{email}/{vaultname}/{}.vcf", group.uuid.unwrap());

        let response = t.call("GET", &path, "", &[("accept", "text/vcard; version=4.0")]).await;
        assert_status(&response, 200);
        let card = body(&response);
        assert!(card.contains("KIND:group\r\n"), "{card}");
        assert!(card.contains(&format!("MEMBER:{}\r\n", contact.id)), "{card}");

        let response = t.call("GET", &path, "", &[]).await;
        let card = body(&response);
        assert!(card.contains("VERSION:3.0\r\n"), "{card}");
        assert!(card.contains("X-ADDRESSBOOKSERVER-KIND:GROUP\r\n"), "{card}");
        assert!(card.contains(&format!("X-ADDRESSBOOKSERVER-MEMBER:{}\r\n", contact.id)), "{card}");

        let response = t.call("DELETE", &path, "", &[]).await;
        assert_status(&response, 204);
        let group = groups::Entity::find_by_id(group.id).one(t.db()).await.unwrap().unwrap();
        assert!(group.deleted_at.is_some());
        t.finish().await;
    }

    #[tokio::test]
    async fn exports() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let email = t.user.email.clone();
        let today = chrono::Utc::now().format("%Y-%m-%d");

        let response = t.call("GET", &format!("/dav/addressbooks/{email}/{vaultname}?export"), "", &[]).await;
        assert_status(&response, 200);
        assert_eq!(header(&response, "content-type").as_deref(), Some("text/directory; charset=UTF-8"));
        let filename: String = vault.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect();
        assert_eq!(
            header(&response, "content-disposition"),
            Some(format!("attachment; filename=\"{filename}-{today}.vcf\""))
        );
        let body = body(&response);
        assert_eq!(body.matches("BEGIN:VCARD\r\n").count(), 2);
        assert!(body.contains(&format!("UID:{}\r\n", contact.id)), "{body}");
        assert!(body.contains("BDAY:19811029\r\n"), "{body}");

        let response = t.call("GET", &format!("/dav/calendars/{email}/dates-{vaultname}?export"), "", &[]).await;
        assert_status(&response, 200);
        assert_eq!(header(&response, "content-type").as_deref(), Some("text/calendar; charset=UTF-8"));
        let body = super::body(&response);
        assert!(
            body.starts_with(
                "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nCALSCALE:GREGORIAN\r\nPRODID:-//SabreDAV//SabreDAV 4.7.0//EN\r\n"
            ),
            "{body}"
        );
        assert!(body.contains(&format!("X-WR-CALNAME:Contact important dates of {}\r\n", vault.name)), "{body}");
        assert!(body.contains(&format!("UID:{}\r\n", date.uuid.unwrap())), "{body}");
        assert_eq!(body.matches("BEGIN:VTIMEZONE").count(), 1);

        // Without `?export`, a collection redirects to the app.
        let response = t.call("GET", &format!("/dav/calendars/{email}/dates-{vaultname}"), "", &[]).await;
        assert_status(&response, 302);
        t.finish().await;
    }

    #[tokio::test]
    async fn calendar_query_with_a_time_range() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::View).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let date = important_date(t.db(), &contact, "birthdate").await;
        let email = t.user.email.clone();
        let query = |start: &str, end: &str| {
            format!(
                r#"<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/></d:prop><c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT"><c:time-range start="{start}" end="{end}"/></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#
            )
        };
        let path = format!("/dav/calendars/{email}/dates-{vaultname}/");
        let href = format!("{path}{}.ics", date.uuid.unwrap());

        // The yearly birthday of 1981-10-29 happens in October 2026…
        let response = t.call("REPORT", &path, &query("20261001T000000Z", "20261101T000000Z"), &[("depth", "1")]).await;
        assert_status(&response, 207);
        assert_eq!(hrefs(&body(&response)), vec![href.clone()]);

        // … not in January.
        let response = t.call("REPORT", &path, &query("20260101T000000Z", "20260201T000000Z"), &[("depth", "1")]).await;
        assert_status(&response, 207);
        assert!(hrefs(&body(&response)).is_empty());

        // A calendar-multiget of the date.
        let response = t
            .call(
                "REPORT",
                &path,
                &format!(
                    r#"<c:calendar-multiget xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><c:calendar-data/></d:prop><d:href>{href}</d:href></c:calendar-multiget>"#
                ),
                &[("depth", "1")],
            )
            .await;
        assert_status(&response, 207);
        let calendar = get_date(t.db(), &date).await.replace('\r', "");
        assert_see(&response, &format!("<cal:calendar-data>{calendar}</cal:calendar-data>"));

        // Depth 0 on a calendar is undefined.
        let response = t.call("REPORT", &path, &query("20261001T000000Z", "20261101T000000Z"), &[("depth", "0")]).await;
        assert_status(&response, 400);
        t.finish().await;
    }

    #[tokio::test]
    async fn mkcol_mkcalendar_copy_move_and_acl() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let contact = contact_random(t.db(), &vault).await;
        let email = t.user.email.clone();
        let book = format!("/dav/addressbooks/{email}/{vaultname}");
        let card = format!("{book}/{}.vcf", contact.id);

        // An address book: accepted, and the backend creates nothing.
        let mkcol = r#"<d:mkcol xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:set><d:prop><d:resourcetype><d:collection/><card:addressbook/></d:resourcetype><d:displayname>New</d:displayname></d:prop></d:set></d:mkcol>"#;
        let response = t.call("MKCOL", &format!("/dav/addressbooks/{email}/new/"), mkcol, &[XML]).await;
        assert_status(&response, 201);
        let response = t.call("PROPFIND", &format!("/dav/addressbooks/{email}/new/"), "", &[("depth", "0")]).await;
        assert_status(&response, 404);

        // A plain collection is not an address book.
        let response = t.call("MKCOL", &format!("/dav/addressbooks/{email}/plain/"), "", &[]).await;
        assert_status(&response, 403);
        assert_see(
            &response,
            "<s:exception>Sabre\\DAV\\Exception\\InvalidResourceType</s:exception>\n  <s:message>Unknown resourceType for this collection</s:message>\n  <d:valid-resourcetype/>\n",
        );

        // An existing node, a non-xml body, a collection in an address book.
        let response = t.call("MKCOL", &format!("{book}/"), "", &[]).await;
        assert_status(&response, 405);
        assert_see(&response, "<s:message>The resource you tried to create already exists</s:message>");
        let response =
            t.call("MKCOL", &format!("/dav/addressbooks/{email}/new/"), "x", &[("content-type", "text/plain")]).await;
        assert_status(&response, 415);
        let response = t.call("MKCOL", &format!("{book}/sub/"), "", &[]).await;
        assert_status(&response, 405);
        assert_see(&response, "<s:message>Creating collections in addressbooks is not allowed</s:message>");

        // A calendar.
        let response = t.call("MKCALENDAR", &format!("/dav/calendars/{email}/new/"), "", &[]).await;
        assert_status(&response, 201);
        let response =
            t.call("MKCALENDAR", &format!("/dav/calendars/{email}/new/"), r#"<d:mkcol xmlns:d="DAV:"/>"#, &[XML]).await;
        assert_status(&response, 400);
        assert_see(
            &response,
            "<s:message>Expected {urn:ietf:params:xml:ns:caldav}mkcalendar but received {DAV:}mkcol as the root element</s:message>",
        );

        // Cards and address books can't be renamed.
        let response = t.call("MOVE", &card, "", &[("destination", &format!("{book}/other.vcf"))]).await;
        assert_status(&response, 403);
        assert_see(&response, "<s:message>Permission denied to rename file</s:message>");
        let response = t
            .call("MOVE", &format!("{book}/"), "", &[("destination", &format!("/dav/addressbooks/{email}/renamed"))])
            .await;
        assert_status(&response, 405);
        assert_see(&response, "<s:message>Renaming addressbooks is not yet supported</s:message>");

        // COPY and MOVE preconditions.
        let response = t.call("COPY", &card, "", &[]).await;
        assert_status(&response, 400);
        assert_see(&response, "<s:message>The destination header was not supplied</s:message>");
        let response = t.call("COPY", &card, "", &[("destination", &card)]).await;
        assert_status(&response, 403);
        assert_see(&response, "<s:message>Source and destination uri are identical.</s:message>");
        let response = t.call("COPY", &card, "", &[("destination", &card), ("overwrite", "F")]).await;
        assert_status(&response, 412);
        let response = t.call("MOVE", &card, "", &[("destination", "/dav/nope/x.vcf")]).await;
        assert_status(&response, 409);
        assert_see(&response, "<s:message>The destination node is not found</s:message>");

        // ACL: the protected ace must stay, and then no node accepts a new ACL.
        let response = t.call("ACL", &format!("{book}/"), "", &[]).await;
        assert_status(&response, 400);
        assert_see(&response, "<s:message>XML body expected in ACL request</s:message>");
        let acl = |protected: &str, privilege: &str| {
            format!(
                r#"<d:acl xmlns:d="DAV:"><d:ace><d:principal><d:href>/dav/principals/{email}/</d:href></d:principal><d:grant><d:privilege><d:{privilege}/></d:privilege></d:grant>{protected}</d:ace></d:acl>"#
            )
        };
        // (an address book's protected ace is `{DAV:}owner`'s, which no request can name)
        let response = t.call("ACL", &format!("{book}/"), &acl("<d:protected/>", "all"), &[XML]).await;
        assert_status(&response, 409);
        assert_see(&response, "<d:no-ace-conflict/>");
        let response = t.call("ACL", &card, &acl("<d:protected/>", "read"), &[XML]).await;
        assert_status(&response, 409);
        let response = t.call("ACL", &card, &acl("<d:protected/>", "all"), &[XML]).await;
        assert_status(&response, 403);
        assert_see(&response, "<s:message>Setting ACL is not supported on this node</s:message>");
        t.finish().await;
    }

    #[tokio::test]
    async fn proppatch() {
        let mut t = Dav::new().await;
        let vault = vault_user(t.db(), &t.user, VaultPermission::Edit).await;
        let vaultname = rawurlencode(&vault.name);
        let email = t.user.email.clone();

        let response = t
            .call(
                "PROPPATCH",
                &format!("/dav/addressbooks/{email}/{vaultname}/"),
                r#"<d:propertyupdate xmlns:d="DAV:"><d:set><d:prop><d:displayname>Renamed</d:displayname><d:getetag>x</d:getetag></d:prop></d:set></d:propertyupdate>"#,
                &[XML],
            )
            .await;
        assert_status(&response, 207);
        assert_see(
            &response,
            &format!(
                "<d:response><d:href>/dav/addressbooks/{email}/{vaultname}</d:href><d:propstat><d:prop><d:getetag/><d:displayname/></d:prop><d:status>HTTP/1.1 403 Forbidden</d:status></d:propstat></d:response>"
            ),
        );

        // The principal accepts a group-member-set (and keeps nothing).
        let response = t
            .call(
                "PROPPATCH",
                &format!("/dav/principals/{email}/"),
                &format!(
                    r#"<d:propertyupdate xmlns:d="DAV:"><d:set><d:prop><d:group-member-set><d:href>/dav/principals/{email}/</d:href></d:group-member-set></d:prop></d:set></d:propertyupdate>"#
                ),
                &[XML],
            )
            .await;
        assert_status(&response, 207);
        assert_see(&response, "<d:prop><d:group-member-set/></d:prop><d:status>HTTP/1.1 200 OK</d:status>");

        // The principal collection is read-only.
        let response = t
            .call(
                "PROPPATCH",
                "/dav/principals/",
                r#"<d:propertyupdate xmlns:d="DAV:"><d:set><d:prop><d:displayname>x</d:displayname></d:prop></d:set></d:propertyupdate>"#,
                &[XML],
            )
            .await;
        assert_status(&response, 403);
        assert_see(
            &response,
            "  <d:need-privileges>\n    <d:resource>\n      <d:href>/dav/principals</d:href>\n      <d:privilege>\n        <d:write-properties/>\n      </d:privilege>\n    </d:resource>\n  </d:need-privileges>\n",
        );
        t.finish().await;
    }

    #[tokio::test]
    async fn authentication() {
        let mut t = Dav::new().await;
        let email = t.user.email.clone();

        // No credentials.
        let response = t.app.raw("PROPFIND", "/dav/", "", &[]).await;
        assert_status(&response, 401);
        assert_eq!(header(&response, "www-authenticate").as_deref(), Some("Basic"));

        // A token without `write`.
        let read_only = token(&t.app, &t.user, &["read"]).await;
        let auth = basic(&email, &read_only);
        let response = t.app.raw("PROPFIND", "/dav/", "", &[("authorization", &auth)]).await;
        assert_status(&response, 403);
        assert_eq!(response.code(), "token_ability_missing");

        // The email must be the user's.
        let auth = basic("someone@example.com", &t.token);
        let response = t.app.raw("PROPFIND", "/dav/", "", &[("authorization", &auth)]).await;
        assert_status(&response, 401);
        assert_eq!(header(&response, "www-authenticate").as_deref(), Some("Basic"));

        // The account password is not a token.
        let auth = basic(&email, monica::testing::factory::PASSWORD);
        let response = t.app.raw("PROPFIND", "/dav/", "", &[("authorization", &auth)]).await;
        assert_status(&response, 401);

        // A bearer token.
        let bearer = format!("Bearer {}", t.token);
        let response = t.app.raw("PROPFIND", "/dav/", "", &[("authorization", &bearer), ("depth", "0")]).await;
        assert_status(&response, 207);

        // The browser session, with its CSRF header on unsafe methods.
        t.app.login(&email).await;
        let principal = r#"<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;
        let response = t.app.raw("PROPFIND", "/dav/", principal, &[("depth", "0")]).await;
        assert_status(&response, 207);
        assert_see(&response, &format!("<d:href>/dav/principals/{email}/</d:href>"));
        t.app.remove_cookie("XSRF-TOKEN");
        let response = t.app.raw("PROPFIND", "/dav/", principal, &[("depth", "0")]).await;
        assert_status(&response, 403);
        assert_eq!(response.code(), "csrf_token_mismatch");
        t.finish().await;
    }
}
