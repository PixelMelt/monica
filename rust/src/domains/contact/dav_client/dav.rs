//! `app/Domains/Contact/DavClient/Services/Utils/Dav`: `DavClient`,
//! `ServiceUrlQuery`, and what they read and write with `Sabre\Xml` and
//! `DOMDocument`.

use std::time::Duration;

use base64::Engine as _;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;
use reqwest::header::{self, HeaderMap, HeaderValue};

use super::Exception;
use crate::config::{Config, Environment};

/// `Sabre\CardDAV\Plugin::NS_CARDDAV`.
pub const NS_CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";

/// `Illuminate\Http\Client\Response`.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: String,
}

impl Response {
    /// `header($header)`: the values joined with `, `, `''` when absent.
    pub fn header(&self, name: &str) -> String {
        self.headers
            .get_all(name)
            .iter()
            .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A value read by `Sabre\Xml\Reader`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// An empty element (`null`).
    Null,
    /// The text of an element without child elements.
    Text(String),
    /// The child elements (`[['name' => …, 'value' => …, 'attributes' => …], …]`).
    Elements(Vec<Element>),
    /// `{DAV:}resourcetype` (`Sabre\DAV\Xml\Property\ResourceType`).
    ResourceType(Vec<String>),
}

impl Value {
    /// The value when it is a string.
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// `['name' => …, 'value' => …, 'attributes' => …]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub value: Value,
    pub attributes: Vec<(String, String)>,
}

impl Element {
    /// `Arr::get($element, "attributes.$name")`.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

/// The properties of a response for one status: `[name => value]`.
pub type Properties = Vec<(String, Value)>;

/// `Arr::get($properties, $name)`.
pub fn get<'a>(properties: &'a Properties, name: &str) -> Option<&'a Value> {
    properties.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

/// A response of `parseMultiStatus()`: `['properties' => [status => properties], 'status' => …]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusList {
    pub properties: Vec<(String, Properties)>,
    pub status: String,
}

impl StatusList {
    /// `Arr::get($statusList, "properties.$status.$name")`.
    pub fn get(&self, status: &str, name: &str) -> Option<&Value> {
        self.properties.iter().find(|(s, _)| s == status).and_then(|(_, p)| get(p, name))
    }
}

/// What `parseMultiStatus()` returns: the responses by href, and the `synctoken`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MultiStatus {
    pub responses: Vec<(String, StatusList)>,
    pub synctoken: Option<String>,
}

/// What `propFind()` returns: the properties of the first response (depth
/// 0), or the properties of each href (depth 1).
#[derive(Debug, Clone, PartialEq)]
pub enum PropFind {
    Properties(Properties),
    Hrefs(Vec<(String, Properties)>),
}

impl PropFind {
    /// The depth 0 result.
    pub fn properties(self) -> Properties {
        match self {
            PropFind::Properties(properties) => properties,
            PropFind::Hrefs(_) => Vec::new(),
        }
    }
}

/// A property of `fetchProperties()`: `'{namespace}value'`, or
/// `['name' => …, 'value' => …, 'attributes' => […]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: String,
    pub value: Option<String>,
    pub attributes: Vec<(String, String)>,
}

impl From<&str> for Prop {
    fn from(name: &str) -> Self {
        Prop { name: name.to_owned(), value: None, attributes: Vec::new() }
    }
}

/// `DavClient`.
#[derive(Debug, Clone)]
pub struct DavClient {
    base_uri: Option<String>,
    username: Option<String>,
    password: Option<String>,
    /// `App::environment('local')`.
    local: bool,
}

impl DavClient {
    /// `app(DavClient::class)`.
    pub fn new(config: &Config) -> DavClient {
        DavClient { base_uri: None, username: None, password: None, local: config.env == Environment::Local }
    }

    /// Set the base uri of client.
    pub fn set_base_uri(&mut self, uri: &str) -> &mut Self {
        self.base_uri = Some(uri.to_owned());

        self
    }

    /// Set credentials.
    pub fn set_credentials(&mut self, username: &str, password: &str) -> &mut Self {
        self.username = Some(username.to_owned());
        self.password = Some(password.to_owned());

        self
    }

    /// Get current uri.
    pub fn path(&self, path: Option<&str>) -> String {
        let uri = Uri::parse(self.base_uri.as_deref().unwrap_or_default());

        let Some(path) = path.filter(|p| !super::php_empty(p)) else {
            return uri.to_string();
        };

        if path.starts_with('/') {
            return uri.with_path(path).to_string();
        }

        let base_path = format!("{}/", uri.path.trim_end_matches('/'));

        uri.with_path(&format!("{base_path}{path}")).to_string()
    }

    /// Get a `PendingRequest`.
    fn get_request(&self) -> reqwest::ClientBuilder {
        let mut request = reqwest::Client::builder()
            .user_agent(format!("Monica DavClient {}/Guzzle", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5));

        if self.local {
            request = request.danger_accept_invalid_certs(true);
        }

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            let credentials = base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            if let Ok(mut value) = HeaderValue::from_str(&format!("Basic {credentials}")) {
                value.set_sensitive(true);
                request = request.default_headers(HeaderMap::from_iter([(header::AUTHORIZATION, value)]));
            }
        }

        request
    }

    /// Follow rfc6764 to get carddav service url.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6764>.
    pub async fn get_service_url(&self) -> Result<Option<String>, Exception> {
        // first attempt on relative url
        let mut target = self.standard_service_url(".well-known/carddav").await?;

        if target.as_deref().is_none_or(super::php_empty) {
            // second attempt on absolute root url
            target = self.standard_service_url("/.well-known/carddav").await?;
        }

        if target.as_deref().is_none_or(super::php_empty) {
            // third attempt for non standard server, like Google API
            target = self.non_standard_service_url("/.well-known/carddav").await?;
        }

        if target.as_deref().is_none_or(super::php_empty) {
            let service_url_query = ServiceUrlQuery::with_client(self);

            // Get service name register (section 9.2)
            target = service_url_query.execute("_carddavs._tcp", true, &self.path(None)).await?;
            if target.is_none() {
                target = service_url_query.execute("_carddav._tcp", false, &self.path(None)).await?;
            }
        }

        Ok(target)
    }

    async fn standard_service_url(&self, url: &str) -> Result<Option<String>, Exception> {
        // Get well-known register (section 9.1)
        let response = send(
            self.get_request().redirect(reqwest::redirect::Policy::none()),
            "GET",
            &self.path(Some(url)),
            None,
            &[],
        )
        .await?;

        let code = response.status;
        if code == 301 || code == 302 {
            return Ok(Some(response.header("Location")));
        }

        if response.status >= 500 {
            return Err(Exception::Request(Box::new(response)));
        }

        Ok(None)
    }

    async fn non_standard_service_url(&self, url: &str) -> Result<Option<String>, Exception> {
        let response = send(
            self.get_request().redirect(reqwest::redirect::Policy::none()),
            "PROPFIND",
            &self.path(Some(url)),
            None,
            &[],
        )
        .await?;

        let code = response.status;
        if code == 301 || code == 302 {
            return Ok(Some(self.path(Some(&response.header("Location")))));
        }

        Ok(None)
    }

    /// Do a PROPFIND request.
    ///
    /// The list of requested properties must be specified in clark notation.
    /// Only properties that are actually returned from the server (without
    /// error) are returned. Depth should be either 0 or 1; a depth of 1 also
    /// returns all child resources.
    pub async fn prop_find(&self, properties: &[Prop], depth: u8, url: &str) -> Result<PropFind, Exception> {
        let body = document(&format!(
            "<d:propfind xmlns:d=\"DAV:\">{}</d:propfind>",
            element("d:prop", &fetch_properties(properties, &[("DAV:", "d")])?)
        ));

        let response = self.request("PROPFIND", url, Some(body), &[("Depth", depth.to_string())]).await?;

        let result = parse_multi_status(&response.body)?;

        // If depth was 0, we only return the top item value
        if depth == 0 {
            let result = result.responses.into_iter().next().map(|(_, status_list)| status_list);

            return Ok(PropFind::Properties(result.map(properties_200).unwrap_or_default()));
        }

        Ok(PropFind::Hrefs(result.responses.into_iter().map(|(href, s)| (href, properties_200(s))).collect()))
    }

    /// Run a REPORT {DAV:}sync-collection.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6578>.
    pub async fn sync_collection(&self, properties: &[Prop], sync_token: &str) -> Result<MultiStatus, Exception> {
        let body = document(&format!(
            "<d:sync-collection xmlns:d=\"DAV:\">{}{}{}</d:sync-collection>",
            element("d:sync-token", &escape_text(sync_token)),
            element("d:sync-level", "1"),
            element("d:prop", &fetch_properties(properties, &[("DAV:", "d")])?)
        ));

        let response = self.request("REPORT", "", Some(body), &[("Depth", "0".to_owned())]).await?;

        parse_multi_status(&response.body)
    }

    /// Run a REPORT card:addressbook-multiget.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6352#section-8.7>.
    pub async fn addressbook_multiget(
        &self,
        properties: &[Prop],
        contacts: &[String],
    ) -> Result<MultiStatus, Exception> {
        let namespaces = [("DAV:", "d"), (NS_CARDDAV, "card")];

        let mut children = element("d:prop", &fetch_properties(properties, &namespaces)?);

        for contact in contacts {
            children.push_str(&element("d:href", &escape_text(contact)));
        }

        let body = document(&format!(
            "<card:addressbook-multiget xmlns:card=\"{NS_CARDDAV}\" xmlns:d=\"DAV:\">{children}</card:addressbook-multiget>"
        ));

        let response = self.request("REPORT", "", Some(body), &[("Depth", "1".to_owned())]).await?;

        parse_multi_status(&response.body)
    }

    /// Run a REPORT card:addressbook-query.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6352#section-8.6>.
    pub async fn addressbook_query(&self, properties: &[Prop]) -> Result<MultiStatus, Exception> {
        let namespaces = [("DAV:", "d"), (NS_CARDDAV, "card")];

        let body = document(&format!(
            "<card:addressbook-query xmlns:card=\"{NS_CARDDAV}\" xmlns:d=\"DAV:\">{}</card:addressbook-query>",
            element("d:prop", &fetch_properties(properties, &namespaces)?)
        ));

        let response = self.request("REPORT", "", Some(body), &[("Depth", "1".to_owned())]).await?;

        parse_multi_status(&response.body)
    }

    /// Get a WebDAV property.
    pub async fn get_property(&self, property: &str, url: &str) -> Result<Value, Exception> {
        let properties = self.prop_find(&[property.into()], 0, url).await?.properties();

        Ok(get(&properties, property).cloned().unwrap_or(Value::Null))
    }

    /// Get a {DAV:}supported-report-set propfind.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc3253#section-3.1.5>.
    pub async fn get_supported_report_set(&self) -> Result<Vec<Option<String>>, Exception> {
        let prop_name = "{DAV:}supported-report-set";

        let properties = self.prop_find(&[prop_name.into()], 0, "").await?.properties();

        match get(&properties, prop_name) {
            Some(Value::Elements(prop)) => Ok(prop
                .iter()
                .map(|supported_report| {
                    iterate_over(supported_report, "{DAV:}supported-report", |report| {
                        iterate_over(report, "{DAV:}report", |r#type| Some(r#type.name.clone()))
                    })
                })
                .collect()),
            _ => Err(Exception::Error("DavClient::getSupportedReportSet(): Return value must be of type array".into())),
        }
    }

    /// Updates a list of properties on the server.
    ///
    /// The list of properties must have clark-notation properties for the
    /// keys, and the actual (string) value for the value. If the value is
    /// null, an attempt is made to delete the property.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc2518#section-12.13>.
    pub async fn prop_patch(&self, properties: &[(&str, Option<&str>)]) -> Result<bool, Exception> {
        let body = prop_patch_body(properties)?;

        let response = self.request("PROPPATCH", "", Some(body), &[]).await?;

        if response.status == 207 {
            // If it's a 207, the request could still have failed, but the
            // information is hidden in the response body.
            let result = parse_multi_status(&response.body)?;

            let mut error_properties = Vec::new();
            for (_, status_list) in &result.responses {
                for (status, properties) in &status_list.properties {
                    if status.parse::<u16>().is_ok_and(|s| s >= 400) {
                        for (prop_name, _) in properties {
                            error_properties.push(format!("{prop_name} ({status})"));
                        }
                    }
                }
            }
            if !error_properties.is_empty() {
                return Err(Exception::DavClient(format!(
                    "PROPPATCH failed. The following properties errored: {}",
                    error_properties.join(", ")
                )));
            }
        }

        Ok(true)
    }

    /// Performs an HTTP options request.
    ///
    /// Returns all the features from the 'DAV:' header; an empty list if there
    /// was no DAV header, or no contents.
    pub async fn options(&self, url: &str) -> Result<Vec<String>, Exception> {
        let response = self.request("OPTIONS", url, None, &[]).await?;

        let dav = response.header("Dav");
        if super::php_empty(&dav) {
            return Ok(Vec::new());
        }
        let davs = dav.split(", ");

        Ok(davs.map(|header| header.trim().to_owned()).collect())
    }

    /// Performs an actual HTTP request, and returns the result.
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        body: Option<String>,
        headers: &[(&str, String)],
    ) -> Result<Response, Exception> {
        let url = if url.starts_with("http") { url.to_owned() } else { self.path(Some(url)) };

        tracing::debug!(body, ?headers, "DavClient request[request]: {method} {url}");

        let response = send(self.get_request(), method, &url, body, headers).await?;
        if response.status >= 400 {
            tracing::debug!(body = response.body, headers = ?response.headers, "DavClient request[error]: {method} {url} {}", response.status);

            return Err(Exception::Request(Box::new(response)));
        }

        tracing::debug!(body = response.body, headers = ?response.headers, "DavClient request[response]: {method} {url} {}", response.status);

        Ok(response)
    }
}

/// `$request->send($method, $url)`; a body is sent as `application/xml; charset=utf-8`.
async fn send(
    request: reqwest::ClientBuilder,
    method: &str,
    url: &str,
    body: Option<String>,
    headers: &[(&str, String)],
) -> Result<Response, Exception> {
    let client = request.build().map_err(|e| Exception::Error(e.to_string()))?;
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| Exception::Error(e.to_string()))?;
    let mut request = client.request(method, url);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/xml; charset=utf-8").body(body);
    }

    let response = request.send().await.map_err(|e| Exception::Connection(e.to_string()))?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response.text().await.map_err(|e| Exception::Connection(e.to_string()))?;

    Ok(Response { status, headers, body })
}

/// `Arr::get($statusList, 'properties.200', [])`.
fn properties_200(status_list: StatusList) -> Properties {
    status_list.properties.into_iter().find(|(s, _)| s == "200").map(|(_, p)| p).unwrap_or_default()
}

/// `iterateOver()`: the callback on the first item of the list's value, if
/// the list is the element `name`.
fn iterate_over(list: &Element, name: &str, callback: impl Fn(&Element) -> Option<String>) -> Option<String> {
    if list.name == name
        && let Value::Elements(value) = &list.value
        && let Some(item) = value.first()
    {
        return callback(item);
    }

    None
}

/// `Sabre\Xml\Service::parseClarkNotation()`.
fn parse_clark_notation(name: &str) -> Result<(&str, &str), Exception> {
    name.strip_prefix('{')
        .and_then(|rest| rest.split_once('}'))
        .ok_or_else(|| Exception::Error(format!("'{name}' is not a valid clark-notation formatted string")))
}

/// `fetchProperties()`: the children of `<d:prop>`.
///
/// A property is a `'{namespace}value'`, or a complex value element with
/// a content and attributes.
fn fetch_properties(properties: &[Prop], namespaces: &[(&str, &str)]) -> Result<String, Exception> {
    let mut prop = String::new();

    for property in properties {
        let (namespace, element_name) = parse_clark_notation(&property.name)?;

        let ns = namespaces.iter().find(|(n, _)| *n == namespace).map(|(_, p)| *p);
        let (name, mut open) = match ns {
            Some(ns) => (format!("{ns}:{element_name}"), format!("{ns}:{element_name}")),
            None => {
                (format!("x:{element_name}"), format!("x:{element_name} xmlns:x=\"{}\"", escape_attribute(namespace)))
            }
        };

        for (name, value) in &property.attributes {
            open.push_str(&format!(" {name}=\"{}\"", escape_attribute(value)));
        }

        let content = property.value.as_deref().map(escape_text).unwrap_or_default();
        if content.is_empty() {
            prop.push_str(&format!("<{open}/>"));
        } else {
            prop.push_str(&format!("<{open}>{content}</{name}>"));
        }
    }

    Ok(prop)
}

/// `$dom->saveXML()` of a document with `root`.
fn document(root: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{root}\n")
}

/// An element created by `DOMDocument::createElement($name, $value)`.
fn element(name: &str, content: &str) -> String {
    if content.is_empty() { format!("<{name}/>") } else { format!("<{name}>{content}</{name}>") }
}

/// The escaping of text by `saveXML()` and `XMLWriter::text()`.
fn escape_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\r', "&#13;")
}

/// The escaping of attribute values by `saveXML()`.
fn escape_attribute(value: &str) -> String {
    escape_text(value).replace('"', "&quot;").replace('\n', "&#10;").replace('\t', "&#9;")
}

/// `(new Service)->write('{DAV:}propertyupdate', $propPatch)`: a
/// `{DAV:}set` (or a `{DAV:}remove` for null) of each property, indented.
fn prop_patch_body(properties: &[(&str, Option<&str>)]) -> Result<String, Exception> {
    let mut body =
        "<?xml version=\"1.0\"?>\n<d:propertyupdate xmlns:d=\"DAV:\" xmlns:s=\"http://sabredav.org/ns\">\n".to_owned();
    let mut adhoc_namespaces: Vec<String> = Vec::new();

    for (property_name, property_value) in properties {
        let (namespace, local_name) = parse_clark_notation(property_name)?;
        let (name, open) = match namespace {
            "DAV:" => (format!("d:{local_name}"), format!("d:{local_name}")),
            "http://sabredav.org/ns" => (format!("s:{local_name}"), format!("s:{local_name}")),
            _ => {
                let index = match adhoc_namespaces.iter().position(|n| n == namespace) {
                    Some(i) => i + 1,
                    None => {
                        adhoc_namespaces.push(namespace.to_owned());
                        adhoc_namespaces.len()
                    }
                };
                (
                    format!("x{index}:{local_name}"),
                    format!("x{index}:{local_name} xmlns:x{index}=\"{}\"", escape_attribute(namespace)),
                )
            }
        };
        let action = if property_value.is_none() { "d:remove" } else { "d:set" };
        let property = match property_value {
            None => format!("<{open}/>"),
            Some(value) => format!("<{open}>{}</{name}>", escape_text(value)),
        };
        body.push_str(&format!(" <{action}>\n  <d:prop>\n   {property}\n  </d:prop>\n </{action}>\n"));
    }

    body.push_str("</d:propertyupdate>\n");
    Ok(body)
}

/// An element of the XML answer, as `XMLReader` sees it.
#[derive(Debug, Default)]
struct Node {
    /// The clark notation of the name (`{namespace}local`).
    name: String,
    attributes: Vec<(String, String)>,
    /// `$reader->isEmptyElement` (`<x/>`).
    empty: bool,
    children: Vec<Node>,
    /// The text and CDATA nodes (whitespace-only text nodes are not text nodes).
    text: Option<String>,
}

/// Reads `body` into its root element.
fn read_tree(body: &str) -> Result<Node, Exception> {
    let error = |e: &dyn std::fmt::Display| Exception::Error(format!("ParseException: {e}"));
    let mut reader = NsReader::from_str(body);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut run = String::new();

    loop {
        let (ns, event) = reader.read_resolved_event().map_err(|e| error(&e))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                flush(&mut stack, &mut run);
                let local = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                let name = match ns {
                    ResolveResult::Bound(ns) => format!("{{{}}}{local}", String::from_utf8_lossy(ns.as_ref())),
                    ResolveResult::Unbound => format!("{{}}{local}"),
                    ResolveResult::Unknown(prefix) => {
                        return Err(error(&format!(
                            "Namespace prefix {} is not defined",
                            String::from_utf8_lossy(&prefix)
                        )));
                    }
                };
                let mut attributes = Vec::new();
                for attribute in e.attributes() {
                    let attribute = attribute.map_err(|e| error(&e))?;
                    let key = attribute.key.as_ref();
                    if key == b"xmlns" || key.starts_with(b"xmlns:") {
                        continue;
                    }
                    let value =
                        attribute.decode_and_unescape_value(reader.decoder()).map_err(|e| error(&e))?.into_owned();
                    let (ns, local) = reader.resolve_attribute(attribute.key);
                    let local = String::from_utf8_lossy(local.as_ref()).into_owned();
                    match ns {
                        ResolveResult::Bound(ns) => {
                            attributes.push((format!("{{{}}}{local}", String::from_utf8_lossy(ns.as_ref())), value));
                        }
                        _ => attributes.push((local, value)),
                    }
                }
                let empty = matches!(event, Event::Empty(_));
                let node = Node { name, attributes, empty, ..Default::default() };
                if empty {
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => root = Some(node),
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                flush(&mut stack, &mut run);
                let node = stack.pop().ok_or_else(|| error(&"unexpected end tag"))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Text(t) => run.push_str(&t.xml_content().map_err(|e| error(&e))?),
            Event::GeneralRef(r) => {
                if let Some(c) = r.resolve_char_ref().map_err(|e| error(&e))? {
                    run.push(c);
                } else {
                    let name = r.decode().map_err(|e| error(&e))?;
                    let c = match name.as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "apos" => '\'',
                        "quot" => '"',
                        other => return Err(error(&format!("Entity '{other}' not defined"))),
                    };
                    run.push(c);
                }
            }
            Event::CData(c) => {
                flush(&mut stack, &mut run);
                if let Some(node) = stack.last_mut() {
                    node.text.get_or_insert_with(String::new).push_str(&c.xml_content().map_err(|e| error(&e))?);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    match (root, stack.is_empty()) {
        (Some(root), true) => Ok(root),
        _ => Err(error(&"The input element to parse is empty. Do not attempt to parse")),
    }
}

/// Ends a run of text: a text node, unless it is only whitespace.
fn flush(stack: &mut [Node], run: &mut String) {
    if !run.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
        && let Some(node) = stack.last_mut()
    {
        node.text.get_or_insert_with(String::new).push_str(run);
    }
    run.clear();
}

/// `Sabre\Xml\Element\Base` (`parseInnerTree()`).
fn base(node: &Node) -> Value {
    if node.empty {
        return Value::Null;
    }
    if !node.children.is_empty() {
        return Value::Elements(node.children.iter().map(parse_current_element).collect());
    }
    node.text.clone().map_or(Value::Null, Value::Text)
}

/// `parseCurrentElement()`.
fn parse_current_element(node: &Node) -> Element {
    Element { name: node.name.clone(), value: deserialize(node), attributes: node.attributes.clone() }
}

/// The deserializer of `Sabre\DAV\Xml\Service::$elementMap` for a property.
fn deserialize(node: &Node) -> Value {
    if node.name == "{DAV:}resourcetype" {
        return Value::ResourceType(node.children.iter().map(|c| c.name.clone()).collect());
    }
    base(node)
}

/// `explode(' ', $status, 3)[1]`.
fn status_code(status: &Value) -> String {
    status.text().and_then(|s| s.split(' ').nth(1)).unwrap_or_default().to_owned()
}

/// `Sabre\DAV\Xml\Element\Response::xmlDeserialize()`.
fn response(node: &Node) -> (String, StatusList) {
    let mut href = String::new();
    let mut property_lists: Vec<(String, Properties)> = Vec::new();
    let mut status_code_value = None;

    for elem in &node.children {
        match elem.name.as_str() {
            "{DAV:}href" => href = base(elem).text().unwrap_or_default().to_owned(),
            "{DAV:}propstat" => {
                let mut status = Value::Null;
                let mut properties = Vec::new();
                for child in &elem.children {
                    match child.name.as_str() {
                        "{DAV:}status" => status = base(child),
                        "{DAV:}prop" => {
                            properties = Vec::new();
                            for p in &child.children {
                                let value = if p.empty { Value::Null } else { deserialize(p) };
                                insert(&mut properties, p.name.clone(), value);
                            }
                        }
                        _ => {}
                    }
                }
                if !properties.is_empty() {
                    insert(&mut property_lists, status_code(&status), properties);
                }
            }
            "{DAV:}status" => status_code_value = Some(status_code(&base(elem))),
            _ => {}
        }
    }

    (href, StatusList { properties: property_lists, status: status_code_value.unwrap_or_else(|| "200".to_owned()) })
}

/// `$array[$key] = $value` on an ordered map.
fn insert<T>(list: &mut Vec<(String, T)>, key: String, value: T) {
    match list.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => list.push((key, value)),
    }
}

/// Parses a WebDAV multistatus response body.
///
/// See <https://datatracker.ietf.org/doc/html/rfc4918#section-9.2.1>.
fn parse_multi_status(body: &str) -> Result<MultiStatus, Exception> {
    let root = read_tree(body)?;
    if root.name != "{DAV:}multistatus" {
        return Err(Exception::Error(format!(
            "ParseException: Expected {{DAV:}}multistatus but received {} as the root element",
            root.name
        )));
    }

    let mut result = MultiStatus::default();

    for elem in &root.children {
        match elem.name.as_str() {
            "{DAV:}response" => {
                let (href, status_list) = response(elem);
                insert(&mut result.responses, href, status_list);
            }
            "{DAV:}sync-token" => {
                result.synctoken = base(elem).text().filter(|t| !super::php_empty(t)).map(str::to_owned);
            }
            _ => {}
        }
    }

    Ok(result)
}

/// `GuzzleHttp\Psr7\Uri`.
#[derive(Debug, Clone)]
struct Uri {
    scheme: String,
    authority: String,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

impl Uri {
    /// `GuzzleUtils::uriFor($uri)`.
    fn parse(uri: &str) -> Uri {
        let (rest, fragment) = match uri.split_once('#') {
            Some((r, f)) => (r, Some(f.to_owned())),
            None => (uri, None),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((r, q)) => (r, Some(q.to_owned())),
            None => (rest, None),
        };
        let (scheme, rest) = match rest.split_once(':') {
            Some((s, r))
                if s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) =>
            {
                (s.to_ascii_lowercase(), r)
            }
            _ => (String::new(), rest),
        };
        let (authority, path) = match rest.strip_prefix("//") {
            Some(r) => match r.find('/') {
                Some(i) => (r[..i].to_owned(), r[i..].to_owned()),
                None => (r.to_owned(), String::new()),
            },
            None => (String::new(), rest.to_owned()),
        };

        Uri { authority: normalize_authority(&scheme, &authority), scheme, path: filter_path(&path), query, fragment }
    }

    /// `withPath($path)`.
    fn with_path(&self, path: &str) -> Uri {
        Uri { path: filter_path(path), ..self.clone() }
    }
}

impl std::fmt::Display for Uri {
    /// `Uri::composeComponents()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.scheme.is_empty() {
            write!(f, "{}:", self.scheme)?;
        }
        if !self.authority.is_empty() || self.scheme == "file" {
            write!(f, "//{}", self.authority)?;
        }
        if !self.authority.is_empty() && !self.path.is_empty() && !self.path.starts_with('/') {
            f.write_str("/")?;
        }
        f.write_str(&self.path)?;
        if let Some(query) = self.query.as_deref().filter(|q| !q.is_empty()) {
            write!(f, "?{query}")?;
        }
        if let Some(fragment) = self.fragment.as_deref().filter(|q| !q.is_empty()) {
            write!(f, "#{fragment}")?;
        }
        Ok(())
    }
}

/// The host in lower case, without the default port of the scheme.
fn normalize_authority(scheme: &str, authority: &str) -> String {
    let (user_info, host_port) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, _)) if !h.ends_with(']') && h.contains(':') => (host_port, None),
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
        _ => (host_port, None),
    };
    let port =
        port.filter(|p| !((scheme == "http" && *p == "80") || (scheme == "https" && *p == "443") || p.is_empty()));
    let mut out = String::new();
    if let Some(user_info) = user_info {
        out.push_str(user_info);
        out.push('@');
    }
    out.push_str(&host.to_lowercase());
    if let Some(port) = port {
        out.push(':');
        out.push_str(port);
    }
    out
}

/// `Uri::filterPath()`: percent-encodes what a path can't hold.
fn filter_path(path: &str) -> String {
    let b = path.as_bytes();
    let mut out = String::with_capacity(path.len());
    for (i, &c) in b.iter().enumerate() {
        let allowed = c.is_ascii_alphanumeric() || b"_-.~!$&'()*+,;=:@/".contains(&c);
        let escape = c == b'%'
            && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
            && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
        if allowed || escape {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// The parts of `parse_url($url)` the client reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UrlParts {
    pub host: Option<String>,
    pub path: Option<String>,
}

/// `parse_url($url)` (`PHP_URL_HOST`, `PHP_URL_PATH`).
pub fn parse_url(url: &str) -> UrlParts {
    let rest = url.split(['#', '?']).next().unwrap_or_default();
    let rest = match rest.split_once("://") {
        Some((scheme, r)) if !scheme.is_empty() && !scheme.contains('/') => format!("//{r}"),
        _ => rest.to_owned(),
    };
    let (authority, path) = match rest.strip_prefix("//") {
        Some(r) => match r.find('/') {
            Some(i) => (Some(r[..i].to_owned()), r[i..].to_owned()),
            None => (Some(r.to_owned()), String::new()),
        },
        None => (None, rest),
    };
    let host = authority.map(|a| {
        let host_port = a.rsplit_once('@').map_or(a.as_str(), |(_, h)| h).to_owned();
        if host_port.starts_with('[') {
            host_port.split_once(']').map_or(host_port.clone(), |(h, _)| format!("{h}]"))
        } else {
            host_port.split(':').next().unwrap_or_default().to_owned()
        }
    });

    UrlParts { host: host.filter(|h| !h.is_empty()), path: Some(path).filter(|p| !p.is_empty()) }
}

/// A `DNS_SRV` entry of `dns_get_record()`.
#[derive(Debug, Clone, PartialEq)]
pub struct SrvEntry {
    pub pri: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

/// `Http::getDnsRecord($hostname, DNS_SRV)`: the macro of `AppServiceProvider`.
async fn get_dns_record(hostname: &str) -> Option<Vec<SrvEntry>> {
    #[cfg(test)]
    if let Ok(entries) = super::tests::GET_DNS_RECORD.try_with(|f| f(hostname)) {
        return entries;
    }

    let resolver = hickory_resolver::Resolver::builder_tokio().ok()?.build();
    let lookup = resolver.srv_lookup(hostname).await.ok()?;
    let entries: Vec<SrvEntry> = lookup
        .iter()
        .map(|srv| SrvEntry {
            pri: srv.priority(),
            weight: srv.weight(),
            port: srv.port(),
            target: srv.target().to_utf8().trim_end_matches('.').to_owned(),
        })
        .collect();

    (!entries.is_empty()).then_some(entries)
}

/// `ServiceUrlQuery`.
pub struct ServiceUrlQuery<'a> {
    client: &'a DavClient,
}

impl<'a> ServiceUrlQuery<'a> {
    /// `withClient($client)`.
    pub fn with_client(client: &'a DavClient) -> Self {
        ServiceUrlQuery { client }
    }

    /// Get service url.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6352#section-11> and
    /// <https://datatracker.ietf.org/doc/html/rfc2782>.
    pub async fn execute(&self, name: &str, https: bool, base_uri: &str) -> Result<Option<String>, Exception> {
        if let Some(host) = self.parse_url(base_uri) {
            let entries = get_dns_record(&format!("{name}.{host}")).await;

            if let Some(entries) = entries.filter(|e| !e.is_empty()) {
                let mut groups: Vec<(u16, Vec<SrvEntry>)> = Vec::new();
                for entry in entries {
                    match groups.iter_mut().find(|(pri, _)| *pri == entry.pri) {
                        Some((_, group)) => group.push(entry),
                        None => groups.push((entry.pri, vec![entry])),
                    }
                }
                groups.sort_by_key(|(pri, _)| *pri);

                for entry in groups.into_iter().flat_map(|(_, group)| group) {
                    match self.get_uri(&entry, https).await {
                        Ok(uri) => return Ok(Some(uri)),
                        // if any exception occurs, it will try the next entry.
                        Err(Exception::Request(_)) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        Ok(None)
    }

    fn parse_url(&self, base_uri: &str) -> Option<String> {
        parse_url(base_uri).host
    }

    /// Get uri from entry.
    async fn get_uri(&self, entry: &SrvEntry, https: bool) -> Result<String, Exception> {
        let scheme = if https { "https" } else { "http" };
        let uri = Uri {
            scheme: scheme.to_owned(),
            authority: normalize_authority(scheme, &format!("{}:{}", entry.target, entry.port)),
            path: String::new(),
            query: None,
            fragment: None,
        }
        .to_string();

        // Test connection
        self.client.request("GET", &uri, None, &[]).await?;

        Ok(uri)
    }
}
