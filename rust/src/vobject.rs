//! The part of `sabre/vobject` 4.5.7 that `app/Domains/Contact/Dav` uses:
//! `Reader::read` (the MimeDir parser, with its `OPTION_FORGIVING` and
//! `OPTION_IGNORE_INVALID_LINES` options), the `Component` / `Property` /
//! `Parameter` model of `VCard` and `VCalendar` documents with their property
//! and value classes, and `serialize()` (75-octet line folding, escaping).
//!
//! A [`Document`] is the root component (`VCard` or `VCalendar`); it creates
//! properties and components (the class of a property depends on the
//! document). A [`Component`] keeps its children grouped by name, in the order
//! each name was first added, like Sabre's `$children` array.

use std::fmt;
use std::ops::{Deref, DerefMut};

use base64::Engine;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone as _};

/// `Sabre\VObject\Version::VERSION`.
pub const VERSION: &str = "4.5.7";

/// `Reader::OPTION_FORGIVING`.
pub const OPTION_FORGIVING: u32 = 1;

/// `Reader::OPTION_IGNORE_INVALID_LINES`.
pub const OPTION_IGNORE_INVALID_LINES: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `ParseException` (and `EofException`).
    Parse(String),
    /// `InvalidDataException`.
    InvalidData(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(m) | Error::InvalidData(m) => f.write_str(m),
        }
    }
}

/// `Document::getDocumentType()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentType {
    Unknown,
    ICalendar20,
    VCard21,
    VCard30,
    VCard40,
}

/// Which document class: `Component\VCard` or `Component\VCalendar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    VCard,
    VCalendar,
}

/// A property value as the PHP passes it to `add()` / `__set()`.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Null,
    Text(String),
    Parts(Vec<Option<String>>),
    DateTime(DateTime),
}

impl From<&str> for Input {
    fn from(s: &str) -> Self {
        Input::Text(s.to_owned())
    }
}

impl From<String> for Input {
    fn from(s: String) -> Self {
        Input::Text(s)
    }
}

impl From<Option<String>> for Input {
    fn from(s: Option<String>) -> Self {
        s.map_or(Input::Null, Input::Text)
    }
}

impl From<Vec<Option<String>>> for Input {
    fn from(parts: Vec<Option<String>>) -> Self {
        Input::Parts(parts)
    }
}

impl From<DateTime> for Input {
    fn from(dt: DateTime) -> Self {
        Input::DateTime(dt)
    }
}

/// A `DateTimeZone`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tz {
    Utc,
    Named(chrono_tz::Tz),
    /// A `+hh:mm` offset zone, in seconds east of UTC.
    Offset(i32),
}

impl Tz {
    /// `DateTimeZone::getName()`.
    pub fn name(&self) -> String {
        match self {
            Tz::Utc => "UTC".to_owned(),
            Tz::Named(tz) => tz.name().to_owned(),
            Tz::Offset(secs) => {
                let sign = if *secs < 0 { '-' } else { '+' };
                let a = secs.abs();
                format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60)
            }
        }
    }

    fn offset_at(self, utc: NaiveDateTime) -> i32 {
        match self {
            Tz::Utc => 0,
            Tz::Named(tz) => tz.offset_from_utc_datetime(&utc).fix().local_minus_utc(),
            Tz::Offset(secs) => secs,
        }
    }

    fn to_utc(self, local: NaiveDateTime) -> NaiveDateTime {
        match self {
            Tz::Utc => local,
            Tz::Named(tz) => match tz.from_local_datetime(&local) {
                chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => t.naive_utc(),
                chrono::LocalResult::None => local - chrono::Duration::seconds(i64::from(self.offset_at(local))),
            },
            Tz::Offset(secs) => local - chrono::Duration::seconds(i64::from(secs)),
        }
    }
}

/// A `DateTimeImmutable`: a wall-clock time in a zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateTime {
    pub local: NaiveDateTime,
    pub tz: Tz,
}

impl DateTime {
    pub fn utc(local: NaiveDateTime) -> DateTime {
        DateTime { local, tz: Tz::Utc }
    }

    fn format(&self, utc_suffix: bool) -> String {
        let s = self.local.format("%Y%m%dT%H%M%S").to_string();
        if utc_suffix { s + "Z" } else { s }
    }

    /// `->setTimeZone($tz)`.
    fn set_time_zone(&self, tz: Tz) -> DateTime {
        let utc = self.tz.to_utc(self.local);
        DateTime { local: utc + chrono::Duration::seconds(i64::from(tz.offset_at(utc))), tz }
    }
}

/// The property classes of `$propertyMap` / `$valueMap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Binary,
    Boolean,
    FlatText,
    FloatValue,
    IntegerValue,
    Text,
    Time,
    Unknown,
    Uri,
    UtcOffset,
    VCardDate,
    VCardDateTime,
    VCardDateAndOrTime,
    LanguageTag,
    PhoneNumber,
    TimeStamp,
    CalAddress,
    ICalDate,
    ICalDateTime,
    Duration,
    Period,
    Recur,
}

impl Class {
    /// Extends `Property\Text`.
    fn is_text(self) -> bool {
        matches!(
            self,
            Class::Text
                | Class::FlatText
                | Class::Unknown
                | Class::Uri
                | Class::UtcOffset
                | Class::Time
                | Class::CalAddress
                | Class::PhoneNumber
                | Class::TimeStamp
        )
    }

    /// The class's `$delimiter`, after `Text::__construct`.
    fn delimiter(self, name: &str) -> &'static str {
        const STRUCTURED: [&str; 6] = ["N", "ADR", "ORG", "GENDER", "CLIENTPIDMAP", "REQUEST-STATUS"];
        if self.is_text() && self != Class::PhoneNumber && STRUCTURED.contains(&name) {
            return ";";
        }
        match self {
            Class::Text | Class::FlatText | Class::Unknown | Class::PhoneNumber => ",",
            Class::Uri | Class::UtcOffset | Class::Time | Class::CalAddress | Class::TimeStamp => "",
            Class::Binary | Class::VCardDate | Class::VCardDateTime | Class::VCardDateAndOrTime => "",
            Class::ICalDate | Class::ICalDateTime | Class::Duration | Class::Period => ",",
            Class::FloatValue | Class::Boolean | Class::IntegerValue | Class::LanguageTag | Class::Recur => ";",
        }
    }

    fn is_ical_date_time(self) -> bool {
        matches!(self, Class::ICalDate | Class::ICalDateTime)
    }
}

impl Kind {
    /// `$componentMap`.
    fn is_component(self, name: &str) -> bool {
        match self {
            Kind::VCard => name == "VCARD",
            Kind::VCalendar => matches!(
                name,
                "VCALENDAR"
                    | "VALARM"
                    | "VEVENT"
                    | "VFREEBUSY"
                    | "VAVAILABILITY"
                    | "AVAILABLE"
                    | "VJOURNAL"
                    | "VTIMEZONE"
                    | "VTODO"
            ),
        }
    }

    /// `getDefaults()` of the component class.
    fn defaults(self, name: &str) -> Vec<(String, Input)> {
        let uid = || Input::Text(format!("sabre-vobject-{}", uuid::Uuid::new_v4()));
        let prodid = || Input::Text(format!("-//Sabre//Sabre VObject {VERSION}//EN"));
        let now = || Input::Text(chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string());
        match (self, name) {
            (Kind::VCard, "VCARD") => {
                vec![("VERSION".into(), "4.0".into()), ("PRODID".into(), prodid()), ("UID".into(), uid())]
            }
            (Kind::VCalendar, "VCALENDAR") => vec![
                ("VERSION".into(), "2.0".into()),
                ("PRODID".into(), prodid()),
                ("CALSCALE".into(), "GREGORIAN".into()),
            ],
            (Kind::VCalendar, "VEVENT" | "VTODO" | "VJOURNAL") => {
                vec![("UID".into(), uid()), ("DTSTAMP".into(), now())]
            }
            _ => vec![],
        }
    }

    /// `getClassNameForPropertyValue()`.
    fn class_for_value(self, value: &str) -> Option<Class> {
        let value = value.to_ascii_uppercase();
        Some(match (self, value.as_str()) {
            (_, "BINARY") => Class::Binary,
            (_, "BOOLEAN") => Class::Boolean,
            (Kind::VCard, "CONTENT-ID") => Class::FlatText,
            (Kind::VCard, "DATE") => Class::VCardDate,
            (Kind::VCard, "DATE-TIME") => Class::VCardDateTime,
            (Kind::VCard, "DATE-AND-OR-TIME") => Class::VCardDateAndOrTime,
            (_, "FLOAT") => Class::FloatValue,
            (_, "INTEGER") => Class::IntegerValue,
            (Kind::VCard, "LANGUAGE-TAG") => Class::LanguageTag,
            (Kind::VCard, "PHONE-NUMBER") => Class::PhoneNumber,
            (Kind::VCard, "TIMESTAMP") => Class::TimeStamp,
            (_, "TEXT") => Class::Text,
            (_, "TIME") => Class::Time,
            (_, "UNKNOWN") => Class::Unknown,
            (_, "URI") => Class::Uri,
            (Kind::VCard, "URL") => Class::Uri,
            (_, "UTC-OFFSET") => Class::UtcOffset,
            (Kind::VCalendar, "CAL-ADDRESS") => Class::CalAddress,
            (Kind::VCalendar, "DATE") => Class::ICalDate,
            (Kind::VCalendar, "DATE-TIME") => Class::ICalDateTime,
            (Kind::VCalendar, "DURATION") => Class::Duration,
            (Kind::VCalendar, "PERIOD") => Class::Period,
            (Kind::VCalendar, "RECUR") => Class::Recur,
            _ => return None,
        })
    }

    /// `getClassNameForPropertyName()` (`Unknown` when not in `$propertyMap`).
    fn class_for_name(self, name: &str, document_type: DocumentType) -> Class {
        match self {
            Kind::VCard => {
                let class = match name {
                    "N" | "ADR" | "ORG" | "TZ" | "CATEGORIES" | "NICKNAME" | "CLIENTPIDMAP" | "GENDER" => Class::Text,
                    "FN" | "LABEL" | "TEL" | "EMAIL" | "MAILER" | "GEO" | "TITLE" | "ROLE" | "NOTE" | "SOUND"
                    | "UID" | "VERSION" | "KEY" | "SORT-STRING" | "PRODID" | "CLASS" | "XML" | "KIND"
                    | "BIRTHPLACE" | "DEATHPLACE" | "EXPERTISE" | "HOBBY" | "INTEREST" | "ORG-DIRECTORY" => {
                        Class::FlatText
                    }
                    "PHOTO" | "LOGO" => Class::Binary,
                    "BDAY" | "ANNIVERSARY" | "DEATHDATE" => Class::VCardDateAndOrTime,
                    "REV" => Class::TimeStamp,
                    "URL" | "FBURL" | "CAPURI" | "CALURI" | "CALADRURI" | "IMPP" | "SOURCE" | "MEMBER" | "RELATED" => {
                        Class::Uri
                    }
                    "LANG" => Class::LanguageTag,
                    _ => Class::Unknown,
                };
                // In vCard 4, BINARY no longer exists, and we need URI instead.
                if class == Class::Binary && document_type == DocumentType::VCard40 { Class::Uri } else { class }
            }
            Kind::VCalendar => match name {
                "CALSCALE" | "METHOD" | "PRODID" | "VERSION" | "CLASS" | "COMMENT" | "DESCRIPTION" | "LOCATION"
                | "STATUS" | "SUMMARY" | "TRANSP" | "TZID" | "TZNAME" | "CONTACT" | "RELATED-TO" | "UID" | "ACTION" => {
                    Class::FlatText
                }
                "ATTACH" | "TZURL" | "URL" => Class::Uri,
                "CATEGORIES" | "RESOURCES" | "REQUEST-STATUS" | "ALARM-AGENT" | "PROXIMITY" | "BUSYTYPE" => Class::Text,
                "GEO" => Class::FloatValue,
                "PERCENT-COMPLETE" | "PRIORITY" | "REPEAT" | "SEQUENCE" => Class::IntegerValue,
                "COMPLETED" | "DTEND" | "DUE" | "DTSTART" | "RECURRENCE-ID" | "EXDATE" | "RDATE" | "CREATED"
                | "DTSTAMP" | "LAST-MODIFIED" | "ACKNOWLEDGED" => Class::ICalDateTime,
                "DURATION" | "TRIGGER" => Class::Duration,
                "FREEBUSY" => Class::Period,
                "TZOFFSETFROM" | "TZOFFSETTO" => Class::UtcOffset,
                "ATTENDEE" | "ORGANIZER" => Class::CalAddress,
                "RRULE" | "EXRULE" => Class::Recur,
                "DEFAULT-ALARM" => Class::Boolean,
                _ => Class::Unknown,
            },
        }
    }
}

/// A child of a component.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Component(Component),
    Property(Property),
}

impl Node {
    fn name(&self) -> &str {
        match self {
            Node::Component(c) => &c.name,
            Node::Property(p) => &p.name,
        }
    }

    fn serialize(&self, document_type: DocumentType) -> String {
        match self {
            Node::Component(c) => c.serialize(document_type),
            Node::Property(p) => p.serialize(document_type),
        }
    }
}

/// `Sabre\VObject\Component`.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    children: Vec<(String, Vec<Node>)>,
}

impl Component {
    fn new(name: &str) -> Component {
        Component { name: name.to_ascii_uppercase(), children: Vec::new() }
    }

    /// `add(Node)`.
    pub fn add(&mut self, node: Node) {
        let name = node.name().to_owned();
        match self.children.iter_mut().find(|(n, _)| *n == name) {
            Some((_, group)) => group.push(node),
            None => self.children.push((name, vec![node])),
        }
    }

    /// `remove($name)`: every child with that name (`GROUP.NAME` for a group).
    pub fn remove(&mut self, item: &str) {
        if !item.contains('.') {
            let name = item.to_ascii_uppercase();
            self.children.retain(|(n, _)| *n != name);
            return;
        }
        let (group, name) = split_group(item);
        for (n, nodes) in &mut self.children {
            if name.as_deref().is_none_or(|name| n == name) {
                nodes.retain(|node| !in_group(node, &group));
            }
        }
    }

    /// `remove($item)` for the `index`-th child named `name` (as `select($name)` lists them).
    pub fn remove_child(&mut self, name: &str, index: usize) {
        let name = name.to_ascii_uppercase();
        if let Some((_, nodes)) = self.children.iter_mut().find(|(n, _)| *n == name)
            && index < nodes.len()
        {
            nodes.remove(index);
        }
    }

    /// `children()`.
    pub fn children(&self) -> Vec<&Node> {
        self.children.iter().flat_map(|(_, nodes)| nodes).collect()
    }

    /// `getComponents()`.
    pub fn components(&self) -> Vec<&Component> {
        self.children()
            .into_iter()
            .filter_map(|n| match n {
                Node::Component(c) => Some(c),
                Node::Property(_) => None,
            })
            .collect()
    }

    /// `select($name)`.
    pub fn select(&self, name: &str) -> Vec<&Node> {
        let (group, name) =
            if name.contains('.') { split_group(name) } else { (None, Some(name.to_ascii_uppercase())) };
        match name {
            Some(name) => self
                .children
                .iter()
                .filter(|(n, _)| *n == name)
                .flat_map(|(_, nodes)| nodes)
                .filter(|node| group.is_none() || in_group(node, &group))
                .collect(),
            None => self.children().into_iter().filter(|node| in_group(node, &group)).collect(),
        }
    }

    /// The properties of `select($name)`.
    pub fn properties(&self, name: &str) -> Vec<&Property> {
        self.select(name)
            .into_iter()
            .filter_map(|n| match n {
                Node::Property(p) => Some(p),
                Node::Component(_) => None,
            })
            .collect()
    }

    /// `$component->NAME` when it is a property (the first match).
    pub fn property(&self, name: &str) -> Option<&Property> {
        match self.select(name).into_iter().next() {
            Some(Node::Property(p)) => Some(p),
            _ => None,
        }
    }

    pub fn property_mut(&mut self, name: &str) -> Option<&mut Property> {
        let name = name.to_ascii_uppercase();
        self.children.iter_mut().filter(|(n, _)| *n == name).flat_map(|(_, nodes)| nodes).find_map(|n| match n {
            Node::Property(p) => Some(p),
            Node::Component(_) => None,
        })
    }

    /// `$component->NAME` when it is a component (the first match).
    pub fn component(&self, name: &str) -> Option<&Component> {
        match self.select(name).into_iter().next() {
            Some(Node::Component(c)) => Some(c),
            _ => None,
        }
    }

    pub fn component_mut(&mut self, name: &str) -> Option<&mut Component> {
        let name = name.to_ascii_uppercase();
        self.children.iter_mut().filter(|(n, _)| *n == name).flat_map(|(_, nodes)| nodes).find_map(|n| match n {
            Node::Component(c) => Some(c),
            Node::Property(_) => None,
        })
    }

    /// `__set($name, $node)`: replaces every child with that name.
    pub fn set(&mut self, name: &str, node: Node) {
        self.remove(name);
        self.add(node);
    }

    /// `serialize()`.
    fn serialize(&self, document_type: DocumentType) -> String {
        let mut str = format!("BEGIN:{}\r\n", self.name);
        let mut children: Vec<(usize, &Node)> = self.children().into_iter().enumerate().collect();
        // VERSION first, then the other properties, then VTIMEZONE, then the
        // other components; the original order inside each category.
        children.sort_by_key(|(key, node)| {
            let score = match node {
                Node::Component(c) if c.name == "VTIMEZONE" => 300_000_000,
                Node::Component(_) => 400_000_000,
                Node::Property(p) if p.name == "VERSION" => 100_000_000,
                Node::Property(_) => 200_000_000,
            };
            score + key
        });
        for (_, child) in children {
            str.push_str(&child.serialize(document_type));
        }
        str.push_str(&format!("END:{}\r\n", self.name));
        str
    }
}

/// `explode('.', strtoupper($name), 2)`, `''` as no name.
fn split_group(item: &str) -> (Option<String>, Option<String>) {
    let item = item.to_ascii_uppercase();
    let (group, name) = item.split_once('.').expect("contains a dot");
    (Some(group.to_owned()), (!name.is_empty()).then(|| name.to_owned()))
}

fn in_group(node: &Node, group: &Option<String>) -> bool {
    match node {
        Node::Property(p) => {
            p.group.as_deref().map(str::to_ascii_uppercase).unwrap_or_default() == group.as_deref().unwrap_or_default()
        }
        Node::Component(_) => false,
    }
}

/// `Sabre\VObject\Document` (`Component\VCard` or `Component\VCalendar`).
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub kind: Kind,
    root: Component,
}

impl Deref for Document {
    type Target = Component;

    fn deref(&self) -> &Component {
        &self.root
    }
}

impl DerefMut for Document {
    fn deref_mut(&mut self) -> &mut Component {
        &mut self.root
    }
}

impl Document {
    /// `new VCard($children)`.
    pub fn vcard(children: Vec<(&str, Input)>) -> Result<Document, Error> {
        Document::new(Kind::VCard, children)
    }

    /// `new VCalendar($children)`.
    pub fn vcalendar(children: Vec<(&str, Input)>) -> Result<Document, Error> {
        Document::new(Kind::VCalendar, children)
    }

    fn new(kind: Kind, children: Vec<(&str, Input)>) -> Result<Document, Error> {
        let name = match kind {
            Kind::VCard => "VCARD",
            Kind::VCalendar => "VCALENDAR",
        };
        let mut doc = Document { kind, root: Component::new(name) };
        for (key, value) in merge_defaults(kind.defaults(name), children) {
            let node = doc.create(&key, value)?;
            doc.root.add(node);
        }
        Ok(doc)
    }

    /// `getDocumentType()`.
    pub fn document_type(&self) -> DocumentType {
        document_type(self.kind, &self.root)
    }

    /// `create($name, $value)`: a component when the name is in `$componentMap`.
    fn create(&self, name: &str, value: Input) -> Result<Node, Error> {
        if self.kind.is_component(&name.to_ascii_uppercase()) {
            Ok(Node::Component(self.create_component(name, Vec::new(), true)?))
        } else {
            Ok(Node::Property(self.create_property(name, value, Vec::new())?))
        }
    }

    /// `createComponent($name, $children, $defaults)`.
    pub fn create_component(
        &self,
        name: &str,
        children: Vec<(&str, Input)>,
        defaults: bool,
    ) -> Result<Component, Error> {
        let mut component = Component::new(name);
        let defaults = if defaults && self.kind.is_component(&component.name) {
            self.kind.defaults(&component.name)
        } else {
            Vec::new()
        };
        for (key, value) in merge_defaults(defaults, children) {
            let node = self.create(&key, value)?;
            component.add(node);
        }
        Ok(component)
    }

    /// `createProperty($name, $value, $parameters)`.
    pub fn create_property(
        &self,
        name: &str,
        value: Input,
        parameters: Vec<(&str, Option<String>)>,
    ) -> Result<Property, Error> {
        let parameters = parameters.into_iter().map(|(k, v)| (k.to_owned(), v.map(|v| vec![v]))).collect();
        let mut property = create_property(self.kind, &self.root, name, parameters);
        if value != Input::Null {
            property.set_value(value)?;
        }
        Ok(property)
    }

    /// `add($name, $value, $parameters)` of a property on the document.
    pub fn add_property(
        &mut self,
        name: &str,
        value: Input,
        parameters: Vec<(&str, Option<String>)>,
    ) -> Result<(), Error> {
        let property = self.create_property(name, value, parameters)?;
        self.root.add(Node::Property(property));
        Ok(())
    }

    /// `$document->NAME = $value` for a property.
    pub fn set_property(&mut self, name: &str, value: Input) -> Result<(), Error> {
        let property = self.create_property(name, value, Vec::new())?;
        self.root.set(name, Node::Property(property));
        Ok(())
    }

    /// `serialize()`.
    pub fn serialize(&self) -> String {
        self.root.serialize(self.document_type())
    }
}

/// The `Component::__construct` merge of `getDefaults()` and the children.
fn merge_defaults(mut list: Vec<(String, Input)>, children: Vec<(&str, Input)>) -> Vec<(String, Input)> {
    for (key, value) in children {
        match list.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value,
            None => list.push((key.to_owned(), value)),
        }
    }
    list
}

fn document_type(kind: Kind, root: &Component) -> DocumentType {
    match kind {
        Kind::VCalendar => DocumentType::ICalendar20,
        Kind::VCard => match root.property("VERSION").map(Property::to_string).as_deref() {
            Some("2.1") => DocumentType::VCard21,
            Some("3.0") => DocumentType::VCard30,
            Some("4.0") => DocumentType::VCard40,
            _ => DocumentType::Unknown,
        },
    }
}

/// `Document::createProperty`, with the raw (parsed) parameters.
fn create_property(
    kind: Kind,
    root: &Component,
    name: &str,
    parameters: Vec<(String, Option<Vec<String>>)>,
) -> Property {
    let (group, name) = match name.find('.') {
        Some(i) => (Some(name[..i].to_owned()), name[i + 1..].to_ascii_uppercase()),
        None => (None, name.to_ascii_uppercase()),
    };
    let value_type = parameters.iter().find(|(k, _)| k == "VALUE").and_then(|(_, v)| v.as_ref()).map(|v| v.join(","));
    let class = value_type
        .and_then(|v| kind.class_for_value(&v))
        .unwrap_or_else(|| kind.class_for_name(&name, document_type(kind, root)));
    let mut property = Property {
        delimiter: class.delimiter(&name),
        name,
        group,
        parameters: Vec::new(),
        value: Value::Parts(Vec::new()),
        class,
    };
    for (k, v) in parameters {
        property.add_parameter(Some(&k), v);
    }
    property
}

/// The value of a property.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// A scalar (one part), a list, or `null` (no parts).
    Parts(Vec<Option<String>>),
    Boolean(bool),
    Bytes(Vec<u8>),
    /// `Recur`: `KEY => value`, in order.
    Recur(Vec<(String, String)>),
}

/// `Sabre\VObject\Property` and its value classes.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub group: Option<String>,
    parameters: Vec<Parameter>,
    value: Value,
    class: Class,
    delimiter: &'static str,
}

impl fmt::Display for Property {
    /// `(string) $property`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.get_value().unwrap_or_default())
    }
}

impl Property {
    /// `getValue()`.
    pub fn get_value(&self) -> Option<String> {
        match &self.value {
            Value::Parts(parts) => match parts.len() {
                0 => None,
                1 => parts[0].clone(),
                _ => Some(self.get_raw_mimedir_value()),
            },
            Value::Boolean(b) => Some(if *b { "1".into() } else { String::new() }),
            Value::Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
            Value::Recur(_) => Some(self.get_raw_mimedir_value()),
        }
    }

    /// `getParts()`.
    pub fn get_parts(&self) -> Vec<Option<String>> {
        match &self.value {
            Value::Parts(parts) => parts.clone(),
            Value::Recur(map) => map.iter().map(|(_, v)| Some(v.clone())).collect(),
            _ => vec![self.get_value()],
        }
    }

    /// `setValue($value)`.
    pub fn set_value(&mut self, value: Input) -> Result<(), Error> {
        match (self.class, value) {
            (Class::ICalDate | Class::ICalDateTime, Input::DateTime(dt)) => {
                self.set_date_times(&[dt], false);
                Ok(())
            }
            (Class::VCardDate, Input::DateTime(dt)) => {
                self.value = Value::Parts(vec![Some(dt.local.format("%Y%m%d").to_string())]);
                Ok(())
            }
            (Class::VCardDateTime | Class::VCardDateAndOrTime, Input::DateTime(dt)) => {
                let value = if matches!(dt.tz.name().as_str(), "UTC" | "GMT" | "Z") {
                    dt.format(true)
                } else {
                    let secs = dt.tz.offset_at(dt.tz.to_utc(dt.local));
                    let sign = if secs < 0 { '-' } else { '+' };
                    format!("{}{sign}{:02}{:02}", dt.format(false), secs.abs() / 3600, secs.abs() % 3600 / 60)
                };
                self.value = Value::Parts(vec![Some(value)]);
                Ok(())
            }
            (Class::Recur, Input::Text(s)) => {
                self.value = Value::Recur(string_to_array(&s)?);
                Ok(())
            }
            (Class::Binary, value) => {
                self.value = Value::Bytes(match value {
                    Input::Text(s) => s.into_bytes(),
                    Input::Parts(parts) => parts.into_iter().next().flatten().unwrap_or_default().into_bytes(),
                    Input::Null | Input::DateTime(_) => Vec::new(),
                });
                Ok(())
            }
            (_, Input::Null) => {
                self.value = Value::Parts(Vec::new());
                Ok(())
            }
            (_, Input::Text(s)) => {
                self.value = Value::Parts(vec![Some(s)]);
                Ok(())
            }
            (_, Input::Parts(parts)) => {
                self.value = Value::Parts(parts);
                Ok(())
            }
            (_, Input::DateTime(dt)) => {
                self.value = Value::Parts(vec![Some(dt.format(false))]);
                Ok(())
            }
        }
    }

    /// `parameters()` (a `Uri` URL or PHOTO also lists `VALUE=URI`).
    pub fn parameters(&self) -> Vec<Parameter> {
        let mut parameters = self.parameters.clone();
        if self.class == Class::Uri
            && !parameters.iter().any(|p| p.name == "VALUE")
            && matches!(self.name.as_str(), "URL" | "PHOTO")
        {
            parameters.push(Parameter { name: "VALUE".into(), no_name: false, value: Some(vec!["URI".into()]) });
        }
        parameters
    }

    /// `Arr::get($property->parameters(), $name)`.
    pub fn parameter(&self, name: &str) -> Option<Parameter> {
        self.parameters().into_iter().find(|p| p.name == name)
    }

    /// `$property[$name]` (offsetGet).
    pub fn param(&self, name: &str) -> Option<&Parameter> {
        let name = name.to_ascii_uppercase();
        self.parameters.iter().find(|p| p.name == name)
    }

    /// `$property[$name] = $value` (offsetSet); on a date-time, `VALUE`
    /// re-encodes the dates.
    pub fn set_param(&mut self, name: &str, value: Option<String>) -> Result<(), Error> {
        let param = Parameter::new(Some(name), value.map(|v| vec![v]));
        match self.parameters.iter_mut().find(|p| p.name == param.name) {
            Some(p) => *p = param,
            None => self.parameters.push(param),
        }
        if self.class.is_ical_date_time() && name.eq_ignore_ascii_case("VALUE") {
            let dts = self.get_date_times()?;
            self.set_date_times(&dts, false);
        }
        Ok(())
    }

    /// `unset($property[$name])` (offsetUnset).
    fn unset_param(&mut self, name: &str) {
        let name = name.to_ascii_uppercase();
        self.parameters.retain(|p| p.name != name);
    }

    /// `add($name, $value)`: a new parameter, or more values for an existing
    /// one; a nameless one gets its name guessed from the value.
    fn add_parameter(&mut self, name: Option<&str>, value: Option<Vec<String>>) {
        let (name, no_name) = match name {
            Some(n) => (n.to_owned(), false),
            None => (guess_parameter_name_by_value(&value.as_deref().unwrap_or_default().join(",")).to_owned(), true),
        };
        let upper = name.to_ascii_uppercase();
        if let Some(p) = self.parameters.iter_mut().find(|p| p.name == upper) {
            p.add_value(value);
        } else {
            let mut param = Parameter::new(Some(&name), value);
            param.no_name = no_name;
            match self.parameters.iter_mut().find(|p| p.name == param.name) {
                Some(p) => *p = param,
                None => self.parameters.push(param),
            }
        }
    }

    /// `setRawMimeDirValue($val)`.
    fn set_raw_mimedir_value(&mut self, val: &str) -> Result<(), Error> {
        self.value = match self.class {
            Class::Uri => {
                Value::Parts(vec![Some(if self.name == "URL" { unescape_url(val) } else { val.replace("\\,", ",") })])
            }
            c if c.is_text() => Value::Parts(unescape_value(val, self.delimiter).into_iter().map(Some).collect()),
            Class::Binary => Value::Bytes(base64_decode(val)),
            Class::Boolean => Value::Boolean(val.eq_ignore_ascii_case("TRUE")),
            Class::FloatValue => {
                Value::Parts(val.split(';').map(|item| Some(php_float_to_string(php_floatval(item)))).collect())
            }
            Class::IntegerValue => Value::Parts(vec![Some(php_intval(val).to_string())]),
            Class::ICalDate | Class::ICalDateTime | Class::Duration | Class::Period => {
                Value::Parts(val.split(',').map(|s| Some(s.to_owned())).collect())
            }
            Class::Recur => Value::Recur(string_to_array(val)?),
            _ => Value::Parts(vec![Some(val.to_owned())]),
        };
        Ok(())
    }

    /// `setQuotedPrintableValue($val)` (`Text` and `FlatText` only).
    fn set_quoted_printable_value(&mut self, val: &str) -> Result<(), Error> {
        let val = String::from_utf8_lossy(&quoted_printable_decode(val.as_bytes())).into_owned();
        self.value = match self.class {
            Class::FlatText => Value::Parts(vec![Some(val)]),
            c if c.is_text() => {
                // Split on a `;` that isn't preceded by a backslash.
                let mut parts = Vec::new();
                let mut start = 0;
                let b = val.as_bytes();
                for i in 0..b.len() {
                    if b[i] == b';' && (i == 0 || b[i - 1] != b'\\') {
                        parts.push(Some(val[start..i].to_owned()));
                        start = i + 1;
                    }
                }
                parts.push(Some(val[start..].to_owned()));
                Value::Parts(parts)
            }
            _ => return Err(Error::InvalidData(format!("{} can't be QUOTED-PRINTABLE", self.name))),
        };
        Ok(())
    }

    /// `getRawMimeDirValue()`.
    fn get_raw_mimedir_value(&self) -> String {
        match (&self.value, self.class) {
            (Value::Boolean(b), _) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
            (Value::Bytes(b), _) => base64::engine::general_purpose::STANDARD.encode(b),
            (Value::Recur(map), _) => {
                map.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(";").to_ascii_uppercase()
            }
            (Value::Parts(parts), Class::Uri) => {
                parts.first().cloned().flatten().unwrap_or_default().replace(',', "\\,")
            }
            (Value::Parts(parts), c) if c.is_text() => {
                let mut parts = parts.clone();
                let minimum = match self.name.as_str() {
                    "N" => 5,
                    "ADR" => 7,
                    _ => 0,
                };
                while parts.len() < minimum {
                    parts.push(Some(String::new()));
                }
                parts
                    .iter()
                    .map(|p| p.as_deref().map(escape_text).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(self.delimiter)
            }
            (Value::Parts(parts), Class::IntegerValue | Class::LanguageTag) => {
                parts.first().cloned().flatten().unwrap_or_default()
            }
            (Value::Parts(parts), _) => {
                parts.iter().map(|p| p.clone().unwrap_or_default()).collect::<Vec<_>>().join(self.delimiter)
            }
        }
    }

    /// `serialize()`.
    fn serialize(&self, document_type: DocumentType) -> String {
        if self.class.is_text() && document_type == DocumentType::VCard21 {
            return self.serialize_vcard21(document_type);
        }
        let mut str = self.prefix();
        for param in self.parameters() {
            str.push(';');
            str.push_str(&param.serialize(document_type));
        }
        str.push(':');
        str.push_str(&self.get_raw_mimedir_value());
        fold(&str)
    }

    /// `$this->name`, or `$this->group.'.'.$this->name` when the group is truthy.
    fn prefix(&self) -> String {
        match &self.group {
            Some(g) if !php_empty(g) => format!("{g}.{}", self.name),
            _ => self.name.clone(),
        }
    }

    /// `Text::serialize()` for a vCard 2.1.
    fn serialize_vcard21(&self, document_type: DocumentType) -> String {
        let mut val = self.get_parts();
        let minimum = match self.name.as_str() {
            "N" => 5,
            "ADR" => 7,
            _ => 0,
        };
        while val.len() < minimum {
            val.push(Some(String::new()));
        }
        let val = if val.len() > 1 {
            val.iter().map(|v| v.as_deref().unwrap_or_default().replace(';', "\\;")).collect::<Vec<_>>().join(";")
        } else {
            val.first().cloned().flatten().unwrap_or_default()
        };
        let mut str = self.prefix();
        for param in &self.parameters {
            if param.get_value().as_deref() == Some("QUOTED-PRINTABLE") {
                continue;
            }
            str.push(';');
            str.push_str(&param.serialize(document_type));
        }
        if val.contains('\n') {
            str.push_str(";ENCODING=QUOTED-PRINTABLE:");
            let mut last_line = Some(str);
            let mut out = String::new();
            for b in val.bytes() {
                let line = last_line.get_or_insert_with(String::new);
                if (32..=126).contains(&b) {
                    line.push(b as char);
                } else {
                    line.push_str(&format!("={b:02X}"));
                }
                if line.len() >= 75 {
                    out.push_str(line);
                    out.push_str("=\r\n ");
                    last_line = None;
                }
            }
            if let Some(line) = last_line {
                out.push_str(&line);
                out.push_str("\r\n");
            }
            out
        } else {
            str.push(':');
            str.push_str(&val);
            fold(&str)
        }
    }

    /// `ICalendar\DateTime::hasTime()`.
    fn has_time(&self) -> bool {
        !self.param("VALUE").map(Parameter::to_string).unwrap_or_default().eq_ignore_ascii_case("DATE")
    }

    /// `ICalendar\DateTime::getDateTime()`: the first date, in its `TZID`.
    pub fn get_date_time(&self) -> Result<Option<DateTime>, Error> {
        Ok(self.get_date_times()?.into_iter().next())
    }

    /// `ICalendar\DateTime::getDateTimes()`.
    fn get_date_times(&self) -> Result<Vec<DateTime>, Error> {
        let tz = self.param("TZID").map(|tzid| get_time_zone(&tzid.to_string()));
        self.get_parts().iter().map(|part| parse(part.as_deref().unwrap_or_default(), tz)).collect()
    }

    /// `ICalendar\DateTime::setDateTimes()`.
    fn set_date_times(&mut self, dts: &[DateTime], is_floating: bool) {
        let mut values = Vec::new();
        if self.has_time() {
            let mut tz: Option<Tz> = None;
            let mut is_utc = false;
            for d in dts {
                if is_floating {
                    values.push(Some(d.format(false)));
                    continue;
                }
                let d = match tz {
                    None => {
                        tz = Some(d.tz);
                        is_utc = matches!(d.tz.name().as_str(), "UTC" | "GMT" | "Z" | "+00:00");
                        if !is_utc {
                            let param = Parameter::new(Some("TZID"), Some(vec![d.tz.name()]));
                            match self.parameters.iter_mut().find(|p| p.name == "TZID") {
                                Some(p) => *p = param,
                                None => self.parameters.push(param),
                            }
                        }
                        *d
                    }
                    Some(tz) => d.set_time_zone(tz),
                };
                values.push(Some(d.format(is_utc)));
            }
            if is_utc || is_floating {
                self.unset_param("TZID");
            }
        } else {
            for d in dts {
                values.push(Some(d.local.format("%Y%m%d").to_string()));
            }
            self.unset_param("TZID");
        }
        self.value = Value::Parts(values);
    }
}

/// `Sabre\VObject\Parameter`.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub no_name: bool,
    value: Option<Vec<String>>,
}

impl fmt::Display for Parameter {
    /// `(string) $parameter`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.get_value().unwrap_or_default())
    }
}

impl Parameter {
    fn new(name: Option<&str>, value: Option<Vec<String>>) -> Parameter {
        let (name, mut no_name) = match name {
            None => (guess_parameter_name_by_value(&value.as_deref().unwrap_or_default().join(",")).to_owned(), true),
            Some(n) => (n.to_ascii_uppercase(), false),
        };
        // An empty guessed name: the value is the name, and there is no value.
        if name.is_empty() {
            no_name = false;
            let name = value.as_deref().unwrap_or_default().join(",").to_ascii_uppercase();
            return Parameter { name, no_name, value: None };
        }
        Parameter { name, no_name, value }
    }

    /// `getValue()`.
    pub fn get_value(&self) -> Option<String> {
        self.value.as_ref().map(|v| v.join(","))
    }

    /// `getParts()`.
    pub fn get_parts(&self) -> Vec<String> {
        self.value.clone().unwrap_or_default()
    }

    /// `has($value)`.
    pub fn has(&self, value: &str) -> bool {
        self.get_parts().iter().any(|v| v.eq_ignore_ascii_case(value))
    }

    /// `addValue($part)`.
    fn add_value(&mut self, part: Option<Vec<String>>) {
        match (&mut self.value, part) {
            (None, part) => self.value = part,
            (Some(v), Some(part)) => v.extend(part),
            (Some(_), None) => {}
        }
    }

    /// `serialize()`.
    fn serialize(&self, document_type: DocumentType) -> String {
        let value = self.get_parts();
        if value.is_empty() {
            return format!("{}=", self.name);
        }
        if document_type == DocumentType::VCard21 && self.no_name {
            return value.join(";");
        }
        let items: Vec<String> = value
            .iter()
            .map(|item| {
                if !item.contains(['\n', '"', ':', ';', '^', ',', '+']) {
                    item.clone()
                } else {
                    format!("\"{}\"", item.replace('^', "^^").replace('\n', "^n").replace('"', "^'"))
                }
            })
            .collect();
        format!("{}={}", self.name, items.join(","))
    }
}

/// `Parameter::guessParameterNameByValue()`.
fn guess_parameter_name_by_value(value: &str) -> &'static str {
    match value.to_ascii_uppercase().as_str() {
        "7-BIT" | "QUOTED-PRINTABLE" | "BASE64" => "ENCODING",
        "WORK" | "HOME" | "PREF" | "DOM" | "INTL" | "POSTAL" | "PARCEL" | "VOICE" | "FAX" | "MSG" | "CELL"
        | "PAGER" | "BBS" | "MODEM" | "CAR" | "ISDN" | "VIDEO" | "AOL" | "APPLELINK" | "ATTMAIL" | "CIS" | "EWORLD"
        | "INTERNET" | "IBMMAIL" | "MCIMAIL" | "POWERSHARE" | "PRODIGY" | "TLX" | "X400" | "GIF" | "CGM" | "WMF"
        | "BMP" | "DIB" | "PICT" | "TIFF" | "PDF" | "PS" | "JPEG" | "MPEG" | "MPEG2" | "AVI" | "QTIME" | "WAVE"
        | "PCM" | "AIFF" | "X509" | "PGP" => "TYPE",
        "INLINE" | "URL" | "CONTENT-ID" | "CID" => "VALUE",
        _ => "",
    }
}

/// `Property::serialize()`'s folding: lines of at most 75 octets, never
/// splitting a UTF-8 character, continued with a space.
fn fold(str: &str) -> String {
    let b = str.as_bytes();
    let mut out = Vec::with_capacity(b.len() + b.len() / 70 * 3 + 2);
    let mut p = 0;
    while p < b.len() {
        let max = if p == 0 { 75 } else { 74 };
        let len = (1..=max.min(b.len() - p)).rev().find(|&len| {
            !b[p..p + len].contains(&b'\n') && !(p + len < b.len() && (0x80..=0xbf).contains(&b[p + len]))
        });
        match len {
            Some(len) => {
                out.extend_from_slice(&b[p..p + len]);
                out.extend_from_slice(b"\r\n ");
                p += len;
            }
            None => {
                out.push(b[p]);
                p += 1;
            }
        }
    }
    out.pop();
    String::from_utf8_lossy(&out).into_owned()
}

/// `Text::getRawMimeDirValue()`'s escaping of one item.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// `MimeDir::unescapeValue($input, $delimiter)` (one part when there is no delimiter).
fn unescape_value(input: &str, delimiter: &str) -> Vec<String> {
    let b = input.as_bytes();
    let mut result_array = Vec::new();
    let mut result = String::new();
    let (mut i, mut literal) = (0, 0);
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() && matches!(b[i + 1], b'\\' | b'N' | b'n' | b';' | b',') {
            result.push_str(&input[literal..i]);
            result.push(match b[i + 1] {
                b'N' | b'n' => '\n',
                c => c as char,
            });
            i += 2;
            literal = i;
        } else if !delimiter.is_empty() && b[i..].starts_with(delimiter.as_bytes()) {
            result.push_str(&input[literal..i]);
            result_array.push(std::mem::take(&mut result));
            i += delimiter.len();
            literal = i;
        } else {
            i += 1;
        }
    }
    result.push_str(&input[literal..]);
    result_array.push(result);
    result_array
}

/// `Uri::setRawMimeDirValue()` of a URL: `\:` is a colon.
fn unescape_url(val: &str) -> String {
    let mut out = String::with_capacity(val.len());
    let mut chars = val.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('\\', Some('\\')) => {
                out.push_str("\\\\");
                chars.next();
            }
            ('\\', Some(':')) => {
                out.push(':');
                chars.next();
            }
            (c, _) => out.push(c),
        }
    }
    out
}

/// `Recur::stringToArray()`.
fn string_to_array(value: &str) -> Result<Vec<(String, String)>, Error> {
    let value = value.to_ascii_uppercase();
    let mut new_value: Vec<(String, String)> = Vec::new();
    for part in value.split(';') {
        if php_empty(part) {
            continue;
        }
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidData(format!("The supplied iCalendar RRULE part is incorrect: {part}")));
        }
        match new_value.iter_mut().find(|(k, _)| k == parts[0]) {
            Some(entry) => entry.1 = parts[1].to_owned(),
            None => new_value.push((parts[0].to_owned(), parts[1].to_owned())),
        }
    }
    Ok(new_value)
}

/// PHP's `empty()` on a string.
fn php_empty(s: &str) -> bool {
    s.is_empty() || s == "0"
}

/// `base64_decode($val)` (non-strict: other characters and `=` are skipped).
fn base64_decode(val: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut n) = (0u32, 0);
    for c in val.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        n += 1;
        if n == 4 {
            out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
            acc = 0;
            n = 0;
        }
    }
    match n {
        2 => out.push((acc >> 4) as u8),
        3 => out.extend_from_slice(&[(acc >> 10) as u8, (acc >> 2) as u8]),
        _ => {}
    }
    out
}

/// `quoted_printable_decode()`.
fn quoted_printable_decode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'=' {
            if i + 2 < s.len() && s[i + 1].is_ascii_hexdigit() && s[i + 2].is_ascii_hexdigit() {
                let hex = std::str::from_utf8(&s[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
                i += 3;
                continue;
            }
            let mut k = 1;
            while i + k < s.len() && (s[i + k] == b' ' || s[i + k] == b'\t') {
                k += 1;
            }
            if i + k >= s.len() {
                i += k;
            } else if s[i + k] == b'\r' && s.get(i + k + 1) == Some(&b'\n') {
                i += k + 2;
            } else if s[i + k] == b'\r' || s[i + k] == b'\n' {
                i += k + 1;
            } else {
                out.push(s[i]);
                i += 1;
            }
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// The leading number of a string, as `(float)` / `(int)` read it.
fn php_numeric_prefix(s: &str) -> (&str, bool) {
    let s = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0B', '\x0C']);
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    let int_digits = digits(&mut i);
    let mut is_float = false;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        if digits(&mut j) || int_digits {
            i = j;
            is_float = true;
        }
    }
    if (int_digits || is_float) && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if digits(&mut j) {
            i = j;
            is_float = true;
        }
    }
    if !int_digits && !is_float { ("", false) } else { (&s[..i], is_float) }
}

/// `(float) $s`.
fn php_floatval(s: &str) -> f64 {
    php_numeric_prefix(s).0.parse().unwrap_or(0.0)
}

/// `(int) $s`.
fn php_intval(s: &str) -> i64 {
    let (prefix, is_float) = php_numeric_prefix(s);
    if is_float {
        prefix.parse::<f64>().map(|f| f as i64).unwrap_or(0)
    } else {
        prefix.parse::<i64>().unwrap_or_else(|_| {
            if prefix.starts_with('-') {
                i64::MIN
            } else if prefix.is_empty() {
                0
            } else {
                i64::MAX
            }
        })
    }
}

/// `(string) $float` (`precision` 14).
fn php_float_to_string(f: f64) -> String {
    if f.is_nan() {
        return "NAN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "INF".into() } else { "-INF".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{:.13e}", f.abs());
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let digits: String = mantissa.replace('.', "");
    let digits = digits.trim_end_matches('0');
    let sign = if f < 0.0 { "-" } else { "" };
    if !(-4..14).contains(&exp) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{first}.{rest}E{esign}{}", exp.abs());
    }
    let exp = exp as isize;
    let s = if exp >= 0 {
        let exp = exp as usize;
        let (int, frac) = if digits.len() > exp + 1 {
            (digits[..=exp].to_owned(), digits[exp + 1..].to_owned())
        } else {
            (format!("{digits}{}", "0".repeat(exp + 1 - digits.len())), String::new())
        };
        if frac.is_empty() { int } else { format!("{int}.{frac}") }
    } else {
        format!("0.{}{digits}", "0".repeat((-exp - 1) as usize))
    };
    format!("{sign}{s}")
}

/// `TimeZoneUtil::getTimeZone($tzid)`: a time zone identifier or a
/// `GMT+hhmm` offset, else the default zone (UTC).
fn get_time_zone(tzid: &str) -> Tz {
    if !tzid.is_empty() && !tzid.starts_with('(') {
        if let Ok(tz) = tzid.parse::<chrono_tz::Tz>() {
            return Tz::Named(tz);
        }
        let b = tzid.as_bytes();
        if b.len() == 8
            && tzid.starts_with("GMT")
            && (b[3] == b'+' || b[3] == b'-')
            && b[4..].iter().all(u8::is_ascii_digit)
        {
            let hours: i32 = tzid[4..6].parse().unwrap_or(0);
            let minutes: i32 = tzid[6..8].parse().unwrap_or(0);
            let secs = hours * 3600 + minutes * 60;
            return Tz::Offset(if b[3] == b'-' { -secs } else { secs });
        }
    }
    Tz::Utc
}

/// `DateTimeParser::parse($date, $referenceTz)`.
fn parse(date: &str, tz: Option<Tz>) -> Result<DateTime, Error> {
    let b = date.as_bytes();
    if b.first() == Some(&b'P') || (b.first() == Some(&b'-') && b.get(1) == Some(&b'P')) {
        Err(Error::InvalidData(format!("A duration is not a date: {date}")))
    } else if b.len() == 8 {
        parse_date(date, tz)
    } else {
        parse_date_time(date, tz)
    }
}

fn digits_at(b: &[u8], range: std::ops::Range<usize>) -> Option<u32> {
    let s = b.get(range)?;
    if s.iter().all(u8::is_ascii_digit) { std::str::from_utf8(s).ok()?.parse().ok() } else { None }
}

/// `DateTimeParser::parseDateTime($dt, $tz)`.
pub fn parse_date_time(dt: &str, tz: Option<Tz>) -> Result<DateTime, Error> {
    let err = || Error::InvalidData(format!("The supplied iCalendar datetime value is incorrect: {dt}"));
    let b = dt.as_bytes();
    if !(b.len() == 15 || (b.len() == 16 && b[15] == b'Z')) || b[8] != b'T' {
        return Err(err());
    }
    let year = digits_at(b, 0..4).ok_or_else(err)?;
    let month = digits_at(b, 4..6).filter(|_| b[4] <= b'1').ok_or_else(err)?;
    let day = digits_at(b, 6..8).filter(|_| b[6] <= b'3').ok_or_else(err)?;
    let hour = digits_at(b, 9..11).filter(|_| b[9] <= b'2').ok_or_else(err)?;
    let minute = digits_at(b, 11..13).filter(|_| b[11] <= b'5').ok_or_else(err)?;
    let second = digits_at(b, 13..15).filter(|_| b[13] <= b'5').ok_or_else(err)?;
    let tz = if b.len() == 16 { Tz::Utc } else { tz.unwrap_or(Tz::Utc) };
    let local = NaiveDate::from_ymd_opt(year as i32, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, second))
        .ok_or_else(err)?;
    Ok(DateTime { local, tz })
}

/// `DateTimeParser::parseDate($date, $tz)`.
pub fn parse_date(date: &str, tz: Option<Tz>) -> Result<DateTime, Error> {
    let err = || Error::InvalidData(format!("The supplied iCalendar date value is incorrect: {date}"));
    let b = date.as_bytes();
    if b.len() != 8 {
        return Err(err());
    }
    let year = digits_at(b, 0..4).ok_or_else(err)?;
    let month = digits_at(b, 4..6).filter(|_| b[4] <= b'1').ok_or_else(err)?;
    let day = digits_at(b, 6..8).filter(|_| b[6] <= b'3').ok_or_else(err)?;
    let local = NaiveDate::from_ymd_opt(year as i32, month, day).map(|d| d.and_time(NaiveTime::MIN)).ok_or_else(err)?;
    Ok(DateTime { local, tz: tz.unwrap_or(Tz::Utc) })
}

/// The parts of `DateTimeParser::parseVCardDateTime()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VCardDateTime {
    pub year: Option<String>,
    pub month: Option<String>,
    pub date: Option<String>,
    pub hour: Option<String>,
    pub minute: Option<String>,
    pub second: Option<String>,
    pub timezone: Option<String>,
}

/// `DateTimeParser::parseVCardDateTime($date)`: the basic format, else the
/// extended one; `-` and `--` placeholders are `None`.
pub fn parse_vcard_date_time(date: &str) -> Result<VCardDateTime, Error> {
    let b = date.as_bytes();
    let found = vcard_basic(b).or_else(|| vcard_extended(b));
    let Some(m) = found else {
        return Err(Error::InvalidData(format!("Invalid vCard date-time string: {date}")));
    };
    let get = |r: Option<(usize, usize)>| {
        r.map(|(s, e)| &date[s..e]).filter(|v| !v.is_empty() && *v != "0" && *v != "-" && *v != "--").map(str::to_owned)
    };
    Ok(VCardDateTime {
        year: get(m[0]),
        month: get(m[1]),
        date: get(m[2]),
        hour: get(m[3]),
        minute: get(m[4]),
        second: get(m[5]),
        timezone: get(m[6]),
    })
}

/// The byte range of a capture group, if it matched.
type Span = Option<(usize, usize)>;

type Spans = [Span; 7];

fn two_digits(b: &[u8], p: usize) -> bool {
    b.len() >= p + 2 && b[p].is_ascii_digit() && b[p + 1].is_ascii_digit()
}

/// The basic-format regex: date part alternatives in the regex's order of
/// preference, each followed by the time part and the end.
fn vcard_basic(b: &[u8]) -> Option<Spans> {
    let mut candidates: Vec<(usize, Span, Span, Span)> = Vec::new();
    // (?: (?: (?:year -?| --) month? | ---) date? )?
    if b.len() >= 4 && b[..4].iter().all(u8::is_ascii_digit) {
        let year = Some((0, 4));
        for dash in [true, false] {
            let p = if dash {
                if b.get(4) != Some(&b'-') {
                    continue;
                }
                5
            } else {
                4
            };
            for month in [true, false] {
                let (mp, ms) = if month {
                    if !two_digits(b, p) {
                        continue;
                    }
                    (p + 2, Some((p, p + 2)))
                } else {
                    (p, None)
                };
                for date in [true, false] {
                    if date {
                        if two_digits(b, mp) {
                            candidates.push((mp + 2, year, ms, Some((mp, mp + 2))));
                        }
                    } else {
                        candidates.push((mp, year, ms, None));
                    }
                }
            }
        }
    }
    if b.starts_with(b"--") {
        for month in [true, false] {
            let (mp, ms) = if month {
                if !two_digits(b, 2) {
                    continue;
                }
                (4, Some((2, 4)))
            } else {
                (2, None)
            };
            for date in [true, false] {
                if date {
                    if two_digits(b, mp) {
                        candidates.push((mp + 2, None, ms, Some((mp, mp + 2))));
                    }
                } else {
                    candidates.push((mp, None, ms, None));
                }
            }
        }
    }
    if b.starts_with(b"---") {
        if two_digits(b, 3) {
            candidates.push((5, None, None, Some((3, 5))));
        }
        candidates.push((3, None, None, None));
    }
    candidates.push((0, None, None, None));
    for (p, year, month, date) in candidates {
        if let Some(time) = vcard_basic_time(b, p) {
            return Some([year, month, date, time[0], time[1], time[2], time[3]]);
        }
    }
    None
}

/// `(?:T (hour: \d\d|-) (minute: \d\d|-)? (second: \d\d)? (\.\d{3})? (tz: Z|[+-]\d{4})? )? $`
fn vcard_basic_time(b: &[u8], p: usize) -> Option<[Span; 4]> {
    if p == b.len() {
        return Some([None; 4]);
    }
    if b[p] != b'T' {
        return None;
    }
    let p = p + 1;
    let alts = |p: usize, allow_dash: bool| -> Vec<(usize, Span)> {
        let mut v = Vec::new();
        if two_digits(b, p) {
            v.push((p + 2, Some((p, p + 2))));
        }
        if allow_dash && b.get(p) == Some(&b'-') {
            v.push((p + 1, Some((p, p + 1))));
        }
        v
    };
    for (hp, hour) in alts(p, true) {
        let mut minutes = alts(hp, true);
        minutes.push((hp, None));
        for (mp, minute) in minutes {
            let mut seconds = alts(mp, false);
            seconds.push((mp, None));
            for (sp, second) in seconds {
                let mut millis = Vec::new();
                if b.get(sp) == Some(&b'.') && b.len() >= sp + 4 && b[sp + 1..sp + 4].iter().all(u8::is_ascii_digit) {
                    millis.push(sp + 4);
                }
                millis.push(sp);
                for mlp in millis {
                    let mut zones = Vec::new();
                    if b.get(mlp) == Some(&b'Z') {
                        zones.push((mlp + 1, Some((mlp, mlp + 1))));
                    }
                    if matches!(b.get(mlp), Some(b'+' | b'-'))
                        && b.len() >= mlp + 5
                        && b[mlp + 1..mlp + 5].iter().all(u8::is_ascii_digit)
                    {
                        zones.push((mlp + 5, Some((mlp, mlp + 5))));
                    }
                    zones.push((mlp, None));
                    for (end, zone) in zones {
                        if end == b.len() {
                            return Some([hour, minute, second, zone]);
                        }
                    }
                }
            }
        }
    }
    None
}

/// The extended-format regex:
/// `(?: (?: year - | -- ) month - date )? (?:T (?: hour : | -) (?: minute : | -)? second? (\.\d{3})? (tz: Z|[+-]\d\d:\d\d)? )? $`
fn vcard_extended(b: &[u8]) -> Option<Spans> {
    let mut candidates = Vec::new();
    for year in [true, false] {
        let p = if year {
            if !(b.len() >= 5 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-') {
                continue;
            }
            5
        } else {
            if !b.starts_with(b"--") {
                continue;
            }
            2
        };
        if two_digits(b, p) && b.get(p + 2) == Some(&b'-') && two_digits(b, p + 3) {
            candidates.push((p + 5, year.then_some((0, 4)), Some((p, p + 2)), Some((p + 3, p + 5))));
        }
    }
    candidates.push((0, None, None, None));
    for (p, year, month, date) in candidates {
        if let Some(time) = vcard_extended_time(b, p) {
            return Some([year, month, date, time[0], time[1], time[2], time[3]]);
        }
    }
    None
}

fn vcard_extended_time(b: &[u8], p: usize) -> Option<[Span; 4]> {
    if p == b.len() {
        return Some([None; 4]);
    }
    if b[p] != b'T' {
        return None;
    }
    let p = p + 1;
    let alts = |p: usize| -> Vec<(usize, Span)> {
        let mut v = Vec::new();
        if two_digits(b, p) && b.get(p + 2) == Some(&b':') {
            v.push((p + 3, Some((p, p + 2))));
        }
        if b.get(p) == Some(&b'-') {
            v.push((p + 1, None));
        }
        v
    };
    for (hp, hour) in alts(p) {
        let mut minutes = alts(hp);
        minutes.push((hp, None));
        for (mp, minute) in minutes {
            let mut seconds = Vec::new();
            if two_digits(b, mp) {
                seconds.push((mp + 2, Some((mp, mp + 2))));
            }
            seconds.push((mp, None));
            for (sp, second) in seconds {
                let mut millis = Vec::new();
                if b.get(sp) == Some(&b'.') && b.len() >= sp + 4 && b[sp + 1..sp + 4].iter().all(u8::is_ascii_digit) {
                    millis.push(sp + 4);
                }
                millis.push(sp);
                for mlp in millis {
                    let mut zones = Vec::new();
                    if b.get(mlp) == Some(&b'Z') {
                        zones.push((mlp + 1, Some((mlp, mlp + 1))));
                    }
                    if matches!(b.get(mlp), Some(b'+' | b'-'))
                        && two_digits(b, mlp + 1)
                        && b.get(mlp + 3) == Some(&b':')
                        && two_digits(b, mlp + 4)
                    {
                        zones.push((mlp + 6, Some((mlp, mlp + 6))));
                    }
                    zones.push((mlp, None));
                    for (end, zone) in zones {
                        if end == b.len() {
                            return Some([hour, minute, second, zone]);
                        }
                    }
                }
            }
        }
    }
    None
}

/// `Reader::read($data, $options)`: the MimeDir parser.
pub fn read(data: &str, options: u32) -> Result<Document, Error> {
    let mut parser = MimeDir { input: data, pos: 0, eof: false, options, line_buffer: None, raw_line: String::new() };
    parser.parse_document()
}

/// `Parser\MimeDir` over a string (`fgets` / `feof` of a `php://temp` stream).
struct MimeDir<'a> {
    input: &'a str,
    pos: usize,
    eof: bool,
    options: u32,
    line_buffer: Option<String>,
    raw_line: String,
}

fn eof_error() -> Error {
    Error::Parse("End of document reached prematurely".into())
}

impl<'a> MimeDir<'a> {
    fn fgets(&mut self) -> Option<&'a str> {
        if self.pos >= self.input.len() {
            self.eof = true;
            return None;
        }
        let rest = &self.input[self.pos..];
        match rest.find('\n') {
            Some(i) => {
                self.pos += i + 1;
                Some(&rest[..=i])
            }
            None => {
                self.pos = self.input.len();
                self.eof = true;
                Some(rest)
            }
        }
    }

    /// `readLine()`: one unfolded line; `None` at the end of the document
    /// (`EofException`).
    fn read_line(&mut self) -> Option<String> {
        let mut raw_line = match self.line_buffer.take() {
            Some(line) => line,
            None => loop {
                let eof = self.eof;
                let line = self.fgets();
                if eof || (self.eof && line.is_none()) {
                    return None;
                }
                let line = line?.trim_end_matches(['\r', '\n']);
                if !line.is_empty() {
                    break line.to_owned();
                }
            },
        };
        let mut line = raw_line.clone();
        loop {
            let next = self.fgets().map(|l| l.trim_end_matches(['\r', '\n'])).unwrap_or("");
            if php_empty(next) {
                break;
            }
            if next.starts_with(['\t', ' ']) {
                line.push_str(&next[1..]);
                raw_line.push_str("\n ");
                raw_line.push_str(&next[1..]);
            } else {
                self.line_buffer = Some(next.to_owned());
                break;
            }
        }
        self.raw_line = raw_line;
        Some(line)
    }

    /// `parseDocument()`.
    fn parse_document(&mut self) -> Result<Document, Error> {
        let mut line = self.read_line().ok_or_else(eof_error)?;
        if let Some(rest) = line.strip_prefix('\u{feff}') {
            line = rest.to_owned();
        }
        let kind = match line.to_ascii_uppercase().as_str() {
            "BEGIN:VCALENDAR" => Kind::VCalendar,
            "BEGIN:VCARD" => Kind::VCard,
            _ => return Err(Error::Parse("This parser only supports VCARD and VCALENDAR files".into())),
        };
        let mut root = Component::new(match kind {
            Kind::VCard => "VCARD",
            Kind::VCalendar => "VCALENDAR",
        });
        let line = loop {
            let line = self.read_line().unwrap_or_else(|| format!("END:{}", root.name));
            if starts_with_ignore_case(&line, "END:") {
                break line;
            }
            if let Some(node) = self.parse_line(&line, kind, &root)? {
                root.add(node);
            }
        };
        let name = line[4..].to_ascii_uppercase();
        if name != root.name {
            return Err(Error::Parse(format!(
                "Invalid MimeDir file. expected: \"END:{}\" got: \"END:{name}\"",
                root.name
            )));
        }
        Ok(Document { kind, root })
    }

    /// `parseLine($line)`.
    fn parse_line(&mut self, line: &str, kind: Kind, root: &Component) -> Result<Option<Node>, Error> {
        if !starts_with_ignore_case(line, "BEGIN:") {
            return Ok(self.read_property(line, kind, root)?.map(Node::Property));
        }
        if line[6..] == root.name {
            return Err(Error::Parse(format!(
                "Invalid MimeDir file. Unexpected component: \"{line}\" in document type {}",
                root.name
            )));
        }
        let mut component = Component::new(&line[6..]);
        let line = loop {
            let line = self.read_line().ok_or_else(eof_error)?;
            if starts_with_ignore_case(&line, "END:") {
                break line;
            }
            if let Some(node) = self.parse_line(&line, kind, root)? {
                component.add(node);
            }
        };
        let name = line[4..].to_ascii_uppercase();
        if name != component.name {
            return Err(Error::Parse(format!(
                "Invalid MimeDir file. expected: \"END:{}\" got: \"END:{name}\"",
                component.name
            )));
        }
        Ok(Some(Node::Component(component)))
    }

    /// `readProperty($line)`.
    fn read_property(&mut self, line: &str, kind: Kind, root: &Component) -> Result<Option<Property>, Error> {
        let mut name: Option<String> = None;
        let mut parameters: Vec<(String, Option<Vec<String>>)> = Vec::new();
        let mut value: Option<String> = None;
        let mut last_param: Option<String> = None;
        for token in tokenize(line, self.options & OPTION_FORGIVING != 0) {
            match token {
                Token::ParamValue(v) => {
                    let v = if v.starts_with('"') { &v[1..v.len() - 1] } else { v };
                    let v = unescape_param(v);
                    let Some(last) = &last_param else {
                        if self.options & OPTION_IGNORE_INVALID_LINES != 0 {
                            continue;
                        }
                        return Err(invalid_line());
                    };
                    let entry = parameters.iter_mut().find(|(k, _)| k == last).expect("lastParam was added");
                    match &mut entry.1 {
                        None => entry.1 = Some(vec![v]),
                        Some(values) if values.len() == 1 && values[0] == v => {}
                        Some(values) => values.push(v),
                    }
                }
                Token::ParamName(n) => {
                    let n = n.to_ascii_uppercase();
                    if !parameters.iter().any(|(k, _)| *k == n) {
                        parameters.push((n.clone(), None));
                    }
                    last_param = Some(n);
                }
                Token::Value(v) => value = Some(v.to_owned()),
                Token::Name(n) => name = Some(n.to_ascii_uppercase()),
            }
        }
        let value = value.unwrap_or_default();
        let Some(name) = name else {
            if self.options & OPTION_IGNORE_INVALID_LINES != 0 {
                return Ok(None);
            }
            return Err(invalid_line());
        };

        // vCard 2.1 parameters without a name get one from their value.
        let (named, nameless): (Vec<_>, Vec<_>) = parameters.into_iter().partition(|(_, v)| v.is_some());
        let mut property = create_property(kind, root, &name, named);
        for (n, _) in nameless {
            property.add_parameter(None, Some(vec![n]));
        }

        if property.param("ENCODING").is_some_and(|e| e.to_string().eq_ignore_ascii_case("QUOTED-PRINTABLE")) {
            let qp = self.extract_quoted_printable_value();
            property.set_quoted_printable_value(&qp)?;
        } else {
            let mut charset = "UTF-8".to_owned();
            if document_type(kind, root) == DocumentType::VCard21
                && let Some(c) = property.param("CHARSET")
            {
                charset = c.to_string();
            }
            let value = match charset.to_ascii_lowercase().as_str() {
                "utf-8" => value,
                "windows-1252" => convert_from_8bit(&value, true),
                "iso-8859-1" => convert_from_8bit(&value, false),
                _ => return Err(Error::Parse(format!("Unsupported CHARSET: {charset}"))),
            };
            property.set_raw_mimedir_value(&value)?;
        }
        Ok(Some(property))
    }

    /// `extractQuotedPrintableValue()`.
    fn extract_quoted_printable_value(&mut self) -> String {
        let mut value = match self.raw_line.find(':') {
            Some(i) if i > 0 => self.raw_line[i + 1..].to_owned(),
            _ => String::new(),
        };
        value = value.replace("\n ", "\n");
        if self.options & OPTION_FORGIVING != 0 {
            while value.ends_with('=') && self.line_buffer.as_deref().is_some_and(|b| !php_empty(b)) {
                self.read_line();
                value.push('\n');
                value.push_str(&self.raw_line);
            }
        }
        value
    }
}

fn invalid_line() -> Error {
    Error::Parse("Invalid Mimedir file. Line did not follow iCalendar/vCard conventions".into())
}

fn starts_with_ignore_case(line: &str, prefix: &str) -> bool {
    line.len() >= prefix.len() && line.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// `mb_convert_encoding($value, 'UTF-8', 'Windows-1252' | 'ISO-8859-1')`.
fn convert_from_8bit(value: &str, windows: bool) -> String {
    const CP1252: [char; 32] = [
        '€', '?', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '?', 'Ž', '?', '?', '‘', '’', '“', '”', '•',
        '–', '—', '˜', '™', 'š', '›', 'œ', '?', 'ž', 'Ÿ',
    ];
    value
        .bytes()
        .map(|b| if windows && (0x80..0xa0).contains(&b) { CP1252[usize::from(b - 0x80)] } else { char::from(b) })
        .collect()
}

/// `unescapeParam()` (RFC 6868: `^n`, `^^`, `^'`).
fn unescape_param(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('^', Some('n')) => {
                out.push('\n');
                chars.next();
            }
            ('^', Some('^')) => {
                out.push('^');
                chars.next();
            }
            ('^', Some('\'')) => {
                out.push('"');
                chars.next();
            }
            (c, _) => out.push(c),
        }
    }
    out
}

enum Token<'a> {
    Name(&'a str),
    Value(&'a str),
    ParamName(&'a str),
    ParamValue(&'a str),
}

/// The tokens `readProperty()`'s regex finds (`preg_match_all`), in order.
fn tokenize(line: &str, forgiving: bool) -> Vec<Token<'_>> {
    let b = line.as_bytes();
    let name_char =
        |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'.' || (forgiving && matches!(c, b'_' | b'\\' | b'/'));
    let param_char = |c: u8| c.is_ascii_alphanumeric() || c == b'-';
    let run = |from: usize, f: &dyn Fn(u8) -> bool| {
        let mut i = from;
        while i < b.len() && f(b[i]) {
            i += 1;
        }
        i
    };
    let mut tokens = Vec::new();
    let mut p = 0;
    while p < b.len() {
        // ^(?P<name> [token]+ ) (?=[;:])
        if p == 0 {
            let end = run(0, &name_char);
            if end > 0 && matches!(b.get(end), Some(b';' | b':')) {
                tokens.push(Token::Name(&line[..end]));
                p = end;
                continue;
            }
        }
        // (?<=:)(?P<propValue> .+)$
        if p > 0 && b[p - 1] == b':' {
            tokens.push(Token::Value(&line[p..]));
            break;
        }
        // ;(?P<paramName> [token]+) (?=[=;:])
        if b[p] == b';' {
            let end = run(p + 1, &param_char);
            if end > p + 1 && matches!(b.get(end), Some(b'=' | b';' | b':')) {
                tokens.push(Token::ParamName(&line[p + 1..end]));
                p = end;
                continue;
            }
        }
        // (=|,)(?P<paramValue> (?: [^";:,]*) | "(?: [^"]+)" ) (?=[;:,])
        if b[p] == b'=' || b[p] == b',' {
            let end = run(p + 1, &|c| !matches!(c, b'"' | b';' | b':' | b','));
            if matches!(b.get(end), Some(b';' | b':' | b',')) {
                tokens.push(Token::ParamValue(&line[p + 1..end]));
                p = end;
                continue;
            }
            if b.get(p + 1) == Some(&b'"')
                && let Some(close) = line[p + 2..].find('"').map(|i| p + 2 + i)
                && close > p + 2
                && matches!(b.get(close + 1), Some(b';' | b':' | b','))
            {
                tokens.push(Token::ParamValue(&line[p + 1..=close]));
                p = close + 1;
                continue;
            }
        }
        p += 1;
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_at_75_octets() {
        let line = format!("NOTE:{}", "a".repeat(100));
        let out = fold(&line);
        let lines: Vec<&str> = out.split("\r\n").collect();
        assert_eq!(lines[0].len(), 75);
        assert_eq!(lines[1], format!(" {}", "a".repeat(30)));
        assert_eq!(lines[2], "");
    }

    #[test]
    fn folds_without_splitting_characters() {
        let line = format!("NOTE:a{}", "é".repeat(40));
        let out = fold(&line);
        let first = out.split("\r\n").next().unwrap();
        assert_eq!(first.len(), 74);
        assert_eq!(out.replace("\r\n ", ""), format!("{line}\r\n"));
    }

    #[test]
    fn reads_and_serializes_a_vcard() {
        let doc = read("BEGIN:VCARD\r\nVERSION:3.0\r\nN:Doe;John;;;\r\nFN:John\r\n  Doe\r\nEMAIL;TYPE=home;TYPE=pref:mail@example.org\r\nNOTE:a\\, b\\nc\r\nEND:VCARD\r\n", 0).unwrap();
        assert_eq!(doc.kind, Kind::VCard);
        assert_eq!(doc.property("FN").unwrap().to_string(), "John Doe");
        assert_eq!(doc.property("N").unwrap().get_parts()[1].as_deref(), Some("John"));
        assert_eq!(doc.property("EMAIL").unwrap().param("TYPE").unwrap().get_parts(), vec!["home", "pref"]);
        assert_eq!(doc.property("NOTE").unwrap().to_string(), "a, b\nc");
        assert_eq!(
            doc.serialize(),
            "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Doe;John;;;\r\nFN:John Doe\r\nEMAIL;TYPE=home,pref:mail@example.org\r\nNOTE:a\\, b\\nc\r\nEND:VCARD\r\n"
        );
    }

    #[test]
    fn strict_reading_rejects_invalid_lines() {
        assert!(read("BEGIN:VCARD\nVERSION:4.0\nX_Y:z\nEND:VCARD", 0).is_err());
        let doc = read("BEGIN:VCARD\nVERSION:4.0\nX_Y:z\nEND:VCARD", OPTION_FORGIVING).unwrap();
        assert_eq!(doc.property("X_Y").unwrap().to_string(), "z");
        let doc = read("BEGIN:VCARD\nVERSION:4.0\n:z\nEND:VCARD", OPTION_IGNORE_INVALID_LINES).unwrap();
        assert_eq!(doc.children().len(), 1);
    }

    #[test]
    fn a_missing_end_is_tolerated_at_the_top_only() {
        assert!(read("BEGIN:VCARD\nFN:x", 0).is_ok());
        assert!(read("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:x", 0).is_err());
        assert!(read("BEGIN:VCARD\nFN:x\nEND:VCALENDAR", 0).is_err());
        assert!(read("FOO", 0).is_err());
    }

    #[test]
    fn serializes_vtimezone_after_properties_and_before_components() {
        let mut cal = Document::vcalendar(vec![("VERSION", "2.0".into())]).unwrap();
        let todo = cal.create_component("VTODO", vec![("UID", "u".into())], false).unwrap();
        cal.add(Node::Component(todo));
        let tz = cal.create_component("VTIMEZONE", vec![("TZID", "UTC".into())], true).unwrap();
        cal.add(Node::Component(tz));
        assert_eq!(
            cal.serialize(),
            format!(
                "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sabre//Sabre VObject {VERSION}//EN\r\nCALSCALE:GREGORIAN\r\nBEGIN:VTIMEZONE\r\nTZID:UTC\r\nEND:VTIMEZONE\r\nBEGIN:VTODO\r\nUID:u\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
            )
        );
    }

    #[test]
    fn dates_in_a_vevent() {
        let mut cal = Document::vcalendar(vec![]).unwrap();
        let mut dtstart = cal.create_property("DTSTART", "19811029".into(), vec![]).unwrap();
        dtstart.set_param("VALUE", Some("DATE".into())).unwrap();
        assert_eq!(dtstart.serialize(DocumentType::ICalendar20), "DTSTART;VALUE=DATE:19811029\r\n");
        let at = NaiveDate::from_ymd_opt(2025, 8, 14).unwrap().and_hms_opt(1, 2, 3).unwrap();
        let dtstamp = cal.create_property("DTSTAMP", DateTime::utc(at).into(), vec![]).unwrap();
        assert_eq!(dtstamp.serialize(DocumentType::ICalendar20), "DTSTAMP:20250814T010203Z\r\n");
        let due =
            read("BEGIN:VCALENDAR\nBEGIN:VTODO\nDUE;TZID=Europe/Paris:20250814T100000\nEND:VTODO\nEND:VCALENDAR", 0)
                .unwrap();
        let dt = due.component("VTODO").unwrap().property("DUE").unwrap().get_date_time().unwrap().unwrap();
        assert_eq!(dt.local, NaiveDate::from_ymd_opt(2025, 8, 14).unwrap().and_hms_opt(10, 0, 0).unwrap());
        assert_eq!(dt.tz.name(), "Europe/Paris");
        cal.set_property("RRULE", "freq=yearly;bymonth=10;;bymonthday=29".into()).unwrap();
        assert_eq!(cal.property("RRULE").unwrap().to_string(), "FREQ=YEARLY;BYMONTH=10;BYMONTHDAY=29");
        assert!(cal.create_property("RRULE", "FREQ".into(), vec![]).is_err());
    }

    #[test]
    fn parses_vcard_dates() {
        let p = parse_vcard_date_time("19811029").unwrap();
        assert_eq!((p.year.as_deref(), p.month.as_deref(), p.date.as_deref()), (Some("1981"), Some("10"), Some("29")));
        let p = parse_vcard_date_time("---29").unwrap();
        assert_eq!((p.year, p.month, p.date.as_deref()), (None, None, Some("29")));
        let p = parse_vcard_date_time("--04-15").unwrap();
        assert_eq!((p.month.as_deref(), p.date.as_deref()), (Some("04"), Some("15")));
        let p = parse_vcard_date_time("1981-10-29T10:20:30Z").unwrap();
        assert_eq!(
            (p.hour.as_deref(), p.second.as_deref(), p.timezone.as_deref()),
            (Some("10"), Some("30"), Some("Z"))
        );
        assert!(parse_vcard_date_time("1981/10").is_err());
    }

    #[test]
    fn parameters_are_quoted_and_guessed() {
        let doc = read("BEGIN:VCARD\nVERSION:3.0\nTEL;CELL;TYPE=home:1\nX-A;P=\"a:b\";Q=c+d:v\nEND:VCARD", 0).unwrap();
        let tel = doc.property("TEL").unwrap();
        assert_eq!(tel.param("TYPE").unwrap().get_parts(), vec!["home", "CELL"]);
        assert_eq!(doc.property("X-A").unwrap().serialize(DocumentType::VCard30), "X-A;P=\"a:b\";Q=\"c+d\":v\r\n");
    }

    #[test]
    fn float_and_integer_values() {
        assert_eq!(php_float_to_string(37.386013), "37.386013");
        assert_eq!(php_float_to_string(-122.082932), "-122.082932");
        assert_eq!(php_float_to_string(1.0), "1");
        assert_eq!(php_float_to_string(1e20), "1.0E+20");
        assert_eq!(php_float_to_string(0.00001), "1.0E-5");
        assert_eq!(php_intval(" 05x"), 5);
        assert_eq!(base64_decode("aGVsbG8=\n"), b"hello");
        assert_eq!(quoted_printable_decode(b"a=3Db=\r\nc"), b"a=bc");
    }
}
