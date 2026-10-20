//! `Sabre\DAV\Sync\Plugin` (RFC 6578): the `{DAV:}sync-token` property, the
//! sync-collection report, and sync tokens in `If` headers.

use super::backend::{self, SYNCTOKEN_PREFIX};
use super::nodes::Node;
use super::props::{PropValue, ResponseElement};
use super::server::{DavError, DavResult, IfCondition, PropFind, Server};
use super::xml::Element;

/// `Xml\Request\SyncCollectionReport`.
struct SyncCollectionReport {
    sync_token: Option<String>,
    properties: Vec<String>,
}

/// `SyncCollectionReport::xmlDeserialize()` (the level and limit are read but unused).
fn sync_collection_request(root: &Element) -> DavResult<SyncCollectionReport> {
    let elems = root.key_value();

    for required in ["{DAV:}sync-token", "{DAV:}prop"] {
        if !elems.iter().any(|(k, _)| *k == required) {
            return Err(DavError::bad_request(format!(
                "The {required} element in the {{DAV:}}sync-collection report is required"
            )));
        }
    }

    let get = |name: &str| elems.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    Ok(SyncCollectionReport {
        properties: get("{DAV:}prop").map(Element::enumeration).unwrap_or_default(),
        sync_token: get("{DAV:}sync-token").and_then(Element::text),
    })
}

impl Server<'_> {
    /// `syncCollection($uri, $report)`.
    pub async fn sync_collection(&mut self, uri: &str, report: &Element) -> DavResult<()> {
        let report = sync_collection_request(report)?;

        // Getting the data
        let node = self.tree.node_for_path(self.ctx, uri).await?;
        if !node.is_sync_collection() {
            return Err(DavError::report_not_supported(
                "The {DAV:}sync-collection REPORT is not supported on this url.",
            ));
        }
        let token = node.sync_token(self.ctx).await?;
        if token.as_deref().is_none_or(|t| t.is_empty() || t == "0") {
            return Err(DavError::report_not_supported("No sync information is available at this node"));
        }

        let mut sync_token = report.sync_token.clone();
        if let Some(token) = &sync_token {
            // Sync-token must start with our prefix
            let Some(rest) = token.strip_prefix(SYNCTOKEN_PREFIX) else {
                return Err(DavError::invalid_sync_token("Invalid or unknown sync token"));
            };
            sync_token = Some(rest.to_owned());
        }

        let change_info = match &node {
            Node::AddressBook(info) => {
                match backend::get_changes_for_address_book(self.ctx, &info.id, sync_token.as_deref()).await? {
                    Some(changes) => changes,
                    None => return Err(DavError::invalid_sync_token("Invalid or unknown sync token")),
                }
            }
            Node::Calendar(info) => {
                // `CalDAVBackend::getChangesForCalendar()` answers `[]` instead
                // of `null`, which has no `syncToken` for the plugin to read.
                match backend::get_changes_for_calendar(self.ctx, &info.id, sync_token.as_deref()).await? {
                    Some(changes) => changes,
                    None => return Err(DavError::internal("ErrorException", "Undefined array key \"syncToken\"")),
                }
            }
            _ => unreachable!("only address books and calendars are sync collections"),
        };

        // Encoding the response
        self.send_sync_collection_response(
            &change_info.sync_token.to_string(),
            uri,
            &change_info.added,
            &change_info.modified,
            &change_info.deleted,
            &report.properties,
        )
        .await
    }

    /// `sendSyncCollectionResponse(…)`.
    async fn send_sync_collection_response(
        &mut self,
        sync_token: &str,
        collection_url: &str,
        added: &[String],
        modified: &[String],
        deleted: &[String],
        properties: &[String],
    ) -> DavResult<()> {
        let full_paths: Vec<String> =
            added.iter().chain(modified.iter()).map(|item| format!("{collection_url}/{item}")).collect();

        let mut responses = Vec::new();
        for (full_path, props) in self.get_properties_for_multiple_paths(&full_paths, properties).await? {
            // The 'Property_Response' class is responsible for generating a single {DAV:}response xml element.
            responses.push(ResponseElement { href: full_path, properties: props.properties, http_status: None });
        }

        // Deleted items also show up as 'responses'. They have no properties,
        // and a single {DAV:}status element set as 'HTTP/1.1 404 Not Found'.
        for item in deleted {
            responses.push(ResponseElement {
                href: format!("{collection_url}/{item}"),
                properties: Vec::new(),
                http_status: Some(404),
            });
        }

        let body = self.write_multi_status(&responses, Some(&format!("{SYNCTOKEN_PREFIX}{sync_token}")));
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.body = Some(body);
        Ok(())
    }

    /// `propFind()`: `{DAV:}sync-token`.
    pub async fn sync_prop_find(&mut self, prop_find: &mut PropFind, node: &Node) -> DavResult<()> {
        if !prop_find.wants("{DAV:}sync-token") {
            return Ok(());
        }
        let mut value = None;
        if node.is_sync_collection()
            && let Some(token) = node.sync_token(self.ctx).await?.filter(|t| !t.is_empty() && t != "0")
        {
            value = Some(PropValue::Text(format!("{SYNCTOKEN_PREFIX}{token}")));
        }
        prop_find.handle("{DAV:}sync-token", value);
        Ok(())
    }

    /// `validateTokens($request, &$conditions)`.
    pub async fn sync_validate_tokens(&mut self, conditions: &mut [IfCondition]) -> DavResult<()> {
        for condition in conditions.iter_mut() {
            for token in condition.tokens.iter_mut() {
                // Sync-tokens must always start with our designated prefix.
                let Some(rest) = token.token.strip_prefix(SYNCTOKEN_PREFIX) else {
                    continue;
                };

                // Checking if the token is a match.
                let node = self.tree.node_for_path(self.ctx, &condition.uri).await?;
                if node.is_sync_collection() && node.sync_token(self.ctx).await?.as_deref() == Some(rest) {
                    token.valid_token = true;
                }
            }
        }
        Ok(())
    }
}
