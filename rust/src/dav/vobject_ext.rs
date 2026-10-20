//! The parts of `sabre/vobject` that the CardDAV and CalDAV plugins use and
//! [`crate::vobject`] doesn't port: `VCardConverter` and `Node::validate()`
//! (with the property classes' `getValueType()`, which they need).

use base64::Engine;

use super::backend::php_intval;
use super::server::{DavError, DavResult};
use crate::domains::contact::dav::php_empty;
use crate::vobject::{self, Class, Component, Document, DocumentType, Input, Kind, Node, Parameter, Property};

/// `Node::REPAIR`.
pub const REPAIR: u32 = 1;
/// `Node::PROFILE_CARDDAV`.
pub const PROFILE_CARDDAV: u32 = 2;
/// `Node::PROFILE_CALDAV`.
pub const PROFILE_CALDAV: u32 = 4;

impl Class {
    /// Extends `Property\VCard\DateAndOrTime`.
    fn is_date_and_or_time(self) -> bool {
        matches!(self, Class::VCardDate | Class::VCardDateTime | Class::VCardDateAndOrTime)
    }
}

/// The class of a property of `doc`: by its `VALUE` parameter, else by name.
fn class_of(doc: &Document, property: &Property) -> Class {
    property
        .param("VALUE")
        .and_then(|v| v.get_value())
        .and_then(|v| doc.kind.class_for_value(&v))
        .unwrap_or_else(|| doc.kind.class_for_name(&property.name, doc.document_type()))
}

/// `getValueType()` of a class (`has_time` for the iCalendar dates).
fn value_type(class: Class, has_time: bool) -> &'static str {
    match class {
        Class::Text | Class::FlatText => "TEXT",
        Class::Unknown => "UNKNOWN",
        Class::Uri => "URI",
        Class::UtcOffset => "UTC-OFFSET",
        Class::Time => "TIME",
        Class::CalAddress => "CAL-ADDRESS",
        Class::PhoneNumber => "PHONE-NUMBER",
        Class::TimeStamp => "TIMESTAMP",
        Class::Binary => "BINARY",
        Class::Boolean => "BOOLEAN",
        Class::FloatValue => "FLOAT",
        Class::IntegerValue => "INTEGER",
        Class::VCardDate => "DATE",
        Class::VCardDateTime => "DATE-TIME",
        Class::VCardDateAndOrTime => "DATE-AND-OR-TIME",
        Class::LanguageTag => "LANGUAGE-TAG",
        Class::ICalDate | Class::ICalDateTime => {
            if has_time {
                "DATE-TIME"
            } else {
                "DATE"
            }
        }
        Class::Duration => "DURATION",
        Class::Period => "PERIOD",
        Class::Recur => "RECUR",
    }
}

/// `$property->getValueType()`.
pub fn property_value_type(doc: &Document, property: &Property) -> &'static str {
    value_type(class_of(doc, property), property.has_time())
}

fn internal(e: vobject::Error) -> DavError {
    DavError::from(e)
}

// ------------------------------------------------------------ VCardConverter

/// The value of a property being converted.
#[derive(Debug, Clone)]
enum NewValue {
    Parts(Vec<Option<String>>),
    Bytes(Vec<u8>),
}

/// The property `convertProperty()` builds: `createProperty($name, $value,
/// [], $valueType)`, its group and its parameters.
#[derive(Debug, Clone)]
struct NewProperty {
    group: Option<String>,
    name: String,
    class: Class,
    value: NewValue,
    parameters: Vec<(String, Vec<String>)>,
}

impl NewProperty {
    /// `$output->createProperty($name, $value, [], $valueType)`.
    fn create(target: DocumentType, name: &str, value: NewValue, value_type: Option<&str>) -> NewProperty {
        let class = value_type
            .and_then(|v| Kind::VCard.class_for_value(v))
            .unwrap_or_else(|| Kind::VCard.class_for_name(name, target));
        NewProperty { group: None, name: name.to_owned(), class, value, parameters: Vec::new() }
    }

    /// `getValue()` of a text value.
    fn value(&self) -> String {
        match &self.value {
            NewValue::Parts(parts) => match parts.len() {
                0 => String::new(),
                1 => parts[0].clone().unwrap_or_default(),
                _ => parts.iter().map(|p| p.clone().unwrap_or_default()).collect::<Vec<_>>().join(";"),
            },
            NewValue::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        }
    }

    /// `$property[$name] = $value`.
    fn set_parameter(&mut self, name: &str, value: &str) {
        let name = name.to_ascii_uppercase();
        match self.parameters.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = vec![value.to_owned()],
            None => self.parameters.push((name, vec![value.to_owned()])),
        }
    }

    /// `$property->add($name, $value)`.
    fn add_parameter(&mut self, name: &str, values: Vec<String>) {
        let name = name.to_ascii_uppercase();
        match self.parameters.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1.extend(values),
            None => self.parameters.push((name, values)),
        }
    }

    /// The property in `output`.
    fn build(self, output: &Document) -> DavResult<Property> {
        let name = match &self.group {
            Some(group) if !php_empty(group) => format!("{group}.{}", self.name),
            _ => self.name.clone(),
        };
        if let NewValue::Bytes(bytes) = &self.value {
            return binary_property(output, &name, &self.parameters, bytes);
        }
        let mut parameters: Vec<(&str, Option<String>)> = Vec::new();
        for (param, values) in &self.parameters {
            if values.is_empty() {
                parameters.push((param, None));
            }
            for value in values {
                parameters.push((param, Some(value.clone())));
            }
        }
        let value = match self.value {
            NewValue::Parts(parts) => Input::Parts(parts),
            NewValue::Bytes(_) => unreachable!("handled above"),
        };
        output.create_property(&name, value, parameters).map_err(internal)
    }
}

/// `Parameter::serialize()` of one parameter.
fn serialize_parameter(name: &str, values: &[String]) -> String {
    if values.is_empty() {
        return format!("{name}=");
    }
    let items: Vec<String> = values
        .iter()
        .map(|item| {
            if !item.contains(['\n', '"', ':', ';', '^', ',', '+']) {
                item.clone()
            } else {
                format!("\"{}\"", item.replace('^', "^^").replace('\n', "^n").replace('"', "^'"))
            }
        })
        .collect();
    format!("{name}={}", items.join(","))
}

/// A `Binary` property of `bytes`: read from its MimeDir line, since a
/// value of [`Input`] is text.
fn binary_property(
    output: &Document,
    name: &str,
    parameters: &[(String, Vec<String>)],
    bytes: &[u8],
) -> DavResult<Property> {
    let mut line = name.to_owned();
    for (param, values) in parameters {
        line.push(';');
        line.push_str(&serialize_parameter(param, values));
    }
    line.push(':');
    line.push_str(&base64::engine::general_purpose::STANDARD.encode(bytes));
    let version = output.property("VERSION").map(ToString::to_string).unwrap_or_default();
    let text = format!("BEGIN:VCARD\r\nVERSION:{version}\r\n{line}\r\nEND:VCARD\r\n");
    let doc = vobject::read(&text, 0).map_err(internal)?;
    doc.children()
        .into_iter()
        .find_map(|node| match node {
            Node::Property(p) if p.name != "VERSION" => Some(p.clone()),
            _ => None,
        })
        .ok_or_else(|| DavError::exception("The binary property could not be read back"))
}

/// The bytes of a `Binary` property (its value is base64 in the MimeDir).
fn binary_bytes(property: &Property) -> DavResult<Vec<u8>> {
    let mut tmp = Document::vcard(vec![("VERSION", "3.0".into())]).map_err(internal)?;
    tmp.remove("PRODID");
    tmp.remove("UID");
    tmp.add(Node::Property(property.clone()));
    let text = tmp.serialize().replace("\r\n ", "");
    let line = text.split("\r\n").nth(2).unwrap_or_default();
    let mut quoted = false;
    let mut value = "";
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                value = &line[i + 1..];
                break;
            }
            _ => {}
        }
    }
    Ok(base64_decode(value))
}

/// PHP's `base64_decode()` (invalid characters are skipped).
fn base64_decode(value: &str) -> Vec<u8> {
    let clean: String = value.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/').collect();
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(clean.as_bytes()).unwrap_or_default()
}

/// `VCardConverter::convert($input, $targetVersion)`.
pub fn convert(input: &Document, target_version: DocumentType) -> DavResult<Document> {
    let input_version = input.document_type();
    if input_version == target_version {
        return Ok(input.clone());
    }

    if !matches!(input_version, DocumentType::VCard21 | DocumentType::VCard30 | DocumentType::VCard40) {
        return Err(DavError::internal(
            "InvalidArgumentException",
            "Only vCard 2.1, 3.0 and 4.0 are supported for the input data",
        ));
    }
    if !matches!(target_version, DocumentType::VCard30 | DocumentType::VCard40) {
        return Err(DavError::internal(
            "InvalidArgumentException",
            "You can only use vCard 3.0 or 4.0 for the target version",
        ));
    }

    let new_version = if target_version == DocumentType::VCard40 { "4.0" } else { "3.0" };

    let mut output = Document::vcard(vec![("VERSION", new_version.into())]).map_err(internal)?;

    // We might have generated a default UID. Remove it!
    output.remove("UID");

    for child in input.children() {
        if let Node::Property(property) = child {
            convert_property(input, &mut output, property, target_version)?;
        }
    }

    Ok(output)
}

const ANNIVERSARY_LABEL: &str = "_$!<Anniversary>!$_";

/// `VCardConverter::convertProperty()`.
fn convert_property(
    input: &Document,
    output: &mut Document,
    property: &Property,
    target_version: DocumentType,
) -> DavResult<()> {
    // Skipping these, those are automatically added.
    if matches!(property.name.as_str(), "VERSION" | "PRODID") {
        return Ok(());
    }

    let mut parameters: Vec<(String, Vec<String>)> =
        property.parameters().iter().map(|p| (p.name.clone(), p.get_parts())).collect();
    let mut value_type_param = None;
    if let Some(i) = parameters.iter().position(|(n, _)| n == "VALUE") {
        value_type_param = Some(parameters[i].1.join(","));
        parameters.remove(i);
    }
    if value_type_param.as_deref().is_none_or(php_empty) {
        value_type_param = Some(property_value_type(input, property).to_owned());
    }
    if target_version != DocumentType::VCard30 && value_type_param.as_deref() == Some("PHONE-NUMBER") {
        value_type_param = None;
    }

    let input_class = class_of(input, property);
    let value = if input_class == Class::Binary {
        NewValue::Bytes(binary_bytes(property)?)
    } else {
        NewValue::Parts(property.get_parts())
    };
    let mut new_property = NewProperty::create(target_version, &property.name, value, value_type_param.as_deref());
    let get_value = || property.get_value().unwrap_or_default();

    if target_version == DocumentType::VCard30 {
        if input_class == Class::Uri && matches!(property.name.as_str(), "PHOTO" | "LOGO" | "SOUND") {
            new_property = convert_uri_to_binary(target_version, new_property);
        } else if input_class.is_date_and_or_time() {
            // In vCard 4, the birth year may be optional. This is not the
            // case for vCard 3. Apple has a workaround for this that
            // allows applications that support Apple's extension still
            // omit birthyears in vCard 3, but applications that do not
            // support this, will just use a random birthyear. We're
            // choosing 1604 for the birthyear, because that's what apple
            // uses.
            let parts = vobject::parse_vcard_date_time(&get_value()).map_err(internal)?;
            if parts.year.is_none() {
                let new_value = format!("1604-{}-{}", parts.month.unwrap_or_default(), parts.date.unwrap_or_default());
                new_property.value = NewValue::Parts(vec![Some(new_value)]);
                new_property.set_parameter("X-APPLE-OMIT-YEAR", "1604");
            }

            if new_property.name == "ANNIVERSARY" {
                // Microsoft non-standard anniversary
                new_property.name = "X-ANNIVERSARY".to_owned();

                // We also need to add a new apple property for the same
                // purpose. This apple property needs a 'label' in the same
                // group, so we first need to find a groupname that doesn't
                // exist yet.
                let mut x = 1;
                while !output.select(&format!("ITEM{x}.")).is_empty() {
                    x += 1;
                }
                output
                    .add_property(
                        &format!("ITEM{x}.X-ABDATE"),
                        Input::Text(new_property.value()),
                        vec![("VALUE", Some("DATE-AND-OR-TIME".to_owned()))],
                    )
                    .map_err(internal)?;
                output
                    .add_property(&format!("ITEM{x}.X-ABLABEL"), Input::Text(ANNIVERSARY_LABEL.to_owned()), Vec::new())
                    .map_err(internal)?;
            }
        } else if property.name == "KIND" {
            match get_value().to_lowercase().as_str() {
                // vCard 3.0 does not have an equivalent to KIND:ORG, but
                // apple has an extension that means the same thing.
                "org" => {
                    new_property = NewProperty::create(
                        target_version,
                        "X-ABSHOWAS",
                        NewValue::Parts(vec![Some("COMPANY".to_owned())]),
                        None,
                    );
                }
                // Individual is implicit, so we skip it.
                "individual" => return Ok(()),
                // OS X addressbook property
                "group" => {
                    new_property = NewProperty::create(
                        target_version,
                        "X-ADDRESSBOOKSERVER-KIND",
                        NewValue::Parts(vec![Some("GROUP".to_owned())]),
                        None,
                    );
                }
                _ => {}
            }
        } else if property.name == "MEMBER" {
            new_property = NewProperty::create(
                target_version,
                "X-ADDRESSBOOKSERVER-MEMBER",
                NewValue::Parts(vec![property.get_value()]),
                None,
            );
        }
    } else if target_version == DocumentType::VCard40 {
        // These properties were removed in vCard 4.0
        if matches!(property.name.as_str(), "NAME" | "MAILER" | "LABEL" | "CLASS") {
            return Ok(());
        }

        if input_class == Class::Binary {
            new_property = convert_binary_to_uri(target_version, new_property, &mut parameters);
        } else if input_class.is_date_and_or_time()
            && let Some(i) = parameters.iter().position(|(n, _)| n == "X-APPLE-OMIT-YEAR")
        {
            // If a property such as BDAY contained 'X-APPLE-OMIT-YEAR',
            // then we're stripping the year from the vcard 4 value.
            let parts = vobject::parse_vcard_date_time(&get_value()).map_err(internal)?;
            if parts.year == property.param("X-APPLE-OMIT-YEAR").and_then(Parameter::get_value) {
                let new_value = format!("--{}-{}", parts.month.unwrap_or_default(), parts.date.unwrap_or_default());
                new_property.value = NewValue::Parts(vec![Some(new_value)]);
            }

            // Regardless if the year matched or not, we do need to strip X-APPLE-OMIT-YEAR.
            parameters.remove(i);
        }

        match property.name.as_str() {
            "X-ABSHOWAS" => {
                if get_value().to_uppercase() == "COMPANY" {
                    new_property = NewProperty::create(
                        target_version,
                        "KIND",
                        NewValue::Parts(vec![Some("ORG".to_owned())]),
                        None,
                    );
                }
            }
            "X-ADDRESSBOOKSERVER-KIND" => {
                if get_value().to_uppercase() == "GROUP" {
                    new_property = NewProperty::create(
                        target_version,
                        "KIND",
                        NewValue::Parts(vec![Some("GROUP".to_owned())]),
                        None,
                    );
                }
            }
            "X-ADDRESSBOOKSERVER-MEMBER" => {
                new_property =
                    NewProperty::create(target_version, "MEMBER", NewValue::Parts(vec![property.get_value()]), None);
            }
            "X-ANNIVERSARY" => {
                new_property.name = "ANNIVERSARY".to_owned();
                // If we already have an anniversary property with the same value, ignore.
                if output
                    .properties("ANNIVERSARY")
                    .iter()
                    .any(|a| a.get_value().unwrap_or_default() == new_property.value())
                {
                    return Ok(());
                }
            }
            "X-ABDATE" => 'abdate: {
                // Find out what the label was, if it exists.
                let Some(group) = property.group.as_deref().filter(|g| !php_empty(g)) else {
                    break 'abdate;
                };
                let label = input.property(&format!("{group}.X-ABLABEL"));

                // We only support converting anniversaries.
                if label.and_then(Property::get_value).as_deref() != Some(ANNIVERSARY_LABEL) {
                    break 'abdate;
                }

                // If we already have an anniversary property with the same value, ignore.
                if output
                    .properties("ANNIVERSARY")
                    .iter()
                    .any(|a| a.get_value().unwrap_or_default() == new_property.value())
                {
                    return Ok(());
                }
                new_property.name = "ANNIVERSARY".to_owned();
            }
            // Apple's per-property label system: we can safely remove the
            // anniversary ones, as they are converted to ANNIVERSARY properties.
            "X-ABLABEL" if new_property.value() == ANNIVERSARY_LABEL => return Ok(()),
            _ => {}
        }
    }

    // set property group
    new_property.group = property.group.clone();

    if target_version == DocumentType::VCard40 {
        convert_parameters_40(&mut new_property, &parameters);
    } else {
        convert_parameters_30(&mut new_property, &parameters);
    }

    // Lastly, we need to see if there's a need for a VALUE parameter.
    //
    // We can do that by instantiating a empty property with that name, and
    // seeing if the default valueType is identical to the current one.
    let temp_class = Kind::VCard.class_for_name(&new_property.name, target_version);
    let new_type = value_type(new_property.class, true);
    if value_type(temp_class, true) != new_type {
        new_property.set_parameter("VALUE", new_type);
    }

    let built = new_property.build(output)?;
    output.add(Node::Property(built));
    Ok(())
}

/// `VCardConverter::convertBinaryToUri()`.
fn convert_binary_to_uri(
    target: DocumentType,
    new_property: NewProperty,
    parameters: &mut Vec<(String, Vec<String>)>,
) -> NewProperty {
    let value = match &new_property.value {
        NewValue::Bytes(b) => b.clone(),
        NewValue::Parts(_) => new_property.value().into_bytes(),
    };
    let mut converted = NewProperty::create(target, &new_property.name, NewValue::Parts(Vec::new()), Some("URI"));

    let mut mime_type = "application/octet-stream".to_owned();

    // See if we can find a better mimetype.
    if let Some(i) = parameters.iter().position(|(n, _)| n == "TYPE") {
        let mut new_types = Vec::new();
        for type_part in parameters[i].1.clone() {
            if matches!(type_part.to_uppercase().as_str(), "JPEG" | "PNG" | "GIF") {
                mime_type = format!("image/{}", type_part.to_lowercase());
            } else {
                new_types.push(type_part);
            }
        }

        // If there were any parameters we're not converting to a
        // mime-type, we need to keep them.
        if new_types.is_empty() {
            parameters.remove(i);
        } else {
            parameters[i].1 = new_types;
        }
    }

    converted.value = NewValue::Parts(vec![Some(format!(
        "data:{mime_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(value)
    ))]);

    converted
}

/// `VCardConverter::convertUriToBinary()`.
fn convert_uri_to_binary(target: DocumentType, new_property: NewProperty) -> NewProperty {
    let value = new_property.value();

    // Only converting data: uris
    if !value.starts_with("data:") {
        return new_property;
    }

    let mut converted = NewProperty::create(target, &new_property.name, NewValue::Bytes(Vec::new()), Some("BINARY"));

    let comma = value.find(',');
    let mut mime_type = match comma {
        Some(c) if c >= 5 => value[5..c].to_owned(),
        _ => String::new(),
    };
    let data = comma.map(|c| &value[c + 1..]).unwrap_or_default();
    if let Some(semicolon) = mime_type.find(';').filter(|i| *i > 0) {
        mime_type.truncate(semicolon);
        converted.value = NewValue::Bytes(base64_decode(data));
    } else {
        converted.value = NewValue::Bytes(data.as_bytes().to_vec());
    }

    converted.set_parameter("ENCODING", "b");
    match mime_type.as_str() {
        "image/jpeg" => converted.set_parameter("TYPE", "JPEG"),
        "image/png" => converted.set_parameter("TYPE", "PNG"),
        "image/gif" => converted.set_parameter("TYPE", "GIF"),
        _ => {}
    }

    converted
}

/// `VCardConverter::convertParameters40()`.
fn convert_parameters_40(new_property: &mut NewProperty, parameters: &[(String, Vec<String>)]) {
    // Adding all parameters.
    for (name, parts) in parameters {
        match name.as_str() {
            // We need to see if there's any TYPE=PREF, because in vCard 4 that's now PREF=1.
            "TYPE" => {
                for param_part in parts {
                    if param_part.to_uppercase() == "PREF" {
                        new_property.add_parameter("PREF", vec!["1".to_owned()]);
                    } else {
                        new_property.add_parameter(name, vec![param_part.clone()]);
                    }
                }
            }
            // These no longer exist in vCard 4
            "ENCODING" | "CHARSET" => {}
            _ => new_property.add_parameter(name, parts.clone()),
        }
    }
}

/// `VCardConverter::convertParameters30()`.
fn convert_parameters_30(new_property: &mut NewProperty, parameters: &[(String, Vec<String>)]) {
    // Adding all parameters.
    for (name, parts) in parameters {
        match name.as_str() {
            // This value only existed in vCard 2.1, and should be removed for anything else.
            "ENCODING" => {
                if parts.join(",").to_uppercase() != "QUOTED-PRINTABLE" {
                    new_property.add_parameter(name, parts.clone());
                }
            }
            // Converting PREF=1 to TYPE=PREF. Any other PREF numbers we'll drop.
            "PREF" => {
                if parts.join(",") == "1" {
                    new_property.add_parameter("TYPE", vec!["PREF".to_owned()]);
                }
            }
            _ => new_property.add_parameter(name, parts.clone()),
        }
    }
}

// ------------------------------------------------------------------ validate

/// A message of `validate()`: its level (1 repaired, 2 warning, 3 error).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub level: u8,
    pub message: String,
}

fn message(level: u8, message: impl Into<String>) -> Message {
    Message { level, message: message.into() }
}

/// The instant of a `DateTimeImmutable` (its UTC time).
pub fn instant(dt: &vobject::DateTime) -> chrono::NaiveDateTime {
    use chrono::{Offset, TimeZone};
    match dt.tz {
        vobject::Tz::Utc => dt.local,
        vobject::Tz::Named(tz) => match tz.from_local_datetime(&dt.local) {
            chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => t.naive_utc(),
            chrono::LocalResult::None => {
                let offset = tz.offset_from_utc_datetime(&dt.local).fix().local_minus_utc();
                dt.local - chrono::Duration::seconds(i64::from(offset))
            }
        },
        vobject::Tz::Offset(secs) => dt.local - chrono::Duration::seconds(i64::from(secs)),
    }
}

/// `getValidationRules()` of a component.
fn validation_rules(kind: Kind, name: &str, parent_has_method: bool) -> Vec<(&'static str, char)> {
    match (kind, name) {
        (Kind::VCard, _) => vec![
            ("ADR", '*'),
            ("ANNIVERSARY", '?'),
            ("BDAY", '?'),
            ("CALADRURI", '*'),
            ("CALURI", '*'),
            ("CATEGORIES", '*'),
            ("CLIENTPIDMAP", '*'),
            ("EMAIL", '*'),
            ("FBURL", '*'),
            ("IMPP", '*'),
            ("GENDER", '?'),
            ("GEO", '*'),
            ("KEY", '*'),
            ("KIND", '?'),
            ("LANG", '*'),
            ("LOGO", '*'),
            ("MEMBER", '*'),
            ("N", '?'),
            ("NICKNAME", '*'),
            ("NOTE", '*'),
            ("ORG", '*'),
            ("PHOTO", '*'),
            ("PRODID", '?'),
            ("RELATED", '*'),
            ("REV", '?'),
            ("ROLE", '*'),
            ("SOUND", '*'),
            ("SOURCE", '*'),
            ("TEL", '*'),
            ("TITLE", '*'),
            ("TZ", '*'),
            ("URL", '*'),
            ("VERSION", '1'),
            ("XML", '*'),
            ("UID", '?'),
        ],
        (Kind::VCalendar, "VCALENDAR") => vec![("PRODID", '1'), ("VERSION", '1'), ("CALSCALE", '?'), ("METHOD", '?')],
        (Kind::VCalendar, "VEVENT") => vec![
            ("UID", '1'),
            ("DTSTAMP", '1'),
            ("DTSTART", if parent_has_method { '?' } else { '1' }),
            ("CLASS", '?'),
            ("CREATED", '?'),
            ("DESCRIPTION", '?'),
            ("GEO", '?'),
            ("LAST-MODIFIED", '?'),
            ("LOCATION", '?'),
            ("ORGANIZER", '?'),
            ("PRIORITY", '?'),
            ("SEQUENCE", '?'),
            ("STATUS", '?'),
            ("SUMMARY", '?'),
            ("TRANSP", '?'),
            ("URL", '?'),
            ("RECURRENCE-ID", '?'),
            ("RRULE", '?'),
            ("DTEND", '?'),
            ("DURATION", '?'),
            ("ATTACH", '*'),
            ("ATTENDEE", '*'),
            ("CATEGORIES", '*'),
            ("COMMENT", '*'),
            ("CONTACT", '*'),
            ("EXDATE", '*'),
            ("REQUEST-STATUS", '*'),
            ("RELATED-TO", '*'),
            ("RESOURCES", '*'),
            ("RDATE", '*'),
        ],
        (Kind::VCalendar, "VTODO") => vec![
            ("UID", '1'),
            ("DTSTAMP", '1'),
            ("CLASS", '?'),
            ("COMPLETED", '?'),
            ("CREATED", '?'),
            ("DESCRIPTION", '?'),
            ("DTSTART", '?'),
            ("GEO", '?'),
            ("LAST-MODIFIED", '?'),
            ("LOCATION", '?'),
            ("ORGANIZER", '?'),
            ("PERCENT", '?'),
            ("PRIORITY", '?'),
            ("RECURRENCE-ID", '?'),
            ("SEQUENCE", '?'),
            ("STATUS", '?'),
            ("SUMMARY", '?'),
            ("URL", '?'),
            ("RRULE", '?'),
            ("DUE", '?'),
            ("DURATION", '?'),
            ("ATTACH", '*'),
            ("ATTENDEE", '*'),
            ("CATEGORIES", '*'),
            ("COMMENT", '*'),
            ("CONTACT", '*'),
            ("EXDATE", '*'),
            ("REQUEST-STATUS", '*'),
            ("RELATED-TO", '*'),
            ("RESOURCES", '*'),
            ("RDATE", '*'),
        ],
        (Kind::VCalendar, "VJOURNAL") => vec![
            ("UID", '1'),
            ("DTSTAMP", '1'),
            ("CLASS", '?'),
            ("CREATED", '?'),
            ("DTSTART", '?'),
            ("LAST-MODIFIED", '?'),
            ("ORGANIZER", '?'),
            ("RECURRENCE-ID", '?'),
            ("SEQUENCE", '?'),
            ("STATUS", '?'),
            ("SUMMARY", '?'),
            ("URL", '?'),
            ("RRULE", '?'),
            ("ATTACH", '*'),
            ("ATTENDEE", '*'),
            ("CATEGORIES", '*'),
            ("COMMENT", '*'),
            ("CONTACT", '*'),
            ("DESCRIPTION", '*'),
            ("EXDATE", '*'),
            ("RELATED-TO", '*'),
            ("RDATE", '*'),
        ],
        (Kind::VCalendar, "VFREEBUSY") => vec![
            ("UID", '1'),
            ("DTSTAMP", '1'),
            ("CONTACT", '?'),
            ("DTSTART", '?'),
            ("DTEND", '?'),
            ("ORGANIZER", '?'),
            ("URL", '?'),
            ("ATTENDEE", '*'),
            ("COMMENT", '*'),
            ("FREEBUSY", '*'),
            ("REQUEST-STATUS", '*'),
        ],
        (Kind::VCalendar, "VAVAILABILITY") => vec![
            ("UID", '1'),
            ("DTSTAMP", '1'),
            ("BUSYTYPE", '?'),
            ("CLASS", '?'),
            ("CREATED", '?'),
            ("DESCRIPTION", '?'),
            ("DTSTART", '?'),
            ("LAST-MODIFIED", '?'),
            ("ORGANIZER", '?'),
            ("PRIORITY", '?'),
            ("SEQUENCE", '?'),
            ("SUMMARY", '?'),
            ("URL", '?'),
            ("DTEND", '?'),
            ("DURATION", '?'),
            ("CATEGORIES", '*'),
            ("COMMENT", '*'),
            ("CONTACT", '*'),
        ],
        (Kind::VCalendar, "AVAILABLE") => vec![
            ("UID", '1'),
            ("DTSTART", '1'),
            ("DTSTAMP", '1'),
            ("DTEND", '?'),
            ("DURATION", '?'),
            ("CREATED", '?'),
            ("DESCRIPTION", '?'),
            ("LAST-MODIFIED", '?'),
            ("RECURRENCE-ID", '?'),
            ("RRULE", '?'),
            ("SUMMARY", '?'),
            ("CATEGORIES", '*'),
            ("COMMENT", '*'),
            ("CONTACT", '*'),
            ("EXDATE", '*'),
            ("RDATE", '*'),
            ("AVAILABLE", '*'),
        ],
        (Kind::VCalendar, "VALARM") => {
            vec![("ACTION", '1'), ("TRIGGER", '1'), ("DURATION", '?'), ("REPEAT", '?'), ("ATTACH", '?')]
        }
        (Kind::VCalendar, "VTIMEZONE") => {
            vec![("TZID", '1'), ("LAST-MODIFIED", '?'), ("TZURL", '?'), ("STANDARD", '*'), ("DAYLIGHT", '*')]
        }
        _ => Vec::new(),
    }
}

/// `Document::validate($options)`: the messages, with the repairs made.
pub fn validate(doc: &mut Document, options: u32) -> DavResult<Vec<Message>> {
    match doc.kind {
        Kind::VCard => validate_vcard(doc, options),
        Kind::VCalendar => validate_vcalendar(doc, options),
    }
}

/// `VCard::validate($options)`.
fn validate_vcard(doc: &mut Document, options: u32) -> DavResult<Vec<Message>> {
    let mut warnings = Vec::new();

    if doc.select("VERSION").len() == 1 {
        let version = doc.property("VERSION").map(ToString::to_string).unwrap_or_default();
        if !matches!(version.as_str(), "2.1" | "3.0" | "4.0") {
            warnings.push(message(
                3,
                "Only vcard version 4.0 (RFC6350), version 3.0 (RFC2426) or version 2.1 (icm-vcard-2.1) are supported.",
            ));
            if options & REPAIR != 0 {
                doc.set_property("VERSION", "2.1".into()).map_err(internal)?;
            }
        }
        if version == "2.1" && options & PROFILE_CARDDAV != 0 {
            warnings.push(message(3, "CardDAV servers are not allowed to accept vCard 2.1."));
        }
    }

    if doc.select("UID").is_empty() {
        let (mut level, text) = if options & PROFILE_CARDDAV != 0 {
            // Required for CardDAV
            (3, "vCards on CardDAV servers MUST have a UID property.")
        } else {
            // Not required for regular vcards
            (2, "Adding a UID to a vCard property is recommended.")
        };
        if options & REPAIR != 0 {
            doc.set_property("UID", uuid::Uuid::new_v4().to_string().into()).map_err(internal)?;
            level = 1;
        }
        warnings.push(message(level, text));
    }

    let fn_count = doc.select("FN").len();
    if fn_count != 1 {
        let mut repaired = false;
        if options & REPAIR != 0 && fn_count == 0 {
            // We're going to try to see if we can use the contents of the N property.
            let value = if let Some(n) = doc.property("N") {
                let value: Vec<String> = n.to_string().split(';').map(str::to_owned).collect();
                Some(match value.get(1).filter(|v| !php_empty(v)) {
                    Some(first) => format!("{first} {}", value[0]),
                    None => value[0].clone(),
                })
            } else {
                // Otherwise, the ORG, NICKNAME or EMAIL property may work
                ["ORG", "NICKNAME", "EMAIL"].iter().find_map(|name| doc.property(name).map(ToString::to_string))
            };
            if let Some(value) = value {
                doc.set_property("FN", value.into()).map_err(internal)?;
                repaired = true;
            }
        }
        warnings.push(message(
            if repaired { 1 } else { 3 },
            "The FN property must appear in the VCARD component exactly 1 time",
        ));
    }

    let ctx = doc.clone();
    let mut messages = validate_component(&ctx, doc, options, false)?;
    messages.extend(warnings);
    Ok(messages)
}

/// `VCalendar::validate($options)`.
fn validate_vcalendar(doc: &mut Document, options: u32) -> DavResult<Vec<Message>> {
    let ctx = doc.clone();
    let mut warnings = validate_component(&ctx, doc, options, false)?;

    if let Some(version) = doc.property("VERSION")
        && version.to_string() != "2.0"
    {
        warnings.push(message(3, "Only iCalendar version 2.0 as defined in rfc5545 is supported."));
    }

    let mut uid_list: Vec<(String, usize, usize)> = Vec::new();
    let mut components_found = 0;
    let mut component_types: Vec<String> = Vec::new();

    for child in doc.components() {
        components_found += 1;

        if !matches!(child.name.as_str(), "VEVENT" | "VTODO" | "VJOURNAL") {
            continue;
        }
        component_types.push(child.name.clone());

        let uid = child.property("UID").map(ToString::to_string).unwrap_or_default();
        let is_master = usize::from(child.property("RECURRENCE-ID").is_none());
        match uid_list.iter_mut().find(|(u, _, _)| *u == uid) {
            Some(entry) => {
                entry.1 += 1;
                if is_master == 1 && entry.2 > 0 {
                    warnings.push(message(
                        3,
                        format!("More than one master object was found for the object with UID {uid}"),
                    ));
                }
                entry.2 += is_master;
            }
            None => uid_list.push((uid, 1, is_master)),
        }
    }

    if components_found == 0 {
        warnings.push(message(3, "An iCalendar object must have at least 1 component."));
    }

    if options & PROFILE_CALDAV != 0 {
        if uid_list.len() > 1 {
            warnings
                .push(message(3, "A calendar object on a CalDAV server may only have components with the same UID."));
        }
        if component_types.is_empty() {
            warnings.push(message(
                3,
                "A calendar object on a CalDAV server must have at least 1 component (VTODO, VEVENT, VJOURNAL).",
            ));
        }
        let mut unique = component_types.clone();
        unique.sort();
        unique.dedup();
        if unique.len() > 1 {
            warnings.push(message(
                3,
                "A calendar object on a CalDAV server may only have 1 type of component (VEVENT, VTODO or VJOURNAL).",
            ));
        }
        if doc.property("METHOD").is_some() {
            warnings.push(message(3, "A calendar object on a CalDAV server MUST NOT have a METHOD property."));
        }
    }

    Ok(warnings)
}

/// `Component::validate($options)` (the children, then the rules).
fn validate_component(
    ctx: &Document,
    component: &mut Component,
    options: u32,
    parent_has_method: bool,
) -> DavResult<Vec<Message>> {
    let rules = validation_rules(ctx.kind, &component.name, parent_has_method);
    let defaults = ctx.kind.defaults(&component.name);

    let mut property_counters: Vec<(String, usize)> = Vec::new();
    let mut messages = Vec::new();

    let has_method = component.property("METHOD").is_some();
    let children: Vec<Node> = component.children().into_iter().cloned().collect();
    let mut names: Vec<String> = Vec::new();
    let mut new_children = Vec::new();
    for child in children {
        let name = match &child {
            Node::Property(p) => p.name.to_uppercase(),
            Node::Component(c) => c.name.to_uppercase(),
        };
        match property_counters.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 += 1,
            None => property_counters.push((name.clone(), 1)),
        }
        if !names.contains(&name) {
            names.push(name);
        }
        match child {
            Node::Property(mut p) => {
                let (property_messages, keep) = validate_property(ctx, &mut p, options)?;
                messages.extend(property_messages);
                if keep {
                    new_children.push(Node::Property(p));
                }
            }
            Node::Component(mut c) => {
                messages.extend(validate_child_component(ctx, &mut c, options, has_method)?);
                new_children.push(Node::Component(c));
            }
        }
    }
    for name in &names {
        component.remove(name);
    }
    for child in new_children {
        component.add(child);
    }

    let count = |name: &str| property_counters.iter().find(|(n, _)| n == name).map(|(_, c)| *c);
    for (prop_name, rule) in rules {
        match rule {
            '0' => {
                if count(prop_name).is_some() {
                    messages.push(message(3, format!("{prop_name} MUST NOT appear in a {} component", component.name)));
                }
            }
            '1' => {
                if count(prop_name) != Some(1) {
                    let mut repaired = false;
                    if options & REPAIR != 0
                        && let Some((_, value)) = defaults.iter().find(|(n, _)| n == prop_name)
                    {
                        let property = ctx.create_property(prop_name, value.clone(), Vec::new()).map_err(internal)?;
                        component.add(Node::Property(property));
                        repaired = true;
                    }
                    messages.push(message(
                        if repaired { 1 } else { 3 },
                        format!("{prop_name} MUST appear exactly once in a {} component", component.name),
                    ));
                }
            }
            '+' => {
                if count(prop_name).is_none_or(|c| c < 1) {
                    messages.push(message(
                        3,
                        format!("{prop_name} MUST appear at least once in a {} component", component.name),
                    ));
                }
            }
            '?' if count(prop_name).is_some_and(|c| c > 1) => {
                let mut level = 3;

                // We try to repair the same property appearing multiple times with the exact same value
                // by removing the duplicates and keeping only one property
                if options & REPAIR != 0 {
                    let mut properties: Vec<Node> = Vec::new();
                    for node in component.select(prop_name) {
                        if !properties.contains(node) {
                            properties.push(node.clone());
                        }
                    }
                    if properties.len() == 1 {
                        component.remove(prop_name);
                        component.add(properties.remove(0));
                        level = 1;
                    }
                }

                messages.push(message(
                    level,
                    format!("{prop_name} MUST NOT appear more than once in a {} component", component.name),
                ));
            }
            _ => {}
        }
    }

    Ok(messages)
}

/// `validate()` of a child component: `Component::validate()` plus the
/// checks of `VTodo`, `VAvailability` and `Available`.
fn validate_child_component(
    ctx: &Document,
    component: &mut Component,
    options: u32,
    parent_has_method: bool,
) -> DavResult<Vec<Message>> {
    let mut result = validate_component(ctx, component, options, parent_has_method)?;

    match component.name.as_str() {
        "VTODO" => {
            if let (Some(due), Some(dt_start)) = (component.property("DUE"), component.property("DTSTART")) {
                if property_value_type(ctx, due) != property_value_type(ctx, dt_start) {
                    result.push(message(3, "The value type (DATE or DATE-TIME) must be identical for DUE and DTSTART"));
                } else {
                    let due = due.get_date_time().map_err(internal)?.map(|d| instant(&d));
                    let start = dt_start.get_date_time().map_err(internal)?.map(|d| instant(&d));
                    if due < start {
                        result.push(message(3, "DUE must occur after DTSTART"));
                    }
                }
            }
        }
        "VAVAILABILITY" | "AVAILABLE"
            if component.property("DTEND").is_some() && component.property("DURATION").is_some() =>
        {
            result.push(message(3, "DTEND and DURATION cannot both be present"));
        }
        _ => {}
    }

    Ok(result)
}

/// `StringUtil::isUTF8()`'s control characters.
fn is_control(c: char) -> bool {
    matches!(u32::from(c), 0x00..=0x08 | 0x0B | 0x0C | 0x0E | 0x0F)
}

/// The control characters `StringUtil::convertToUTF8()` removes.
fn is_removed_control(c: char) -> bool {
    matches!(u32::from(c), 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F)
}

/// PHP's `is_numeric()`.
fn is_numeric(value: &str) -> bool {
    let s = value.trim_start_matches([' ', '\t', '\n', '\r', '\x0B', '\x0C']);
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let digits = mantissa.replacen('.', "", 1);
    let mantissa_ok = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && mantissa != ".";
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit())
    });
    mantissa_ok && exponent_ok
}

/// `Property::validate($options)` and the overrides of `Text`,
/// `VCard\DateAndOrTime`, `ICalendar\DateTime` and `ICalendar\Recur`:
/// the messages, and whether the property stays in its parent.
fn validate_property(ctx: &Document, property: &mut Property, options: u32) -> DavResult<(Vec<Message>, bool)> {
    let mut warnings = Vec::new();
    let repair = options & REPAIR != 0;
    let class = class_of(ctx, property);

    // Checking if our value is UTF-8 (the control characters).
    if class != Class::Binary {
        let parts = property.get_parts();
        let raw: String = if class == Class::Recur {
            property.get_value().unwrap_or_default()
        } else {
            parts.iter().map(|p| p.clone().unwrap_or_default()).collect()
        };
        if raw.chars().any(is_control) {
            let mut level = 3;
            if repair {
                let clean = |s: &str| -> String { s.chars().filter(|c| !is_removed_control(*c)).collect() };
                if class == Class::Recur {
                    property.set_value(Input::Text(clean(&raw))).map_err(internal)?;
                } else {
                    let cleaned = parts.iter().map(|p| p.as_deref().map(clean)).collect();
                    property.set_value(Input::Parts(cleaned)).map_err(internal)?;
                }
                level = 1;
            }
            let found = raw.chars().find(|c| is_removed_control(*c)).unwrap_or('\0');
            warnings
                .push(message(level, format!("Property contained a control character (0x{:02x})", u32::from(found))));
        }
    }

    // Checking if the propertyname does not contain any invalid bytes.
    let valid_name = !property.name.is_empty()
        && property.name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
    if !valid_name {
        warnings.push(message(
            if repair { 1 } else { 3 },
            format!("The propertyname: {} contains invalid characters. Only A-Z, 0-9 and - are allowed", property.name),
        ));
        if repair {
            // Uppercasing and converting underscores to dashes.
            let name = property.name.replace('_', "-").to_uppercase();
            // Removing every other invalid character
            property.name =
                name.chars().filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '-').collect();
        }
    }

    if let Some(encoding) = property.param("ENCODING").map(ToString::to_string) {
        let document_type = ctx.document_type();
        if document_type == DocumentType::VCard40 {
            warnings.push(message(3, "ENCODING parameter is not valid in vCard 4."));
        } else {
            let mut encoding = encoding;
            let allowed: &[&str] = match document_type {
                DocumentType::ICalendar20 => &["8BIT", "BASE64"],
                DocumentType::VCard21 => &["QUOTED-PRINTABLE", "BASE64", "8BIT"],
                DocumentType::VCard30 => {
                    // Repair vCard30 that use BASE64 encoding
                    if repair && encoding.to_uppercase() == "BASE64" {
                        encoding = "B".to_owned();
                        property.set_param("ENCODING", Some(encoding.clone())).map_err(internal)?;
                        warnings.push(message(1, "ENCODING=BASE64 has been transformed to ENCODING=B."));
                    }
                    &["B"]
                }
                _ => &[],
            };
            if !allowed.is_empty() && !allowed.contains(&encoding.to_uppercase().as_str()) {
                warnings.push(message(
                    3,
                    format!("ENCODING={} is not valid for this document type.", encoding.to_uppercase()),
                ));
            }
        }
    }

    // Text::validate(): the minimum number of values.
    if class.is_text() {
        let minimum = match property.name.as_str() {
            "N" => Some(5),
            "ADR" => Some(7),
            _ => None,
        };
        if let Some(minimum) = minimum {
            let mut parts = property.get_parts();
            if parts.len() < minimum {
                warnings.push(message(
                    if repair { 1 } else { 3 },
                    format!(
                        "The {} property must have at least {minimum} values. It only has {}",
                        property.name,
                        parts.len()
                    ),
                ));
                if repair {
                    parts.resize(minimum, Some(String::new()));
                    property.set_value(Input::Parts(parts)).map_err(internal)?;
                }
            }
        }
    }

    // VCard\DateAndOrTime::validate()
    if class.is_date_and_or_time() {
        let value = property.get_value().unwrap_or_default();
        if vobject::parse_vcard_date_time(&value).is_err() {
            warnings
                .push(message(3, format!("The supplied value ({value}) is not a correct DATE-AND-OR-TIME property")));
        }
    }

    // ICalendar\DateTime::validate()
    if matches!(class, Class::ICalDate | Class::ICalDateTime) {
        let value_type = value_type(class, property.has_time());
        for value in property.get_parts() {
            let value = value.unwrap_or_default();
            let parsed = match value_type {
                "DATE" => vobject::parse_date(&value, None).map(|_| ()),
                _ => vobject::parse_date_time(&value, None).map(|_| ()),
            };
            if parsed.is_err() {
                warnings.push(message(3, format!("The supplied value ({value}) is not a correct {value_type}")));
                break;
            }
        }
    }

    // ICalendar\Recur::validate()
    if class == Class::Recur {
        let raw = property.get_value().unwrap_or_default();
        let mut values: Vec<(String, Vec<String>, bool)> = Vec::new();
        for part in raw.split(';').filter(|p| !php_empty(p)) {
            if let Some((key, value)) = part.split_once('=') {
                let list = value.contains(',');
                values.push((key.to_owned(), value.split(',').map(str::to_owned).collect(), list));
            }
        }
        let name = property.name.clone();
        let mut kept: Vec<(String, Vec<String>, bool)> = Vec::new();
        for (key, value, list) in values {
            if !list && value.first().is_some_and(String::is_empty) {
                warnings.push(message(if repair { 1 } else { 3 }, format!("Invalid value for {key} in {name}")));
                if !repair {
                    kept.push((key, value, list));
                }
                continue;
            }
            let bounds: Option<(i64, i64, bool, &str)> = match key.as_str() {
                "BYMONTH" => Some((1, 12, false, "BYMONTH in RRULE must have value(s) between 1 and 12!")),
                "BYWEEKNO" => Some((-53, 53, true, "BYWEEKNO in RRULE must have value(s) from -53 to -1, or 1 to 53!")),
                "BYYEARDAY" => {
                    Some((-366, 366, true, "BYYEARDAY in RRULE must have value(s) from -366 to -1, or 1 to 366!"))
                }
                _ => None,
            };
            let Some((min, max, no_zero, text)) = bounds else {
                kept.push((key, value, list));
                continue;
            };
            let mut remaining = Vec::new();
            let mut remove_all = false;
            for v in &value {
                let n = php_intval(v);
                if !is_numeric(v) || n < min || (no_zero && n == 0) || n > max {
                    warnings.push(message(if repair { 1 } else { 3 }, text));
                    if repair && !list {
                        remove_all = true;
                    }
                } else {
                    remaining.push(v.clone());
                }
            }
            if !repair {
                kept.push((key, value, list));
            } else if !remove_all && !(list && remaining.is_empty()) {
                kept.push((key, if list { remaining } else { value }, list));
            }
        }

        if !kept.iter().any(|(k, _, _)| k == "FREQ") {
            warnings.push(message(if repair { 1 } else { 3 }, format!("FREQ is required in {name}")));
            if repair {
                return Ok((warnings, false));
            }
        }

        if repair {
            let value: Vec<String> = kept.iter().map(|(k, v, _)| format!("{k}={}", v.join(","))).collect();
            property.set_value(Input::Text(value.join(";"))).map_err(internal)?;
        }
    }

    Ok((warnings, true))
}
