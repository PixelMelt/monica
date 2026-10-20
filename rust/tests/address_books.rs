//! The vCard download and the address book subscriptions over HTTP, with a
//! remote CardDAV server on 127.0.0.1.

mod common;

use axum::Router;
use axum::extract::{Request, State};
use axum::response::IntoResponse;
use common::TestApp;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::entities::{addressbook_subscriptions, users, vaults};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::{Value, json};

const CONTACTS: &str = "/dav/addressbooks/user@test.com/contacts/";

fn multistatus(inner: &str) -> String {
    format!("<d:multistatus xmlns:d=\"DAV:\" xmlns:card=\"urn:ietf:params:xml:ns:carddav\">{inner}</d:multistatus>")
}

fn prop(href: &str, prop: &str) -> String {
    multistatus(&format!(
        "<d:response><d:href>{href}</d:href><d:propstat><d:prop>{prop}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
    ))
}

/// The remote server: answers the discovery and the syncs of an address
/// book at `CONTACTS`; with `broken`, every request fails.
async fn remote(State(broken): State<bool>, request: Request) -> axum::response::Response {
    let (parts, body) = request.into_parts();
    let body = String::from_utf8_lossy(&axum::body::to_bytes(body, usize::MAX).await.unwrap()).into_owned();
    if broken {
        let status = if parts.uri.path() == "/.well-known/carddav" { 500 } else { 404 };
        return axum::http::StatusCode::from_u16(status).unwrap().into_response();
    }
    let xml = |s: String| ([("content-type", "application/xml")], s).into_response();
    match (parts.method.as_str(), parts.uri.path()) {
        ("OPTIONS", _) => ([("Dav", "1, 3, addressbook")], "").into_response(),
        ("PROPFIND", "/") => xml(prop(
            "/",
            "<d:current-user-principal><d:href>/dav/principals/user@test.com/</d:href></d:current-user-principal>",
        )),
        ("PROPFIND", "/dav/principals/user@test.com/") => xml(prop(
            "/dav/principals/user@test.com/",
            "<card:addressbook-home-set><d:href>/dav/addressbooks/user@test.com/</d:href></card:addressbook-home-set>",
        )),
        ("PROPFIND", "/dav/addressbooks/user@test.com/") => {
            xml(prop(CONTACTS, "<d:resourcetype><d:collection/><card:addressbook/></d:resourcetype>"))
        }
        ("PROPFIND", CONTACTS) if body.contains("supported-report-set") => xml(prop(
            CONTACTS,
            "<d:supported-report-set><d:supported-report><d:report><card:addressbook-multiget/></d:report></d:supported-report><d:supported-report><d:report><card:addressbook-query/></d:report></d:supported-report><d:supported-report><d:report><d:sync-collection/></d:report></d:supported-report></d:supported-report-set>",
        )),
        ("PROPFIND", CONTACTS) if body.contains("supported-address-data") => xml(prop(
            CONTACTS,
            "<card:supported-address-data><card:address-data-type content-type=\"text/vcard\" version=\"3.0\"/><card:address-data-type content-type=\"text/vcard\" version=\"4.0\"/></card:supported-address-data>",
        )),
        ("PROPFIND", CONTACTS) if body.contains("displayname") => {
            xml(prop(CONTACTS, "<d:displayname>Contacts</d:displayname>"))
        }
        ("PROPFIND", CONTACTS) if body.contains("sync-token") => {
            xml(prop(CONTACTS, "<d:sync-token>\"test\"</d:sync-token>"))
        }
        ("REPORT", CONTACTS) => xml(multistatus("<d:sync-token>\"test\"</d:sync-token>")),
        _ => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Starts the remote server; its root URL.
async fn remote_server(broken: bool) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(remote).with_state(broken);
    tokio::spawn(async move { axum::serve(listener, router).await });
    root
}

/// A manager logged in, with a vault created through the API.
async fn setup() -> (TestApp, users::Model, vaults::Model, String) {
    let mut app = TestApp::new().await;
    let manager = factory::user(&app.state.db, None).await;
    app.login(&manager.username).await;
    let vault = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    let id: Uuid = vault["id"].as_str().unwrap().parse().unwrap();
    let vault = vaults::Entity::find_by_id(id).one(&app.state.db).await.unwrap().unwrap();
    let base = format!("/vaults/{}", vault.id);
    (app, manager, vault, base)
}

/// Another user of the manager's account with `p` in the vault.
async fn member(app: &TestApp, manager: &users::Model, vault: &vaults::Model, p: VaultPermission) -> users::Model {
    let u = factory::user(&app.state.db, Some(manager.account_id)).await;
    factory::set_permission_in_vault(&app.state.db, &u, p, vault).await;
    u
}

/// A subscription of the vault, as `CreateAddressBookSubscription` stores it.
async fn subscription(
    app: &TestApp,
    user: &users::Model,
    vault: &vaults::Model,
    uri: &str,
) -> addressbook_subscriptions::Model {
    addressbook_subscriptions::ActiveModel {
        id: Set(Uuid::new()),
        user_id: Set(user.id),
        vault_id: Set(vault.id),
        uri: Set(uri.to_owned()),
        username: Set("me@example.com".into()),
        password: Set(monica::crypto::encrypt(&app.state.config.key, "secret")),
        capabilities: Set(json!({
            "addressbookMultiget": true,
            "addressbookQuery": true,
            "syncCollection": true,
            "addressData": {"content-type": "text/vcard", "version": "4.0"},
        })
        .to_string()),
        distant_sync_token: Set(Some("\"test\"".into())),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap()
}

async fn row(app: &TestApp, id: Uuid) -> Option<addressbook_subscriptions::Model> {
    addressbook_subscriptions::Entity::find_by_id(id).one(&app.state.db).await.unwrap()
}

fn error(res: &Value, i: usize) -> (&str, &str) {
    (res["errors"][i]["field"].as_str().unwrap(), res["errors"][i]["code"].as_str().unwrap())
}

#[tokio::test]
async fn downloads_a_contact_as_a_vcard() {
    let (mut app, manager, vault, base) = setup().await;
    let contact = factory::contact(&app.state.db, &vault).await;

    let res = app.get(&format!("{base}/contacts/{}/vcard", contact.id)).await.assert(200);
    assert_eq!(res.headers["content-type"], "text/vcard");
    assert_eq!(res.headers["content-disposition"], "attachment; filename=\"dr-regis-troyat-iii.vcf\"");
    let card = String::from_utf8(res.body.clone()).unwrap();
    assert!(card.starts_with("BEGIN:VCARD\r\nVERSION:4.0\r\n"), "{card}");
    assert!(card.contains(&format!("UID:{}\r\n", contact.id)));
    assert!(card.contains("FN:Dr. Regis Troyat III\r\n"));

    // the export is cached on the contact
    let cached = monica::entities::contacts::Entity::find_by_id(contact.id).one(&app.state.db).await.unwrap().unwrap();
    assert_eq!(cached.vcard.as_deref(), Some(card.as_str()));
    assert_eq!(cached.updated_at, contact.updated_at);

    app.get(&format!("{base}/contacts/{}/vcard", Uuid::new())).await.assert(404);
    let other = app.post("/vaults", json!({"name": "Work"})).await.assert(201).json();
    let other: Uuid = other["id"].as_str().unwrap().parse().unwrap();
    app.get(&format!("/vaults/{other}/contacts/{}/vcard", contact.id)).await.assert(404);

    let viewer = member(&app, &manager, &vault, VaultPermission::View).await;
    app.login(&viewer.username).await;
    app.get(&format!("{base}/contacts/{}/vcard", contact.id)).await.assert(200);

    let stranger = factory::user(&app.state.db, None).await;
    app.login(&stranger.username).await;
    app.get(&format!("{base}/contacts/{}/vcard", contact.id)).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn creates_a_subscription_and_runs_the_first_sync() {
    let (mut app, manager, vault, base) = setup().await;
    let remote = remote_server(false).await;
    let path = format!("{base}/address-book-subscriptions");

    let res = app.post(&path, json!({"base_uri": " ", "password": ""})).await.assert(422).json();
    assert_eq!(error(&res, 0), ("base_uri", "required"));
    assert_eq!(error(&res, 1), ("username", "required"));
    assert_eq!(error(&res, 2), ("password", "required"));
    let res = app
        .post(&path, json!({"base_uri": remote, "username": "x".repeat(1025), "password": "p", "sync_way": 4}))
        .await
        .assert(422)
        .json();
    assert_eq!(error(&res, 0), ("username", "max"));
    assert_eq!(error(&res, 1), ("sync_way", "in"));
    let res =
        app.post(&path, json!({"base_uri": "not a url", "username": "u", "password": "p"})).await.assert(422).json();
    assert_eq!(error(&res, 0), ("base_uri", "url"));
    app.post(&path, json!({"base_uri": remote, "username": "u", "password": "p", "sync_way": "both"}))
        .await
        .assert(422);

    // discovery fails: a 422 on base_uri
    let broken = remote_server(true).await;
    let res = app.post(&path, json!({"base_uri": broken, "username": "u", "password": "p"})).await.assert(422).json();
    assert_eq!(error(&res, 0), ("base_uri", "discovery"));
    assert!(res["errors"][0]["message"].as_str().unwrap().contains("500"), "{res}");

    let res = app
        .post(&path, json!({"base_uri": remote, "username": "me@example.com", "password": "secret", "sync_way": 2}))
        .await
        .assert(201)
        .json();
    assert_eq!(res["uri"], format!("{remote}{CONTACTS}"));
    assert_eq!(res["user_id"], manager.id.to_string());
    assert_eq!(res["vault_id"], vault.id.to_string());
    assert_eq!(res["username"], "me@example.com");
    assert_eq!(res["active"], true);
    assert_eq!(res["sync_way"], 2);
    assert_eq!(res["frequency"], 180);
    assert_eq!(
        res["capabilities"],
        json!({
            "addressbookMultiget": true,
            "addressbookQuery": true,
            "syncCollection": true,
            "addressData": {"content-type": "text/vcard", "version": "4.0"},
        })
    );
    assert!(res["last_synchronized_at"].is_string(), "the forced first sync ran: {res}");
    assert!(res.get("password").is_none());

    let id: Uuid = res["id"].as_str().unwrap().parse().unwrap();
    let stored = row(&app, id).await.unwrap();
    assert_eq!(monica::crypto::decrypt(&app.state.config.key, &stored.password).as_deref(), Some("secret"));
    assert_eq!(stored.current_logid.map(|l| l.0), Some(1));
    assert!(stored.last_batch.is_some());

    let list = app.get(&path).await.assert(200).json();
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"][0]["id"], id.to_string());

    let editor = member(&app, &manager, &vault, VaultPermission::Edit).await;
    app.login(&editor.username).await;
    app.post(&path, json!({"base_uri": remote, "username": "u", "password": "p"})).await.assert(403);
    app.get(&path).await.assert(403);

    let stranger = factory::user(&app.state.db, None).await;
    app.login(&stranger.username).await;
    app.post(&path, json!({"base_uri": remote, "username": "u", "password": "p"})).await.assert(404);
    app.get(&path).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn shows_updates_and_deletes_a_subscription() {
    let (mut app, manager, vault, base) = setup().await;
    let subscription = subscription(&app, &manager, &vault, "https://example.com/dav/").await;
    let path = format!("{base}/address-book-subscriptions/{}", subscription.id);

    let res = app.get(&path).await.assert(200).json();
    assert_eq!(res["id"], subscription.id.to_string());
    assert_eq!(res["uri"], "https://example.com/dav/");
    assert_eq!(res["sync_way"], 3);
    assert!(res["last_synchronized_at"].is_null());
    app.get(&format!("{base}/address-book-subscriptions/{}", Uuid::new())).await.assert(404);
    let other = app.post("/vaults", json!({"name": "Work"})).await.assert(201).json();
    app.get(&format!("/vaults/{}/address-book-subscriptions/{}", other["id"].as_str().unwrap(), subscription.id))
        .await
        .assert(404);

    let res = app
        .patch(&path, json!({"frequency": 0, "sync_way": 4, "username": " ", "password": ""}))
        .await
        .assert(422)
        .json();
    assert_eq!(error(&res, 0), ("username", "required"));
    assert_eq!(error(&res, 1), ("password", "required"));
    assert_eq!(error(&res, 2), ("sync_way", "in"));
    assert_eq!(error(&res, 3), ("frequency", "min"));
    app.patch(&path, json!({"frequency": 32768})).await.assert(422);
    app.patch(&path, json!({"active": "no"})).await.assert(422);

    let res = app
        .patch(&path, json!({"username": " other@example.com ", "password": "new", "active": false, "sync_way": 1, "frequency": 60}))
        .await
        .assert(200)
        .json();
    assert_eq!(res["username"], "other@example.com");
    assert_eq!(res["active"], false);
    assert_eq!(res["sync_way"], 1);
    assert_eq!(res["frequency"], 60);
    let stored = row(&app, subscription.id).await.unwrap();
    assert_eq!(monica::crypto::decrypt(&app.state.config.key, &stored.password).as_deref(), Some("new"));
    app.patch(&path, json!({})).await.assert(200);

    let viewer = member(&app, &manager, &vault, VaultPermission::View).await;
    app.login(&viewer.username).await;
    app.get(&path).await.assert(403);
    app.patch(&path, json!({"frequency": 30})).await.assert(403);
    app.delete(&path).await.assert(403);

    let stranger = factory::user(&app.state.db, None).await;
    app.login(&stranger.username).await;
    app.get(&path).await.assert(404);
    app.patch(&path, json!({"frequency": 30})).await.assert(404);
    app.delete(&path).await.assert(404);

    app.login(&manager.username).await;
    app.delete(&path).await.assert(204);
    assert!(row(&app, subscription.id).await.is_none());
    app.get(&path).await.assert(404);
    app.delete(&path).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn synchronizes_a_subscription_now() {
    let (mut app, manager, vault, base) = setup().await;
    let remote = remote_server(false).await;
    let subscription = subscription(&app, &manager, &vault, &format!("{remote}{CONTACTS}")).await;
    let path = format!("{base}/address-book-subscriptions/{}/sync", subscription.id);

    app.post(&path, json!({"force": "yes"})).await.assert(422);

    app.post(&path, json!({})).await.assert(202);
    let stored = row(&app, subscription.id).await.unwrap();
    assert!(stored.last_synchronized_at.is_some());
    assert_eq!(stored.current_logid.map(|l| l.0), Some(1));
    assert_eq!(stored.distant_sync_token.as_deref(), Some("\"test\""));

    app.post(&path, json!({"force": true})).await.assert(202);
    assert_eq!(row(&app, subscription.id).await.unwrap().current_logid.map(|l| l.0), Some(2));

    app.post(&format!("{base}/address-book-subscriptions/{}/sync", Uuid::new()), json!({})).await.assert(404);

    let viewer = member(&app, &manager, &vault, VaultPermission::View).await;
    app.login(&viewer.username).await;
    app.post(&path, json!({})).await.assert(403);

    let stranger = factory::user(&app.state.db, None).await;
    app.login(&stranger.username).await;
    app.post(&path, json!({})).await.assert(404);
    app.finish().await;
}
