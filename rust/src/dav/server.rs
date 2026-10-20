//! `Sabre\DAV\Server` as laravel-sabre runs it (base uri `/dav/`), with
//! `CorePlugin`, `PropFind`, `PropPatch` and the exceptions; the other
//! plugins add their handlers in their own modules.

use std::collections::HashMap;

use axum::http::HeaderMap;

use super::backend::{self, Ctx};
use super::http::{Prefer, decode_path, get_http_prefer, normalize, parse_date, split, to_date};
use super::nodes::{NOT_FOUND, Node, Tree};
use super::props::{PropStats, PropValue, ResponseElement};
use super::xml::{self, Element, NS_CALDAV, Writer};
use crate::error::{ApiError, Code};

/// `Sabre\DAV\Version::VERSION`.
pub const VERSION: &str = "4.7.0";

/// laravel-sabre's base uri (`config('laravelsabre.path')`).
pub const BASE_URI: &str = "/dav/";

/// `Server::DEPTH_INFINITY`.
pub const DEPTH_INFINITY: i32 = -1;

// ---------------------------------------------------------------- exceptions

/// What an exception adds to the error document (`serialize()`).
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorDetail {
    None,
    /// `PreconditionFailed::$header`: `<s:header>`.
    Header(String),
    /// An empty `{DAV:}` element (`d:valid-sync-token`, `d:supported-report`, …).
    DavElement(&'static str),
    /// `InvalidComponentType`: `<cal:supported-calendar-component/>`.
    SupportedCalendarComponent,
    /// `NeedPrivileges`.
    NeedPrivileges {
        uri: String,
        privileges: Vec<String>,
    },
}

/// An exception that reaches `Server::start()`: its class, HTTP code and message.
#[derive(Debug, Clone, PartialEq)]
pub struct DavError {
    pub class: String,
    pub code: u16,
    pub message: String,
    pub detail: ErrorDetail,
}

pub type DavResult<T> = Result<T, DavError>;

impl DavError {
    fn new(class: &str, code: u16, message: impl Into<String>) -> DavError {
        DavError { class: class.to_owned(), code, message: message.into(), detail: ErrorDetail::None }
    }

    /// Any other exception: a 500.
    pub fn internal(class: &str, message: impl Into<String>) -> DavError {
        DavError::new(class, 500, message)
    }

    /// `Sabre\DAV\Exception`.
    pub fn exception(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception", 500, message)
    }

    pub fn bad_request(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\BadRequest", 400, message)
    }

    pub fn not_authenticated(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\NotAuthenticated", 401, message)
    }

    pub fn forbidden(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\Forbidden", 403, message)
    }

    pub fn not_found(message: impl Into<String>) -> DavError {
        DavError::new(NOT_FOUND, 404, message)
    }

    pub fn method_not_allowed(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\MethodNotAllowed", 405, message)
    }

    pub fn conflict(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\Conflict", 409, message)
    }

    pub fn precondition_failed(message: impl Into<String>, header: &str) -> DavError {
        let mut e = DavError::new("Sabre\\DAV\\Exception\\PreconditionFailed", 412, message);
        e.detail = ErrorDetail::Header(header.to_owned());
        e
    }

    pub fn unsupported_media_type(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\UnsupportedMediaType", 415, message)
    }

    pub fn report_not_supported(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAV\\Exception\\ReportNotSupported", 415, message);
        e.detail = ErrorDetail::DavElement("supported-report");
        e
    }

    pub fn not_implemented(message: impl Into<String>) -> DavError {
        DavError::new("Sabre\\DAV\\Exception\\NotImplemented", 501, message)
    }

    pub fn invalid_sync_token(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAV\\Exception\\InvalidSyncToken", 403, message);
        e.detail = ErrorDetail::DavElement("valid-sync-token");
        e
    }

    pub fn invalid_resource_type(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAV\\Exception\\InvalidResourceType", 403, message);
        e.detail = ErrorDetail::DavElement("valid-resourcetype");
        e
    }

    pub fn invalid_component_type(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\CalDAV\\Exception\\InvalidComponentType", 403, message);
        e.detail = ErrorDetail::SupportedCalendarComponent;
        e
    }

    /// `DAVACL\Exception\AceConflict`.
    pub fn ace_conflict(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAVACL\\Exception\\AceConflict", 409, message);
        e.detail = ErrorDetail::DavElement("no-ace-conflict");
        e
    }

    /// `DAVACL\Exception\NotSupportedPrivilege`.
    pub fn not_supported_privilege(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAVACL\\Exception\\NotSupportedPrivilege", 412, message);
        e.detail = ErrorDetail::DavElement("not-supported-privilege");
        e
    }

    /// `DAVACL\Exception\NoAbstract`.
    pub fn no_abstract(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAVACL\\Exception\\NoAbstract", 412, message);
        e.detail = ErrorDetail::DavElement("no-abstract");
        e
    }

    /// `DAVACL\Exception\NotRecognizedPrincipal`.
    pub fn not_recognized_principal(message: impl Into<String>) -> DavError {
        let mut e = DavError::new("Sabre\\DAVACL\\Exception\\NotRecognizedPrincipal", 412, message);
        e.detail = ErrorDetail::DavElement("recognized-principal");
        e
    }

    pub fn need_privileges(uri: &str, privileges: Vec<String>) -> DavError {
        let message =
            format!("User did not have the required privileges ({}) for path \"{uri}\"", privileges.join(","));
        let mut e = DavError::new("Sabre\\DAVACL\\Exception\\NeedPrivileges", 403, message);
        e.detail = ErrorDetail::NeedPrivileges { uri: uri.to_owned(), privileges };
        e
    }
}

impl From<ApiError> for DavError {
    /// An exception of Monica's services: its class by code.
    fn from(e: ApiError) -> Self {
        let class = match e.code {
            Code::NotFound => "Illuminate\\Database\\Eloquent\\ModelNotFoundException",
            Code::Forbidden => "App\\Exceptions\\NotEnoughPermissionException",
            Code::ValidationFailed => "Illuminate\\Validation\\ValidationException",
            _ => "Exception",
        };
        let message = match &e.detail {
            Some(detail) => format!("{} {detail}", e.title),
            None => e.title.clone(),
        };
        DavError::internal(class, message)
    }
}

impl From<sea_orm::DbErr> for DavError {
    fn from(e: sea_orm::DbErr) -> Self {
        DavError::from(ApiError::from(e))
    }
}

impl From<crate::vobject::Error> for DavError {
    fn from(e: crate::vobject::Error) -> Self {
        match e {
            crate::vobject::Error::Parse(m) => DavError::internal("Sabre\\VObject\\ParseException", m),
            crate::vobject::Error::InvalidData(m) => DavError::internal("Sabre\\VObject\\InvalidDataException", m),
        }
    }
}

impl From<xml::ParseError> for DavError {
    /// A `ParseException` (empty input) or `LibXMLException` of the XML reader that nothing catches.
    fn from(e: xml::ParseError) -> Self {
        if e.0.starts_with("The input element to parse is empty") {
            DavError::internal("Sabre\\Xml\\ParseException", e.0)
        } else {
            DavError::internal("Sabre\\Xml\\LibXMLException", e.0)
        }
    }
}

/// `htmlspecialchars($v, ENT_NOQUOTES)` as the DOM writes it back.
fn dom_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    out
}

// ------------------------------------------------------- request and response

/// `Sabre\HTTP\Request`, as laravel-sabre fills it.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// `getUrl()`: the path with a trailing slash, then the query.
    pub url: String,
    pub headers: HeaderMap,
    pub body: String,
}

impl Request {
    /// `getHeader($name)`: the values joined with commas.
    pub fn header(&self, name: &str) -> Option<String> {
        let values: Vec<String> =
            self.headers.get_all(name).iter().map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned()).collect();
        if values.is_empty() { None } else { Some(values.join(",")) }
    }

    /// `getPath()`.
    pub fn path(&self) -> DavResult<String> {
        let uri = normalize(&self.url.replace("//", "/"));
        let base_uri = normalize(BASE_URI);
        if let Some(rest) = uri.strip_prefix(&base_uri) {
            let rest = rest.split('?').next().unwrap_or_default();
            return Ok(decode_path(rest).trim_matches('/').to_owned());
        }
        if format!("{uri}/") == base_uri {
            return Ok(String::new());
        }
        Err(DavError::internal(
            "LogicException",
            format!("Requested uri ({}) is out of base uri ({BASE_URI})", self.url),
        ))
    }

    /// `getQueryParameters()`.
    pub fn query_parameters(&self) -> Vec<(String, String)> {
        let Some((_, query)) = self.url.split_once('?') else {
            return Vec::new();
        };
        url::form_urlencoded::parse(query.as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
    }
}

/// `Sabre\HTTP\Response`.
#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status: Option<u16>,
    /// Header names as set, with their values.
    pub headers: Vec<(String, Vec<String>)>,
    pub body: Option<String>,
}

impl Response {
    /// `setHeader($name, $value)`.
    pub fn set_header(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.headers.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some(entry) => *entry = (name.to_owned(), vec![value]),
            None => self.headers.push((name.to_owned(), vec![value])),
        }
    }

    /// `addHeader($name, $value)`.
    pub fn add_header(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.headers.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some(entry) => entry.1.push(value),
            None => self.headers.push((name.to_owned(), vec![value])),
        }
    }

    /// `getHeader($name)`.
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.join(","))
    }
}

// ---------------------------------------------------------------- PropFind

/// `Sabre\DAV\PropFind`.
#[derive(Debug, Clone)]
pub struct PropFind {
    path: String,
    properties: Vec<String>,
    depth: i32,
    all_props: bool,
    result: Vec<(String, u16, Option<PropValue>)>,
    items_left: usize,
}

/// `PropFind::ALLPROPS`'s properties.
const ALL_PROPS: [&str; 7] = [
    "{DAV:}getlastmodified",
    "{DAV:}getcontentlength",
    "{DAV:}resourcetype",
    "{DAV:}quota-used-bytes",
    "{DAV:}quota-available-bytes",
    "{DAV:}getetag",
    "{DAV:}getcontenttype",
];

impl PropFind {
    /// `new PropFind($path, $properties, $depth, $requestType)`.
    pub fn new(path: &str, properties: &[String], depth: i32, all_props: bool) -> PropFind {
        let properties: Vec<String> =
            if all_props { ALL_PROPS.iter().map(|p| (*p).to_owned()).collect() } else { properties.to_vec() };
        let mut result: Vec<(String, u16, Option<PropValue>)> = Vec::new();
        for name in &properties {
            match result.iter_mut().find(|(n, _, _)| n == name) {
                Some(entry) => *entry = (name.clone(), 404, None),
                None => result.push((name.clone(), 404, None)),
            }
        }
        let items_left = result.len();
        PropFind { path: path.to_owned(), properties, depth, all_props, result, items_left }
    }

    /// Whether `handle($propertyName, …)` would call its callback.
    pub fn wants(&self, name: &str) -> bool {
        self.items_left > 0 && self.result.iter().any(|(n, s, _)| n == name && *s == 404)
    }

    /// `handle($propertyName, $value)` with the computed value.
    pub fn handle(&mut self, name: &str, value: Option<PropValue>) {
        if !self.wants(name) {
            return;
        }
        if let Some(value) = value
            && let Some(entry) = self.result.iter_mut().find(|(n, _, _)| n == name)
        {
            self.items_left -= 1;
            *entry = (name.to_owned(), 200, Some(value));
        }
    }

    /// `set($propertyName, $value, $status)`.
    pub fn set(&mut self, name: &str, value: Option<PropValue>, status: Option<u16>) {
        let status = status.unwrap_or(if value.is_none() { 404 } else { 200 });
        let Some(entry) = self.result.iter_mut().find(|(n, _, _)| n == name) else {
            if self.all_props {
                self.result.push((name.to_owned(), status, value));
            }
            return;
        };
        if status != 404 && entry.1 == 404 {
            self.items_left -= 1;
        } else if status == 404 && entry.1 != 404 {
            self.items_left += 1;
        }
        *entry = (name.to_owned(), status, value);
    }

    /// `get($propertyName)`.
    pub fn get(&self, name: &str) -> Option<&PropValue> {
        self.result.iter().find(|(n, _, _)| n == name).and_then(|(_, _, v)| v.as_ref())
    }

    /// `getStatus($propertyName)`.
    pub fn status(&self, name: &str) -> Option<u16> {
        self.result.iter().find(|(n, _, _)| n == name).map(|(_, s, _)| *s)
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// `get404Properties()`.
    pub fn properties_404(&self) -> Vec<String> {
        if self.items_left == 0 {
            return Vec::new();
        }
        self.result.iter().filter(|(_, s, _)| *s == 404).map(|(n, _, _)| n.clone()).collect()
    }

    /// `getRequestedProperties()`.
    pub fn requested_properties(&self) -> &[String] {
        &self.properties
    }

    /// `isAllProps()`.
    pub fn is_all_props(&self) -> bool {
        self.all_props
    }

    /// `getResultForMultiStatus()`.
    pub fn result_for_multi_status(&self) -> PropStats {
        let mut r: PropStats = vec![(200, Vec::new()), (404, Vec::new())];
        for (name, status, value) in &self.result {
            match r.iter_mut().find(|(s, _)| s == status) {
                Some((_, list)) => list.push((name.clone(), value.clone())),
                None => r.push((*status, vec![(name.clone(), value.clone())])),
            }
        }
        if self.all_props {
            r.retain(|(s, _)| *s != 404);
        }
        r
    }
}

/// A row of `getPropertiesForPath()`: the href and the properties by status.
#[derive(Debug, Clone)]
pub struct PropFindResult {
    pub href: String,
    pub properties: PropStats,
}

impl PropFindResult {
    /// `$props[200][$name]`.
    pub fn found(&self, name: &str) -> Option<&PropValue> {
        self.properties
            .iter()
            .find(|(s, _)| *s == 200)
            .and_then(|(_, list)| list.iter().find(|(n, _)| n == name))
            .and_then(|(_, v)| v.as_ref())
    }

    /// `$props[200][$name] = $value`.
    pub fn replace_found(&mut self, name: &str, value: PropValue) {
        if let Some((_, list)) = self.properties.iter_mut().find(|(s, _)| *s == 200)
            && let Some(entry) = list.iter_mut().find(|(n, _)| n == name)
        {
            entry.1 = Some(value);
        }
    }

    /// `unset($props[200][$name])`.
    pub fn remove_found(&mut self, name: &str) {
        if let Some((_, list)) = self.properties.iter_mut().find(|(s, _)| *s == 200) {
            list.retain(|(n, _)| n != name);
        }
    }
}

/// `{DAV:}group-member-set`, the property `DAVACL\Plugin::propPatch()` handles.
const GROUP_MEMBER_SET: &str = "{DAV:}group-member-set";

/// `Sabre\DAV\PropPatch`, and `Sabre\DAV\MkCol` (with a resource type).
#[derive(Debug, Clone, Default)]
pub struct PropPatch {
    pub resource_type: Vec<String>,
    /// The properties and their value (`None` to remove).
    pub mutations: Vec<(String, Option<Element>)>,
    pub result: Vec<(String, u16)>,
    failed: bool,
    /// `DAVACL\Plugin::propPatch()` registered its callback.
    handles_members: bool,
}

impl PropPatch {
    /// `new MkCol($resourceType, $mutations)` (`new PropPatch($mutations)` without a resource type).
    pub fn new(resource_type: Vec<String>, mutations: Vec<(String, Option<Element>)>) -> PropPatch {
        PropPatch { resource_type, mutations, ..Default::default() }
    }

    /// `MkCol::hasResourceType($resourceType)`.
    pub fn has_resource_type(&self, types: &[&str]) -> bool {
        types.iter().all(|t| self.resource_type.iter().any(|r| r == t))
    }

    /// `setResultCode($properties, $resultCode)`.
    fn set_result_code(&mut self, properties: &[String], code: u16) {
        for name in properties {
            match self.result.iter_mut().find(|(n, _)| n == name) {
                Some(entry) => entry.1 = code,
                None => self.result.push((name.clone(), code)),
            }
        }
        if code >= 400 {
            self.failed = true;
        }
    }

    /// `getRemainingMutations()`.
    pub fn remaining_mutations(&self) -> Vec<String> {
        self.mutations
            .iter()
            .filter(|(n, _)| !self.result.iter().any(|(r, _)| r == n))
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// `setRemainingResultCode($resultCode)`.
    pub fn set_remaining_result_code(&mut self, code: u16) {
        let remaining = self.remaining_mutations();
        self.set_result_code(&remaining, code);
    }
}

/// `Server::$protectedProperties`, then what the CardDAV, CalDAV and ACL plugins add.
const PROTECTED_PROPERTIES: [&str; 46] = [
    "{DAV:}getcontentlength",
    "{DAV:}getetag",
    "{DAV:}getlastmodified",
    "{DAV:}lockdiscovery",
    "{DAV:}supportedlock",
    "{DAV:}quota-available-bytes",
    "{DAV:}quota-used-bytes",
    "{DAV:}supported-privilege-set",
    "{DAV:}current-user-privilege-set",
    "{DAV:}acl",
    "{DAV:}acl-restrictions",
    "{DAV:}inherited-acl-set",
    "{DAV:}supported-method-set",
    "{DAV:}supported-report-set",
    "{DAV:}sync-token",
    "{http://calendarserver.org/ns/}ctag",
    "{http://sabredav.org/ns}sync-token",
    "{urn:ietf:params:xml:ns:carddav}supported-address-data",
    "{urn:ietf:params:xml:ns:carddav}max-resource-size",
    "{urn:ietf:params:xml:ns:carddav}addressbook-home-set",
    "{urn:ietf:params:xml:ns:carddav}supported-collation-set",
    "{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set",
    "{urn:ietf:params:xml:ns:caldav}supported-calendar-data",
    "{urn:ietf:params:xml:ns:caldav}max-resource-size",
    "{urn:ietf:params:xml:ns:caldav}min-date-time",
    "{urn:ietf:params:xml:ns:caldav}max-date-time",
    "{urn:ietf:params:xml:ns:caldav}max-instances",
    "{urn:ietf:params:xml:ns:caldav}max-attendees-per-instance",
    "{urn:ietf:params:xml:ns:caldav}calendar-home-set",
    "{urn:ietf:params:xml:ns:caldav}supported-collation-set",
    "{urn:ietf:params:xml:ns:caldav}calendar-data",
    "{http://calendarserver.org/ns/}getctag",
    "{http://calendarserver.org/ns/}calendar-proxy-read-for",
    "{http://calendarserver.org/ns/}calendar-proxy-write-for",
    "{DAV:}alternate-URI-set",
    "{DAV:}principal-URL",
    "{DAV:}group-membership",
    "{DAV:}principal-collection-set",
    "{DAV:}current-user-principal",
    "{DAV:}supported-privilege-set",
    "{DAV:}current-user-privilege-set",
    "{DAV:}acl",
    "{DAV:}acl-restrictions",
    "{DAV:}inherited-acl-set",
    "{DAV:}owner",
    "{DAV:}group",
];

// ----------------------------------------------------------------- the server

/// `Sabre\DAV\Server` for one request.
pub struct Server<'a> {
    pub ctx: &'a Ctx<'a>,
    pub request: Request,
    pub response: Response,
    pub tree: Tree,
    /// `Auth\Plugin::$currentPrincipal`.
    pub current_principal: Option<String>,
    /// `DAVACL\Plugin::$principalMembershipCache`.
    pub principal_membership_cache: HashMap<String, Vec<String>>,
}

impl<'a> Server<'a> {
    pub fn new(ctx: &'a Ctx<'a>, request: Request) -> Server<'a> {
        Server {
            ctx,
            request,
            response: Response::default(),
            tree: Tree::default(),
            current_principal: None,
            principal_membership_cache: HashMap::new(),
        }
    }

    /// `start()`: runs the request; an exception becomes an error document.
    pub async fn start(&mut self) {
        if let Err(e) = self.invoke_method().await {
            self.exception_response(e).await;
        }
    }

    /// The `catch` of `start()`.
    async fn exception_response(&mut self, e: DavError) {
        let mut body = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        body.push_str("<d:error xmlns:d=\"DAV:\" xmlns:s=\"http://sabredav.org/ns\">\n");
        body.push_str(&format!("  <s:sabredav-version>{}</s:sabredav-version>\n", dom_escape(VERSION)));
        body.push_str(&format!("  <s:exception>{}</s:exception>\n", dom_escape(&e.class)));
        body.push_str(&format!("  <s:message>{}</s:message>\n", dom_escape(&e.message)));
        match &e.detail {
            ErrorDetail::None => {}
            ErrorDetail::Header(header) => body.push_str(&format!("  <s:header>{}</s:header>\n", dom_escape(header))),
            ErrorDetail::DavElement(name) => body.push_str(&format!("  <d:{name}/>\n")),
            ErrorDetail::SupportedCalendarComponent => {
                body.push_str(&format!("  <cal:supported-calendar-component xmlns:cal=\"{NS_CALDAV}\"/>\n"));
            }
            ErrorDetail::NeedPrivileges { uri, privileges } => {
                body.push_str("  <d:need-privileges>\n");
                for privilege in privileges {
                    body.push_str("    <d:resource>\n");
                    body.push_str(&format!("      <d:href>{}</d:href>\n", dom_escape(&format!("{BASE_URI}{uri}"))));
                    body.push_str("      <d:privilege>\n");
                    match xml::parse_clark(privilege) {
                        Some(("DAV:", local)) => body.push_str(&format!("        <d:{local}/>\n")),
                        Some((ns, local)) => body.push_str(&format!("        <d:{local} xmlns:d=\"{ns}\"/>\n")),
                        None => body.push_str(&format!("        <d:{privilege}/>\n")),
                    }
                    body.push_str("      </d:privilege>\n");
                    body.push_str("    </d:resource>\n");
                }
                body.push_str("  </d:need-privileges>\n");
            }
        }
        body.push_str("</d:error>\n");

        if e.class == "Sabre\\DAV\\Exception\\MethodNotAllowed"
            && let Ok(uri) = self.request_uri()
            && let Ok(methods) = self.allowed_methods(&uri).await
        {
            self.response.set_header("Allow", methods.join(", ").to_uppercase());
        }
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.status = Some(e.code);
        self.response.body = Some(body);
    }

    /// `invokeMethod($request, $response)`.
    pub async fn invoke_method(&mut self) -> DavResult<()> {
        let method = self.request.method.clone();
        self.before_method(&method).await?;

        self.response.set_header("X-Sabre-Version", VERSION);

        if !self.check_preconditions().await? {
            return Ok(());
        }

        let handled = match method.as_str() {
            "GET" => self.method_get().await?,
            "HEAD" => self.http_head().await?,
            "OPTIONS" => self.http_options().await?,
            "DELETE" => self.http_delete().await?,
            "PROPFIND" => self.http_prop_find().await?,
            "PROPPATCH" => self.http_prop_patch().await?,
            "PUT" => self.http_put().await?,
            "REPORT" => self.http_report().await?,
            "MKCOL" => self.http_mkcol().await?,
            "MOVE" => self.http_move().await?,
            "COPY" => self.http_copy().await?,
            "MKCALENDAR" => self.caldav_http_mkcalendar().await?,
            "ACL" => self.acl_http_acl().await?,
            _ => false,
        };
        if !handled {
            let mut message =
                format!("There was no plugin in the system that was willing to handle this {method} method.");
            if method == "GET" {
                message.push_str(" Enable the Browser plugin to get a better result here.");
            }
            return Err(DavError::not_implemented(message));
        }

        if method == "GET" {
            self.carddav_after_get().await?;
        }

        if self.response.status.is_none() {
            return Err(DavError::exception(
                "No subsystem set a valid HTTP status code. Something must have interrupted the request without providing further detail.",
            ));
        }
        Ok(())
    }

    /// `beforeMethod:*`: the `Auth` then the `DAVACL` plugin.
    async fn before_method(&mut self, method: &str) -> DavResult<()> {
        // Auth\Plugin::beforeMethod()
        if self.current_principal.is_none() {
            let (ok, principal) = backend::auth_check(self.ctx);
            if !ok {
                return Err(DavError::not_authenticated(principal));
            }
            self.current_principal = Some(principal);
        }

        self.acl_before_method(method).await
    }

    /// The `method:GET` listeners: `VCFExportPlugin`, `ICSExportPlugin`,
    /// `CorePlugin`, then `DAVRedirect`.
    async fn method_get(&mut self) -> DavResult<bool> {
        if self.vcf_http_get().await? || self.ics_http_get().await? || self.http_get().await? {
            return Ok(true);
        }
        // DAVRedirect::httpGet()
        self.response.status = Some(302);
        self.response.set_header("Location", self.ctx.config.url.clone());
        Ok(true)
    }

    // ------------------------------------------------------ protocol helpers

    /// `getAllowedMethods($path)`.
    pub async fn allowed_methods(&mut self, path: &str) -> DavResult<Vec<String>> {
        let mut methods: Vec<String> =
            ["OPTIONS", "GET", "HEAD", "DELETE", "PROPFIND", "PUT", "PROPPATCH", "COPY", "MOVE", "REPORT"]
                .iter()
                .map(|m| (*m).to_owned())
                .collect();

        // The MKCOL is only allowed on an unmapped uri
        match self.tree.node_for_path(self.ctx, path).await {
            Ok(_) => {}
            Err(e) if e.class == NOT_FOUND => methods.push("MKCOL".to_owned()),
            Err(e) => return Err(e),
        }

        // CalDAV\Plugin::getHTTPMethods(): MKCALENDAR on an unmapped uri of an extended collection.
        let (mut parent, name) = split(path);
        if path.is_empty() {
            parent = String::new();
        }
        let node = self.tree.node_for_path(self.ctx, &parent).await?;
        if node.is_extended_collection() {
            match node.child(self.ctx, &name).await {
                Ok(_) => {}
                Err(e) if e.class == NOT_FOUND => methods.push("MKCALENDAR".to_owned()),
                Err(e) => return Err(e),
            }
        }

        Ok(methods)
    }

    /// `getRequestUri()`.
    pub fn request_uri(&self) -> DavResult<String> {
        calculate_uri(&self.request.url)
    }

    /// `getHTTPDepth($default)`.
    pub fn http_depth(&self, default: i32) -> i32 {
        match self.request.header("Depth") {
            None => default,
            Some(d) if d == "infinity" => DEPTH_INFINITY,
            Some(d) if !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()) => d.parse().unwrap_or(default),
            Some(_) => default,
        }
    }

    /// `getHTTPPrefer()`.
    pub fn http_prefer(&self) -> Prefer {
        get_http_prefer(self.request.header("Prefer").as_deref(), self.request.header("Brief").as_deref())
    }

    /// `getProperties($path, $propertyNames)`: the found properties.
    pub async fn get_properties(&mut self, path: &str, names: &[String]) -> DavResult<Vec<(String, PropValue)>> {
        let result = self.get_properties_for_path(path, names, 0).await?;
        Ok(result
            .into_iter()
            .next()
            .and_then(|r| r.properties.into_iter().find(|(s, _)| *s == 200))
            .map(|(_, list)| list.into_iter().filter_map(|(n, v)| v.map(|v| (n, v))).collect())
            .unwrap_or_default())
    }

    /// `getHTTPHeaders($path)`.
    async fn http_headers(&mut self, path: &str) -> DavResult<Vec<(&'static str, String)>> {
        let map = [
            ("{DAV:}getcontenttype", "Content-Type"),
            ("{DAV:}getcontentlength", "Content-Length"),
            ("{DAV:}getlastmodified", "Last-Modified"),
            ("{DAV:}getetag", "ETag"),
        ];
        let names: Vec<String> = map.iter().map(|(p, _)| (*p).to_owned()).collect();
        let properties = self.get_properties(path, &names).await?;
        let mut headers = Vec::new();
        for (property, header) in map {
            match properties.iter().find(|(n, _)| n == property).map(|(_, v)| v) {
                Some(PropValue::Text(value)) => headers.push((header, value.clone())),
                Some(PropValue::GetLastModified(time)) => headers.push((header, to_date(*time))),
                _ => {}
            }
        }
        Ok(headers)
    }

    /// `getPropertiesForPath($path, $propertyNames, $depth)`.
    pub async fn get_properties_for_path(
        &mut self,
        path: &str,
        property_names: &[String],
        depth: i32,
    ) -> DavResult<Vec<PropFindResult>> {
        // The only two options for the depth of a propfind is 0 or 1 - as long as depth infinity is not enabled
        let depth = if depth != 0 { 1 } else { 0 };

        let path = path.trim_matches('/').to_owned();
        let all_props = property_names.is_empty();
        let prop_find = PropFind::new(&path, property_names, depth, all_props);

        let parent_node = self.tree.node_for_path(self.ctx, &path).await?;

        let mut requests = vec![(prop_find.clone(), parent_node.clone())];
        if depth > 0 && parent_node.is_collection() {
            for child in self.tree.children(self.ctx, &path).await? {
                let sub_path = if path.is_empty() { child.name() } else { format!("{path}/{}", child.name()) };
                let sub =
                    PropFind::new(&sub_path, prop_find.requested_properties(), depth - 1, prop_find.is_all_props());
                requests.push((sub, child));
            }
        }

        let mut results = Vec::new();
        for (mut prop_find, node) in requests {
            if self.get_properties_by_node(&mut prop_find, &node).await? {
                let mut href = prop_find.path().to_owned();
                // WebDAV recommends adding a slash to the path, if the path is a collection.
                let resource_type = node.resource_type();
                if resource_type.iter().any(|t| t == "{DAV:}collection" || t == "{DAV:}principal") {
                    href.push('/');
                }
                results.push(PropFindResult { href, properties: prop_find.result_for_multi_status() });
            }
        }
        Ok(results)
    }

    /// `getPropertiesForMultiplePaths($paths, $propertyNames)`.
    pub async fn get_properties_for_multiple_paths(
        &mut self,
        paths: &[String],
        property_names: &[String],
    ) -> DavResult<Vec<(String, PropFindResult)>> {
        let nodes = self.tree.multiple_nodes(self.ctx, paths).await?;
        let mut result: Vec<(String, PropFindResult)> = Vec::new();
        for (path, node) in nodes {
            let mut prop_find = PropFind::new(&path, property_names, 0, false);
            if self.get_properties_by_node(&mut prop_find, &node).await? {
                let mut href = path.clone();
                let resource_type = node.resource_type();
                if resource_type.iter().any(|t| t == "{DAV:}collection" || t == "{DAV:}principal") {
                    href.push('/');
                }
                result.push((path, PropFindResult { href, properties: prop_find.result_for_multi_status() }));
            }
        }
        Ok(result)
    }

    /// `getPropertiesByNode($propFind, $node)`: the `propFind` listeners, in
    /// the order sabre/event sorts them (by priority, then as
    /// `array_multisort()` leaves equal priorities).
    pub async fn get_properties_by_node(&mut self, prop_find: &mut PropFind, node: &Node) -> DavResult<bool> {
        if !self.acl_prop_find(prop_find, node).await? {
            return Ok(false);
        }
        self.caldav_prop_find(prop_find, node).await?;
        self.sync_prop_find(prop_find, node).await?;
        self.carddav_prop_find_early(prop_find, node);
        self.core_prop_find(prop_find, node).await?;
        self.core_prop_find_node(prop_find, node);
        self.carddav_prop_find_late(prop_find);
        self.core_prop_find_late(prop_find).await?;
        Ok(true)
    }

    /// `generateMultiStatus($fileProperties, $strip404s)` (not indented).
    pub fn generate_multi_status(&self, file_properties: Vec<PropFindResult>, strip_404s: bool) -> String {
        let mut w = Writer::new(BASE_URI);
        w.start_document();
        w.start_element("{DAV:}multistatus");
        for entry in file_properties {
            let mut properties = entry.properties;
            if strip_404s {
                properties.retain(|(s, _)| *s != 404);
            }
            let response =
                ResponseElement { href: entry.href.trim_start_matches('/').to_owned(), properties, http_status: None };
            w.start_element("{DAV:}response");
            response.serialize(&mut w);
            w.end_element();
        }
        w.end_element();
        w.end_document();
        w.output()
    }

    /// `$server->xml->write('{DAV:}multistatus', $responses, $baseUri)`
    /// (indented), with the sync token of a `MultiStatus`.
    pub fn write_multi_status(&self, responses: &[ResponseElement], sync_token: Option<&str>) -> String {
        let mut w = Writer::new(BASE_URI);
        w.set_indent();
        w.start_document();
        w.start_element("{DAV:}multistatus");
        for response in responses {
            w.start_element("{DAV:}response");
            response.serialize(&mut w);
            w.end_element();
        }
        if let Some(token) = sync_token {
            w.write_text_element("{DAV:}sync-token", token);
        }
        w.end_element();
        w.output()
    }

    /// `checkPreconditions($request, $response)`.
    async fn check_preconditions(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let mut node: Option<Node> = None;
        let mut etag: Option<String> = None;

        if let Some(if_match) = self.request.header("If-Match").filter(|h| !h.is_empty()) {
            // If-Match contains an entity tag. Only if the entity-tag
            // matches we are allowed to make the request succeed.
            let found = match self.tree.node_for_path(self.ctx, &path).await {
                Ok(n) => n,
                Err(e) if e.class == NOT_FOUND => {
                    return Err(DavError::precondition_failed(
                        "An If-Match header was specified and the resource did not exist",
                        "If-Match",
                    ));
                }
                Err(e) => return Err(e),
            };
            if if_match != "*" {
                let mut have_match = false;
                for item in if_match.split(',') {
                    let item = item.trim_matches(' ');
                    etag = if found.is_file() { found.etag() } else { None };
                    if etag.as_deref() == Some(item) || Some(item.replace("\\\"", "\"").as_str()) == etag.as_deref() {
                        have_match = true;
                    }
                }
                if !have_match {
                    if let Some(etag) = etag.as_deref().filter(|e| !e.is_empty()) {
                        self.response.set_header("ETag", etag);
                    }
                    return Err(DavError::precondition_failed(
                        "An If-Match header was specified, but none of the specified ETags matched.",
                        "If-Match",
                    ));
                }
            }
            node = Some(found);
        }

        let if_none_match = self.request.header("If-None-Match").filter(|h| !h.is_empty());
        if let Some(if_none_match) = &if_none_match {
            let mut node_exists = true;
            if node.is_none() {
                match self.tree.node_for_path(self.ctx, &path).await {
                    Ok(n) => node = Some(n),
                    Err(e) if e.class == NOT_FOUND => node_exists = false,
                    Err(e) => return Err(e),
                }
            }
            if node_exists {
                let found = node.as_ref().expect("the node exists");
                let mut have_match = false;
                if if_none_match == "*" {
                    have_match = true;
                } else {
                    etag = if found.is_file() { found.etag() } else { None };
                    for item in if_none_match.split(',') {
                        if etag.as_deref() == Some(item.trim_matches(' ')) {
                            have_match = true;
                        }
                    }
                }
                if have_match {
                    if let Some(etag) = etag.as_deref().filter(|e| !e.is_empty()) {
                        self.response.set_header("ETag", etag);
                    }
                    if self.request.method == "GET" {
                        self.response.status = Some(304);
                        return Ok(false);
                    }
                    return Err(DavError::precondition_failed(
                        "An If-None-Match header was specified, but the ETag matched (or * was specified).",
                        "If-None-Match",
                    ));
                }
            }
        }

        if if_none_match.is_none()
            && let Some(if_modified_since) = self.request.header("If-Modified-Since").filter(|h| !h.is_empty())
            && let Some(date) = parse_date(&if_modified_since)
        {
            if node.is_none() {
                node = Some(self.tree.node_for_path(self.ctx, &path).await?);
            }
            let last_mod = node.as_ref().expect("set above").last_modified(self.ctx).await?;
            if let Some(last_mod) = last_mod.filter(|t| *t != 0)
                && last_mod <= date
            {
                self.response.status = Some(304);
                self.response.set_header("Last-Modified", to_date(last_mod));
                return Ok(false);
            }
        }

        if let Some(if_unmodified_since) = self.request.header("If-Unmodified-Since").filter(|h| !h.is_empty())
            && let Some(date) = parse_date(&if_unmodified_since)
        {
            if node.is_none() {
                node = Some(self.tree.node_for_path(self.ctx, &path).await?);
            }
            let last_mod = node.as_ref().expect("set above").last_modified(self.ctx).await?;
            if let Some(last_mod) = last_mod.filter(|t| *t != 0)
                && last_mod > date
            {
                return Err(DavError::precondition_failed(
                    "An If-Unmodified-Since header was specified, but the entity has been changed since the specified date.",
                    "If-Unmodified-Since",
                ));
            }
        }

        // The If: header (lock and sync tokens).
        let mut if_conditions = self.if_conditions()?;
        self.sync_validate_tokens(&mut if_conditions).await?;
        for condition in &if_conditions {
            let mut ok = false;
            for token in &condition.tokens {
                let token_valid = token.valid_token || token.token.is_empty();
                let mut etag_valid = token.etag.is_empty();
                if !token.etag.is_empty() && token_valid {
                    let node = self.tree.node_for_path(self.ctx, &condition.uri).await?;
                    etag_valid = node.is_file() && node.etag().as_deref() == Some(token.etag.as_str());
                }
                if (token_valid && etag_valid) ^ token.negate {
                    ok = true;
                    break;
                }
            }
            if !ok {
                return Err(DavError::precondition_failed(
                    format!("Failed to find a valid token/etag combination for {}", condition.uri),
                    "If",
                ));
            }
        }

        Ok(true)
    }

    /// `getIfConditions($request)`.
    fn if_conditions(&self) -> DavResult<Vec<IfCondition>> {
        let Some(header) = self.request.header("If").filter(|h| !h.is_empty()) else {
            return Ok(Vec::new());
        };
        let mut conditions: Vec<IfCondition> = Vec::new();
        for m in parse_if_header(&header) {
            let token = IfToken { negate: m.not, token: m.token, etag: m.etag, valid_token: false };
            if m.uri.is_empty() && !conditions.is_empty() {
                conditions.last_mut().expect("not empty").tokens.push(token);
            } else {
                let uri = if m.uri.is_empty() { self.request.path()? } else { calculate_uri(&m.uri)? };
                conditions.push(IfCondition { uri, tokens: vec![token] });
            }
        }
        Ok(conditions)
    }

    // ------------------------------------------------------------ CorePlugin

    /// `CorePlugin::httpGet()`: `false` when the node isn't a file.
    async fn http_get(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let node = self.tree.node_for_path(self.ctx, &path).await?;

        if !node.is_file() {
            return Ok(false);
        }

        let body = if self.request.header("X-Sabre-Original-Method").as_deref() == Some("HEAD") {
            String::new()
        } else {
            node.get()
        };

        let mut http_headers = self.http_headers(&path).await?;

        // ContentType needs to get a default, because many webservers will otherwise
        // default to text/html, and we don't want this for security reasons.
        if !http_headers.iter().any(|(h, _)| *h == "Content-Type") {
            http_headers.push(("Content-Type", "application/octet-stream".to_owned()));
        }

        let node_size = http_headers.iter().find(|(h, _)| *h == "Content-Length").map(|(_, v)| v.clone());
        http_headers.retain(|(h, _)| *h != "Content-Length");

        for (header, value) in http_headers {
            self.response.add_header(header, value);
        }

        if let Some(size) = node_size.filter(|s| !s.is_empty() && s != "0") {
            self.response.set_header("Content-Length", size);
        }
        self.response.status = Some(200);
        self.response.body = Some(body);

        Ok(true)
    }

    /// `CorePlugin::httpOptions()`.
    async fn http_options(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let methods = self.allowed_methods(&path).await?;
        self.response.set_header("Allow", methods.join(", ").to_uppercase());
        self.response.set_header("DAV", features().join(", "));
        self.response.set_header("MS-Author-Via", "DAV");
        self.response.set_header("Accept-Ranges", "bytes");
        self.response.set_header("Content-Length", "0");
        self.response.status = Some(200);
        Ok(true)
    }

    /// `CorePlugin::httpHead()`: a `GET` sub-request without a body.
    async fn http_head(&mut self) -> DavResult<bool> {
        let original = self.request.clone();
        self.request.method = "GET".to_owned();
        self.request.headers.insert("x-sabre-original-method", axum::http::HeaderValue::from_static("HEAD"));
        let result = Box::pin(self.invoke_method()).await;
        self.request = original;
        match result {
            Ok(()) => {}
            // Some clients may do HEAD requests on collections, however, GET
            // requests and HEAD requests _may_ not be defined on a collection,
            // which would trigger a 501.
            Err(e) if e.class == "Sabre\\DAV\\Exception\\NotImplemented" => {
                self.response.status = Some(200);
                self.response.body = Some(String::new());
                self.response.set_header("Content-Type", "text/plain");
                self.response.set_header("X-Sabre-Real-Status", e.code.to_string());
            }
            Err(e) => return Err(e),
        }
        Ok(true)
    }

    /// `CorePlugin::httpDelete()`.
    async fn http_delete(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;

        self.acl_before_unbind(&path).await?;

        self.tree.delete(self.ctx, &path).await?;

        self.response.status = Some(204);
        self.response.set_header("Content-Length", "0");
        Ok(true)
    }

    /// `CorePlugin::httpPropFind()`.
    async fn http_prop_find(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let request_body = self.request.body.clone();
        let properties = if !request_body.is_empty() {
            let root = xml::parse(&request_body).map_err(|e| DavError::bad_request(e.0))?;
            if root.clark != "{DAV:}propfind" {
                return Err(DavError::bad_request(format!(
                    "Expected {{DAV:}}propfind but received {} as the root element",
                    root.clark
                )));
            }
            prop_find_request(&root)
        } else {
            Vec::new()
        };

        let mut depth = self.http_depth(1);
        // The only two options for the depth of a propfind is 0 or 1 - as long as depth infinity is not enabled
        if depth != 0 {
            depth = 1;
        }

        let new_properties = self.get_properties_for_path(&path, &properties, depth).await?;

        // This is a multi-status response
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.set_header("DAV", features().join(", "));

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        let data = self.generate_multi_status(new_properties, minimal);
        self.response.body = Some(data);
        Ok(true)
    }

    /// `CorePlugin::httpPropPatch()`.
    async fn http_prop_patch(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let root = xml::parse(&self.request.body).map_err(|e| DavError::bad_request(e.0))?;
        if root.clark != "{DAV:}propertyupdate" {
            return Err(DavError::bad_request(format!(
                "Expected {{DAV:}}propertyupdate but received {} as the root element",
                root.clark
            )));
        }
        let new_properties = prop_patch_request(&root)?;
        let result = self.update_properties(&path, new_properties).await?;

        let prefer = self.http_prefer();
        self.response.set_header("Vary", "Brief,Prefer");

        if prefer.return_.as_deref() == Some("minimal") {
            // If return-minimal is specified, we only have to check if the
            // request was successful, and don't need to return the multi-status.
            if result.iter().all(|(_, code)| *code <= 299) {
                self.response.status = Some(204);
                return Ok(true);
            }
        }

        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");

        // Reorganizing the result for generateMultiStatus
        let mut multi_status: PropStats = Vec::new();
        for (name, code) in result {
            match multi_status.iter_mut().find(|(c, _)| *c == code) {
                Some((_, list)) => list.push((name, None)),
                None => multi_status.push((code, vec![(name, None)])),
            }
        }
        let body = self.generate_multi_status(vec![PropFindResult { href: path, properties: multi_status }], false);
        self.response.body = Some(body);
        Ok(true)
    }

    /// `updateProperties($path, $properties)`.
    async fn update_properties(
        &mut self,
        path: &str,
        mutations: Vec<(String, Option<Element>)>,
    ) -> DavResult<Vec<(String, u16)>> {
        let mut prop_patch = PropPatch::new(Vec::new(), mutations);
        self.emit_prop_patch(path, &mut prop_patch).await?;
        self.commit_prop_patch(path, &mut prop_patch).await?;
        Ok(prop_patch.result)
    }

    /// The `propPatch` listeners: `CorePlugin`'s protected properties, the
    /// `DAVACL` group-member-set handler, then `CorePlugin`'s node update
    /// (the backends handle nothing).
    async fn emit_prop_patch(&mut self, path: &str, prop_patch: &mut PropPatch) -> DavResult<()> {
        // CorePlugin::propPatchProtectedPropertyCheck()
        let protected: Vec<String> = PROTECTED_PROPERTIES
            .iter()
            .filter(|p| prop_patch.mutations.iter().any(|(n, _)| n == *p))
            .map(|p| (*p).to_owned())
            .collect();
        if !protected.is_empty() {
            prop_patch.set_result_code(&protected, 403);
        }

        // DAVACL\Plugin::propPatch(): handle('{DAV:}group-member-set', …)
        if prop_patch.mutations.iter().any(|(n, _)| n == GROUP_MEMBER_SET)
            && !prop_patch.result.iter().any(|(n, _)| n == GROUP_MEMBER_SET)
        {
            prop_patch.result.push((GROUP_MEMBER_SET.to_owned(), 202));
            prop_patch.handles_members = true;
        }

        // CorePlugin::propPatchNodeUpdate(): this should trigger a 404 if the node doesn't exist.
        self.tree.node_for_path(self.ctx, path).await?;
        Ok(())
    }

    /// `PropPatch::commit()`: `false` when a property failed.
    async fn commit_prop_patch(&mut self, path: &str, prop_patch: &mut PropPatch) -> DavResult<bool> {
        // First we validate if every property has a handler
        let unhandled: Vec<String> = prop_patch
            .mutations
            .iter()
            .filter(|(n, _)| !prop_patch.result.iter().any(|(r, _)| r == n))
            .map(|(n, _)| n.clone())
            .collect();
        for name in unhandled {
            prop_patch.failed = true;
            prop_patch.result.push((name, 403));
        }

        if prop_patch.handles_members && !prop_patch.failed {
            // `Href::xmlDeserialize()` is `null` without hrefs.
            let hrefs = prop_patch
                .mutations
                .iter()
                .find(|(n, _)| n == GROUP_MEMBER_SET)
                .and_then(|(_, v)| v.as_ref())
                .map(href_values)
                .unwrap_or_default();
            for href in &hrefs {
                calculate_uri(href)?;
            }
            let node = self.tree.node_for_path(self.ctx, path).await?;
            let code = if node.is_principal() {
                // PrincipalBackend::setGroupMemberSet() does nothing.
                self.principal_membership_cache.clear();
                if hrefs.is_empty() { 204 } else { 200 }
            } else {
                403
            };
            prop_patch.set_result_code(&[GROUP_MEMBER_SET.to_owned()], code);
        }

        // If anywhere in this operation updating a property failed, we must update all other properties accordingly.
        if prop_patch.failed {
            for entry in prop_patch.result.iter_mut().filter(|(_, code)| *code == 202) {
                entry.1 = 424;
            }
        }
        Ok(!prop_patch.failed)
    }

    /// `CorePlugin::httpMkcol()`.
    async fn http_mkcol(&mut self) -> DavResult<bool> {
        let request_body = self.request.body.clone();
        let path = self.request.path()?;

        let (resource_type, properties) = if !request_body.is_empty() {
            let content_type = self.request.header("Content-Type");
            if content_type.as_deref().is_none_or(|c| !c.starts_with("application/xml") && !c.starts_with("text/xml")) {
                // We must throw 415 for unsupported mkcol bodies
                return Err(DavError::unsupported_media_type(
                    "The request body for the MKCOL request must have an xml Content-Type",
                ));
            }

            let root = xml::parse(&request_body).map_err(|e| DavError::bad_request(e.0))?;
            if root.clark != "{DAV:}mkcol" {
                return Err(DavError::bad_request(format!(
                    "Expected {{DAV:}}mkcol but received {} as the root element",
                    root.clark
                )));
            }
            let mut properties = set_properties(&root)?;

            let Some(i) = properties.iter().position(|(n, _)| n == "{DAV:}resourcetype") else {
                return Err(DavError::bad_request("The mkcol request must include a {DAV:}resourcetype property"));
            };
            let resource_type = properties[i].1.as_ref().map(Element::enumeration).unwrap_or_default();
            properties.remove(i);
            (resource_type, properties)
        } else {
            (vec!["{DAV:}collection".to_owned()], Vec::new())
        };

        let result = self.create_collection(&path, PropPatch::new(resource_type, properties)).await?;

        match result {
            Some(result) => {
                self.response.status = Some(207);
                self.response.set_header("Content-Type", "application/xml; charset=utf-8");
                self.response.body = Some(self.generate_multi_status(vec![result], false));
            }
            None => {
                self.response.set_header("Content-Length", "0");
                self.response.status = Some(201);
            }
        }
        Ok(true)
    }

    /// `createCollection($uri, $mkCol)`: the multistatus row of a failed `MkCol`.
    pub async fn create_collection(&mut self, uri: &str, mut mk_col: PropPatch) -> DavResult<Option<PropFindResult>> {
        let (parent_uri, new_name) = split(uri);

        // Making sure the parent exists
        let parent = match self.tree.node_for_path(self.ctx, &parent_uri).await {
            Ok(parent) => parent,
            Err(e) if e.class == NOT_FOUND => return Err(DavError::conflict("Parent node does not exist")),
            Err(e) => return Err(e),
        };

        // Making sure the parent is a collection
        if !parent.is_collection() {
            return Err(DavError::conflict("Parent node is not a collection"));
        }

        // Making sure the child does not already exist
        match parent.child(self.ctx, &new_name).await {
            Ok(_) => return Err(DavError::method_not_allowed("The resource you tried to create already exists")),
            Err(e) if e.class == NOT_FOUND => {}
            Err(e) => return Err(e),
        }

        self.acl_before_bind(uri).await?;

        if parent.is_extended_collection() {
            // The parent can store the properties of the MkCol.
            self.create_extended_collection(&parent, &mut mk_col)?;
        } else {
            // Only 'standard' collections can be created there.
            if mk_col.resource_type.len() > 1 {
                return Err(DavError::invalid_resource_type(
                    "The {DAV:}resourcetype you specified is not supported here.",
                ));
            }
            parent.create_directory()?;
        }

        // If there are any properties that have not been handled/stored,
        // we ask the 'propPatch' event to handle them.
        if !mk_col.remaining_mutations().is_empty() {
            self.emit_prop_patch(uri, &mut mk_col).await?;
        }
        let success = self.commit_prop_patch(uri, &mut mk_col).await?;
        if !success {
            let mut formatted: PropStats = Vec::new();
            for (name, status) in mk_col.result {
                match formatted.iter_mut().find(|(s, _)| *s == status) {
                    Some((_, list)) => list.push((name, None)),
                    None => formatted.push((status, vec![(name, None)])),
                }
            }
            return Ok(Some(PropFindResult { href: uri.to_owned(), properties: formatted }));
        }

        self.tree.mark_dirty(&parent_uri);
        Ok(None)
    }

    /// `createExtendedCollection($name, $mkCol)` of the principal collection
    /// and the homes (whose backends create nothing).
    fn create_extended_collection(&mut self, parent: &Node, mk_col: &mut PropPatch) -> DavResult<()> {
        match parent {
            Node::PrincipalCollection => {
                if !mk_col.has_resource_type(&["{DAV:}principal"]) {
                    return Err(DavError::invalid_resource_type(
                        "Only resources of type {DAV:}principal may be created here",
                    ));
                }
                Err(DavError::internal(
                    "Error",
                    "Call to undefined method App\\Domains\\Contact\\Dav\\Web\\DAVACL\\PrincipalBackend::createPrincipal()",
                ))
            }
            Node::AddressBookHome { .. } => {
                if !mk_col.has_resource_type(&["{urn:ietf:params:xml:ns:carddav}addressbook"]) {
                    return Err(DavError::invalid_resource_type("Unknown resourceType for this collection"));
                }
                mk_col.set_remaining_result_code(201);
                // CardDAVBackend::createAddressBook() returns false.
                Ok(())
            }
            _ => {
                let mut is_calendar = false;
                let mut is_subscription = false;
                for rt in &mk_col.resource_type {
                    match rt.as_str() {
                        "{DAV:}collection" | "{http://calendarserver.org/ns/}shared-owner" => {}
                        "{urn:ietf:params:xml:ns:caldav}calendar" => is_calendar = true,
                        "{http://calendarserver.org/ns/}subscribed" => is_subscription = true,
                        _ => return Err(DavError::invalid_resource_type(format!("Unknown resourceType: {rt}"))),
                    }
                }
                mk_col.set_remaining_result_code(201);
                if is_subscription {
                    Err(DavError::invalid_resource_type("This backend does not support subscriptions"))
                } else if is_calendar {
                    // CalDAVBackend::createCalendar() does nothing.
                    Ok(())
                } else {
                    Err(DavError::invalid_resource_type(
                        "You can only create calendars and subscriptions in this collection",
                    ))
                }
            }
        }
    }

    /// `getCopyAndMoveInfo($request)`.
    async fn copy_and_move_info(&mut self) -> DavResult<(String, bool)> {
        // Collecting the relevant HTTP headers
        let Some(destination) = self.request.header("Destination").filter(|d| !d.is_empty()) else {
            return Err(DavError::bad_request("The destination header was not supplied"));
        };
        let destination = calculate_uri(&destination)?;
        let overwrite = self.request.header("Overwrite").filter(|o| !o.is_empty()).unwrap_or_else(|| "T".to_owned());
        let overwrite = match overwrite.to_uppercase().as_str() {
            "T" => true,
            "F" => false,
            // We need to throw a bad request exception, if the header was invalid
            _ => return Err(DavError::bad_request("The HTTP Overwrite header should be either T or F")),
        };

        let (destination_dir, _) = split(&destination);

        match self.tree.node_for_path(self.ctx, &destination_dir).await {
            Ok(parent) if !parent.is_collection() => {
                return Err(DavError::unsupported_media_type("The destination node is not a collection"));
            }
            Ok(_) => {}
            // If the destination parent node is not found, we throw a 409
            Err(e) if e.class == NOT_FOUND => return Err(DavError::conflict("The destination node is not found")),
            Err(e) => return Err(e),
        }

        let destination_exists = match self.tree.node_for_path(self.ctx, &destination).await {
            // If this succeeded, it means the destination already exists
            // we'll need to throw precondition failed in case overwrite is false
            Ok(_) if !overwrite => {
                return Err(DavError::precondition_failed(
                    "The destination node already exists, and the overwrite header is set to false",
                    "Overwrite",
                ));
            }
            Ok(_) => true,
            // Destination didn't exist, we're all good
            Err(e) if e.class == NOT_FOUND => false,
            Err(e) => return Err(e),
        };

        let request_path = self.request.path()?;
        if destination == request_path {
            return Err(DavError::forbidden("Source and destination uri are identical."));
        }
        if destination.starts_with(&format!("{request_path}/")) {
            return Err(DavError::conflict("The destination may not be part of the same subtree as the source path."));
        }

        Ok((destination, destination_exists))
    }

    /// `CorePlugin::httpMove()`.
    async fn http_move(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;

        let (destination, destination_exists) = self.copy_and_move_info().await?;

        if destination_exists {
            self.acl_before_unbind(&destination).await?;
        }
        self.acl_before_unbind(&path).await?;
        self.acl_before_bind(&destination).await?;

        if destination_exists {
            self.tree.delete(self.ctx, &destination).await?;
        }

        self.tree.move_node(self.ctx, &path, &destination).await?;

        // If a resource was overwritten we should send a 204, otherwise a 201
        self.response.set_header("Content-Length", "0");
        self.response.status = Some(if destination_exists { 204 } else { 201 });
        Ok(true)
    }

    /// `CorePlugin::httpCopy()`.
    async fn http_copy(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;

        let (destination, destination_exists) = self.copy_and_move_info().await?;

        self.acl_before_bind(&destination).await?;
        if destination_exists {
            self.acl_before_unbind(&destination).await?;
            self.tree.delete(self.ctx, &destination).await?;
        }

        self.tree.copy(self.ctx, &path, &destination).await?;

        // If a resource was overwritten we should send a 204, otherwise a 201
        self.response.set_header("Content-Length", "0");
        self.response.status = Some(if destination_exists { 204 } else { 201 });
        Ok(true)
    }

    /// `CorePlugin::httpPut()`.
    async fn http_put(&mut self) -> DavResult<bool> {
        let body = self.request.body.clone();
        let path = self.request.path()?;

        // Intercepting Content-Range
        if self.request.header("Content-Range").is_some_and(|h| !h.is_empty()) {
            return Err(DavError::bad_request("Content-Range on PUT requests are forbidden."));
        }

        // Intercepting the Finder problem
        if let Some(expected) = self.request.header("X-Expected-Entity-Length")
            && backend_php_intval(&expected) > 0
            && body.is_empty()
        {
            return Err(DavError::forbidden(
                "This server is not compatible with OS/X finder. Consider using a different WebDAV client or webserver.",
            ));
        }

        if self.tree.node_exists(self.ctx, &path).await? {
            let node = self.tree.node_for_path(self.ctx, &path).await?;

            // If the node is a collection, we'll deny it
            if !node.is_file() {
                return Err(DavError::conflict("PUT is not allowed on non-files."));
            }
            let etag = self.update_file(&path, node, body).await?;

            self.response.set_header("Content-Length", "0");
            if let Some(etag) = etag.filter(|e| !e.is_empty()) {
                self.response.set_header("ETag", etag);
            }
            self.response.status = Some(204);
        } else {
            // If we got here, the resource didn't exist yet.
            let etag = self.create_file(&path, body).await?;

            self.response.set_header("Content-Length", "0");
            if let Some(etag) = etag.filter(|e| !e.is_empty()) {
                self.response.set_header("ETag", etag);
            }
            self.response.status = Some(201);
        }
        Ok(true)
    }

    /// `createFile($uri, $data, &$etag)`.
    async fn create_file(&mut self, uri: &str, data: String) -> DavResult<Option<String>> {
        let (dir, name) = split(uri);

        self.acl_before_bind(uri).await?;

        let parent = match self.tree.node_for_path(self.ctx, &dir).await {
            Ok(parent) => parent,
            Err(e) if e.class == NOT_FOUND => {
                return Err(DavError::conflict("Files cannot be created in non-existent collections"));
            }
            Err(e) => return Err(e),
        };

        if !parent.is_collection() {
            return Err(DavError::conflict("Files can only be created as children of collections"));
        }

        // It is possible for an event handler to modify the content of the
        // body, before it gets written. If this is the case, $modified
        // should be set to true.
        let mut data = data;
        let mut modified = false;
        if matches!(parent, Node::AddressBook(_)) {
            self.validate_vcard(&mut data, &mut modified)?;
        }
        if matches!(parent, Node::Calendar(_)) {
            self.validate_icalendar(&mut data, uri, &mut modified).await?;
        }

        let mut etag = parent.create_file(self.ctx, &name, &data).await?;
        if modified {
            etag = None;
        }

        self.tree.mark_dirty(&format!("{dir}/{name}"));
        Ok(etag)
    }

    /// `updateFile($uri, $data, &$etag)`.
    async fn update_file(&mut self, uri: &str, node: Node, data: String) -> DavResult<Option<String>> {
        let mut data = data;
        let mut modified = false;
        if matches!(node, Node::Card { .. }) {
            self.validate_vcard(&mut data, &mut modified)?;
        }
        if matches!(node, Node::CalendarObject { .. }) {
            let (parent, _) = split(uri);
            let parent_node = self.tree.node_for_path(self.ctx, &parent).await?;
            if matches!(parent_node, Node::Calendar(_)) {
                self.validate_icalendar(&mut data, uri, &mut modified).await?;
            }
        }

        let mut etag = node.put(self.ctx, &data).await?;
        if modified {
            etag = None;
        }
        Ok(etag)
    }

    /// `CorePlugin::httpReport()`.
    async fn http_report(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let report = xml::parse(&self.request.body)?;

        match report.clark.as_str() {
            "{DAV:}principal-property-search"
            | "{DAV:}principal-search-property-set"
            | "{DAV:}expand-property"
            | "{DAV:}principal-match"
            | "{DAV:}acl-principal-prop-set" => self.acl_report(&report, &path).await?,
            "{urn:ietf:params:xml:ns:caldav}calendar-multiget"
            | "{urn:ietf:params:xml:ns:caldav}calendar-query"
            | "{urn:ietf:params:xml:ns:caldav}free-busy-query" => self.caldav_report(&report).await?,
            "{urn:ietf:params:xml:ns:carddav}addressbook-multiget"
            | "{urn:ietf:params:xml:ns:carddav}addressbook-query" => self.carddav_report(&report).await?,
            "{DAV:}sync-collection" => self.sync_collection(&path, &report).await?,
            // If emit returned true, it means the report was not supported
            _ => return Err(DavError::report_not_supported("")),
        }
        Ok(true)
    }

    /// `CorePlugin::propFind()`.
    async fn core_prop_find(&mut self, prop_find: &mut PropFind, node: &Node) -> DavResult<()> {
        if prop_find.wants("{DAV:}getlastmodified") {
            let lm = node.last_modified(self.ctx).await?;
            prop_find.handle("{DAV:}getlastmodified", lm.filter(|t| *t != 0).map(PropValue::GetLastModified));
        }

        if node.is_file() {
            prop_find.handle("{DAV:}getcontentlength", Some(PropValue::Text(node.size().to_string())));
            prop_find.handle("{DAV:}getetag", node.etag().map(PropValue::Text));
            prop_find.handle("{DAV:}getcontenttype", node.content_type().map(PropValue::Text));
        }

        if prop_find.wants("{DAV:}supported-report-set") {
            let reports = self.supported_report_set(prop_find.path()).await?;
            prop_find.handle("{DAV:}supported-report-set", Some(PropValue::SupportedReportSet(reports)));
        }
        prop_find.handle("{DAV:}resourcetype", Some(PropValue::ResourceType(node.resource_type())));
        if prop_find.wants("{DAV:}supported-method-set") {
            let methods = self.allowed_methods(prop_find.path()).await?;
            prop_find.handle("{DAV:}supported-method-set", Some(PropValue::SupportedMethodSet(methods)));
        }
        Ok(())
    }

    /// The plugins' `getSupportedReportSet($uri)`: CardDAV, CalDAV, Sync, ACL.
    async fn supported_report_set(&mut self, uri: &str) -> DavResult<Vec<String>> {
        let node = self.tree.node_for_path(self.ctx, uri).await?;
        let mut reports = Vec::new();

        // CardDAV\Plugin
        if matches!(node, Node::AddressBook(_) | Node::Card { .. }) {
            reports.push("{urn:ietf:params:xml:ns:carddav}addressbook-multiget".to_owned());
            reports.push("{urn:ietf:params:xml:ns:carddav}addressbook-query".to_owned());
        }

        // CalDAV\Plugin
        if matches!(node, Node::Calendar(_) | Node::CalendarObject { .. }) {
            reports.push("{urn:ietf:params:xml:ns:caldav}calendar-multiget".to_owned());
            reports.push("{urn:ietf:params:xml:ns:caldav}calendar-query".to_owned());
        }
        if matches!(node, Node::Calendar(_)) {
            reports.push("{urn:ietf:params:xml:ns:caldav}free-busy-query".to_owned());
        }
        // iCal has a bug where it assumes that sync support is enabled, only
        // if we say we support it on the calendar-home.
        if matches!(node, Node::CalendarHome { .. }) {
            reports.push("{DAV:}sync-collection".to_owned());
        }

        // Sync\Plugin
        if node.is_sync_collection() && node.sync_token(self.ctx).await?.is_some_and(|t| !t.is_empty() && t != "0") {
            reports.push("{DAV:}sync-collection".to_owned());
        }

        // DAVACL\Plugin
        reports.extend(
            [
                "{DAV:}expand-property",
                "{DAV:}principal-match",
                "{DAV:}principal-property-search",
                "{DAV:}principal-search-property-set",
            ]
            .iter()
            .map(|r| (*r).to_owned()),
        );
        Ok(reports)
    }

    /// `CorePlugin::propFindNode()`.
    fn core_prop_find_node(&mut self, prop_find: &mut PropFind, node: &Node) {
        if !node.is_properties() {
            return;
        }
        let property_names = prop_find.properties_404();
        if property_names.is_empty() {
            return;
        }
        for (name, value) in node.properties(&property_names) {
            prop_find.set(&name, Some(value), Some(200));
        }
    }

    /// `CorePlugin::propFindLate()`: `getctag` from a sync token.
    async fn core_prop_find_late(&mut self, prop_find: &mut PropFind) -> DavResult<()> {
        let name = "{http://calendarserver.org/ns/}getctag";
        if !prop_find.wants(name) {
            return Ok(());
        }
        // If we already have a sync-token from the current propFind request, we can re-use that.
        let mut value = prop_find.get("{http://sabredav.org/ns}sync-token").cloned();
        if value.as_ref().is_none_or(|v| !truthy(v)) {
            value = match prop_find.get("{DAV:}sync-token") {
                Some(PropValue::Text(t)) if !t.is_empty() && t != "0" => Some(PropValue::Text(t.clone())),
                Some(PropValue::Href(hrefs)) if !hrefs.is_empty() => Some(PropValue::Text(
                    hrefs[0].get(backend::SYNCTOKEN_PREFIX.len()..).unwrap_or_default().to_owned(),
                )),
                _ => None,
            };
        }
        if value.is_none() {
            // If we got here, the earlier two properties may simply not have
            // been part of the earlier request. We're going to fetch them.
            let path = prop_find.path().to_owned();
            let names = ["{http://sabredav.org/ns}sync-token".to_owned(), "{DAV:}sync-token".to_owned()];
            let result = Box::pin(self.get_properties(&path, &names)).await?;
            let get = |n: &str| result.iter().find(|(name, _)| name == n).map(|(_, v)| v.clone());
            value = match (get("{http://sabredav.org/ns}sync-token"), get("{DAV:}sync-token")) {
                (Some(v), _) => Some(v),
                (None, Some(PropValue::Text(t))) => Some(PropValue::Text(t)),
                (None, Some(PropValue::Href(hrefs))) if !hrefs.is_empty() => Some(PropValue::Text(
                    hrefs[0].get(backend::SYNCTOKEN_PREFIX.len()..).unwrap_or_default().to_owned(),
                )),
                _ => None,
            };
        }
        prop_find.handle(name, value);
        Ok(())
    }
}

/// A value PHP finds truthy.
fn truthy(value: &PropValue) -> bool {
    match value {
        PropValue::Text(t) => !t.is_empty() && t != "0",
        _ => true,
    }
}

/// `(int)` of a header.
fn backend_php_intval(value: &str) -> i64 {
    let digits: String = value.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

/// The features of the `DAV` header: the core ones, then the plugins'.
fn features() -> Vec<&'static str> {
    vec![
        "1",
        "3",
        "extended-mkcol",
        "addressbook",
        "calendar-access",
        "calendar-proxy",
        "access-control",
        "calendarserver-principal-property-search",
    ]
}

/// `calculateUri($uri)`.
pub fn calculate_uri(uri: &str) -> DavResult<String> {
    let mut uri = uri.to_owned();
    if !uri.is_empty() && !uri.starts_with('/') && uri.find("://").is_some_and(|i| i > 0) {
        uri = url::Url::parse(&uri).map(|u| u.path().to_owned()).unwrap_or_default();
    }
    let mut collapsed = String::with_capacity(uri.len());
    for c in uri.chars() {
        if !(c == '/' && collapsed.ends_with('/')) {
            collapsed.push(c);
        }
    }
    let uri = normalize(&collapsed);
    let base_uri = normalize(BASE_URI);

    if let Some(rest) = uri.strip_prefix(&base_uri) {
        Ok(decode_path(rest).trim_matches('/').to_owned())
    } else if format!("{uri}/") == base_uri {
        // A special case, if the baseUri was accessed without a trailing slash, we'll accept it as well.
        Ok(String::new())
    } else {
        Err(DavError::forbidden(format!("Requested uri ({uri}) is out of base uri ({BASE_URI})")))
    }
}

/// `Xml\Request\PropFind`: the requested properties (none for `allprop`).
fn prop_find_request(root: &Element) -> Vec<String> {
    let mut properties = Vec::new();
    for (name, value) in root.key_value() {
        if name == "{DAV:}prop" {
            properties = value.enumeration();
        }
    }
    properties
}

/// `Xml\Request\PropPatch`: the properties to set (with their value) or
/// remove (`None`).
fn prop_patch_request(root: &Element) -> DavResult<Vec<(String, Option<Element>)>> {
    let mut properties: Vec<(String, Option<Element>)> = Vec::new();
    let mut put = |name: &str, value: Option<Element>| match properties.iter_mut().find(|(n, _)| n == name) {
        Some(entry) => entry.1 = value,
        None => properties.push((name.to_owned(), value)),
    };
    for elem in root.elements() {
        if elem.clark != "{DAV:}set" && elem.clark != "{DAV:}remove" {
            continue;
        }
        let Some((_, prop)) = elem.key_value().into_iter().find(|(n, _)| *n == "{DAV:}prop") else {
            return Err(DavError::internal("ErrorException", "Undefined array key \"{DAV:}prop\""));
        };
        for value in prop.elements() {
            if elem.clark == "{DAV:}set" && !value.children.is_empty() {
                put(&value.clark, Some(value.clone()));
            } else {
                put(&value.clark, None);
            }
        }
    }
    Ok(properties)
}

/// `Xml\Request\MkCol` (and `MkCalendar`): the properties of its `{DAV:}set`s.
pub fn set_properties(root: &Element) -> DavResult<Vec<(String, Option<Element>)>> {
    let mut properties: Vec<(String, Option<Element>)> = Vec::new();
    for elem in root.elements().filter(|e| e.clark == "{DAV:}set") {
        let Some((_, prop)) = elem.key_value().into_iter().find(|(n, _)| *n == "{DAV:}prop") else {
            return Err(DavError::internal("ErrorException", "Undefined array key \"{DAV:}prop\""));
        };
        for value in prop.elements() {
            // A `{DAV:}resourcetype` is a `ResourceType`; another empty property is `null`.
            let keep = !value.children.is_empty() || value.clark == "{DAV:}resourcetype";
            let parsed = keep.then(|| value.clone());
            match properties.iter_mut().find(|(n, _)| *n == value.clark) {
                Some(entry) => entry.1 = parsed,
                None => properties.push((value.clark.clone(), parsed)),
            }
        }
    }
    Ok(properties)
}

/// `Href::xmlDeserialize()`: the `{DAV:}href` values.
pub fn href_values(element: &Element) -> Vec<String> {
    element.elements().filter(|e| e.clark == "{DAV:}href").filter_map(Element::text).collect()
}

/// A condition of the `If` header.
#[derive(Debug, Clone)]
pub struct IfCondition {
    pub uri: String,
    pub tokens: Vec<IfToken>,
}

/// A token of an `If` condition.
#[derive(Debug, Clone)]
pub struct IfToken {
    pub negate: bool,
    pub token: String,
    pub etag: String,
    pub valid_token: bool,
}

/// A match of `getIfConditions()`'s regex.
struct IfMatch {
    uri: String,
    not: bool,
    token: String,
    etag: String,
}

/// `(?:\<(?P<uri>.*?)\>\s)?\((?P<not>Not\s)?(?:\<(?P<token>[^\>]*)\>)?(?:\s?)(?:\[(?P<etag>[^\]]*)\])?\)`
/// (case-insensitive), all matches.
fn parse_if_header(header: &str) -> Vec<IfMatch> {
    let mut matches = Vec::new();
    let mut i = 0;
    while i < header.len() {
        if !header.is_char_boundary(i) {
            i += 1;
            continue;
        }
        match match_if_at(header, i) {
            Some((m, end)) => {
                matches.push(m);
                i = end;
            }
            None => i += 1,
        }
    }
    matches
}

fn match_if_at(header: &str, start: usize) -> Option<(IfMatch, usize)> {
    let rest = &header[start..];
    // The optional uri: "<…> " then "(".
    let mut uri = String::new();
    let mut pos = 0;
    if let Some(after) = rest.strip_prefix('<')
        && let Some(close) = after.find('>')
        && after[close + 1..].starts_with(|c: char| c.is_whitespace())
    {
        let ws_len = after[close + 1..].chars().next().map(char::len_utf8).unwrap_or(1);
        if after[close + 1 + ws_len..].starts_with('(') {
            uri = after[..close].to_owned();
            pos = 1 + close + 1 + ws_len;
        }
    }
    let body = rest[pos..].strip_prefix('(')?;
    let mut cursor = 0;
    let mut not = false;
    if body.len() >= 4 && body[..3].eq_ignore_ascii_case("not") && body[3..].starts_with(|c: char| c.is_whitespace()) {
        not = true;
        cursor = 4;
    }
    let mut token = String::new();
    if let Some(after) = body[cursor..].strip_prefix('<')
        && let Some(close) = after.find('>')
    {
        token = after[..close].to_owned();
        cursor += 1 + close + 1;
    }
    if body[cursor..].starts_with(|c: char| c.is_whitespace()) {
        cursor += 1;
    }
    let mut etag = String::new();
    if let Some(after) = body[cursor..].strip_prefix('[')
        && let Some(close) = after.find(']')
    {
        etag = after[..close].to_owned();
        cursor += 1 + close + 1;
    }
    if !body[cursor..].starts_with(')') {
        return None;
    }
    let end = start + pos + 1 + cursor + 1;
    Some((IfMatch { uri, not, token, etag }, end))
}
