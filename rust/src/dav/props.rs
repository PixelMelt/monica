//! The property values the server writes (`Sabre\DAV\Xml\Property\*`,
//! `Sabre\CardDAV\Xml\Property\*`, `Sabre\CalDAV\Xml\Property\*`,
//! `Sabre\DAVACL\Xml\Property\*`) and the `{DAV:}response` element
//! (`Sabre\DAV\Xml\Element\Response`).

use super::http::{encode_path, resolve, status_text, to_date};
use super::xml::{NS_CALDAV, NS_CALENDARSERVER, NS_CARDDAV, Writer};

/// The principal of a `DAVACL\Xml\Property\Principal`.
#[derive(Debug, Clone, PartialEq)]
pub enum PrincipalKind {
    Unauthenticated,
    Authenticated,
    Href(String),
    All,
}

/// An access control entry (`getACL()`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ace {
    pub privilege: String,
    pub principal: String,
    pub protected: bool,
}

/// A privilege of `getSupportedPrivilegeSet()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Privilege {
    pub name: String,
    pub abstract_: bool,
    pub aggregates: Vec<Privilege>,
}

/// A property value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    /// A scalar.
    Text(String),
    /// `Href` (a `LocalHref` is an `Href` of encoded paths).
    Href(Vec<String>),
    ResourceType(Vec<String>),
    SupportedReportSet(Vec<String>),
    SupportedMethodSet(Vec<String>),
    /// `GetLastModified`: a timestamp.
    GetLastModified(i64),
    SupportedAddressData,
    /// `CardDAV\Xml\Property\SupportedCollationSet`.
    CardSupportedCollationSet,
    /// `CalDAV\Xml\Property\SupportedCollationSet`.
    CalSupportedCollationSet,
    SupportedCalendarData,
    SupportedCalendarComponentSet(Vec<String>),
    ScheduleCalendarTransp(String),
    EmailAddressSet(Vec<String>),
    Principal(PrincipalKind),
    CurrentUserPrivilegeSet(Vec<String>),
    Acl(Vec<Ace>),
    AclRestrictions,
    SupportedPrivilegeSet(Vec<Privilege>),
    /// The expanded hrefs of the expand-property report (`{DAV:}response` elements).
    Responses(Vec<ResponseElement>),
}

impl PropValue {
    /// `new LocalHref($hrefs)`.
    pub fn local_href(hrefs: Vec<String>) -> PropValue {
        PropValue::Href(hrefs.iter().map(|h| encode_path(h)).collect())
    }

    /// The scalar value (`is_scalar()`).
    pub fn as_text(&self) -> Option<&str> {
        match self {
            PropValue::Text(t) => Some(t),
            _ => None,
        }
    }

    /// `xmlSerialize($writer)` (or the text of a scalar).
    pub fn serialize(&self, w: &mut Writer) {
        match self {
            PropValue::Text(t) => w.text(t),
            PropValue::Href(hrefs) => {
                for href in hrefs {
                    let href = resolve(&w.context_uri.clone(), href);
                    w.write_text_element("{DAV:}href", &href);
                }
            }
            PropValue::ResourceType(types) => {
                for t in types {
                    w.write_empty(t);
                }
            }
            PropValue::SupportedReportSet(reports) => {
                for report in reports {
                    w.start_element("{DAV:}supported-report");
                    w.start_element("{DAV:}report");
                    w.write_empty(report);
                    w.end_element();
                    w.end_element();
                }
            }
            PropValue::SupportedMethodSet(methods) => {
                for method in methods {
                    w.start_element("{DAV:}supported-method");
                    w.write_attribute("name", method);
                    w.end_element();
                }
            }
            PropValue::GetLastModified(time) => w.text(&to_date(*time)),
            PropValue::SupportedAddressData => {
                for (content_type, version) in
                    [("text/vcard", "3.0"), ("text/vcard", "4.0"), ("application/vcard+json", "4.0")]
                {
                    w.start_element(&format!("{{{NS_CARDDAV}}}address-data-type"));
                    w.write_attribute("content-type", content_type);
                    w.write_attribute("version", version);
                    w.end_element();
                }
            }
            PropValue::CardSupportedCollationSet => {
                for coll in ["i;ascii-casemap", "i;octet", "i;unicode-casemap"] {
                    w.write_text_element(&format!("{{{NS_CARDDAV}}}supported-collation"), coll);
                }
            }
            PropValue::CalSupportedCollationSet => {
                for coll in ["i;ascii-casemap", "i;octet", "i;unicode-casemap"] {
                    w.write_text_element(&format!("{{{NS_CALDAV}}}supported-collation"), coll);
                }
            }
            PropValue::SupportedCalendarData => {
                w.start_element(&format!("{{{NS_CALDAV}}}calendar-data"));
                w.write_attribute("content-type", "text/calendar");
                w.write_attribute("version", "2.0");
                w.end_element();
                w.start_element(&format!("{{{NS_CALDAV}}}calendar-data"));
                w.write_attribute("content-type", "application/calendar+json");
                w.end_element();
            }
            PropValue::SupportedCalendarComponentSet(components) => {
                for component in components {
                    w.start_element(&format!("{{{NS_CALDAV}}}comp"));
                    w.write_attribute("name", component);
                    w.end_element();
                }
            }
            PropValue::ScheduleCalendarTransp(value) => match value.as_str() {
                "transparent" => w.write_empty(&format!("{{{NS_CALDAV}}}transparent")),
                _ => w.write_empty(&format!("{{{NS_CALDAV}}}opaque")),
            },
            PropValue::EmailAddressSet(emails) => {
                for email in emails {
                    w.write_text_element(&format!("{{{NS_CALENDARSERVER}}}email-address"), email);
                }
            }
            PropValue::Principal(kind) => serialize_principal(w, kind),
            PropValue::CurrentUserPrivilegeSet(privileges) => {
                for privilege in privileges {
                    w.start_element("{DAV:}privilege");
                    w.write_empty(privilege);
                    w.end_element();
                }
            }
            PropValue::Acl(aces) => {
                for ace in aces {
                    w.start_element("{DAV:}ace");
                    let principal = match ace.principal.as_str() {
                        "{DAV:}authenticated" => PrincipalKind::Authenticated,
                        "{DAV:}unauthenticated" => PrincipalKind::Unauthenticated,
                        "{DAV:}all" => PrincipalKind::All,
                        p => PrincipalKind::Href(p.to_owned()),
                    };
                    w.start_element("{DAV:}principal");
                    serialize_principal(w, &principal);
                    w.end_element();
                    w.start_element("{DAV:}grant");
                    w.start_element("{DAV:}privilege");
                    w.write_empty(&ace.privilege);
                    w.end_element();
                    w.end_element();
                    if ace.protected {
                        w.write_empty("{DAV:}protected");
                    }
                    w.end_element();
                }
            }
            PropValue::AclRestrictions => {
                w.write_empty("{DAV:}grant-only");
                w.write_empty("{DAV:}no-invert");
            }
            PropValue::SupportedPrivilegeSet(privileges) => {
                let all = Privilege { name: "{DAV:}all".to_owned(), abstract_: false, aggregates: privileges.clone() };
                serialize_privilege(w, &all);
            }
            PropValue::Responses(responses) => {
                for response in responses {
                    w.start_element("{DAV:}response");
                    response.serialize(w);
                    w.end_element();
                }
            }
        }
    }
}

/// `DAVACL\Xml\Property\Principal::xmlSerialize()`.
fn serialize_principal(w: &mut Writer, kind: &PrincipalKind) {
    match kind {
        PrincipalKind::Unauthenticated => w.write_empty("{DAV:}unauthenticated"),
        PrincipalKind::Authenticated => w.write_empty("{DAV:}authenticated"),
        PrincipalKind::Href(href) => {
            let href = format!("{}/", href.trim_end_matches('/'));
            PropValue::Href(vec![href]).serialize(w);
        }
        PrincipalKind::All => w.write_empty("{DAV:}all"),
    }
}

/// `SupportedPrivilegeSet::serializePriv()`.
fn serialize_privilege(w: &mut Writer, privilege: &Privilege) {
    w.start_element("{DAV:}supported-privilege");
    w.start_element("{DAV:}privilege");
    w.write_empty(&privilege.name);
    w.end_element();
    if privilege.abstract_ {
        w.write_empty("{DAV:}abstract");
    }
    for sub in &privilege.aggregates {
        serialize_privilege(w, sub);
    }
    w.end_element();
}

/// The properties of a response by status (`$propertyList[$status]`).
pub type PropStats = Vec<(u16, Vec<(String, Option<PropValue>)>)>;

/// `Sabre\DAV\Xml\Element\Response`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseElement {
    pub href: String,
    pub properties: PropStats,
    pub http_status: Option<u16>,
}

impl ResponseElement {
    /// `xmlSerialize($writer)`.
    pub fn serialize(&self, w: &mut Writer) {
        let href = format!("{}{}", w.context_uri, encode_path(&self.href));
        w.write_text_element("{DAV:}href", &href);

        let mut empty = true;
        for (status, properties) in &self.properties {
            if properties.is_empty() {
                continue;
            }
            empty = false;
            w.start_element("{DAV:}propstat");
            w.start_element("{DAV:}prop");
            for (name, value) in properties {
                w.start_element(name);
                if let Some(value) = value {
                    value.serialize(w);
                }
                w.end_element();
            }
            w.end_element();
            w.write_text_element("{DAV:}status", &format!("HTTP/1.1 {status} {}", status_text(*status)));
            w.end_element();
        }

        if empty {
            match self.http_status {
                Some(status) => {
                    w.write_text_element("{DAV:}status", &format!("HTTP/1.1 {status} {}", status_text(status)))
                }
                None => {
                    w.start_element("{DAV:}propstat");
                    w.write_empty("{DAV:}prop");
                    w.write_text_element("{DAV:}status", &format!("HTTP/1.1 418 {}", status_text(418)));
                    w.end_element();
                }
            }
        }
    }
}
