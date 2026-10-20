//! `Sabre\CalDAV\Plugin` (properties, the calendar-multiget and
//! calendar-query reports, iCalendar validation), `Sabre\CalDAV\ICSExportPlugin`,
//! `CalDAV\CalendarQueryValidator` with the `isInTimeRange()` of the
//! components it calls, and the backends' `calendarQuery()`.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use super::backend::{self, Ctx};
use super::carddav::string_text_match;
use super::http::{resolve, split};
use super::nodes::Node;
use super::props::PropValue;
use super::server::{
    BASE_URI, DavError, DavResult, PropFind, PropFindResult, PropPatch, Server, calculate_uri, set_properties,
};
use super::vobject_ext::{self, PROFILE_CALDAV, REPAIR, instant};
use super::xml::{self, Element, NS_CALDAV, NS_CALENDARSERVER};
use crate::vobject::{self, Component, Document, Node as VNode, Property};

/// `Plugin::$maxResourceSize`.
const MAX_RESOURCE_SIZE: i64 = 10_000_000;

/// A `time-range`: its start and end (`null` for none).
type TimeRange = (Option<NaiveDateTime>, Option<NaiveDateTime>);

/// A text-match of a CalDAV filter.
#[derive(Debug, Clone)]
struct TextMatch {
    negate_condition: bool,
    collation: String,
    value: String,
}

/// `Xml\Filter\ParamFilter`.
#[derive(Debug, Clone)]
struct ParamFilter {
    name: String,
    is_not_defined: bool,
    text_match: Option<TextMatch>,
}

/// `Xml\Filter\PropFilter`.
#[derive(Debug, Clone)]
struct PropFilter {
    name: String,
    is_not_defined: bool,
    param_filters: Vec<ParamFilter>,
    text_match: Option<TextMatch>,
    time_range: Option<TimeRange>,
}

/// `Xml\Filter\CompFilter`.
#[derive(Debug, Clone)]
pub struct CompFilter {
    name: String,
    is_not_defined: bool,
    comp_filters: Vec<CompFilter>,
    prop_filters: Vec<PropFilter>,
    time_range: Option<TimeRange>,
}

fn ns(name: &str) -> String {
    format!("{{{NS_CALDAV}}}{name}")
}

fn attribute_name(elem: &Element) -> DavResult<String> {
    elem.attribute("name")
        .map(str::to_owned)
        .ok_or_else(|| DavError::internal("ErrorException", "Undefined array key \"name\""))
}

/// `DateTimeParser::parseDateTime()` of a filter attribute (UTC).
fn parse_filter_date(value: Option<&str>) -> DavResult<Option<NaiveDateTime>> {
    match value {
        Some(v) => Ok(Some(instant(&vobject::parse_date_time(v, None)?))),
        None => Ok(None),
    }
}

/// The `time-range` element of a filter.
fn time_range(elem: &Element) -> DavResult<TimeRange> {
    let start = parse_filter_date(elem.attribute("start"))?;
    let end = parse_filter_date(elem.attribute("end"))?;
    if let (Some(s), Some(e)) = (start, end)
        && e <= s
    {
        return Err(DavError::bad_request("The end-date must be larger than the start-date"));
    }
    Ok((start, end))
}

/// The `text-match` element of a CalDAV filter.
fn text_match(elem: &Element) -> TextMatch {
    TextMatch {
        negate_condition: elem.attribute("negate-condition") == Some("yes"),
        collation: elem.attribute("collation").unwrap_or("i;ascii-casemap").to_owned(),
        value: elem.text().unwrap_or_default(),
    }
}

/// `ParamFilter::xmlDeserialize()`.
fn param_filter(element: &Element) -> DavResult<ParamFilter> {
    let mut result = ParamFilter { name: attribute_name(element)?, is_not_defined: false, text_match: None };
    for elem in element.elements() {
        if elem.clark == ns("is-not-defined") {
            result.is_not_defined = true;
        } else if elem.clark == ns("text-match") {
            result.text_match = Some(text_match(elem));
        }
    }
    Ok(result)
}

/// `PropFilter::xmlDeserialize()`.
fn prop_filter(element: &Element) -> DavResult<PropFilter> {
    let mut result = PropFilter {
        name: attribute_name(element)?,
        is_not_defined: false,
        param_filters: Vec::new(),
        text_match: None,
        time_range: None,
    };
    for elem in element.elements() {
        if elem.clark == ns("param-filter") {
            result.param_filters.push(param_filter(elem)?);
        } else if elem.clark == ns("is-not-defined") {
            result.is_not_defined = true;
        } else if elem.clark == ns("time-range") {
            result.time_range = Some(time_range(elem)?);
        } else if elem.clark == ns("text-match") {
            result.text_match = Some(text_match(elem));
        }
    }
    Ok(result)
}

/// `CompFilter::xmlDeserialize()`.
fn comp_filter(element: &Element) -> DavResult<CompFilter> {
    let mut result = CompFilter {
        name: attribute_name(element)?,
        is_not_defined: false,
        comp_filters: Vec::new(),
        prop_filters: Vec::new(),
        time_range: None,
    };
    for elem in element.elements() {
        if elem.clark == ns("comp-filter") {
            result.comp_filters.push(comp_filter(elem)?);
        } else if elem.clark == ns("prop-filter") {
            result.prop_filters.push(prop_filter(elem)?);
        } else if elem.clark == ns("is-not-defined") {
            result.is_not_defined = true;
        } else if elem.clark == ns("time-range") {
            if result.name == "VCALENDAR" {
                return Err(DavError::bad_request("You cannot add time-range filters on the VCALENDAR component"));
            }
            result.time_range = Some(time_range(elem)?);
        }
    }
    Ok(result)
}

/// `CalendarData::xmlDeserialize()`: its `expand` is checked (expanding
/// isn't ported, nor jCal, so its content type is not read).
fn calendar_data(element: &Element) -> DavResult<()> {
    for elem in element.elements() {
        if elem.clark == ns("expand") {
            let start = parse_filter_date(elem.attribute("start"))?;
            let end = parse_filter_date(elem.attribute("end"))?;
            let (Some(start), Some(end)) = (start, end) else {
                return Err(DavError::bad_request(
                    "The \"start\" and \"end\" attributes are required when expanding calendar-data",
                ));
            };
            if end <= start {
                return Err(DavError::bad_request(
                    "The end-date must be larger than the start-date when expanding calendar-data",
                ));
            }
        }
    }
    Ok(())
}

/// The `{DAV:}prop` of a report: the property names (with the
/// `calendar-data` checked).
fn report_prop(element: &Element) -> DavResult<Vec<String>> {
    let values = element.key_value();
    if let Some((_, cd)) = values.iter().find(|(k, _)| *k == ns("calendar-data")) {
        calendar_data(cd)?;
    }
    Ok(values.into_iter().map(|(k, _)| k.to_owned()).collect())
}

impl Server<'_> {
    /// `propFind()`.
    pub async fn caldav_prop_find(&mut self, prop_find: &mut PropFind, node: &Node) -> DavResult<()> {
        if matches!(node, Node::Calendar(_)) {
            prop_find.handle(&ns("max-resource-size"), Some(PropValue::Text(MAX_RESOURCE_SIZE.to_string())));
            prop_find.handle(&ns("supported-calendar-data"), Some(PropValue::SupportedCalendarData));
            prop_find.handle(&ns("supported-collation-set"), Some(PropValue::CalSupportedCollationSet));
        }

        if let Node::Principal(principal) = node {
            let principal_url = principal.uri.clone();
            if prop_find.wants(&ns("calendar-home-set")) {
                let home = calendar_home_for_principal(&principal_url);
                prop_find.handle(&ns("calendar-home-set"), home.map(|h| PropValue::local_href(vec![format!("{h}/")])));
            }

            // The calendar-user-address-set property is basically mapped to the {DAV:}alternate-URI-set property.
            let alternate = vec![format!("mailto:{}", principal.email_address)];
            let mut addresses = alternate.clone();
            addresses.push(format!("{BASE_URI}{principal_url}/"));
            prop_find.handle(&ns("calendar-user-address-set"), Some(PropValue::local_href(addresses)));

            // For some reason somebody thought it was a good idea to add
            // another one of these properties. We're supporting it too.
            let emails: Vec<String> =
                alternate.iter().filter_map(|a| a.strip_prefix("mailto:").map(str::to_owned)).collect();
            prop_find
                .handle(&format!("{{{NS_CALENDARSERVER}}}email-address-set"), Some(PropValue::EmailAddressSet(emails)));

            // These two properties are shortcuts for ical to easily find
            // other principals this principal has access to.
            let prop_read = format!("{{{NS_CALENDARSERVER}}}calendar-proxy-read-for");
            let prop_write = format!("{{{NS_CALENDARSERVER}}}calendar-proxy-write-for");
            if prop_find.status(&prop_read) == Some(404) || prop_find.status(&prop_write) == Some(404) {
                let path = prop_find.path().to_owned();
                // No principal is a proxy group: both lists stay empty.
                self.principal_membership(&path).await?;
                prop_find.set(&prop_read, Some(PropValue::local_href(Vec::new())), None);
                prop_find.set(&prop_write, Some(PropValue::local_href(Vec::new())), None);
            }
        }

        if let Node::CalendarObject { .. } = node {
            // The calendar-data property is not supposed to be a 'real'
            // property, but in large chunks of the spec it does act as such.
            // Taking out \r to not screw up the xml output
            prop_find.handle(&ns("calendar-data"), Some(PropValue::Text(node.get().replace('\r', ""))));
        }
        Ok(())
    }

    /// `httpMkCalendar()`: the backend creates nothing.
    pub async fn caldav_http_mkcalendar(&mut self) -> DavResult<bool> {
        let body = self.request.body.clone();
        let path = self.request.path()?;

        let mut properties = Vec::new();
        if !body.is_empty() {
            let root = xml::parse(&body).map_err(|e| DavError::bad_request(e.0))?;
            if root.clark != ns("mkcalendar") {
                return Err(DavError::bad_request(format!(
                    "Expected {} but received {} as the root element",
                    ns("mkcalendar"),
                    root.clark
                )));
            }
            properties = set_properties(&root)?;
        }

        // iCal abuses MKCALENDAR since iCal 10.9.2 to create server-stored
        // subscriptions. If the body had a {DAV:}resourcetype, we simply use
        // it instead of the pre-defined list.
        let resource_type = match properties.iter().find(|(n, _)| n == "{DAV:}resourcetype") {
            Some((_, value)) => value.as_ref().map(Element::enumeration).unwrap_or_default(),
            None => vec!["{DAV:}collection".to_owned(), ns("calendar")],
        };

        self.create_collection(&path, PropPatch::new(resource_type, properties)).await?;

        self.response.status = Some(201);
        self.response.set_header("Content-Length", "0");
        Ok(true)
    }

    /// `report()` of the CalDAV reports.
    pub async fn caldav_report(&mut self, report: &Element) -> DavResult<()> {
        if report.clark == ns("calendar-multiget") {
            self.calendar_multi_get_report(report).await
        } else if report.clark == ns("calendar-query") {
            self.calendar_query_report(report).await
        } else {
            Err(DavError::not_implemented("The free-busy-query REPORT is not implemented"))
        }
    }

    /// `calendarMultiGetReport($report)` (no `expand`, no jCal).
    async fn calendar_multi_get_report(&mut self, report: &Element) -> DavResult<()> {
        // CalendarMultiGetReport::xmlDeserialize()
        let mut properties = Vec::new();
        let mut hrefs = Vec::new();
        for elem in report.elements() {
            match elem.clark.as_str() {
                "{DAV:}prop" => properties = report_prop(elem)?,
                "{DAV:}href" => hrefs.push(resolve(&self.request.url, &elem.text().unwrap_or_default())),
                _ => {}
            }
        }

        let mut paths = Vec::new();
        for href in &hrefs {
            paths.push(calculate_uri(href)?);
        }

        let property_list: Vec<PropFindResult> =
            self.get_properties_for_multiple_paths(&paths, &properties).await?.into_iter().map(|(_, p)| p).collect();

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.body = Some(self.generate_multi_status(property_list, minimal));
        Ok(())
    }

    /// `calendarQueryReport($report)` (no `expand`, no jCal).
    async fn calendar_query_report(&mut self, report: &Element) -> DavResult<()> {
        // CalendarQueryReport::xmlDeserialize()
        let mut filters: Option<CompFilter> = None;
        let mut properties = Vec::new();
        for elem in report.elements() {
            if elem.clark == "{DAV:}prop" {
                properties = report_prop(elem)?;
            } else if elem.clark == ns("filter") {
                for sub in elem.elements() {
                    if sub.clark == ns("comp-filter") {
                        if filters.is_some() {
                            return Err(DavError::bad_request("Only one top-level comp-filter may be defined"));
                        }
                        filters = Some(comp_filter(sub)?);
                    }
                }
            }
        }
        let Some(filters) = filters else {
            return Err(DavError::bad_request(format!(
                "The {{{NS_CALDAV}}}filter element is required for this request"
            )));
        };

        let path = self.request_uri()?;

        let node = self.tree.node_for_path(self.ctx, &path).await?;
        let mut depth = self.http_depth(0);

        // The default result is an empty array
        let mut result = Vec::new();

        // The calendarobject was requested directly. In this case we handle this locally.
        if depth == 0 && matches!(node, Node::CalendarObject { .. }) {
            let mut requested_calendar_data = true;
            let mut requested_properties = properties.clone();

            let calendar_data_name = ns("calendar-data");
            if !requested_properties.contains(&calendar_data_name) {
                // We always retrieve calendar-data, as we need it for filtering.
                requested_properties.push(calendar_data_name.clone());

                // If calendar-data wasn't explicitly requested, we need to remove it after processing.
                requested_calendar_data = false;
            }

            let found = self.get_properties_for_path(&path, &requested_properties, 0).await?;

            // This array should have only 1 element, the first calendar object.
            if let Some(mut properties) = found.into_iter().next()
                && let Some(PropValue::Text(calendar_data)) = properties.found(&calendar_data_name).cloned()
            {
                let v_object = vobject::read(&calendar_data, 0)?;
                if validate(&v_object, &filters)? {
                    // If the client didn't require the calendar-data property, we won't give it back.
                    if !requested_calendar_data {
                        properties.remove_found(&calendar_data_name);
                    }
                    result = vec![properties];
                }
            }
        }

        if matches!(node, Node::Calendar(_)) && depth == 0 {
            if self.request.header("User-Agent").unwrap_or_default().starts_with("MSFT-") {
                // Microsoft clients incorrectly supplied depth as 0, when it actually should have set depth to 1.
                depth = 1;
            } else {
                return Err(DavError::bad_request(
                    "A calendar-query REPORT on a calendar with a Depth: 0 is undefined. Set Depth to 1",
                ));
            }
        }

        // If we're dealing with a calendar, the calendar itself is responsible for the calendar-query.
        if let Node::Calendar(info) = &node
            && depth == 1
        {
            let node_paths = calendar_query(self.ctx, &info.id, &filters).await?;

            for node_path in node_paths {
                if let Some(properties) = self
                    .get_properties_for_path(&format!("{path}/{node_path}"), &properties, 0)
                    .await?
                    .into_iter()
                    .next()
                {
                    result.push(properties);
                }
            }
        }

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.body = Some(self.generate_multi_status(result, minimal));
        Ok(())
    }

    /// `validateICalendar(&$data, $path, &$modified, …)`.
    pub async fn validate_icalendar(&mut self, data: &mut String, path: &str, modified: &mut bool) -> DavResult<()> {
        let mut vobj = if data.starts_with('[') {
            // jCal (`VObject\Reader::readJson()`) isn't ported.
            return Err(DavError::unsupported_media_type(
                "This resource only supports valid iCalendar 2.0 data. Parse error: jCal is not supported",
            ));
        } else {
            match vobject::read(data, 0) {
                Ok(vobj) => vobj,
                Err(vobject::Error::Parse(message)) => {
                    return Err(DavError::unsupported_media_type(format!(
                        "This resource only supports valid iCalendar 2.0 data. Parse error: {message}"
                    )));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if vobj.name != "VCALENDAR" {
            return Err(DavError::unsupported_media_type("This collection can only support iCalendar objects."));
        }

        let sccs = ns("supported-calendar-component-set");

        // Get the Supported Components for the target calendar
        let (parent_path, _) = split(path);
        let calendar_properties = self.get_properties(&parent_path, std::slice::from_ref(&sccs)).await?;

        let supported_components: Vec<String> =
            match calendar_properties.into_iter().find(|(n, _)| *n == sccs).map(|(_, v)| v) {
                Some(PropValue::SupportedCalendarComponentSet(components)) => components,
                _ => vec!["VJOURNAL".to_owned(), "VTODO".to_owned(), "VEVENT".to_owned()],
            };

        let mut found_type = None;
        for component in vobj.components() {
            match component.name.as_str() {
                "VTIMEZONE" => continue,
                "VEVENT" | "VTODO" | "VJOURNAL" => found_type = Some(component.name.clone()),
                _ => {}
            }
        }

        if found_type.as_ref().is_none_or(|t| !supported_components.contains(t)) {
            return Err(DavError::invalid_component_type(format!(
                "iCalendar objects must at least have a component of type {}",
                supported_components.join(", ")
            )));
        }

        let mut options = PROFILE_CALDAV;
        if self.http_prefer().handling.as_deref() != Some("strict") {
            options |= REPAIR;
        }

        let messages = vobject_ext::validate(&mut vobj, options)?;

        let mut highest_level = 0;
        let mut warning_message = None;

        // $messages contains a list of problems with the vcard, along with their severity.
        for message in messages {
            if message.level > highest_level {
                // Recording the highest reported error level.
                highest_level = message.level;
                warning_message = Some(message.message.clone());
            }
            match message.level {
                // Level 1 means that there was a problem, but it was repaired.
                1 => *modified = true,
                // Level 3 means a critical error
                3 => {
                    return Err(DavError::unsupported_media_type(format!(
                        "Validation error in iCalendar: {}",
                        message.message
                    )));
                }
                _ => {}
            }
        }
        if let Some(warning) = warning_message {
            self.response.set_header("X-Sabre-Ew-Gross", format!("iCalendar validation warning: {warning}"));
        }

        if *modified {
            // An event handler told us that it modified the object.
            *data = vobj.serialize();
        }
        Ok(())
    }

    /// `ICSExportPlugin::httpGet()`: `false` when it isn't an export.
    pub async fn ics_http_get(&mut self) -> DavResult<bool> {
        let query_params = self.request.query_parameters();
        if !query_params.iter().any(|(k, _)| k == "export") {
            return Ok(false);
        }
        let param = |name: &str| query_params.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());

        let path = self.request.path()?;

        let names: Vec<String> = [
            "{DAV:}resourcetype",
            "{DAV:}displayname",
            "{http://sabredav.org/ns}sync-token",
            "{DAV:}sync-token",
            "{http://apple.com/ns/ical/}calendar-color",
        ]
        .iter()
        .map(|n| (*n).to_owned())
        .collect();
        let properties = self.get_properties(&path, &names).await?;

        let is_calendar = matches!(
            properties.iter().find(|(n, _)| n == "{DAV:}resourcetype").map(|(_, v)| v),
            Some(PropValue::ResourceType(types)) if types.contains(&ns("calendar"))
        );
        if !is_calendar {
            return Ok(false);
        }

        let mut start = None;
        let mut end = None;
        let mut component_type: Option<String> = None;
        if let Some(s) = param("start") {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(DavError::bad_request("The start= parameter must contain a unix timestamp"));
            }
            start = chrono::DateTime::from_timestamp(s.parse().unwrap_or(0), 0).map(|d| d.naive_utc());
        }
        if let Some(e) = param("end") {
            if e.is_empty() || !e.bytes().all(|b| b.is_ascii_digit()) {
                return Err(DavError::bad_request("The end= parameter must contain a unix timestamp"));
            }
            end = chrono::DateTime::from_timestamp(e.parse().unwrap_or(0), 0).map(|d| d.naive_utc());
        }
        // `expand` (`VCalendar::expand()`) isn't ported: the parameter is checked, the events aren't expanded.
        if param("expand").is_some_and(|v| !v.is_empty() && v != "0") {
            if start.is_none() || end.is_none() {
                return Err(DavError::bad_request(
                    "If you'd like to expand recurrences, you must specify both a start= and end= parameter.",
                ));
            }
            component_type = Some("VEVENT".to_owned());
        }
        if let Some(c) = param("componentType") {
            if !matches!(c.as_str(), "VEVENT" | "VTODO" | "VJOURNAL") {
                return Err(DavError::bad_request(format!(
                    "You are not allowed to search for components of type: {c} here"
                )));
            }
            component_type = Some(c);
        }
        // The jCal format (`application/calendar+json`) isn't ported: always `text/calendar`.
        self.ics_generate_response(&path, start, end, component_type, &properties).await?;
        Ok(true)
    }

    /// `ICSExportPlugin::generateResponse(…)` (text/calendar).
    async fn ics_generate_response(
        &mut self,
        path: &str,
        start: Option<NaiveDateTime>,
        end: Option<NaiveDateTime>,
        component_type: Option<String>,
        properties: &[(String, PropValue)],
    ) -> DavResult<()> {
        let cal_data_prop = ns("calendar-data");
        let calendar_node = self.tree.node_for_path(self.ctx, path).await?;

        let nodes: Vec<PropFindResult> = if start.is_some() || end.is_some() || component_type.is_some() {
            // If there was a start or end filter, we need to enlist calendarQuery for speed.
            let filters = CompFilter {
                name: "VCALENDAR".to_owned(),
                is_not_defined: false,
                comp_filters: vec![CompFilter {
                    name: component_type.unwrap_or_default(),
                    is_not_defined: false,
                    comp_filters: Vec::new(),
                    prop_filters: Vec::new(),
                    time_range: Some((start, end)),
                }],
                prop_filters: Vec::new(),
                time_range: None,
            };
            let Node::Calendar(info) = &calendar_node else {
                return Err(DavError::internal("Error", "Call to undefined method calendarQuery()"));
            };
            let query_result: Vec<String> = calendar_query(self.ctx, &info.id, &filters)
                .await?
                .into_iter()
                .map(|item| format!("{path}/{item}"))
                .collect();
            self.get_properties_for_multiple_paths(&query_result, std::slice::from_ref(&cal_data_prop))
                .await?
                .into_iter()
                .map(|(_, p)| p)
                .collect()
        } else {
            self.get_properties_for_path(path, std::slice::from_ref(&cal_data_prop), 1).await?
        };

        // Flattening the arrays
        let mut blobs: Vec<String> = Vec::new();
        for node in nodes {
            if let Some(PropValue::Text(data)) = node.found(&cal_data_prop) {
                blobs.push(data.clone());
            }
        }

        let merged_calendar = merge_objects(properties, &blobs)?.serialize();

        let mut filename: String = calendar_node
            .name()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '))
            .collect();
        filename.push_str(&format!("-{}.ics", chrono::Utc::now().format("%Y-%m-%d")));

        self.response.set_header("Content-Disposition", format!("attachment; filename=\"{filename}\""));
        self.response.set_header("Content-Type", "text/calendar");
        self.response.status = Some(200);
        self.response.body = Some(merged_calendar);
        Ok(())
    }
}

/// `getCalendarHomeForPrincipal($principalUrl)`.
fn calendar_home_for_principal(principal_url: &str) -> Option<String> {
    // This function assumes that there are two components in a principal path.
    let parts: Vec<&str> = principal_url.trim_matches('/').split('/').collect();
    if parts.len() != 2 || parts[0] != "principals" {
        return None;
    }
    Some(format!("calendars/{}", parts[1]))
}

/// `ICSExportPlugin::mergeObjects($properties, $inputObjects)`.
fn merge_objects(properties: &[(String, PropValue)], input_objects: &[String]) -> DavResult<Document> {
    let mut calendar = Document::vcalendar(Vec::new())?;
    calendar.set_property("VERSION", "2.0".into())?;
    calendar.set_property("PRODID", format!("-//SabreDAV//SabreDAV {}//EN", super::server::VERSION).into())?;
    if let Some(PropValue::Text(name)) = properties.iter().find(|(n, _)| n == "{DAV:}displayname").map(|(_, v)| v) {
        calendar.set_property("X-WR-CALNAME", name.clone().into())?;
    }

    let mut collected_timezones: Vec<String> = Vec::new();
    let mut timezones = Vec::new();
    let mut objects = Vec::new();

    for input_object in input_objects {
        let node_comp = vobject::read(input_object, 0)?;

        for child in node_comp.children() {
            if let VNode::Component(component) = child {
                match component.name.as_str() {
                    "VEVENT" | "VTODO" | "VJOURNAL" => objects.push(component.clone()),
                    // VTIMEZONE is special, because we need to filter out the duplicates
                    "VTIMEZONE" => {
                        // Naively just checking tzid.
                        let tzid = component.property("TZID").map(ToString::to_string).unwrap_or_default();
                        if collected_timezones.contains(&tzid) {
                            continue;
                        }
                        timezones.push(component.clone());
                        collected_timezones.push(tzid);
                    }
                    _ => {}
                }
            }
        }
    }

    for tz in timezones {
        calendar.add(VNode::Component(tz));
    }
    for obj in objects {
        calendar.add(VNode::Component(obj));
    }

    Ok(calendar)
}

/// `AbstractBackend::calendarQuery($calendarId, $filters)`: the uris of the
/// objects that `validateFilterForObject()` accepts.
async fn calendar_query(ctx: &Ctx<'_>, calendar_id: &str, filters: &CompFilter) -> DavResult<Vec<String>> {
    let mut result = Vec::new();
    for object in backend::get_calendar_objects(ctx, calendar_id).await? {
        let v_object = vobject::read(&object.calendardata, 0)?;
        if validate(&v_object, filters)? {
            result.push(object.uri);
        }
    }
    Ok(result)
}

// ------------------------------------------------------ CalendarQueryValidator

/// `CalendarQueryValidator::validate($vObject, $filters)`.
fn validate(v_object: &Document, filters: &CompFilter) -> DavResult<bool> {
    // The top level object is always a component filter.
    if v_object.name != filters.name {
        return Ok(false);
    }

    Ok(validate_comp_filters(v_object, &filters.comp_filters)?
        && validate_prop_filters(v_object, &filters.prop_filters)?)
}

/// `validateCompFilters($parent, $filters)`.
fn validate_comp_filters(parent: &Component, filters: &[CompFilter]) -> DavResult<bool> {
    'filters: for filter in filters {
        let matches = parent.select(&filter.name);
        let is_defined = !matches.is_empty();

        if filter.is_not_defined {
            if is_defined {
                return Ok(false);
            }
            continue;
        }
        if !is_defined {
            return Ok(false);
        }

        if let Some((start, end)) = filter.time_range {
            for sub in &matches {
                if validate_time_range(sub, parent, start, end)? {
                    continue 'filters;
                }
            }
            return Ok(false);
        }

        if filter.comp_filters.is_empty() && filter.prop_filters.is_empty() {
            continue;
        }

        // If there are sub-filters, we need to find at least one component
        // for which the subfilters hold true.
        for sub in &matches {
            if let VNode::Component(sub) = sub
                && validate_comp_filters(sub, &filter.comp_filters)?
                && validate_prop_filters(sub, &filter.prop_filters)?
            {
                // We had a match, so this comp-filter succeeds
                continue 'filters;
            }
        }

        // If we got here it means there were sub-comp-filters or
        // sub-prop-filters and there was no match. This means this filter
        // needs to return false.
        return Ok(false);
    }

    // If we got here it means we got through all comp-filters alive so the filters were all true.
    Ok(true)
}

/// `validatePropFilters($parent, $filters)`.
fn validate_prop_filters(parent: &Component, filters: &[PropFilter]) -> DavResult<bool> {
    'filters: for filter in filters {
        let matches = parent.select(&filter.name);
        let is_defined = !matches.is_empty();

        if filter.is_not_defined {
            if is_defined {
                return Ok(false);
            }
            continue;
        }
        if !is_defined {
            return Ok(false);
        }

        if let Some((start, end)) = filter.time_range {
            for sub in &matches {
                if validate_time_range(sub, parent, start, end)? {
                    continue 'filters;
                }
            }
            return Ok(false);
        }

        if filter.param_filters.is_empty() && filter.text_match.is_none() {
            continue;
        }

        // If there are sub-filters, we need to find at least one property
        // for which the subfilters hold true.
        for sub in &matches {
            if let VNode::Property(sub) = sub
                && validate_param_filters(sub, &filter.param_filters)?
                && filter
                    .text_match
                    .as_ref()
                    .map_or(Ok(true), |tm| validate_text_match(&sub.get_value().unwrap_or_default(), tm))?
            {
                // We had a match, so this prop-filter succeeds
                continue 'filters;
            }
        }

        // If we got here it means there were sub-param-filters or text-match
        // filters and there was no match. This means the filter needs to return false.
        return Ok(false);
    }

    // If we got here it means we got through all prop-filters alive so the filters were all true.
    Ok(true)
}

/// `validateParamFilters($parent, $filters)`.
fn validate_param_filters(parent: &Property, filters: &[ParamFilter]) -> DavResult<bool> {
    'filters: for filter in filters {
        let param = parent.param(&filter.name);
        let is_defined = param.is_some();

        if filter.is_not_defined {
            if is_defined {
                return Ok(false);
            }
            continue;
        }
        let Some(param) = param else {
            return Ok(false);
        };

        let Some(text_match) = &filter.text_match else {
            continue;
        };

        // If there are sub-filters, we need to find at least one parameter
        // for which the subfilters hold true.
        for param_part in param.get_parts() {
            if validate_text_match(&param_part, text_match)? {
                // We had a match, so this param-filter succeeds
                continue 'filters;
            }
        }

        // If we got here it means there was a text-match filter and there
        // were no matches. This means the filter needs to return false.
        return Ok(false);
    }

    // If we got here it means we got through all param-filters alive so the filters were all true.
    Ok(true)
}

/// `validateTextMatch($check, $textMatch)`.
fn validate_text_match(check: &str, text_match: &TextMatch) -> DavResult<bool> {
    let is_matching = string_text_match(check, &text_match.value, &text_match.collation, "contains")?;

    Ok(text_match.negate_condition ^ is_matching)
}

/// `validateTimeRange($component, $start, $end)` (`$parent` is the
/// component's parent, for alarms).
fn validate_time_range(
    node: &VNode,
    parent: &Component,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
) -> DavResult<bool> {
    let start = start.unwrap_or_else(|| NaiveDate::from_ymd_opt(1900, 1, 1).expect("a date").into());
    let end = end.unwrap_or_else(|| NaiveDate::from_ymd_opt(3000, 1, 1).expect("a date").into());

    match node {
        VNode::Component(component) => match component.name.as_str() {
            "VEVENT" => vevent_in_time_range(component, start, end),
            "VTODO" => vtodo_in_time_range(component, start, end),
            "VJOURNAL" => vjournal_in_time_range(component, start, end),
            "VALARM" => {
                // If the valarm is wrapped in a recurring event, the PHP expands the
                // recursions; any alarm of the event's first instance is checked here.
                valarm_in_time_range(component, parent, start, end)
            }
            "VFREEBUSY" => Err(DavError::not_implemented(format!(
                "time-range filters are currently not supported on {} components",
                component.name
            ))),
            name => Err(DavError::bad_request(format!("You cannot create a time-range filter on a {name} component"))),
        },
        VNode::Property(property) => match property.name.as_str() {
            "COMPLETED" | "CREATED" | "DTEND" | "DTSTAMP" | "DTSTART" | "DUE" | "LAST-MODIFIED" => {
                let dt = date_time(property)?;
                Ok(start <= dt && end >= dt)
            }
            name => Err(DavError::bad_request(format!("You cannot create a time-range filter on a {name} component"))),
        },
    }
}

/// `$property->getDateTime()` as an instant.
fn date_time(property: &Property) -> DavResult<NaiveDateTime> {
    let dt = property.get_date_time()?.ok_or_else(|| DavError::internal("ErrorException", "No date in property"))?;
    Ok(instant(&dt))
}

/// `DateTimeParser::parseDuration($duration)` as seconds.
fn parse_duration(value: &str) -> DavResult<Duration> {
    let invalid =
        || DavError::internal("Sabre\\VObject\\InvalidDataException", format!("Invalid duration supplied: {value}"));
    let v = value.trim();
    let (negative, rest) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v.strip_prefix('+').unwrap_or(v)),
    };
    let rest = rest.strip_prefix('P').ok_or_else(invalid)?;
    let (date_part, time_part) = match rest.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (rest, None),
    };
    let mut seconds: i64 = 0;
    let mut number = String::new();
    for c in date_part.chars() {
        match c {
            '0'..='9' => number.push(c),
            'W' => seconds += number.parse::<i64>().map_err(|_| invalid())? * 7 * 86400,
            'D' => seconds += number.parse::<i64>().map_err(|_| invalid())? * 86400,
            _ => return Err(invalid()),
        }
        if !c.is_ascii_digit() {
            number.clear();
        }
    }
    if !number.is_empty() {
        return Err(invalid());
    }
    if let Some(time_part) = time_part {
        for c in time_part.chars() {
            match c {
                '0'..='9' | '.' => number.push(c),
                'H' => seconds += number.parse::<i64>().map_err(|_| invalid())? * 3600,
                'M' => seconds += number.parse::<i64>().map_err(|_| invalid())? * 60,
                'S' => seconds += number.parse::<f64>().map_err(|_| invalid())? as i64,
                _ => return Err(invalid()),
            }
            if !(c.is_ascii_digit() || c == '.') {
                number.clear();
            }
        }
        if !number.is_empty() {
            return Err(invalid());
        }
    }
    Ok(Duration::seconds(if negative { -seconds } else { seconds }))
}

/// `VEvent::isInTimeRange($start, $end)`.
fn vevent_in_time_range(event: &Component, start: NaiveDateTime, end: NaiveDateTime) -> DavResult<bool> {
    let dtstart = event
        .property("DTSTART")
        .ok_or_else(|| DavError::internal("Error", "Call to a member function getDateTime() on null"))?;
    let effective_start = date_time(dtstart)?;
    let effective_end = if let Some(dtend) = event.property("DTEND") {
        // The DTEND property is considered non inclusive.
        date_time(dtend)?
    } else if let Some(duration) = event.property("DURATION") {
        effective_start + parse_duration(&duration.to_string())?
    } else if !dtstart.has_time() {
        effective_start + Duration::days(1)
    } else {
        effective_start
    };

    if let Some(rrule) = event.property("RRULE") {
        return Ok(recurrence_in_time_range(
            &rrule.get_value().unwrap_or_default(),
            dtstart,
            effective_end - effective_start,
            start,
            end,
        ));
    }

    Ok(start < effective_end && end > effective_start)
}

/// The `EventIterator` of a recurring event, for its plain rules (a
/// frequency with an interval, a count or an end, and the `BYMONTH` /
/// `BYMONTHDAY` of the first instance): whether an instance overlaps the
/// range. Any other rule is taken to overlap from its first instance on.
fn recurrence_in_time_range(
    rrule: &str,
    dtstart: &Property,
    duration: Duration,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> bool {
    let Ok(Some(first)) = dtstart.get_date_time() else {
        return false;
    };
    let mut freq = String::new();
    let mut interval: i64 = 1;
    let mut count: Option<i64> = None;
    let mut until: Option<NaiveDateTime> = None;
    let mut simple = true;
    for part in rrule.split(';').filter(|p| !p.is_empty()) {
        let Some((key, value)) = part.split_once('=') else { continue };
        match key {
            "FREQ" => freq = value.to_owned(),
            "INTERVAL" => interval = value.parse().unwrap_or(1).max(1),
            "COUNT" => count = value.parse().ok(),
            "UNTIL" => {
                until = vobject::parse_date_time(value, None)
                    .or_else(|_| vobject::parse_date(value, None))
                    .ok()
                    .map(|d| instant(&d))
            }
            "WKST" => {}
            "BYMONTH" => simple &= value.parse::<u32>().ok() == Some(first.local.month()),
            "BYMONTHDAY" => simple &= value.parse::<u32>().ok() == Some(first.local.day()),
            _ => simple = false,
        }
    }
    let first_instant = instant(&first);
    if !simple || !matches!(freq.as_str(), "YEARLY" | "MONTHLY" | "WEEKLY" | "DAILY") {
        return first_instant < end && until.is_none_or(|u| u + duration > start);
    }

    let mut produced: i64 = 0;
    let mut step: i64 = 0;
    while step < 100_000 {
        let local = match freq.as_str() {
            "YEARLY" => add_months(first.local, step * interval * 12),
            "MONTHLY" => add_months(first.local, step * interval),
            "WEEKLY" => Some(first.local + Duration::weeks(step * interval)),
            _ => Some(first.local + Duration::days(step * interval)),
        };
        step += 1;
        let Some(local) = local else {
            // An instance on a day the month doesn't have is skipped.
            continue;
        };
        let occurrence = instant(&vobject::DateTime { local, tz: first.tz });
        if count.is_some_and(|c| produced >= c) || until.is_some_and(|u| occurrence > u) || occurrence >= end {
            return false;
        }
        produced += 1;
        if occurrence + duration > start {
            return occurrence < end;
        }
    }
    false
}

/// `$date` plus `months`, `None` when the day doesn't exist in that month.
fn add_months(date: NaiveDateTime, months: i64) -> Option<NaiveDateTime> {
    let total = i64::from(date.year()) * 12 + i64::from(date.month0()) + months;
    let year = i32::try_from(total.div_euclid(12)).ok()?;
    let month = u32::try_from(total.rem_euclid(12)).ok()? + 1;
    NaiveDate::from_ymd_opt(year, month, date.day()).map(|d| d.and_time(date.time()))
}

/// `VTodo::isInTimeRange($start, $end)`.
fn vtodo_in_time_range(todo: &Component, start: NaiveDateTime, end: NaiveDateTime) -> DavResult<bool> {
    let get = |name: &str| -> DavResult<Option<NaiveDateTime>> {
        match todo.property(name) {
            Some(p) => Ok(Some(date_time(p)?)),
            None => Ok(None),
        }
    };
    let dtstart = get("DTSTART")?;
    let duration = match todo.property("DURATION") {
        Some(d) => Some(parse_duration(&d.to_string())?),
        None => None,
    };
    let due = get("DUE")?;
    let completed = get("COMPLETED")?;
    let created = get("CREATED")?;

    if let Some(dtstart) = dtstart {
        if let Some(duration) = duration {
            let effective_end = dtstart + duration;
            return Ok(start <= effective_end && end > dtstart);
        } else if let Some(due) = due {
            return Ok((start < due || start <= dtstart) && (end > dtstart || end >= due));
        } else {
            return Ok(start <= dtstart && end > dtstart);
        }
    }
    if let Some(due) = due {
        return Ok(start < due && end >= due);
    }
    if let (Some(completed), Some(created)) = (completed, created) {
        return Ok((start <= created || start <= completed) && (end >= created || end >= completed));
    }
    if let Some(completed) = completed {
        return Ok(start <= completed && end >= completed);
    }
    if let Some(created) = created {
        return Ok(end > created);
    }
    Ok(true)
}

/// `VJournal::isInTimeRange($start, $end)`.
fn vjournal_in_time_range(journal: &Component, start: NaiveDateTime, end: NaiveDateTime) -> DavResult<bool> {
    let Some(property) = journal.property("DTSTART") else {
        return Ok(false);
    };
    let dtstart = date_time(property)?;
    let mut effective_end = dtstart;
    if !property.has_time() {
        effective_end += Duration::days(1);
    }
    Ok(start <= effective_end && end > dtstart)
}

/// `VAlarm::getEffectiveTriggerTime()`.
fn effective_trigger_time(alarm: &Component, parent: &Component) -> DavResult<NaiveDateTime> {
    let trigger = alarm
        .property("TRIGGER")
        .ok_or_else(|| DavError::internal("Error", "Call to a member function getDateTime() on null"))?;
    let value = trigger.param("VALUE").and_then(vobject::Parameter::get_value);
    if value.as_deref().is_none_or(|v| v.to_uppercase() == "DURATION") {
        let trigger_duration = parse_duration(&trigger.to_string())?;
        let related_end =
            trigger.param("RELATED").and_then(vobject::Parameter::get_value).is_some_and(|r| r.to_uppercase() == "END");

        let required = |name: &str| -> DavResult<NaiveDateTime> {
            let property = parent
                .property(name)
                .ok_or_else(|| DavError::internal("Error", "Call to a member function getDateTime() on null"))?;
            date_time(property)
        };
        if !related_end {
            let prop_name = if parent.name == "VTODO" { "DUE" } else { "DTSTART" };
            Ok(required(prop_name)? + trigger_duration)
        } else {
            let end_prop = match parent.name.as_str() {
                "VTODO" => "DUE",
                "VEVENT" => "DTEND",
                _ => {
                    return Err(DavError::internal(
                        "Sabre\\VObject\\InvalidDataException",
                        "time-range filters on VALARM components are only supported when they are a child of VTODO or VEVENT",
                    ));
                }
            };
            if parent.property(end_prop).is_some() {
                Ok(required(end_prop)? + trigger_duration)
            } else if let Some(duration) = parent.property("DURATION") {
                Ok(required("DTSTART")? + parse_duration(&duration.to_string())? + trigger_duration)
            } else {
                Ok(required("DTSTART")? + trigger_duration)
            }
        }
    } else {
        date_time(trigger)
    }
}

/// `VAlarm::isInTimeRange($start, $end)`.
fn valarm_in_time_range(
    alarm: &Component,
    parent: &Component,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> DavResult<bool> {
    let effective_trigger = effective_trigger_time(alarm, parent)?;
    if let Some(duration) = alarm.property("DURATION") {
        let duration = parse_duration(&duration.to_string())?;
        let repeat =
            alarm.property("REPEAT").map(ToString::to_string).and_then(|r| r.parse::<i64>().ok()).filter(|r| *r != 0);
        let repeat = repeat.unwrap_or(1);

        for i in 0..=repeat {
            let occurrence = effective_trigger + duration * i32::try_from(i).unwrap_or(i32::MAX);
            if start <= occurrence && end > occurrence {
                return Ok(true);
            }
        }
        Ok(false)
    } else {
        Ok(start <= effective_trigger && end > effective_trigger)
    }
}
