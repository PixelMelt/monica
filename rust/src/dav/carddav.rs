//! `Sabre\CardDAV\Plugin` (properties, the addressbook-multiget and
//! addressbook-query reports, vCard validation and conversion) and
//! `Sabre\CardDAV\VCFExportPlugin`.

use super::backend::php_intval;
use super::http::{negotiate_content_type, resolve};
use super::nodes::Node;
use super::props::PropValue;
use super::server::{DavError, DavResult, PropFind, PropFindResult, Server, calculate_uri};
use super::vobject_ext::{self, PROFILE_CARDDAV, REPAIR};
use super::xml::{Element, NS_CARDDAV};
use crate::vobject::{self, DocumentType};

/// `Plugin::$maxResourceSize`.
const MAX_RESOURCE_SIZE: i64 = 10_000_000;

/// The target of `negotiateVCard()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    VCard3,
    VCard4,
    JCard,
}

/// The `{…}address-data` element of a report (`Xml\Filter\AddressData`).
#[derive(Debug, Clone, Default)]
struct AddressData {
    content_type: Option<String>,
    version: Option<String>,
    address_data_properties: Vec<String>,
}

/// `AddressData::xmlDeserialize()`.
fn address_data(element: &Element) -> AddressData {
    let content_type = element.attribute("content-type").filter(|v| !v.is_empty()).unwrap_or("text/vcard");
    let version = element.attribute("version").filter(|v| !v.is_empty()).unwrap_or("3.0");
    let address_data_properties = element
        .elements()
        .filter(|e| e.clark == format!("{{{NS_CARDDAV}}}prop"))
        .filter_map(|e| e.attribute("name").map(str::to_owned))
        .collect();
    AddressData {
        content_type: Some(content_type.to_owned()),
        version: Some(version.to_owned()),
        address_data_properties,
    }
}

/// The `{DAV:}prop` of a report: the property names, and the first `address-data`.
fn report_prop(element: &Element, data: &mut Option<AddressData>) -> Vec<String> {
    let values = element.key_value();
    if data.is_none()
        && let Some((_, ad)) = values.iter().find(|(k, _)| *k == format!("{{{NS_CARDDAV}}}address-data"))
    {
        *data = Some(address_data(ad));
    }
    values.into_iter().map(|(k, _)| k.to_owned()).collect()
}

/// A text-match of a filter.
#[derive(Debug, Clone)]
struct TextMatch {
    negate_condition: bool,
    collation: String,
    value: String,
    match_type: String,
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
    test: String,
    is_not_defined: bool,
    param_filters: Vec<ParamFilter>,
    text_matches: Vec<TextMatch>,
}

/// The `text-match` element of a filter.
fn text_match(elem: &Element) -> DavResult<TextMatch> {
    let match_type = elem.attribute("match-type").unwrap_or("contains").to_owned();
    if !matches!(match_type.as_str(), "contains" | "equals" | "starts-with" | "ends-with") {
        return Err(DavError::bad_request(format!("Unknown match-type: {match_type}")));
    }
    Ok(TextMatch {
        negate_condition: elem.attribute("negate-condition") == Some("yes"),
        collation: elem.attribute("collation").unwrap_or("i;unicode-casemap").to_owned(),
        value: elem.text().unwrap_or_default(),
        match_type,
    })
}

fn attribute_name(elem: &Element) -> DavResult<String> {
    elem.attribute("name")
        .map(str::to_owned)
        .ok_or_else(|| DavError::internal("ErrorException", "Undefined array key \"name\""))
}

/// `ParamFilter::xmlDeserialize()`.
fn param_filter(element: &Element) -> DavResult<ParamFilter> {
    let mut result = ParamFilter { name: attribute_name(element)?, is_not_defined: false, text_match: None };
    for elem in element.elements() {
        if elem.clark == format!("{{{NS_CARDDAV}}}is-not-defined") {
            result.is_not_defined = true;
        } else if elem.clark == format!("{{{NS_CARDDAV}}}text-match") {
            result.text_match = Some(text_match(elem)?);
        }
    }
    Ok(result)
}

/// `PropFilter::xmlDeserialize()`.
fn prop_filter(element: &Element) -> DavResult<PropFilter> {
    let mut result = PropFilter {
        name: attribute_name(element)?,
        test: if element.attribute("test") == Some("allof") { "allof".to_owned() } else { "anyof".to_owned() },
        is_not_defined: false,
        param_filters: Vec::new(),
        text_matches: Vec::new(),
    };
    for elem in element.elements() {
        if elem.clark == format!("{{{NS_CARDDAV}}}param-filter") {
            result.param_filters.push(param_filter(elem)?);
        } else if elem.clark == format!("{{{NS_CARDDAV}}}is-not-defined") {
            result.is_not_defined = true;
        } else if elem.clark == format!("{{{NS_CARDDAV}}}text-match") {
            result.text_matches.push(text_match(elem)?);
        }
    }
    Ok(result)
}

/// `DAV\StringUtil::textMatch($haystack, $needle, $collation, $matchType)`.
pub fn string_text_match(haystack: &str, needle: &str, collation: &str, match_type: &str) -> DavResult<bool> {
    let (haystack, needle) = match collation {
        // default strtolower takes locale into consideration, we don't want this.
        "i;ascii-casemap" => (haystack.to_ascii_uppercase(), needle.to_ascii_uppercase()),
        "i;octet" => (haystack.to_owned(), needle.to_owned()),
        "i;unicode-casemap" => (haystack.to_uppercase(), needle.to_uppercase()),
        _ => return Err(DavError::bad_request(format!("Collation type: {collation} is not supported"))),
    };

    match match_type {
        "contains" => Ok(haystack.contains(&needle)),
        "equals" => Ok(haystack == needle),
        "starts-with" => Ok(haystack.starts_with(&needle)),
        "ends-with" => Ok(haystack.ends_with(&needle)),
        _ => Err(DavError::bad_request(format!("Match-type: {match_type} is not supported"))),
    }
}

/// `negotiateVCard($input, &$mimeType)`: the target and the mime type.
fn negotiate_vcard(input: Option<&str>) -> (Target, String) {
    let result = negotiate_content_type(
        input,
        &[
            // Most often used mime-type. Version 3
            "text/x-vcard",
            // The correct standard mime-type. Defaults to version 3 as well.
            "text/vcard",
            // vCard 4
            "text/vcard; version=4.0",
            // vCard 3
            "text/vcard; version=3.0",
            // jCard
            "application/vcard+json",
        ],
    );
    match result {
        Some("text/vcard; version=4.0") => (Target::VCard4, "text/vcard; version=4.0".to_owned()),
        Some("application/vcard+json") => (Target::JCard, "application/vcard+json".to_owned()),
        _ => (Target::VCard3, "text/vcard".to_owned()),
    }
}

/// `convertVCard($data, $target, $propertiesFilter)`.
fn convert_vcard(data: &str, target: Target, properties_filter: Option<&[String]>) -> DavResult<String> {
    let mut data = data.to_owned();
    let mut input = vobject::read(&data, 0)?;

    if let Some(filter) = properties_filter.filter(|f| !f.is_empty()) {
        let mut properties_filter: Vec<String> = vec!["UID".into(), "VERSION".into(), "FN".into()];
        properties_filter.extend(filter.iter().cloned());
        let mut keys: Vec<String> = Vec::new();
        for child in input.children() {
            let name = match child {
                vobject::Node::Property(p) => p.name.clone(),
                vobject::Node::Component(c) => c.name.clone(),
            };
            if !keys.contains(&name) {
                keys.push(name);
            }
        }
        for key in keys.iter().filter(|k| !properties_filter.contains(k)) {
            input.remove(key);
        }

        data = input.serialize();
    }

    let output = match target {
        Target::VCard3 => {
            if input.document_type() == DocumentType::VCard30 {
                // Do nothing
                return Ok(data);
            }
            vobject_ext::convert(&input, DocumentType::VCard30)?
        }
        Target::VCard4 | Target::JCard => {
            if target == Target::VCard4 && input.document_type() == DocumentType::VCard40 {
                // Do nothing
                return Ok(data);
            }
            // jCard (`json_encode()` of the vCard 4) isn't ported: the vCard 4 is sent.
            vobject_ext::convert(&input, DocumentType::VCard40)?
        }
    };
    Ok(output.serialize())
}

impl Server<'_> {
    /// `propFindEarly()`.
    pub fn carddav_prop_find_early(&mut self, prop_find: &mut PropFind, node: &Node) {
        let ns = format!("{{{NS_CARDDAV}}}");
        if matches!(node, Node::AddressBook(_)) {
            prop_find.handle(&format!("{ns}max-resource-size"), Some(PropValue::Text(MAX_RESOURCE_SIZE.to_string())));
            prop_find.handle(&format!("{ns}supported-address-data"), Some(PropValue::SupportedAddressData));
            prop_find.handle(&format!("{ns}supported-collation-set"), Some(PropValue::CardSupportedCollationSet));
        }
        if node.is_principal() {
            let path = prop_find.path().to_owned();
            let (_, principal_id) = super::http::split(&path);
            prop_find.handle(
                &format!("{ns}addressbook-home-set"),
                Some(PropValue::local_href(vec![format!("addressbooks/{principal_id}/")])),
            );
        }
        if matches!(node, Node::Card { .. }) {
            // The address-data property is not supposed to be a 'real'
            // property, but in large chunks of the spec it does act as such.
            // Therefore we simply expose it as a property.
            prop_find.handle(&format!("{ns}address-data"), Some(PropValue::Text(node.get())));
        }
    }

    /// `propFindLate()`: the content type Thunderbird's SOGo connector wants.
    pub fn carddav_prop_find_late(&mut self, prop_find: &mut PropFind) {
        if !self.request.header("User-Agent").unwrap_or_default().contains("Thunderbird") {
            return;
        }
        if let Some(content_type) = prop_find.get("{DAV:}getcontenttype").and_then(PropValue::as_text) {
            let part = content_type.split(';').next().unwrap_or_default();
            if part == "text/x-vcard" || part == "text/vcard" {
                prop_find.set("{DAV:}getcontenttype", Some(PropValue::Text("text/x-vcard".to_owned())), None);
            }
        }
    }

    /// `report()` of the CardDAV reports.
    pub async fn carddav_report(&mut self, report: &Element) -> DavResult<()> {
        if report.clark == format!("{{{NS_CARDDAV}}}addressbook-multiget") {
            self.addressbook_multi_get_report(report).await
        } else {
            self.addressbook_query_report(report).await
        }
    }

    /// `addressbookMultiGetReport($report)`.
    async fn addressbook_multi_get_report(&mut self, report: &Element) -> DavResult<()> {
        // AddressBookMultiGetReport::xmlDeserialize()
        let mut properties = Vec::new();
        let mut hrefs = Vec::new();
        let mut data = None;
        for elem in report.elements() {
            match elem.clark.as_str() {
                "{DAV:}prop" => properties = report_prop(elem, &mut data),
                "{DAV:}href" => hrefs.push(resolve(&self.request.url, &elem.text().unwrap_or_default())),
                _ => {}
            }
        }
        let data = data.unwrap_or_default();

        let mut content_type = data.content_type.clone().unwrap_or_default();
        if let Some(version) = &data.version {
            content_type.push_str(&format!("; version={version}"));
        }
        let content_type = (data.content_type.is_some() || data.version.is_some()).then_some(content_type);

        let (vcard_type, _) = negotiate_vcard(content_type.as_deref());

        let mut paths = Vec::new();
        for href in &hrefs {
            paths.push(calculate_uri(href)?);
        }

        let mut property_list = Vec::new();
        for (_, mut props) in self.get_properties_for_multiple_paths(&paths, &properties).await? {
            let name = format!("{{{NS_CARDDAV}}}address-data");
            if let Some(PropValue::Text(card)) = props.found(&name).cloned() {
                props.replace_found(&name, PropValue::Text(convert_vcard(&card, vcard_type, None)?));
            }
            property_list.push(props);
        }

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.body = Some(self.generate_multi_status(property_list, minimal));
        Ok(())
    }

    /// `addressbookQueryReport($report)`.
    async fn addressbook_query_report(&mut self, report: &Element) -> DavResult<()> {
        // AddressBookQueryReport::xmlDeserialize()
        let mut filters: Option<Vec<PropFilter>> = None;
        let mut properties = Vec::new();
        let mut test = "anyof".to_owned();
        let mut limit: Option<i64> = None;
        let mut data = None;
        for elem in report.elements() {
            if elem.clark == "{DAV:}prop" {
                properties = report_prop(elem, &mut data);
            } else if elem.clark == format!("{{{NS_CARDDAV}}}filter") {
                if filters.is_some() {
                    return Err(DavError::bad_request(format!(
                        "You can only include 1 {{{NS_CARDDAV}}}filter element"
                    )));
                }
                if let Some(t) = elem.attribute("test") {
                    test = t.to_owned();
                    if test != "allof" && test != "anyof" {
                        return Err(DavError::bad_request(
                            "The \"test\" attribute must be one of \"allof\" or \"anyof\"",
                        ));
                    }
                }
                let mut list = Vec::new();
                for sub in elem.elements() {
                    if sub.clark == format!("{{{NS_CARDDAV}}}prop-filter") {
                        list.push(prop_filter(sub)?);
                    }
                }
                filters = Some(list);
            } else if elem.clark == format!("{{{NS_CARDDAV}}}limit") {
                for child in elem.elements() {
                    if child.clark == format!("{{{NS_CARDDAV}}}nresults") {
                        limit = Some(php_intval(&child.text().unwrap_or_default()));
                    }
                }
            }
        }
        // KDE sometimes does not include the filter element: no filters.
        let filters = filters.unwrap_or_default();
        let data = data.unwrap_or_default();

        let depth = self.http_depth(0);
        let request_uri = self.request_uri()?;

        let candidate_nodes = if depth == 0 {
            let node = self.tree.node_for_path(self.ctx, &request_uri).await?;
            if !matches!(node, Node::Card { .. }) {
                return Err(DavError::report_not_supported(
                    "The addressbook-query report is not supported on this url with Depth: 0",
                ));
            }
            vec![node]
        } else {
            self.tree.children(self.ctx, &request_uri).await?
        };

        let mut content_type = data.content_type.clone().unwrap_or_default();
        if let Some(version) = &data.version {
            content_type.push_str(&format!("; version={version}"));
        }
        let content_type = (data.content_type.is_some() || data.version.is_some()).then_some(content_type);
        let (vcard_type, _) = negotiate_vcard(content_type.as_deref());

        let mut valid_nodes = Vec::new();
        for node in candidate_nodes {
            if !matches!(node, Node::Card { .. }) {
                continue;
            }

            let blob = node.get();
            if !validate_filters(&blob, &filters, &test)? {
                continue;
            }

            valid_nodes.push(node);

            if limit.is_some_and(|l| l != 0 && l <= valid_nodes.len() as i64) {
                // We hit the maximum number of items, we can stop now.
                break;
            }
        }

        let mut result: Vec<PropFindResult> = Vec::new();
        for valid_node in valid_nodes {
            let href = if depth == 0 { request_uri.clone() } else { format!("{request_uri}/{}", valid_node.name()) };

            let Some(mut props) = self.get_properties_for_path(&href, &properties, 0).await?.into_iter().next() else {
                continue;
            };

            let name = format!("{{{NS_CARDDAV}}}address-data");
            if let Some(PropValue::Text(card)) = props.found(&name).cloned() {
                let converted = convert_vcard(&card, vcard_type, Some(&data.address_data_properties))?;
                props.replace_found(&name, PropValue::Text(converted));
            }
            result.push(props);
        }

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.body = Some(self.generate_multi_status(result, minimal));
        Ok(())
    }

    /// `validateVCard(&$data, &$modified)` (`beforeWriteContent` / `beforeCreateFile`).
    pub fn validate_vcard(&mut self, data: &mut String, modified: &mut bool) -> DavResult<()> {
        let before = data.clone();

        let mut vobj = if data.starts_with('[') {
            // jCard (`VObject\Reader::readJson()`) isn't ported.
            return Err(DavError::unsupported_media_type(
                "This resource only supports valid vCard or jCard data. Parse error: jCard is not supported",
            ));
        } else {
            match vobject::read(data, 0) {
                Ok(vobj) => vobj,
                Err(vobject::Error::Parse(message)) => {
                    return Err(DavError::unsupported_media_type(format!(
                        "This resource only supports valid vCard or jCard data. Parse error: {message}"
                    )));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if vobj.name != "VCARD" {
            return Err(DavError::unsupported_media_type("This collection can only support vcard objects."));
        }

        let mut options = PROFILE_CARDDAV;
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
                        "Validation error in vCard: {}",
                        message.message
                    )));
                }
                _ => {}
            }
        }
        if let Some(warning) = warning_message {
            self.response.set_header("X-Sabre-Ew-Gross", format!("vCard validation warning: {warning}"));

            // Re-serializing object.
            *data = vobj.serialize();
            if !*modified && *data != before {
                // This ensures that the system does not send an ETag back.
                *modified = true;
            }
        }
        Ok(())
    }

    /// `httpAfterGet()`: the vCard version the client accepts.
    pub async fn carddav_after_get(&mut self) -> DavResult<()> {
        let content_type = self.response.header("Content-Type");
        if content_type.as_deref().is_none_or(|c| !c.contains("text/vcard")) {
            return Ok(());
        }

        let (target, mime_type) = negotiate_vcard(self.request.header("Accept").as_deref());

        let new_body = convert_vcard(self.response.body.as_deref().unwrap_or_default(), target, None)?;

        self.response.set_header("Content-Type", format!("{mime_type}; charset=utf-8"));
        self.response.set_header("Content-Length", new_body.len().to_string());
        self.response.body = Some(new_body);
        Ok(())
    }

    /// `VCFExportPlugin::httpGet()`: `false` when it isn't an export.
    pub async fn vcf_http_get(&mut self) -> DavResult<bool> {
        if !self.request.query_parameters().iter().any(|(k, _)| k == "export") {
            return Ok(false);
        }

        let path = self.request.path()?;

        let node = self.tree.node_for_path(self.ctx, &path).await?;

        if !matches!(node, Node::AddressBook(_)) {
            return Ok(false);
        }

        // Checking ACL, if available.
        self.check_privileges(&path, &["{DAV:}read"], true).await?;

        let name = format!("{{{NS_CARDDAV}}}address-data");
        let nodes = self.get_properties_for_path(&path, std::slice::from_ref(&name), 1).await?;

        let mut output = String::new();
        for node in nodes {
            let Some(PropValue::Text(node_data)) = node.found(&name) else {
                continue;
            };

            // Parsing this node so VObject can clean up the output.
            let vcard = vobject::read(node_data, 0)?;
            output.push_str(&vcard.serialize());
        }

        let mut filename: String =
            node.name().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ')).collect();
        filename.push_str(&format!("-{}.vcf", chrono::Utc::now().format("%Y-%m-%d")));

        self.response.set_header("Content-Disposition", format!("attachment; filename=\"{filename}\""));
        self.response.set_header("Content-Type", "text/directory");
        self.response.status = Some(200);
        self.response.body = Some(output);

        Ok(true)
    }
}

/// `validateFilters($vcardData, $filters, $test)`.
fn validate_filters(vcard_data: &str, filters: &[PropFilter], test: &str) -> DavResult<bool> {
    if filters.is_empty() {
        return Ok(true);
    }
    let vcard = vobject::read(vcard_data, 0)?;

    for filter in filters {
        let is_defined = !vcard.select(&filter.name).is_empty();
        let success = if filter.is_not_defined {
            !is_defined
        } else if (filter.param_filters.is_empty() && filter.text_matches.is_empty()) || !is_defined {
            // We only need to check for existence
            is_defined
        } else {
            let v_properties = vcard.properties(&filter.name);
            let mut results = Vec::new();
            if !filter.param_filters.is_empty() {
                results.push(validate_param_filters(&v_properties, &filter.param_filters, &filter.test)?);
            }
            if !filter.text_matches.is_empty() {
                let texts: Vec<String> = v_properties.iter().map(|p| p.get_value().unwrap_or_default()).collect();
                results.push(validate_text_matches(&texts, &filter.text_matches, &filter.test)?);
            }

            if results.len() == 1 {
                results[0]
            } else if filter.test == "anyof" {
                results[0] || results[1]
            } else {
                results[0] && results[1]
            }
        };

        // There are two conditions where we can already determine whether or not this filter succeeds.
        if test == "anyof" && success {
            return Ok(true);
        }
        if test == "allof" && !success {
            return Ok(false);
        }
    }

    // If we got all the way here, it means we haven't been able to
    // determine early if the test failed or not.
    //
    // This implies for 'anyof' that the test failed, and for 'allof' that
    // we succeeded. Sounds weird, but makes sense.
    Ok(test == "allof")
}

/// `validateParamFilters($vProperties, $filters, $test)`.
fn validate_param_filters(v_properties: &[&vobject::Property], filters: &[ParamFilter], test: &str) -> DavResult<bool> {
    for filter in filters {
        let is_defined = v_properties.iter().any(|p| p.param(&filter.name).is_some());

        let success = if filter.is_not_defined {
            !is_defined
        } else if filter.text_match.is_none() || !is_defined {
            // If there's no text-match, we can just check for existence
            is_defined
        } else {
            let text_match = filter.text_match.as_ref().expect("checked above");
            let mut success = false;
            for v_property in v_properties {
                // If we got all the way here, we'll need to validate the text-match filter.
                if let Some(param) = v_property.param(&filter.name) {
                    success = string_text_match(
                        &param.get_value().unwrap_or_default(),
                        &text_match.value,
                        &text_match.collation,
                        &text_match.match_type,
                    )?;
                    if text_match.negate_condition {
                        success = !success;
                    }
                }
                if success {
                    break;
                }
            }
            success
        };

        // There are two conditions where we can already determine whether or not this filter succeeds.
        if test == "anyof" && success {
            return Ok(true);
        }
        if test == "allof" && !success {
            return Ok(false);
        }
    }

    Ok(test == "allof")
}

/// `validateTextMatches($texts, $filters, $test)`.
fn validate_text_matches(texts: &[String], filters: &[TextMatch], test: &str) -> DavResult<bool> {
    for filter in filters {
        let mut success = false;
        for haystack in texts {
            success = string_text_match(haystack, &filter.value, &filter.collation, &filter.match_type)?;
            if filter.negate_condition {
                success = !success;
            }

            // Breaking on the first match
            if success {
                break;
            }
        }

        if success && test == "anyof" {
            return Ok(true);
        }

        if !success && test == "allof" {
            return Ok(false);
        }
    }

    Ok(test == "allof")
}
