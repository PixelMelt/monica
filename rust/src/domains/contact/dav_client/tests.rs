//! `tests/Unit/Domains/Contact/DavClient`. The fakes of the PHP tests
//! (`Http::fake()` through `Tests\Helpers\DavTester`) are a mock DAV server
//! on 127.0.0.1 that answers the same responses in the same order and
//! records the requests; its address replaces `https://test`. Mocked
//! services and the mocked `CardDAVBackend` are the real ones, on rows
//! that make them answer what the mocks answered. `Bus::fake()` tests look
//! at the jobs a step builds without running them.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::IntoResponse;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use sha2::{Digest, Sha256};

use super::dav::{DavClient, MultiStatus, PropFind, ServiceUrlQuery, SrvEntry, StatusList, Value};
use super::jobs::{
    DeleteLocalVCard, DeleteMultipleVCard, DeleteVCard, GetMultipleVCard, GetVCard, Job, MODE_MATCH_ETAG,
    MODE_MATCH_NONE, PushVCard, SynchronizeAddressBooks, UpdateAddressBooks, dispatch_batch,
};
use super::services::{CreateAddressBookSubscription, SynchronizeAddressBook, UpdateSubscriptionLocalSyncToken};
use super::utils::{
    AddressBookGetter, AddressBookSynchronizer, AddressData, Capabilities, ContactDto, LocalChanges,
    PrepareJobsContactPush, PrepareJobsContactPushMissed, PrepareJobsContactUpdater,
};
use super::{Exception, WAY_BOTH, get_client};
use crate::config::Config;
use crate::crypto::random_string;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::domains::contact::dav::VCardResource;
use crate::entities::{addressbook_subscriptions, contacts, genders, job_batches, sync_tokens, users, vaults};
use crate::testing::{TestDb, factory};

type DnsRecord = Box<dyn Fn(&str) -> Option<Vec<SrvEntry>> + Send + Sync>;

tokio::task_local! {
    /// `$tester->http->macro('getDnsRecord', …)`.
    pub(super) static GET_DNS_RECORD: DnsRecord;
}

const CONTACT_ID: &str = "affacde9-b2fe-4371-9acb-6612aaee6971";
const CONTACT_ID2: &str = "d403af1c-8492-4e9b-9833-cf18c795dfa9";
const CONTACTS: &str = "/dav/addressbooks/user@test.com/contacts/";

fn config() -> Config {
    Config::for_tests("sqlite::memory:", std::env::temp_dir())
}

fn time(value: &str) -> Timestamp {
    Timestamp(chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap())
}

// ---- Factories

/// `AddressBookSubscription::factory()->create($attributes)`.
async fn subscription_with(
    db: &TestDb,
    f: impl FnOnce(&mut addressbook_subscriptions::ActiveModel),
) -> addressbook_subscriptions::Model {
    let user = factory::user(&db.conn, None).await;
    let vault = factory::vault(&db.conn, user.account_id).await;
    let mut am = addressbook_subscriptions::ActiveModel {
        id: Set(Uuid::new()),
        user_id: Set(user.id),
        vault_id: Set(vault.id),
        uri: Set(format!("https://{}.example.com/", random_string(8).to_lowercase())),
        capabilities: Set(serde_json::json!({
            "addressbookMultiget": true,
            "addressbookQuery": true,
            "syncCollection": true,
            "addressData": {"content-type": "text/vcard", "version": "4.0"},
        })
        .to_string()),
        username: Set(format!("{}@example.com", random_string(8).to_lowercase())),
        password: Set(crate::crypto::encrypt(&config().key, "password")),
        distant_sync_token: Set(Some("\"test\"".to_owned())),
        active: Set(true),
        sync_way: Set(WAY_BOTH.into()),
        ..Default::default()
    };
    f(&mut am);
    am.insert(&db.conn).await.unwrap()
}

/// `AddressBookSubscription::factory()->create()`.
async fn subscription(db: &TestDb) -> addressbook_subscriptions::Model {
    subscription_with(db, |_| {}).await
}

async fn user_of(db: &TestDb, subscription: &addressbook_subscriptions::Model) -> users::Model {
    users::Entity::find_by_id(subscription.user_id).one(&db.conn).await.unwrap().unwrap()
}

async fn vault_of(db: &TestDb, subscription: &addressbook_subscriptions::Model) -> vaults::Model {
    vaults::Entity::find_by_id(subscription.vault_id).one(&db.conn).await.unwrap().unwrap()
}

async fn fresh(db: &TestDb, subscription: &addressbook_subscriptions::Model) -> addressbook_subscriptions::Model {
    addressbook_subscriptions::Entity::find_by_id(subscription.id).one(&db.conn).await.unwrap().unwrap()
}

/// `$this->setPermissionInVault($subscription->user, $permission, $subscription->vault)`.
async fn set_permission(db: &TestDb, subscription: &addressbook_subscriptions::Model, permission: VaultPermission) {
    let user = user_of(db, subscription).await;
    let vault = vault_of(db, subscription).await;
    factory::set_permission_in_vault(&db.conn, &user, permission, &vault).await;
}

/// `SyncToken::factory()->create([...])`.
async fn sync_token(
    db: &TestDb,
    subscription: &addressbook_subscriptions::Model,
    name: &str,
    timestamp: Timestamp,
) -> sync_tokens::Model {
    let user = user_of(db, subscription).await;
    sync_tokens::ActiveModel {
        account_id: Set(user.account_id),
        user_id: Set(user.id),
        name: Set(name.to_owned()),
        timestamp: Set(timestamp),
        created_at: Set(Some(timestamp)),
        updated_at: Set(Some(timestamp)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `$subscription->sync_token_id = $token->id; $subscription->save();`.
async fn set_sync_token(
    db: &TestDb,
    subscription: addressbook_subscriptions::Model,
    token: &sync_tokens::Model,
) -> addressbook_subscriptions::Model {
    let mut am: addressbook_subscriptions::ActiveModel = subscription.into();
    am.sync_token_id = Set(Some(token.id));
    am.update(&db.conn).await.unwrap()
}

/// `Contact::factory()->create(['vault_id' => …, 'id' => $id, …])`.
async fn contact_with(
    db: &TestDb,
    vault_id: Uuid,
    id: &str,
    f: impl FnOnce(&mut contacts::ActiveModel),
) -> contacts::Model {
    let vault = vaults::Entity::find_by_id(vault_id).one(&db.conn).await.unwrap().unwrap();
    let gender = factory::gender(&db.conn, vault.account_id).await;
    let mut am = contacts::ActiveModel {
        id: Set(id.parse().unwrap()),
        vault_id: Set(vault_id),
        gender_id: Set(Some(gender.id)),
        first_name: Set(Some("Regis".into())),
        last_name: Set(Some("Troyat".into())),
        can_be_deleted: Set(true),
        listed: Set(true),
        prefix: Set(Some("Dr.".into())),
        suffix: Set(Some("III".into())),
        show_quick_facts: Set(false),
        ..Default::default()
    };
    f(&mut am);
    am.insert(&db.conn).await.unwrap()
}

/// `Carbon::setTestNow(…)` before the contact was created.
async fn set_timestamps(db: &TestDb, contact: &contacts::Model, at: Timestamp) {
    let mut am: contacts::ActiveModel = contact.clone().into();
    am.created_at = Set(Some(at));
    am.updated_at = Set(Some(at));
    am.update(&db.conn).await.unwrap();
}

// ---- tests/Unit/Domains/Contact/DAV/CardEtag.php

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

/// `CardEtag::getCard($contact, $realFormat)`.
async fn get_card(db: &TestDb, contact: &contacts::Model, real_format: bool) -> String {
    let contact = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    let url = format!("{}/vaults/{}/contacts/{}", config().url, contact.vault_id, contact.id);
    let timestamp = contact.updated_at.unwrap().0.format("%Y%m%dT%H%M%SZ").to_string();
    let s = |v: &Option<String>| v.clone().unwrap_or_default();
    let (first, last) = (s(&contact.first_name), s(&contact.last_name));
    let separator = if !first.is_empty() && !last.is_empty() { " " } else { "" };
    let name = format!("{first}{separator}{last}");

    let mut data = append("BEGIN:VCARD", "");
    data = append("VERSION:4.0", &data);
    data = append(&format!("PRODID:-//Sabre//Sabre VObject {}//EN", crate::vobject::VERSION), &data);
    data = append(&format!("UID:{}", contact.id), &data);
    data = append(&format!("SOURCE:{url}"), &data);
    data = append(&format!("FN:{name}"), &data);
    data = append(&format!("N:{last};{first};{};;", s(&contact.middle_name)), &data);

    if let Some(nickname) = &contact.nickname {
        data = append(&format!("NICKNAME:{nickname}"), &data);
    }

    if let Some(gender_id) = contact.gender_id {
        let gender = genders::Entity::find_by_id(gender_id).one(&db.conn).await.unwrap().unwrap();
        data = append(&format!("GENDER:{}", s(&gender.r#type)), &data);
    }

    data = append(&format!("REV:{timestamp}"), &data);
    data = append("END:VCARD", &data);

    if real_format { data.replace('\n', "\r\n") } else { data }
}

/// `CardEtag::getEtag($contact, $quotes)`.
async fn get_etag(db: &TestDb, contact: &contacts::Model, quotes: bool) -> String {
    let etag = hex::encode(Sha256::digest(get_card(db, contact, true).await.as_bytes()));
    if quotes { format!("\"{etag}\"") } else { etag }
}

// ---- tests/Helpers/DavTester.php

#[derive(Debug, Clone)]
struct MockResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

/// `Http::response($body, $status, $headers)`.
fn response(body: &str, status: u16, headers: &[(&str, &str)]) -> MockResponse {
    MockResponse {
        status,
        headers: headers.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
        body: body.to_owned(),
    }
}

#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    uri: String,
    body: String,
    headers: Vec<(String, String)>,
}

#[derive(Default)]
struct Mock {
    responses: Mutex<VecDeque<MockResponse>>,
    recorded: Mutex<Vec<Recorded>>,
}

/// `Http::fake(fn ($request) => $this->responses[$this->current++]['response'])`.
async fn answer(State(mock): State<Arc<Mock>>, request: Request) -> axum::response::Response {
    let (parts, body) = request.into_parts();
    let host = parts.headers.get("host").and_then(|h| h.to_str().ok()).unwrap_or_default().to_owned();
    let uri = format!("http://{host}{}", parts.uri.path_and_query().map_or("/", |p| p.as_str()));
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
    mock.recorded.lock().unwrap().push(Recorded {
        method: parts.method.to_string(),
        uri,
        body: String::from_utf8_lossy(&body).into_owned(),
        headers: parts
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or_default().to_owned()))
            .collect(),
    });
    let response = mock.responses.lock().unwrap().pop_front();
    let Some(response) = response else {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut builder = axum::http::Response::builder().status(response.status);
    for (k, v) in response.headers {
        builder = builder.header(k, v);
    }
    builder.body(Body::from(response.body)).unwrap()
}

struct Expected {
    uri: String,
    method: String,
    body: Option<String>,
    headers: Option<Vec<(String, Vec<String>)>>,
}

/// `Tests\Helpers\DavTester`.
struct DavTester {
    base_uri: String,
    root: String,
    mock: Arc<Mock>,
    expected: Vec<Expected>,
}

impl DavTester {
    /// `new DavTester($baseUri)` and `->fake()`: `path` is the base uri on the mock server.
    async fn new(path: &str) -> DavTester {
        let mock = Arc::new(Mock::default());
        let root = serve(mock.clone()).await;
        DavTester { base_uri: format!("{root}{path}"), root, mock, expected: Vec::new() }
    }

    /// Another address of the same fake.
    async fn listen(&self) -> String {
        serve(self.mock.clone()).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.root)
    }

    fn client(&self) -> DavClient {
        let mut client = DavClient::new(&config());
        client.set_base_uri(&self.base_uri);
        client
    }

    fn add_response(
        mut self,
        uri: &str,
        response: MockResponse,
        body: Option<&str>,
        method: &str,
        headers: Option<Vec<(&str, Vec<String>)>>,
    ) -> Self {
        self.mock.responses.lock().unwrap().push_back(response);
        self.expected.push(Expected {
            uri: uri.to_owned(),
            method: method.to_owned(),
            body: body.map(str::to_owned),
            headers: headers.map(|h| h.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()),
        });
        self
    }

    /// `Http::assertSentInOrder(...)`.
    fn assert(&self) {
        let normalize = |u: &str| url::Url::parse(u).map_or_else(|_| u.to_owned(), |u| u.to_string());
        let recorded = self.mock.recorded.lock().unwrap().clone();
        assert_eq!(recorded.len(), self.expected.len(), "The expected requests do not match: {recorded:#?}");
        for (data, request) in self.expected.iter().zip(&recorded) {
            let srequest = format!("{} {}", request.method, request.uri);
            assert_eq!(data.method, request.method, "method for request {srequest} differs");
            assert_eq!(normalize(&data.uri), normalize(&request.uri), "uri for request {srequest} differs");
            if let Some(body) = &data.body {
                assert_eq!(body, &request.body, "body for request {srequest} differs");
            }
            if let Some(headers) = &data.headers {
                for (key, value) in headers {
                    let values: Vec<String> = request
                        .headers
                        .iter()
                        .filter(|(k, _)| k.eq_ignore_ascii_case(key))
                        .map(|(_, v)| v.clone())
                        .collect();
                    assert_eq!(value, &values, "header {key} for request {srequest} differs");
                }
            }
        }
    }

    fn address_book_base_uri(self) -> Self {
        let root = self.root.clone();
        let contacts = self.url(CONTACTS);
        self.user_principal(&root).addressbook_home().resource_type_address_book(None).options_ok(Some(&contacts))
    }

    fn capabilities(self) -> Self {
        self.supported_report_set().supported_address_data()
    }

    fn service_url(self) -> Self {
        let uri = self.url("/.well-known/carddav");
        let location = format!("{}/dav/", self.base_uri);
        self.add_response(&uri, response("", 301, &[("Location", &location)]), None, "GET", None)
    }

    fn non_standard_service_url(self) -> Self {
        let uri = self.url("/.well-known/carddav");
        self.add_response(&uri, response("", 301, &[("Location", "/dav/")]), None, "PROPFIND", None)
    }

    fn options_ok(self, url: Option<&str>) -> Self {
        let url = url.map_or_else(|| self.url("/dav/"), str::to_owned);
        self.add_response(&url, response("", 200, &[("Dav", "1, 3, addressbook")]), None, "OPTIONS", None)
    }

    fn options_fail(self) -> Self {
        let url = self.url("/dav/");
        self.add_response(&url, response("", 200, &[("Dav", "bad")]), None, "OPTIONS", None)
    }

    fn user_principal(self, url: &str) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/dav/principals/user@test.com/</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        self.add_response(url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn user_principal_empty(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/</d:href><d:propstat><d:prop><d:current-user-principal/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url("/dav/");
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn addressbook_home(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/principals/user@test.com/</d:href><d:propstat><d:prop><card:addressbook-home-set><d:href>/dav/addressbooks/user@test.com/</d:href></card:addressbook-home-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url("/dav/principals/user@test.com/");
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn addressbook_empty(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/principals/user@test.com/</d:href><d:propstat><d:prop><card:addressbook-home-set/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url("/dav/principals/user@test.com/");
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn resource_type_address_book(self, uri: Option<&str>) -> Self {
        let uri = uri.map_or_else(|| self.url("/dav/addressbooks/user@test.com/"), str::to_owned);
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/contacts/</d:href><d:propstat><d:prop><d:resourcetype><card:addressbook/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        self.add_response(&uri, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn resource_type_home_only(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/</d:href><d:propstat><d:prop><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url("/dav/addressbooks/user@test.com/");
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn supported_report_set(self) -> Self {
        let report_set = ["card:addressbook-multiget", "card:addressbook-query", "d:sync-collection"];
        let reports: String = report_set
            .iter()
            .map(|report| format!("<d:supported-report><d:report><{report}/></d:report></d:supported-report>"))
            .collect();
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/contacts/</d:href><d:propstat><d:prop><d:supported-report-set>{reports}</d:supported-report-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url(CONTACTS);
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn supported_address_data(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/contacts/</d:href><d:propstat><d:prop><card:supported-address-data><card:address-data-type content-type=\"text/vcard\" version=\"4.0\"/></card:supported-address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url(CONTACTS);
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn display_name(self) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/contacts/</d:href><d:propstat><d:prop><d:displayname>Test</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url(CONTACTS);
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn get_synctoken(self, synctoken: &str) -> Self {
        let body = format!(
            "{}<d:response><d:href>/dav/addressbooks/user@test.com/contacts/</d:href><d:propstat><d:prop><d:sync-token>{synctoken}</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
            multistatus_header()
        );
        let url = self.url(CONTACTS);
        self.add_response(&url, response(&body, 200, &[]), None, "PROPFIND", None)
    }

    fn get_sync_collection(self, synctoken: &str, etag: &str, uuid: &str) -> Self {
        let body = format!(
            "{}<d:response><d:href>{}{uuid}</d:href><d:propstat><d:prop><d:getetag>{etag}</d:getetag><d:getcontenttype>text/vcard</d:getcontenttype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response><d:sync-token>{synctoken}</d:sync-token></d:multistatus>",
            multistatus_header(),
            self.url(CONTACTS)
        );
        let url = self.url(CONTACTS);
        self.add_response(&url, response(&body, 200, &[]), None, "REPORT", None)
    }
}

/// `DavTester::multistatusHeader()`.
fn multistatus_header() -> &'static str {
    "<d:multistatus xmlns:d=\"DAV:\" xmlns:card=\"urn:ietf:params:xml:ns:carddav\">"
}

/// Starts the fake on a new port; its root URL.
async fn serve(mock: Arc<Mock>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(answer).with_state(mock);
    tokio::spawn(async move { axum::serve(listener, router).await });
    root
}

const ADDRESSBOOK_QUERY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<card:addressbook-query xmlns:card=\"urn:ietf:params:xml:ns:carddav\" xmlns:d=\"DAV:\"><d:prop><d:getetag/></d:prop></card:addressbook-query>\n";

fn multiget_body(href: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<card:addressbook-multiget xmlns:card=\"urn:ietf:params:xml:ns:carddav\" xmlns:d=\"DAV:\"><d:prop><d:getetag/><card:address-data content-type=\"text/vcard\" version=\"4.0\"/></d:prop><d:href>{href}</d:href></card:addressbook-multiget>\n"
    )
}

fn expected_capabilities() -> Capabilities {
    Capabilities {
        addressbook_multiget: true,
        addressbook_query: true,
        sync_collection: true,
        address_data: AddressData { content_type: Some("text/vcard".into()), version: Some("4.0".into()) },
    }
}

// ---- DeleteLocalVCardTest

#[tokio::test]
async fn it_delete_card_mock() {
    let db = TestDb::migrated().await;
    let address_book_subscription = subscription(&db).await;

    let pending_batch = vec![Job::DeleteLocalVCard(DeleteLocalVCard::new(
        address_book_subscription.clone(),
        "https://test/dav/uri".to_owned(),
    ))];

    assert_eq!(pending_batch.len(), 1);
    assert!(matches!(pending_batch.first(), Some(Job::DeleteLocalVCard(_))));

    let batch = dispatch_batch(&db.conn, &config(), pending_batch, None).await.unwrap();
    let batch = job_batches::Entity::find_by_id(batch).one(&db.conn).await.unwrap().unwrap();
    assert_eq!((batch.total_jobs, batch.pending_jobs, batch.failed_jobs), (1, 0, 0));
    db.drop().await;
}

#[tokio::test]
async fn it_delete_card_contact() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    set_permission(&db, &subscription, VaultPermission::Edit).await;

    let contact =
        contact_with(&db, subscription.vault_id, CONTACT_ID, |c| c.first_name = Set(Some("Test".into()))).await;

    DeleteLocalVCard::new(subscription, contact.id.to_string()).handle(&db.conn).await.unwrap();

    let contact = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert!(contact.deleted_at.is_some());
    db.drop().await;
}

// ---- DeleteMultipleVCardTest

#[tokio::test]
async fn delete_multiple_v_card_it_delete_cards() {
    let db = TestDb::migrated().await;
    let address_book_subscription = subscription(&db).await;

    let job = DeleteMultipleVCard::new(address_book_subscription, vec!["https://test/dav/uri".to_owned()]);

    let added = job.handle();

    assert_eq!(added.len(), 1);
    match &added[0] {
        Job::DeleteLocalVCard(update_v_card) => assert_eq!(update_v_card.uri, "https://test/dav/uri"),
        _ => panic!("expected a DeleteLocalVCard"),
    }
    db.drop().await;
}

// ---- DeleteVCardTest

#[tokio::test]
async fn delete_v_card_it_delete_card() {
    let db = TestDb::migrated().await;
    let address_book_subscription = subscription(&db).await;

    let tester = DavTester::new("").await;
    let uri = tester.url("/dav/uri");
    let tester = tester.add_response(&uri, response("", 204, &[]), None, "DELETE", None);

    let job = DeleteVCard::new(address_book_subscription, uri);
    job.handle(&config()).await.unwrap();

    tester.assert();
    db.drop().await;
}

// ---- GetMultipleVCardTest

async fn get_multiple_v_card_it_get_cards_test() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let base_uri = tester.base_uri.clone();
    let subscription = subscription_with(&db, |s| s.uri = Set(base_uri.clone())).await;
    set_permission(&db, &subscription, VaultPermission::Edit).await;

    let contact =
        contact_with(&db, subscription.vault_id, CONTACT_ID, |c| c.first_name = Set(Some("Test".into()))).await;
    let card = get_card(&db, &contact, false).await;
    let etag = get_etag(&db, &contact, true).await;

    let href = tester.url("/dav/uri");
    let body = format!(
        "{}<d:response><d:href>{href}</d:href><d:propstat><d:prop><card:address-data>{card}</card:address-data><d:getetag>{etag}</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let tester = tester.add_response(&base_uri, response(&body, 200, &[]), Some(&multiget_body(&href)), "REPORT", None);

    let job = GetMultipleVCard::new(subscription.clone(), vec![href.clone()]);
    let added = job.handle(&db.conn, &config()).await.unwrap();

    tester.assert();
    assert_eq!(added.len(), 1);
    let vault = vault_of(&db, &subscription).await;
    match &added[0] {
        Job::UpdateVCard(update_v_card) => {
            assert_eq!(update_v_card.account_id, vault.account_id);
            assert_eq!(update_v_card.author_id, subscription.user_id);
            assert_eq!(update_v_card.vault_id, subscription.vault_id);
            assert_eq!(update_v_card.uri, href);
            assert_eq!(update_v_card.etag.as_deref(), Some(etag.as_str()));
            assert_eq!(update_v_card.card, card);
            assert_eq!(update_v_card.external, Some(true));
        }
        _ => panic!("expected an UpdateVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn get_multiple_v_card_it_get_cards() {
    get_multiple_v_card_it_get_cards_test().await;
}

#[tokio::test]
async fn get_multiple_v_card_it_get_cards_mock_http() {
    get_multiple_v_card_it_get_cards_test().await;
}

// ---- GetVCardTest

#[tokio::test]
async fn get_v_card_it_get_card() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    set_permission(&db, &subscription, VaultPermission::Edit).await;

    let contact =
        contact_with(&db, subscription.vault_id, CONTACT_ID, |c| c.first_name = Set(Some("Test".into()))).await;
    let card = get_card(&db, &contact, false).await;
    let etag = get_etag(&db, &contact, true).await;

    let tester = DavTester::new("").await;
    let uri = tester.url("/dav/uri");
    let _tester = tester.add_response(&uri, response(&card, 200, &[]), None, "GET", None);

    let job = GetVCard::new(subscription.clone(), ContactDto::Contact { uri: uri.clone(), etag: Some(etag.clone()) });
    let added = job.handle(&db.conn, &config()).await.unwrap();

    assert_eq!(added.len(), 1);
    let vault = vault_of(&db, &subscription).await;
    match &added[0] {
        Job::UpdateVCard(update_v_card) => {
            assert_eq!(update_v_card.account_id, vault.account_id);
            assert_eq!(update_v_card.author_id, subscription.user_id);
            assert_eq!(update_v_card.vault_id, subscription.vault_id);
            assert_eq!(update_v_card.uri, uri);
            assert_eq!(update_v_card.etag.as_deref(), Some(etag.as_str()));
            assert_eq!(update_v_card.card, card);
            assert_eq!(update_v_card.external, Some(true));
        }
        _ => panic!("expected an UpdateVCard"),
    }
    db.drop().await;
}

// ---- PushVCardTest

#[tokio::test]
async fn push_v_card_it_create_dto_string() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    let dto = PushVCard::new(
        subscription,
        "uri".to_owned(),
        Some("etag".to_owned()),
        "card".to_owned(),
        "id".to_owned(),
        MODE_MATCH_NONE,
    );
    assert_eq!(dto.uri, "uri");
    assert_eq!(dto.etag.as_deref(), Some("etag"));
    assert_eq!(dto.contact_id, "id");
    assert_eq!(dto.card, "card");
    db.drop().await;
}

// it_create_dto_resource: not ported; the card is a string here, there is
// no stream resource to read it from.

#[tokio::test]
async fn push_v_card_it_push_card() {
    for (mode, ifmatch) in [(0, Vec::<&str>::new()), (1, vec!["etag"]), (2, vec!["*"])] {
        let db = TestDb::migrated().await;
        let tester = DavTester::new("/dav").await;
        let base_uri = tester.base_uri.clone();
        let subscription = subscription_with(&db, |s| s.uri = Set(base_uri)).await;

        let contact = contact_with(&db, subscription.vault_id, CONTACT_ID, |c| {
            c.first_name = Set(Some("John".into()));
            c.last_name = Set(Some("Doe".into()));
            c.updated_at = Set(Some(time("2021-09-01 00:00:00")));
        })
        .await;
        let card = get_card(&db, &contact, false).await;
        let etag = get_etag(&db, &contact, true).await;

        let ifmatch: Vec<String> =
            if ifmatch == ["etag"] { vec![etag.clone()] } else { ifmatch.into_iter().map(str::to_owned).collect() };

        let uri = tester.url("/dav/uri");
        let tester =
            tester.add_response(&uri, response(&card, 200, &[]), None, "PUT", Some(vec![("If-Match", ifmatch)]));

        let pending_batch =
            vec![Job::PushVCard(PushVCard::new(subscription, uri, Some(etag), card, contact.id.to_string(), mode))];
        assert_eq!(pending_batch.len(), 1);
        let Some(Job::PushVCard(job)) = pending_batch.into_iter().next() else { panic!("expected a PushVCard") };

        job.handle(&db.conn, &config()).await.unwrap();

        tester.assert();
        db.drop().await;
    }
}

// ---- SynchronizeAddressBooksTest

#[tokio::test]
async fn synchronize_address_books_it_get_card() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await.get_synctoken("\"test\"");
    let base_uri = tester.base_uri.clone();
    let subscription = subscription_with(&db, |s| {
        s.uri = Set(base_uri);
        s.last_synchronized_at = Set(None);
    })
    .await;

    let before = crate::db::now();
    SynchronizeAddressBooks { subscription: subscription.clone(), force: false }
        .handle(&db.conn, &config())
        .await
        .unwrap();

    tester.assert();
    let synchronized = fresh(&db, &subscription).await.last_synchronized_at.unwrap();
    assert!(synchronized >= before && synchronized <= crate::db::now());
    db.drop().await;
}

// ---- UpdateAddressBooksTest

/// A subscription whose synchronization fails at once (the fake has no answer).
async fn unanswered_subscription(
    db: &TestDb,
    tester: &DavTester,
    f: impl FnOnce(&mut addressbook_subscriptions::ActiveModel),
) -> addressbook_subscriptions::Model {
    let base_uri = tester.base_uri.clone();
    subscription_with(db, |s| {
        s.uri = Set(base_uri);
        f(s);
    })
    .await
}

#[tokio::test]
async fn update_address_books_it_dispatch_subscription_update() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = unanswered_subscription(&db, &tester, |_| {}).await;

    UpdateAddressBooks.handle(&db.conn, &config()).await.unwrap();

    let subscription = fresh(&db, &subscription).await;
    assert_eq!(subscription.current_logid.map(|l| l.0), Some(1));
    assert!(subscription.last_synchronized_at.is_some());
    db.drop().await;
}

#[tokio::test]
async fn update_address_books_it_does_not_dispatch_inactive_subscription() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = unanswered_subscription(&db, &tester, |s| s.active = Set(false)).await;

    UpdateAddressBooks.handle(&db.conn, &config()).await.unwrap();

    let subscription = fresh(&db, &subscription).await;
    assert_eq!(subscription.current_logid, None);
    assert_eq!(subscription.last_synchronized_at, None);
    db.drop().await;
}

#[tokio::test]
async fn update_address_books_it_dispatch_subscription_at_time_update() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = unanswered_subscription(&db, &tester, |s| {
        s.last_synchronized_at = Set(Some(time("2020-01-01 00:00:00")));
        s.frequency = Set(60);
    })
    .await;

    // `Carbon::setTestNow('2020-01-01 01:01:00')`: the `$now` of `handle()`.
    UpdateAddressBooks
        .manage_subscriptions(&db.conn, &config(), vec![subscription.clone()], time("2020-01-01 01:01:00"))
        .await;

    let subscription = fresh(&db, &subscription).await;
    assert_eq!(subscription.current_logid.map(|l| l.0), Some(1));
    assert_ne!(subscription.last_synchronized_at, Some(time("2020-01-01 00:00:00")));
    db.drop().await;
}

#[tokio::test]
async fn update_address_books_it_does_not_dispatch_subscription_at_time_update() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = unanswered_subscription(&db, &tester, |s| {
        s.last_synchronized_at = Set(Some(time("2020-01-01 00:00:00")));
        s.frequency = Set(60);
    })
    .await;

    // `Carbon::setTestNow('2020-01-01 01:00:00')`.
    UpdateAddressBooks
        .manage_subscriptions(&db.conn, &config(), vec![subscription.clone()], time("2020-01-01 01:00:00"))
        .await;

    let subscription = fresh(&db, &subscription).await;
    assert_eq!(subscription.current_logid, None);
    assert_eq!(subscription.last_synchronized_at, Some(time("2020-01-01 00:00:00")));
    db.drop().await;
}

// ---- AddAddressBookTest

#[tokio::test]
async fn add_address_book_it_creates_an_addressbook() {
    let db = TestDb::migrated().await;
    let user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
    let mut am: vaults::ActiveModel = vault.into();
    am.name = Set("contacts1".into());
    let vault = am.update(&db.conn).await.unwrap();

    let tester = DavTester::new("").await.address_book_base_uri().capabilities().display_name();

    let request = CreateAddressBookSubscription {
        account_id: user.account_id,
        vault_id: vault.id,
        author_id: user.id,
        base_uri: tester.base_uri.clone(),
        username: "test".into(),
        password: "test".into(),
    };

    let subscription = request.execute(&db.conn, &config()).await.unwrap();

    let row = fresh(&db, &subscription).await;
    assert_eq!(row.user_id, user.id);
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.username, "test");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row.capabilities).unwrap(),
        serde_json::json!({
            "addressbookMultiget": true,
            "addressbookQuery": true,
            "syncCollection": true,
            "addressData": {"content-type": "text/vcard", "version": "4.0"},
        })
    );

    assert_eq!(crate::crypto::decrypt(&config().key, &row.password).as_deref(), Some("test"));
    db.drop().await;
}

// ---- SynchronizeAddressBookTest

#[tokio::test]
async fn synchronize_address_book_it_runs_sync() {
    let db = TestDb::migrated().await;
    // `$force` is false: the sync-token is asked for.
    let tester = DavTester::new(CONTACTS).await.get_synctoken("\"test\"");
    let base_uri = tester.base_uri.clone();
    let subscription = subscription_with(&db, |s| s.uri = Set(base_uri)).await;

    let request = SynchronizeAddressBook {
        account_id: user_of(&db, &subscription).await.account_id,
        addressbook_subscription_id: subscription.id,
        force: None,
    };

    request.execute(&db.conn, &config()).await.unwrap();

    tester.assert();
    db.drop().await;
}

#[tokio::test]
async fn synchronize_address_book_it_runs_sync_force() {
    let db = TestDb::migrated().await;
    // `$force` is true: all the etags are asked for.
    let tester = DavTester::new(CONTACTS).await;
    let base_uri = tester.base_uri.clone();
    let body = format!("{}</d:multistatus>", multistatus_header());
    let tester = tester.add_response(&base_uri, response(&body, 200, &[]), Some(ADDRESSBOOK_QUERY), "REPORT", None);
    let subscription = subscription_with(&db, |s| s.uri = Set(base_uri)).await;

    let request = SynchronizeAddressBook {
        account_id: user_of(&db, &subscription).await.account_id,
        addressbook_subscription_id: subscription.id,
        force: Some(true),
    };

    request.execute(&db.conn, &config()).await.unwrap();

    tester.assert();
    db.drop().await;
}

// ---- UpdateSubscriptionLocalSyncTokenTest

#[tokio::test]
async fn update_subscription_local_sync_token_it_update_token() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    // the name `getCurrentSyncToken($subscription->vault_id)` looks for
    let name = format!("contacts-{}", subscription.vault_id);
    let token = sync_token(&db, &subscription, &name, Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;

    UpdateSubscriptionLocalSyncToken { addressbook_subscription_id: subscription.id }.execute(&db.conn).await.unwrap();

    let subscription = fresh(&db, &subscription).await;

    assert_eq!(subscription.sync_token_id, Some(token.id));
    db.drop().await;
}

#[tokio::test]
async fn update_subscription_local_sync_token_it_wont_update_null_token() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;

    UpdateSubscriptionLocalSyncToken { addressbook_subscription_id: subscription.id }.execute(&db.conn).await.unwrap();

    let subscription = fresh(&db, &subscription).await;

    assert_eq!(subscription.sync_token_id, None);
    db.drop().await;
}

// ---- AddressBookGetterTest

#[tokio::test]
async fn address_book_getter_it_get_address_book_data() {
    let tester = DavTester::new("").await.address_book_base_uri().capabilities().display_name();
    let client = tester.client();
    let result = AddressBookGetter::with_client(client).execute().await.unwrap();

    tester.assert();
    assert_eq!(result.uri, tester.url(CONTACTS));
    assert_eq!(result.capabilities, expected_capabilities());
    assert_eq!(result.name, Value::Text("Test".into()));
}

#[tokio::test]
async fn address_book_getter_it_get_address_book_data_direct() {
    let tester = DavTester::new(CONTACTS).await;
    let contacts = tester.url(CONTACTS);
    let tester =
        tester.resource_type_address_book(Some(&contacts)).options_ok(Some(&contacts)).capabilities().display_name();
    let client = tester.client();
    let result = AddressBookGetter::with_client(client).execute().await.unwrap();

    tester.assert();
    assert_eq!(result.uri, contacts);
    assert_eq!(result.capabilities, expected_capabilities());
    assert_eq!(result.name, Value::Text("Test".into()));
}

#[tokio::test]
async fn address_book_getter_it_fails_on_server_not_compliant() {
    let tester = DavTester::new("").await.user_principal_empty().service_url().options_fail();
    let client = tester.client();

    let result = AddressBookGetter::with_client(client).execute().await;
    assert!(matches!(result, Err(Exception::DavServerNotCompliant(_))));
}

#[tokio::test]
async fn address_book_getter_it_fails_if_no_userprincipal() {
    let tester = DavTester::new("").await.user_principal_empty().service_url().options_ok(None).user_principal_empty();
    let client = tester.client();

    let result = AddressBookGetter::with_client(client).execute().await;
    assert!(matches!(result, Err(Exception::DavServerNotCompliant(_))));
}

#[tokio::test]
async fn address_book_getter_it_fails_if_no_addressbook() {
    let tester = DavTester::new("").await.user_principal_empty().service_url().options_ok(None);
    let url = tester.url("/dav/");
    let tester = tester.user_principal(&url).addressbook_empty();
    let client = tester.client();

    let result = AddressBookGetter::with_client(client).execute().await;
    assert!(matches!(result, Err(Exception::DavServerNotCompliant(_))));
}

#[tokio::test]
async fn address_book_getter_it_fails_if_no_addressbook_url() {
    let tester = DavTester::new("").await.user_principal_empty().service_url().options_ok(None);
    let url = tester.url("/dav/");
    let tester = tester.user_principal(&url).addressbook_home().resource_type_home_only().options_ok(None);
    let client = tester.client();

    let result = AddressBookGetter::with_client(client).execute().await;
    // `DavServerNotCompliantException` is a `DavClientException`.
    assert!(matches!(result, Err(Exception::DavClient(_) | Exception::DavServerNotCompliant(_))));
}

// ---- AddressBookSynchronizerTest

/// `getSubscription()`.
async fn get_subscription(db: &TestDb, tester: &DavTester) -> addressbook_subscriptions::Model {
    // Carbon::setTestNow(2023-01-01 00:00:00)
    let uri = tester.url(CONTACTS);
    let subscription = subscription_with(db, |s| {
        s.uri = Set(uri);
        s.created_at = Set(Some(time("2023-01-01 00:00:00")));
        s.updated_at = Set(Some(time("2023-01-01 00:00:00")));
    })
    .await;
    set_permission(db, &subscription, VaultPermission::View).await;
    for contact in vault_contacts(db, &subscription).await {
        set_timestamps(db, &contact, time("2023-01-01 00:00:00")).await;
    }

    // Carbon::setTestNow(2023-01-03 00:00:00)
    let token =
        sync_token(db, &subscription, &format!("contacts-{}", subscription.vault_id), time("2023-01-03 00:00:00"))
            .await;

    let subscription = set_sync_token(db, subscription, &token).await;

    assert_eq!(fresh(db, &subscription).await.sync_token_id, Some(token.id));

    // Carbon::setTestNow(2023-01-04 00:00:00): what the tests create next is newer.
    subscription
}

async fn vault_contacts(db: &TestDb, subscription: &addressbook_subscriptions::Model) -> Vec<contacts::Model> {
    use sea_orm::{ColumnTrait, QueryFilter};
    contacts::Entity::find().filter(contacts::Column::VaultId.eq(subscription.vault_id)).all(&db.conn).await.unwrap()
}

/// `(new AddressBookSynchronizer)->withSubscription($subscription)` with its client.
async fn synchronizer(db: &TestDb, subscription: addressbook_subscriptions::Model) -> AddressBookSynchronizer {
    let mut synchronizer = AddressBookSynchronizer::with_subscription(&db.conn, &config(), subscription).await.unwrap();
    synchronizer.client = get_client(&config(), &synchronizer.subscription).unwrap();
    synchronizer
}

#[tokio::test]
async fn address_book_synchronizer_it_sync_empty_changes() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let tester = tester.get_synctoken(subscription.distant_sync_token.as_deref().unwrap());

    let jobs = synchronizer(&db, subscription).await.sync(&db.conn, &config()).await.unwrap();

    tester.assert();
    assert!(jobs.is_empty());
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_sync_no_changes() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let tester = tester.get_synctoken("\"test21\"").get_sync_collection("\"test20\"", "\"etag\"", CONTACT_ID2);

    synchronizer(&db, subscription).await.sync(&db.conn, &config()).await.unwrap();

    tester.assert();
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_sync_changes_added_local_contact() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let contact = contact_with(&db, subscription.vault_id, CONTACT_ID2, |_| {}).await;

    let tester =
        tester.get_synctoken("\"token\"").get_sync_collection("\"token\"", "\"test2\"", &contact.id.to_string());

    // what `PrepareJobsContactUpdater` and `PrepareJobsContactPush` receive
    let contacts = synchronizer(&db, subscription).await.get_distant_changes(&db.conn, &config()).await.unwrap();

    assert_eq!(contacts.first().map(ContactDto::uri), Some(format!("{}{}", tester.url(CONTACTS), contact.id).as_str()));
    assert_eq!(contacts.first().and_then(ContactDto::etag), Some("\"test2\""));

    tester.assert();
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_sync_changes_added_local_contact_batched() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let contact = contact_with(&db, subscription.vault_id, CONTACT_ID2, |_| {}).await;

    let tester =
        tester.get_synctoken("\"token\"").get_sync_collection("\"token\"", "\"test2\"", &contact.id.to_string());

    let jobs = synchronizer(&db, subscription).await.sync(&db.conn, &config()).await.unwrap();

    tester.assert();

    assert_eq!(jobs.len(), 2);
    match &jobs[0] {
        Job::GetMultipleVCard(job) => assert_eq!(job.hrefs, vec![format!("{}{}", tester.url(CONTACTS), contact.id)]),
        _ => panic!("expected a GetMultipleVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_sync_changes_deleted_contact_batched() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let contacts = tester.url(CONTACTS);
    let body = format!(
        "{}<d:response><d:status>HTTP/1.1 404 Not Found</d:status><d:href>{contacts}uuid</d:href><d:propstat><d:prop/><d:status>HTTP/1.1 418 I'm a teapot</d:status></d:propstat></d:response><d:sync-token>token</d:sync-token></d:multistatus>",
        multistatus_header()
    );
    let tester =
        tester.get_synctoken("\"token\"").add_response(&contacts, response(&body, 200, &[]), None, "REPORT", None);

    let jobs = synchronizer(&db, subscription).await.sync(&db.conn, &config()).await.unwrap();

    tester.assert();

    assert_eq!(jobs.len(), 1);
    match &jobs[0] {
        Job::DeleteMultipleVCard(job) => assert_eq!(job.hrefs, vec![format!("{contacts}uuid")]),
        _ => panic!("expected a DeleteMultipleVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_forcesync_changes_added_local_contact() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let contact = contact_with(&db, subscription.vault_id, CONTACT_ID2, |_| {}).await;
    let etag = get_etag(&db, &contact, true).await;

    let uuid1 = tester.url("/dav/uuid1");
    let body = format!(
        "{}<d:response><d:href>{uuid1}</d:href><d:propstat><d:prop><d:getetag>{etag}</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let contacts = tester.url(CONTACTS);
    let tester = tester.add_response(&contacts, response(&body, 200, &[]), Some(ADDRESSBOOK_QUERY), "REPORT", None);

    // what `PrepareJobsContactPushMissed` receives
    let synchronizer = synchronizer(&db, subscription.clone()).await;
    let dist_contacts = synchronizer.get_all_contacts_etag().await.unwrap();
    let user = user_of(&db, &subscription).await;
    let local_contacts = super::get_objects(&db.conn, &user, subscription.vault_id).await.unwrap();

    let local_ids: Vec<Uuid> = local_contacts
        .iter()
        .filter_map(|c| match c {
            VCardResource::Contact(c) => Some(c.id),
            VCardResource::Group(_) => None,
        })
        .collect();
    assert!(local_ids.contains(&contact.id));
    assert_eq!(dist_contacts.first().map(ContactDto::uri), Some(uuid1.as_str()));
    assert_eq!(dist_contacts.first().and_then(ContactDto::etag), Some(etag.as_str()));

    tester.assert();
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_forcesync_changes_added_local_contact_batched() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let contact1 = vault_contacts(&db, &subscription).await.into_iter().next().unwrap();
    let contact2 =
        contact_with(&db, subscription.vault_id, CONTACT_ID2, |c| c.first_name = Set(Some("Ryan".into()))).await;

    let uuid1 = tester.url("/dav/uuid1");
    let body = format!(
        "{}<d:response><d:href>{uuid1}</d:href><d:propstat><d:prop><d:getetag>{}</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response><d:response><d:href>{uuid1}</d:href><d:propstat><d:prop><d:getetag>{}</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header(),
        get_etag(&db, &contact1, true).await,
        get_etag(&db, &contact2, true).await
    );
    let contacts = tester.url(CONTACTS);
    let tester = tester.add_response(&contacts, response(&body, 200, &[]), Some(ADDRESSBOOK_QUERY), "REPORT", None);

    let jobs = synchronizer(&db, subscription).await.forcesync(&db.conn, &config()).await.unwrap();

    tester.assert();

    assert_eq!(jobs.len(), 3);

    match &jobs[0] {
        Job::GetMultipleVCard(job) => assert_eq!(job.hrefs, vec![uuid1.clone()]),
        _ => panic!("expected a GetMultipleVCard"),
    }

    match &jobs[1] {
        Job::PushVCard(job) => assert_eq!(job.contact_id, contact2.id.to_string()),
        _ => panic!("expected a PushVCard"),
    }

    match &jobs[2] {
        Job::PushVCard(job) => assert_eq!(job.contact_id, contact1.id.to_string()),
        _ => panic!("expected a PushVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn address_book_synchronizer_it_forcesync_changes_deleted_contact_batched() {
    let db = TestDb::migrated().await;
    let tester = DavTester::new(CONTACTS).await;
    let subscription = get_subscription(&db, &tester).await;

    let uuid1 = tester.url("/dav/uuid1");
    let body = format!(
        "{}<d:response><d:status>HTTP/1.1 404 Not Found</d:status><d:href>{uuid1}</d:href><d:propstat><d:prop/><d:status>HTTP/1.1 418 I'm a teapot</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let contacts = tester.url(CONTACTS);
    let tester = tester.add_response(&contacts, response(&body, 200, &[]), Some(ADDRESSBOOK_QUERY), "REPORT", None);

    let jobs = synchronizer(&db, subscription).await.forcesync(&db.conn, &config()).await.unwrap();

    tester.assert();

    assert_eq!(jobs.len(), 2);
    match &jobs[0] {
        Job::DeleteMultipleVCard(job) => assert_eq!(job.hrefs, vec![uuid1]),
        _ => panic!("expected a DeleteMultipleVCard"),
    }
    db.drop().await;
}

// ---- PrepareJobsContactPushTest

/// The subscription, its `contacts1` token and the contact the mocked
/// backend answered for `uricontact2` (here: its `distant_uuid`).
async fn push_fixture(db: &TestDb, distant_etag: bool) -> (addressbook_subscriptions::Model, contacts::Model, String) {
    let subscription = subscription(db).await;
    set_permission(db, &subscription, VaultPermission::View).await;
    let token =
        sync_token(db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(db, subscription, &token).await;

    let contact = contact_with(db, subscription.vault_id, CONTACT_ID, |c| {
        c.first_name = Set(Some("Test".into()));
        c.distant_uuid = Set(Some("uricontact2".into()));
    })
    .await;
    let etag = get_etag(db, &contact, true).await;
    if distant_etag {
        let mut am: contacts::ActiveModel = contact.clone().into();
        am.distant_etag = Set(Some(etag.clone()));
        am.update(&db.conn).await.unwrap();
    }

    (subscription, contact, etag)
}

#[tokio::test]
async fn prepare_jobs_contact_push_it_push_contacts_added() {
    let db = TestDb::migrated().await;
    let (subscription, _, etag) = push_fixture(&db, true).await;

    let batchs = PrepareJobsContactPush::with_subscription(&db.conn, subscription)
        .await
        .unwrap()
        .execute(
            &db.conn,
            &config(),
            &LocalChanges { added: vec!["uricontact2".into()], ..Default::default() },
            Some(&[ContactDto::Contact { uri: "https://test/dav/uricontact1".into(), etag: Some(etag) }]),
        )
        .await
        .unwrap();

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::PushVCard(batch) => {
            assert_eq!(batch.uri, "uricontact2");
            assert_eq!(batch.mode, MODE_MATCH_NONE);
        }
        _ => panic!("expected a PushVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn prepare_jobs_contact_push_it_push_contacts_modified() {
    let db = TestDb::migrated().await;
    let (subscription, _, etag) = push_fixture(&db, true).await;

    let batchs = PrepareJobsContactPush::with_subscription(&db.conn, subscription)
        .await
        .unwrap()
        .execute(
            &db.conn,
            &config(),
            &LocalChanges { modified: vec!["uricontact2".into()], ..Default::default() },
            Some(&[ContactDto::Contact { uri: "https://test/dav/uricontact1".into(), etag: Some(etag) }]),
        )
        .await
        .unwrap();

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::PushVCard(batch) => {
            assert_eq!(batch.uri, "uricontact2");
            assert_eq!(batch.mode, MODE_MATCH_ETAG);
        }
        _ => panic!("expected a PushVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn prepare_jobs_contact_push_it_delete_contacts_removed() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;

    let batchs = PrepareJobsContactPush::with_subscription(&db.conn, subscription)
        .await
        .unwrap()
        .execute(&db.conn, &config(), &LocalChanges { deleted: vec!["uricontact2".into()], ..Default::default() }, None)
        .await
        .unwrap();

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::DeleteVCard(batch) => assert_eq!(batch.uri, "uricontact2"),
        _ => panic!("expected a DeleteVCard"),
    }
    db.drop().await;
}

// ---- PrepareJobsContactPushMissedTest

#[tokio::test]
async fn prepare_jobs_contact_push_missed_it_push_contacts_missed() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    set_permission(&db, &subscription, VaultPermission::Manage).await;
    let token =
        sync_token(&db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(&db, subscription, &token).await;

    // the mocked `prepareCard()` answered the uri `uuid3`: the contact's `distant_uuid`
    let contact = contact_with(&db, subscription.vault_id, CONTACT_ID, |c| {
        c.first_name = Set(Some("Test".into()));
        c.distant_uuid = Set(Some("uuid3".into()));
    })
    .await;
    let etag = get_etag(&db, &contact, true).await;

    let batchs = PrepareJobsContactPushMissed::with_subscription(&db.conn, subscription)
        .await
        .unwrap()
        .execute(
            &db.conn,
            &config(),
            &LocalChanges::default(),
            &[ContactDto::Contact { uri: "uuid6".into(), etag: Some(etag) }],
            &[VCardResource::Contact(Box::new(contact))],
        )
        .await
        .unwrap();

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::PushVCard(batch) => {
            assert_eq!(batch.uri, "uuid3.vcf");
            assert_eq!(batch.mode, MODE_MATCH_NONE);
        }
        _ => panic!("expected a PushVCard"),
    }
    db.drop().await;
}

// ---- PrepareJobsContactUpdaterTest

#[tokio::test]
async fn prepare_jobs_contact_updater_it_sync_changes_multiget() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    let token =
        sync_token(&db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(&db, subscription, &token).await;

    let contact =
        contact_with(&db, subscription.vault_id, CONTACT_ID, |c| c.first_name = Set(Some("Test".into()))).await;
    let etag = get_etag(&db, &contact, true).await;

    let batchs = PrepareJobsContactUpdater::with_subscription(subscription)
        .execute(&[ContactDto::Contact { uri: "https://test/dav/uuid2".into(), etag: Some(etag) }]);

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::GetMultipleVCard(batch) => assert_eq!(batch.hrefs, vec!["https://test/dav/uuid2".to_owned()]),
        _ => panic!("expected a GetMultipleVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn prepare_jobs_contact_updater_it_sync_deleted_multiget() {
    let db = TestDb::migrated().await;
    let subscription = subscription(&db).await;
    let token =
        sync_token(&db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(&db, subscription, &token).await;

    let batchs = PrepareJobsContactUpdater::with_subscription(subscription)
        .execute(&[ContactDto::Delete { uri: "https://test/dav/uuid2".into() }]);

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::DeleteMultipleVCard(batch) => assert_eq!(batch.hrefs, vec!["https://test/dav/uuid2".to_owned()]),
        _ => panic!("expected a DeleteMultipleVCard"),
    }
    db.drop().await;
}

/// The factory capabilities without `addressbookMultiget`.
fn simple_capabilities() -> String {
    serde_json::json!({
        "addressbookMultiget": false,
        "addressbookQuery": true,
        "syncCollection": true,
        "addressData": {"content-type": "text/vcard", "version": "4.0"},
    })
    .to_string()
}

#[tokio::test]
async fn prepare_jobs_contact_updater_it_sync_changes_simple() {
    let db = TestDb::migrated().await;
    let subscription = subscription_with(&db, |s| s.capabilities = Set(simple_capabilities())).await;
    let token =
        sync_token(&db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(&db, subscription, &token).await;

    let contact =
        contact_with(&db, subscription.vault_id, CONTACT_ID, |c| c.first_name = Set(Some("Test".into()))).await;
    let etag = get_etag(&db, &contact, true).await;

    let batchs = PrepareJobsContactUpdater::with_subscription(subscription)
        .execute(&[ContactDto::Contact { uri: "https://test/dav/uuid2".into(), etag: Some(etag) }]);

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::GetVCard(batch) => {
            assert!(matches!(batch.contact, ContactDto::Contact { .. }));
            assert_eq!(batch.contact.uri(), "https://test/dav/uuid2");
        }
        _ => panic!("expected a GetVCard"),
    }
    db.drop().await;
}

#[tokio::test]
async fn prepare_jobs_contact_updater_it_sync_deleted_simple() {
    let db = TestDb::migrated().await;
    let subscription = subscription_with(&db, |s| s.capabilities = Set(simple_capabilities())).await;
    let token =
        sync_token(&db, &subscription, "contacts1", Timestamp(crate::db::now().0 - chrono::Duration::days(1))).await;
    let subscription = set_sync_token(&db, subscription, &token).await;

    let batchs = PrepareJobsContactUpdater::with_subscription(subscription)
        .execute(&[ContactDto::Delete { uri: "https://test/dav/uuid2".into() }]);

    assert_eq!(batchs.len(), 1);
    match &batchs[0] {
        Job::DeleteLocalVCard(batch) => assert_eq!(batch.uri, "https://test/dav/uuid2"),
        _ => panic!("expected a DeleteLocalVCard"),
    }
    db.drop().await;
}

// ---- DavClientTest

#[tokio::test]
async fn dav_client_it_get_options() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester
        .add_response(&base, response("", 200, &[]), None, "OPTIONS", None)
        .add_response(&base, response("", 200, &[("Dav", "test")]), None, "OPTIONS", None)
        .add_response(&base, response("", 200, &[("Dav", " test ")]), None, "OPTIONS", None);
    let client = tester.client();

    let result = client.options("").await.unwrap();
    assert_eq!(result, Vec::<String>::new());

    let result = client.options("").await.unwrap();
    assert_eq!(result, vec!["test".to_owned()]);

    let result = client.options("").await.unwrap();
    assert_eq!(result, vec!["test".to_owned()]);

    tester.assert();
}

#[tokio::test]
async fn dav_client_it_get_serviceurl() {
    let tester = DavTester::new("").await.service_url();
    let client = tester.client();

    let result = client.get_service_url().await.unwrap();

    tester.assert();
    assert_eq!(result, Some(format!("{}/dav/", tester.base_uri)));
}

#[tokio::test]
async fn dav_client_it_get_non_standard_serviceurl() {
    let tester = DavTester::new("").await;
    let well_known = tester.url("/.well-known/carddav");
    let tester = tester
        .add_response(&well_known, response("", 200, &[]), None, "GET", None)
        .add_response(&well_known, response("", 200, &[]), None, "GET", None)
        .non_standard_service_url();
    let client = tester.client();

    let result = client.get_service_url().await.unwrap();

    tester.assert();
    assert_eq!(result, Some(format!("{}/dav/", tester.base_uri)));
}

#[tokio::test]
async fn dav_client_it_get_serviceurl_name() {
    let tester = DavTester::new("").await;
    let well_known = tester.url("/.well-known/carddav");
    let port: u16 = tester.root.rsplit(':').next().unwrap().parse().unwrap();
    let root = tester.root.clone();
    let tester = tester
        .add_response(&well_known, response("", 200, &[]), None, "GET", None)
        .add_response(&well_known, response("", 200, &[]), None, "GET", None)
        .add_response(&well_known, response("", 200, &[]), None, "PROPFIND", None)
        // the connection test of the SRV target
        .add_response(&root, response("", 200, &[]), None, "GET", None);
    let client = tester.client();

    // `ServiceUrlQuery` is asked for `_carddavs._tcp` (https) first; a TLS
    // target can't be faked here, so that one has no record and the
    // `_carddav._tcp` (http) one points at the fake.
    let names = Arc::new(Mutex::new(Vec::new()));
    let seen = names.clone();
    let fake: DnsRecord = Box::new(move |hostname: &str| {
        seen.lock().unwrap().push(hostname.to_owned());
        hostname
            .starts_with("_carddav._tcp")
            .then(|| vec![SrvEntry { pri: 0, weight: 1, port, target: "127.0.0.1".into() }])
    });
    let result = GET_DNS_RECORD.scope(fake, client.get_service_url()).await.unwrap();

    tester.assert();
    assert_eq!(
        *names.lock().unwrap(),
        vec!["_carddavs._tcp.127.0.0.1".to_owned(), "_carddav._tcp.127.0.0.1".to_owned()]
    );
    assert_eq!(result, Some(root));
}

#[tokio::test]
async fn dav_client_it_get_non_standard_serviceurl2() {
    let tester = DavTester::new("").await;
    let well_known = tester.url("/.well-known/carddav");
    let tester = tester
        .add_response(&well_known, response("", 404, &[]), None, "GET", None)
        .add_response(&well_known, response("", 404, &[]), None, "GET", None)
        .non_standard_service_url();
    let client = tester.client();

    let result = client.get_service_url().await.unwrap();

    tester.assert();
    assert_eq!(result, Some(format!("{}/dav/", tester.base_uri)));
}

#[tokio::test]
async fn dav_client_it_fail_non_standard() {
    let tester = DavTester::new("").await;
    let well_known = tester.url("/.well-known/carddav");
    let tester = tester.add_response(&well_known, response("", 500, &[]), None, "GET", None);
    let client = tester.client();

    let result = client.get_service_url().await;
    assert!(matches!(result, Err(Exception::Request(_))));
}

#[tokio::test]
async fn dav_client_it_get_base_uri() {
    let mut client = DavClient::new(&config());
    client.set_base_uri("https://test");

    let result = client.path(None);

    assert_eq!(result, "https://test");

    let result = client.path(Some("xxx"));

    assert_eq!(result, "https://test/xxx");
}

#[tokio::test]
async fn dav_client_it_set_base_uri() {
    let mut client = DavClient::new(&config());
    client.set_base_uri("https://test");

    let result = client.set_base_uri("https://new").path(None);

    assert_eq!(result, "https://new");
}

fn test_value_response() -> MockResponse {
    let body = format!(
        "{}<d:response><d:href>href</d:href><d:propstat><d:prop><d:test>value</d:test></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    response(&body, 200, &[])
}

const PROPFIND_TEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<d:propfind xmlns:d=\"DAV:\"><d:prop><d:test/></d:prop></d:propfind>\n";

#[tokio::test]
async fn dav_client_it_call_propfind() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester.add_response(&base, test_value_response(), Some(PROPFIND_TEST), "PROPFIND", None);

    let client = tester.client();

    let result = client.prop_find(&["{DAV:}test".into()], 0, "").await.unwrap();

    tester.assert();
    assert_eq!(result, PropFind::Properties(vec![("{DAV:}test".into(), Value::Text("value".into()))]));
}

#[tokio::test]
async fn dav_client_it_get_property() {
    let tester = DavTester::new("").await;
    let url = tester.url("/test");
    let tester = tester.add_response(&url, test_value_response(), Some(PROPFIND_TEST), "PROPFIND", None);

    let client = tester.client();

    let result = client.get_property("{DAV:}test", &url).await.unwrap();

    tester.assert();
    assert_eq!(result, Value::Text("value".into()));
}

#[tokio::test]
async fn dav_client_it_get_supported_report() {
    let tester = DavTester::new("/dav").await;
    let base = tester.base_uri.clone();
    let body = format!(
        "{}<d:response><d:href>/dav</d:href><d:propstat><d:prop><d:supported-report-set><d:supported-report><d:report><d:test1/></d:report></d:supported-report><d:supported-report><d:report><d:test2/></d:report></d:supported-report></d:supported-report-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let tester = tester.add_response(
        &base,
        response(&body, 200, &[]),
        Some("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<d:propfind xmlns:d=\"DAV:\"><d:prop><d:supported-report-set/></d:prop></d:propfind>\n"),
        "PROPFIND",
        None,
    );

    let client = tester.client();

    let result = client.get_supported_report_set().await.unwrap();

    tester.assert();
    assert_eq!(result, vec![Some("{DAV:}test1".to_owned()), Some("{DAV:}test2".to_owned())]);
}

fn sync_collection_response() -> MockResponse {
    let body = format!(
        "{}<d:response><d:href>href</d:href><d:propstat><d:prop><d:getetag>\"00001-abcd1\"</d:getetag><d:test>value</d:test></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response><d:sync-token>\"00001-abcd1\"</d:sync-token></d:multistatus>",
        multistatus_header()
    );
    response(&body, 200, &[])
}

fn sync_collection_result() -> MultiStatus {
    MultiStatus {
        responses: vec![(
            "href".into(),
            StatusList {
                properties: vec![(
                    "200".into(),
                    vec![
                        ("{DAV:}getetag".into(), Value::Text("\"00001-abcd1\"".into())),
                        ("{DAV:}test".into(), Value::Text("value".into())),
                    ],
                )],
                status: "200".into(),
            },
        )],
        synctoken: Some("\"00001-abcd1\"".into()),
    }
}

#[tokio::test]
async fn dav_client_it_sync_collection() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester.add_response(
        &base,
        sync_collection_response(),
        Some("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<d:sync-collection xmlns:d=\"DAV:\"><d:sync-token/><d:sync-level>1</d:sync-level><d:prop><d:test/></d:prop></d:sync-collection>\n"),
        "REPORT",
        None,
    );

    let client = tester.client();

    let result = client.sync_collection(&["{DAV:}test".into()], "").await.unwrap();

    tester.assert();
    assert_eq!(result, sync_collection_result());
}

#[tokio::test]
async fn dav_client_it_sync_collection_with_synctoken() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester.add_response(
        &base,
        sync_collection_response(),
        Some("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<d:sync-collection xmlns:d=\"DAV:\"><d:sync-token>\"00000-abcd0\"</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:test/></d:prop></d:sync-collection>\n"),
        "REPORT",
        None,
    );

    let client = tester.client();

    let result = client.sync_collection(&["{DAV:}test".into()], "\"00000-abcd0\"").await.unwrap();

    tester.assert();
    assert_eq!(result, sync_collection_result());
}

fn etag_value_response() -> MockResponse {
    let body = format!(
        "{}<d:response><d:href>href</d:href><d:propstat><d:prop><d:getetag>\"00001-abcd1\"</d:getetag><d:test>value</d:test></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    response(&body, 200, &[])
}

fn etag_value_result() -> MultiStatus {
    MultiStatus { synctoken: None, ..sync_collection_result() }
}

#[tokio::test]
async fn dav_client_it_run_addressbook_multiget_report() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester.add_response(
        &base,
        etag_value_response(),
        Some("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<card:addressbook-multiget xmlns:card=\"urn:ietf:params:xml:ns:carddav\" xmlns:d=\"DAV:\"><d:prop><d:test/></d:prop><d:href>https://test/contacts/1</d:href></card:addressbook-multiget>\n"),
        "REPORT",
        None,
    );

    let client = tester.client();

    let result =
        client.addressbook_multiget(&["{DAV:}test".into()], &["https://test/contacts/1".to_owned()]).await.unwrap();

    assert_eq!(result, etag_value_result());

    tester.assert();
}

#[tokio::test]
async fn dav_client_it_run_addressbook_query_report() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let tester = tester.add_response(
        &base,
        etag_value_response(),
        Some("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<card:addressbook-query xmlns:card=\"urn:ietf:params:xml:ns:carddav\" xmlns:d=\"DAV:\"><d:prop><d:test/></d:prop></card:addressbook-query>\n"),
        "REPORT",
        None,
    );

    let client = tester.client();

    let result = client.addressbook_query(&["{DAV:}test".into()]).await.unwrap();

    tester.assert();
    assert_eq!(result, etag_value_result());
}

#[tokio::test]
async fn dav_client_it_run_proppatch() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let body = format!(
        "{}<d:response><d:href>href</d:href><d:propstat><d:prop><d:test>value</d:test></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let tester = tester.add_response(
        &base,
        response(&body, 207, &[]),
        Some("<?xml version=\"1.0\"?>\n<d:propertyupdate xmlns:d=\"DAV:\" xmlns:s=\"http://sabredav.org/ns\">\n <d:set>\n  <d:prop>\n   <d:test>value</d:test>\n  </d:prop>\n </d:set>\n</d:propertyupdate>\n"),
        "PROPPATCH",
        None,
    );

    let client = tester.client();

    let result = client.prop_patch(&[("{DAV:}test", Some("value"))]).await.unwrap();

    tester.assert();
    assert!(result);
}

#[tokio::test]
async fn dav_client_it_run_proppatch_error() {
    let tester = DavTester::new("").await;
    let base = tester.base_uri.clone();
    let body = format!(
        "{}<d:response><d:href>href</d:href><d:propstat><d:prop><d:test>x</d:test></d:prop><d:status>HTTP/1.1 405 OK</d:status></d:propstat><d:propstat><d:prop><d:excerpt>x</d:excerpt></d:prop><d:status>HTTP/1.1 500 OK</d:status></d:propstat></d:response></d:multistatus>",
        multistatus_header()
    );
    let tester = tester.add_response(&base, response(&body, 207, &[]), None, "PROPPATCH", None);

    let client = tester.client();

    let result = client.prop_patch(&[("{DAV:}test", Some("value")), ("{DAV:}excerpt", Some("value"))]).await;
    match result {
        Err(e @ Exception::DavClient(_)) => assert_eq!(
            e.message(),
            "PROPPATCH failed. The following properties errored: {DAV:}test (405), {DAV:}excerpt (500)"
        ),
        _ => panic!("expected a DavClientException"),
    }
}

// ---- ServiceUrlQueryTest

/// `$tester->http->macro('getDnsRecord', …)` answering `entries` for `srv.test.com`.
fn dns(entries: Option<Vec<SrvEntry>>) -> DnsRecord {
    Box::new(move |hostname: &str| {
        assert_eq!(hostname, "srv.test.com");
        entries.clone()
    })
}

fn port_of(root: &str) -> u16 {
    root.rsplit(':').next().unwrap().parse().unwrap()
}

#[tokio::test]
async fn service_url_query_it_get_service_url() {
    let tester = DavTester::new("").await;
    let carddav = tester.root.clone();
    let tester = tester.add_response(&carddav, response("", 200, &[]), None, "GET", None);

    let entries = vec![SrvEntry { pri: 0, weight: 1, port: port_of(&carddav), target: "127.0.0.1".into() }];
    let client = tester.client();
    GET_DNS_RECORD
        .scope(dns(Some(entries)), ServiceUrlQuery::with_client(&client).execute("srv", false, "https://test.com"))
        .await
        .unwrap();

    tester.assert();
}

#[tokio::test]
async fn service_url_query_it_get_service_url_by_weight() {
    let tester = DavTester::new("").await;
    let bad = tester.listen().await;
    let carddav = tester.root.clone();
    let tester = tester.add_response(&bad, response("", 404, &[]), None, "GET", None).add_response(
        &carddav,
        response("", 200, &[]),
        None,
        "GET",
        None,
    );

    let entries = vec![
        SrvEntry { pri: 0, weight: 10, port: port_of(&bad), target: "127.0.0.1".into() },
        SrvEntry { pri: 0, weight: 1, port: port_of(&carddav), target: "127.0.0.1".into() },
    ];
    let client = tester.client();
    GET_DNS_RECORD
        .scope(dns(Some(entries)), ServiceUrlQuery::with_client(&client).execute("srv", false, "https://test.com"))
        .await
        .unwrap();

    tester.assert();
}

#[tokio::test]
async fn service_url_query_it_get_service_url_by_pri() {
    let tester = DavTester::new("").await;
    let bad = tester.listen().await;
    let carddav = tester.root.clone();
    let tester = tester.add_response(&bad, response("", 404, &[]), None, "GET", None).add_response(
        &carddav,
        response("", 200, &[]),
        None,
        "GET",
        None,
    );

    let entries = vec![
        SrvEntry { pri: 0, weight: 1, port: port_of(&bad), target: "127.0.0.1".into() },
        SrvEntry { pri: 1, weight: 1, port: port_of(&carddav), target: "127.0.0.1".into() },
    ];
    let client = tester.client();
    GET_DNS_RECORD
        .scope(dns(Some(entries)), ServiceUrlQuery::with_client(&client).execute("srv", false, "https://test.com"))
        .await
        .unwrap();

    tester.assert();
}

#[tokio::test]
async fn service_url_query_it_get_service_null() {
    let tester = DavTester::new("").await;

    let client = tester.client();
    GET_DNS_RECORD
        .scope(dns(None), ServiceUrlQuery::with_client(&client).execute("srv", false, "https://test.com"))
        .await
        .unwrap();

    tester.assert();
}
