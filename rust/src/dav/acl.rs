//! `Sabre\DAVACL\Plugin` with `allowUnauthenticatedAccess = false` and
//! `hideNodesFromListings = true`: privileges, the ACL properties and the
//! principal reports.

use super::backend;
use super::http::split;
use super::nodes::Node;
use super::props::{Ace, PrincipalKind, Privilege, PropValue, ResponseElement};
use super::server::{BASE_URI, DavError, DavResult, PropFind, PropFindResult, Server, calculate_uri};
use super::xml::{self, Element, Inner, Writer};

/// `DAVACL\Plugin::$principalCollectionSet`.
const PRINCIPAL_COLLECTION_SET: [&str; 1] = ["principals"];

/// `DAVACL\Plugin::$principalSearchPropertySet`, with CalDAV's addition.
const PRINCIPAL_SEARCH_PROPERTY_SET: [(&str, &str); 3] = [
    ("{DAV:}displayname", "Display name"),
    ("{http://sabredav.org/ns}email-address", "Email address"),
    ("{urn:ietf:params:xml:ns:caldav}calendar-user-address-set", "Calendar address"),
];

/// A flattened privilege (`getFlatPrivilegeSet()`).
#[derive(Debug, Clone)]
struct FlatPrivilege {
    privilege: String,
    abstract_: bool,
    aggregates: Vec<String>,
}

impl Server<'_> {
    /// `checkPrivileges($uri, $privileges, $recursion, $throwExceptions)`.
    pub async fn check_privileges(&mut self, uri: &str, privileges: &[&str], throw: bool) -> DavResult<bool> {
        let node = self.tree.node_for_path(self.ctx, uri).await?;
        let acl = self.current_user_privilege_set(&node).await?;

        let failed: Vec<String> =
            privileges.iter().filter(|p| !acl.iter().any(|a| a == *p)).map(|p| (*p).to_owned()).collect();

        if !failed.is_empty() {
            if throw {
                return Err(DavError::need_privileges(uri, failed));
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// `getCurrentUserPrincipals()`.
    async fn current_user_principals(&mut self) -> DavResult<Vec<String>> {
        let Some(current) = self.current_principal.clone() else {
            return Ok(Vec::new());
        };
        let mut principals = vec![current.clone()];
        principals.extend(self.principal_membership(&current).await?);
        Ok(principals)
    }

    /// `getPrincipalMembership($mainPrincipal)`.
    pub async fn principal_membership(&mut self, main_principal: &str) -> DavResult<Vec<String>> {
        // First check our cache
        if let Some(cached) = self.principal_membership_cache.get(main_principal) {
            return Ok(cached.clone());
        }

        let mut check = vec![main_principal.to_owned()];
        let mut principals: Vec<String> = Vec::new();

        while !check.is_empty() {
            let principal = check.remove(0);

            let node = self.tree.node_for_path(self.ctx, &principal).await?;
            if let Node::Principal(p) = node {
                for group_member in backend::get_group_membership(self.ctx, &p.uri) {
                    if !principals.contains(&group_member) {
                        check.push(group_member.clone());
                        principals.push(group_member);
                    }
                }
            }
        }

        // Store the result in the cache
        self.principal_membership_cache.insert(main_principal.to_owned(), principals.clone());

        Ok(principals)
    }

    /// `principalMatchesPrincipal($checkPrincipal)`.
    async fn principal_matches_principal(&mut self, check_principal: &str) -> DavResult<bool> {
        let Some(current) = self.current_principal.clone() else {
            return Ok(false);
        };
        if current == check_principal {
            return Ok(true);
        }
        Ok(self.principal_membership(&current).await?.iter().any(|p| p == check_principal))
    }

    /// `getSupportedPrivilegeSet($node)`, with CalDAV's `read-free-busy` on calendars.
    fn supported_privilege_set(node: &Node) -> Vec<Privilege> {
        let leaf = |name: &str| Privilege { name: name.to_owned(), abstract_: false, aggregates: Vec::new() };
        let mut read = vec![leaf("{DAV:}read-acl"), leaf("{DAV:}read-current-user-privilege-set")];
        let mut write = vec![leaf("{DAV:}write-properties"), leaf("{DAV:}write-content"), leaf("{DAV:}unlock")];
        if node.is_collection() {
            write.push(leaf("{DAV:}bind"));
            write.push(leaf("{DAV:}unbind"));
        }
        if node.is_acl() {
            write.push(leaf("{DAV:}write-acl"));
        }
        if matches!(node, Node::Calendar(_)) {
            read.push(leaf("{urn:ietf:params:xml:ns:caldav}read-free-busy"));
        }
        vec![
            Privilege { name: "{DAV:}read".to_owned(), abstract_: false, aggregates: read },
            Privilege { name: "{DAV:}write".to_owned(), abstract_: false, aggregates: write },
        ]
    }

    /// `getFlatPrivilegeSet($node)`.
    fn flat_privilege_set(node: &Node) -> Vec<FlatPrivilege> {
        fn traverse(privilege: &Privilege, flat: &mut Vec<FlatPrivilege>) {
            let entry = FlatPrivilege {
                privilege: privilege.name.clone(),
                abstract_: privilege.abstract_,
                aggregates: privilege.aggregates.iter().map(|a| a.name.clone()).collect(),
            };
            match flat.iter_mut().find(|f| f.privilege == entry.privilege) {
                Some(existing) => *existing = entry,
                None => flat.push(entry),
            }
            for sub in &privilege.aggregates {
                traverse(sub, flat);
            }
        }
        let all = Privilege {
            name: "{DAV:}all".to_owned(),
            abstract_: false,
            aggregates: Server::supported_privilege_set(node),
        };
        let mut flat = Vec::new();
        traverse(&all, &mut flat);
        flat
    }

    /// `getAcl($node)`: the node's, or the default ACL.
    fn acl_of(node: &Node) -> Vec<Ace> {
        if !node.is_acl() {
            return vec![Ace {
                principal: "{DAV:}authenticated".to_owned(),
                protected: true,
                privilege: "{DAV:}all".to_owned(),
            }];
        }
        node.acl()
    }

    /// `getCurrentUserPrivilegeSet($node)`.
    pub async fn current_user_privilege_set(&mut self, node: &Node) -> DavResult<Vec<String>> {
        let acl = Server::acl_of(node);

        let mut collected: Vec<String> = Vec::new();
        let is_authenticated = self.current_principal.is_some();

        for ace in acl {
            let principal = ace.principal.as_str();
            let matches = match principal {
                "{DAV:}owner" => match node.owner() {
                    Some(owner) if !owner.is_empty() => self.principal_matches_principal(&owner).await?,
                    _ => false,
                },
                // 'all' matches for every user
                "{DAV:}all" => true,
                // Authenticated users only
                "{DAV:}authenticated" => is_authenticated,
                // Unauthenticated users only
                "{DAV:}unauthenticated" => !is_authenticated,
                _ => self.principal_matches_principal(principal).await?,
            };
            if matches {
                collected.push(ace.privilege);
            }
        }

        // Now we deduct all aggregated privileges.
        let flat = Server::flat_privilege_set(node);

        let mut collected2: Vec<String> = Vec::new();
        while let Some(current) = collected.pop() {
            collected2.push(current.clone());

            let Some(entry) = flat.iter().find(|f| f.privilege == current) else {
                // Ignoring privileges that are not in the supported-privileges list.
                continue;
            };

            for sub in &entry.aggregates {
                collected2.push(sub.clone());
                collected.push(sub.clone());
            }
        }

        let mut unique: Vec<String> = Vec::new();
        for privilege in collected2 {
            if !unique.contains(&privilege) {
                unique.push(privilege);
            }
        }
        Ok(unique)
    }

    /// `beforeMethod()`: the privileges a method needs on an existing node.
    pub async fn acl_before_method(&mut self, method: &str) -> DavResult<()> {
        let path = self.request.path()?;

        let exists = self.tree.node_exists(self.ctx, &path).await?;

        // If the node doesn't exists, none of these checks apply
        if !exists {
            return Ok(());
        }

        match method {
            // For these 3 we only need to know if the node is readable.
            "GET" | "HEAD" | "OPTIONS" => {
                self.check_privileges(&path, &["{DAV:}read"], true).await?;
            }
            // This method requires the write-content priv if the node
            // already exists, and bind on the parent if the node is being created.
            "PUT" | "LOCK" => {
                self.check_privileges(&path, &["{DAV:}write-content"], true).await?;
            }
            "PROPPATCH" => {
                self.check_privileges(&path, &["{DAV:}write-properties"], true).await?;
            }
            "ACL" => {
                self.check_privileges(&path, &["{DAV:}write-acl"], true).await?;
            }
            // Copy requires read privileges on the entire source tree.
            "COPY" | "MOVE" => {
                self.check_privileges(&path, &["{DAV:}read"], true).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// `beforeBind($uri)`.
    pub async fn acl_before_bind(&mut self, uri: &str) -> DavResult<()> {
        let (parent_uri, _) = split(uri);
        self.check_privileges(&parent_uri, &["{DAV:}bind"], true).await?;
        Ok(())
    }

    /// `beforeUnbind($uri)`.
    pub async fn acl_before_unbind(&mut self, uri: &str) -> DavResult<()> {
        let (parent_uri, _) = split(uri);
        self.check_privileges(&parent_uri, &["{DAV:}unbind"], true).await?;
        Ok(())
    }

    /// `propFind()`: `false` hides the node.
    pub async fn acl_prop_find(&mut self, prop_find: &mut PropFind, node: &Node) -> DavResult<bool> {
        let path = prop_find.path().to_owned();

        // Checking the read permission
        if !self.check_privileges(&path, &["{DAV:}read"], false).await? {
            // User is not allowed to read properties; the node is hidden.
            return Ok(false);
        }

        // Adding principal properties
        if let Node::Principal(principal) = node {
            // `getAlternateUriSet()`: a principal without an email address has none.
            prop_find.handle("{DAV:}alternate-URI-set", Some(PropValue::Href(Vec::new())));
            prop_find.handle("{DAV:}principal-URL", Some(PropValue::Href(vec![format!("{}/", principal.uri)])));
            if prop_find.wants("{DAV:}group-member-set") {
                let members = backend::get_group_member_set(self.ctx, &principal.uri)
                    .iter()
                    .map(|m| format!("{}/", m.trim_end_matches('/')))
                    .collect();
                prop_find.handle("{DAV:}group-member-set", Some(PropValue::Href(members)));
            }
            if prop_find.wants("{DAV:}group-membership") {
                let members = backend::get_group_membership(self.ctx, &principal.uri)
                    .iter()
                    .map(|m| format!("{}/", m.trim_end_matches('/')))
                    .collect();
                prop_find.handle("{DAV:}group-membership", Some(PropValue::Href(members)));
            }
            prop_find.handle("{DAV:}displayname", Some(PropValue::Text(principal.displayname.clone())));
        }

        prop_find.handle(
            "{DAV:}principal-collection-set",
            Some(PropValue::Href(PRINCIPAL_COLLECTION_SET.iter().map(|v| format!("{v}/")).collect())),
        );
        let current_user_principal = match &self.current_principal {
            Some(url) => PrincipalKind::Href(format!("{url}/")),
            None => PrincipalKind::Unauthenticated,
        };
        prop_find.handle("{DAV:}current-user-principal", Some(PropValue::Principal(current_user_principal)));
        prop_find.handle(
            "{DAV:}supported-privilege-set",
            Some(PropValue::SupportedPrivilegeSet(Server::supported_privilege_set(node))),
        );
        if prop_find.wants("{DAV:}current-user-privilege-set") {
            if !self.check_privileges(&path, &["{DAV:}read-current-user-privilege-set"], false).await? {
                prop_find.set("{DAV:}current-user-privilege-set", None, Some(403));
            } else {
                let value = self.current_user_privilege_set(node).await?;
                prop_find.handle("{DAV:}current-user-privilege-set", Some(PropValue::CurrentUserPrivilegeSet(value)));
            }
        }
        if prop_find.wants("{DAV:}acl") {
            // The ACL property contains all the permissions
            if !self.check_privileges(&path, &["{DAV:}read-acl"], false).await? {
                prop_find.set("{DAV:}acl", None, Some(403));
            } else {
                prop_find.handle("{DAV:}acl", Some(PropValue::Acl(Server::acl_of(node))));
            }
        }
        prop_find.handle("{DAV:}acl-restrictions", Some(PropValue::AclRestrictions));

        // Adding ACL properties
        if node.is_acl() {
            prop_find
                .handle("{DAV:}owner", Some(PropValue::Href(vec![format!("{}/", node.owner().unwrap_or_default())])));
        }
        Ok(true)
    }

    /// `httpAcl()`: no node accepts a new ACL.
    pub async fn acl_http_acl(&mut self) -> DavResult<bool> {
        let path = self.request.path()?;
        let body = self.request.body.clone();

        if body.is_empty() {
            return Err(DavError::bad_request("XML body expected in ACL request"));
        }

        let root = xml::parse(&body)?;
        if root.clark != "{DAV:}acl" {
            return Err(DavError::internal(
                "Sabre\\Xml\\ParseException",
                format!("Expected {{DAV:}}acl but received {} as the root element", root.clark),
            ));
        }
        let mut new_acl = acl_request(&root)?;

        // Normalizing urls
        for ace in &mut new_acl {
            ace.principal = calculate_uri(&ace.principal)?;
        }

        let node = self.tree.node_for_path(self.ctx, &path).await?;

        if !node.is_acl() {
            return Err(DavError::method_not_allowed("This node does not support the ACL method"));
        }

        let old_acl = Server::acl_of(&node);
        let supported_privileges = Server::flat_privilege_set(&node);

        // Checking if protected principals from the existing principal set are not overwritten.
        for old_ace in old_acl.iter().filter(|a| a.protected) {
            let found = new_acl.iter().any(|new_ace| {
                new_ace.privilege == old_ace.privilege && new_ace.principal == old_ace.principal && new_ace.protected
            });
            if !found {
                return Err(DavError::ace_conflict(
                    "This resource contained a protected {DAV:}ace, but this privilege did not occur in the ACL request",
                ));
            }
        }

        for new_ace in &new_acl {
            // Do we recognize the privilege
            let Some(supported) = supported_privileges.iter().find(|p| p.privilege == new_ace.privilege) else {
                return Err(DavError::not_supported_privilege(format!(
                    "The privilege you specified ({}) is not recognized by this server",
                    new_ace.privilege
                )));
            };

            if supported.abstract_ {
                return Err(DavError::no_abstract(format!(
                    "The privilege you specified ({}) is an abstract privilege",
                    new_ace.privilege
                )));
            }

            // Looking up the principal
            let principal = match self.tree.node_for_path(self.ctx, &new_ace.principal).await {
                Ok(principal) => principal,
                Err(e) if e.class == super::nodes::NOT_FOUND => {
                    return Err(DavError::not_recognized_principal(format!(
                        "The specified principal ({}) does not exist",
                        new_ace.principal
                    )));
                }
                Err(e) => return Err(e),
            };
            if !principal.is_principal() {
                return Err(DavError::not_recognized_principal(format!(
                    "The specified uri ({}) is not a principal",
                    new_ace.principal
                )));
            }
        }

        // ACLTrait::setACL()
        Err(DavError::forbidden("Setting ACL is not supported on this node"))
    }

    /// `report($reportName, $report, $path)` for the ACL reports.
    pub async fn acl_report(&mut self, report: &Element, path: &str) -> DavResult<()> {
        match report.clark.as_str() {
            "{DAV:}principal-property-search" => self.principal_property_search_report(path, report).await,
            "{DAV:}principal-search-property-set" => self.principal_search_property_set_report(report),
            "{DAV:}expand-property" => self.expand_property_report(path, report).await,
            "{DAV:}principal-match" => self.principal_match_report(path, report).await,
            _ => self.acl_principal_prop_set_report(path, report).await,
        }
    }

    /// `principalMatchReport($path, $report)`.
    async fn principal_match_report(&mut self, path: &str, report: &Element) -> DavResult<()> {
        // PrincipalMatchReport::xmlDeserialize(): keys in the DAV: namespace by local name.
        let elems = report.key_value();
        let get = |name: &str| elems.iter().find(|(k, _)| *k == format!("{{DAV:}}{name}")).map(|(_, v)| *v);
        let mut is_self = get("self").is_some();
        let mut principal_property = None;
        if let Some(pp) = get("principal-property") {
            is_self = false;
            principal_property = Some(
                pp.elements()
                    .next()
                    .map(|e| e.clark.clone())
                    .ok_or_else(|| DavError::internal("ErrorException", "Trying to access array offset on null"))?,
            );
        }
        let properties = get("prop").map(Element::enumeration).unwrap_or_default();

        let depth = self.http_depth(0);
        if depth != 0 {
            return Err(DavError::bad_request("The principal-match report is only defined on Depth: 0"));
        }

        let current_principals = self.current_user_principals().await?;

        let mut result = Vec::new();

        if is_self {
            // Finding all principals under the request uri that match the current principal.
            for current_principal in &current_principals {
                if current_principal == path || current_principal.starts_with(&format!("{path}/")) {
                    result.push(current_principal.clone());
                }
            }
        } else if let Some(principal_property) = principal_property {
            // We need to find all resources that have a property that matches one of the current principals.
            let candidates = self.get_properties_for_path(path, std::slice::from_ref(&principal_property), 1).await?;

            for candidate in candidates {
                let Some(PropValue::Href(hrefs)) = candidate.found(&principal_property) else {
                    continue;
                };
                if hrefs.iter().any(|href| current_principals.iter().any(|p| p == href.trim_matches('/'))) {
                    result.push(candidate.href.clone());
                }
            }
        }

        let mut responses = Vec::new();
        for item in result {
            let mut item = item;
            let mut found = Vec::new();
            if !properties.is_empty() {
                let first = self
                    .get_properties_for_path(&item, &properties, 0)
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| DavError::internal("ErrorException", "Undefined array key 0"))?;
                item = first.href;
                found = first.properties;
            }
            responses.push(ResponseElement { href: item, properties: found, http_status: Some(200) });
        }

        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.status = Some(207);
        // `$xml->write('{DAV:}multistatus', $responses)`: the responses are
        // written without their `{DAV:}response` element.
        let mut w = Writer::new(BASE_URI);
        w.set_indent();
        w.start_document();
        w.start_element("{DAV:}multistatus");
        for response in &responses {
            response.serialize(&mut w);
        }
        w.end_element();
        self.response.body = Some(w.output());
        Ok(())
    }

    /// `expandPropertyReport($path, $report)`.
    async fn expand_property_report(&mut self, path: &str, report: &Element) -> DavResult<()> {
        let properties = expand_property_request(report)?;
        let depth = self.http_depth(0);

        let result = self.expand_properties(path, &properties, depth).await?;

        let xml = self.write_multi_status(&result, None);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.status = Some(207);
        self.response.body = Some(xml);
        Ok(())
    }

    /// `expandProperties($path, $requestedProperties, $depth)`.
    async fn expand_properties(
        &mut self,
        path: &str,
        requested_properties: &[ExpandProperty],
        depth: i32,
    ) -> DavResult<Vec<ResponseElement>> {
        let names: Vec<String> = requested_properties.iter().map(|p| p.name.clone()).collect();
        let found_properties = self.get_properties_for_path(path, &names, depth).await?;

        let mut result = Vec::new();

        for mut node in found_properties {
            for requested in requested_properties {
                // We're only traversing if sub-properties were requested
                let Some(children) = requested.children.as_ref().filter(|c| !c.is_empty()) else {
                    continue;
                };

                // We only have to do the expansion if the property was found and it contains an href element.
                let Some(PropValue::Href(child_hrefs)) = node.found(&requested.name).cloned() else {
                    continue;
                };

                let mut child_props = Vec::new();
                for href in child_hrefs {
                    // Gathering the result of the children
                    let expanded = Box::pin(self.expand_properties(&href, children, 0)).await?;
                    child_props.push(
                        expanded
                            .into_iter()
                            .next()
                            .ok_or_else(|| DavError::internal("ErrorException", "Undefined array key 0"))?,
                    );
                }

                // Replacing the property with its expanded form.
                node.replace_found(&requested.name, PropValue::Responses(child_props));
            }
            result.push(ResponseElement { href: node.href, properties: node.properties, http_status: None });
        }

        Ok(result)
    }

    /// `principalSearchPropertySetReport($path, $report)`.
    fn principal_search_property_set_report(&mut self, report: &Element) -> DavResult<()> {
        // PrincipalSearchPropertySetReport::xmlDeserialize()
        if !report.self_closing {
            return Err(DavError::bad_request("The {DAV:}principal-search-property-set element must be empty"));
        }

        let http_depth = self.http_depth(0);
        if http_depth != 0 {
            return Err(DavError::bad_request("This report is only defined when Depth: 0"));
        }

        let mut writer = Writer::new("");
        writer.start_document();
        writer.start_element("{DAV:}principal-search-property-set");

        for (property_name, description) in PRINCIPAL_SEARCH_PROPERTY_SET {
            writer.start_element("{DAV:}principal-search-property");
            writer.start_element("{DAV:}prop");

            writer.write_empty(property_name);

            writer.end_element(); // prop

            writer.start_element("{DAV:}description");
            writer.write_attribute("xml:lang", "en");
            writer.text(description);
            writer.end_element();

            writer.end_element(); // principal-search-property
        }

        writer.end_element(); // principal-search-property-set

        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.status = Some(200);
        self.response.body = Some(writer.output());
        Ok(())
    }

    /// `principalPropertySearchReport($path, $report)`.
    async fn principal_property_search_report(&mut self, path: &str, report: &Element) -> DavResult<()> {
        let report = principal_property_search_request(report)?;
        let path = if report.apply_to_principal_collection_set { None } else { Some(path.to_owned()) };

        // `getHttpDepth('0')`: the default is the string '0', which isn't 0.
        let depth_ok = self.request.header("Depth").is_some_and(|d| {
            !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()) && d.parse::<i64>().is_ok_and(|n| n == 0)
        });
        if !depth_ok {
            return Err(DavError::bad_request("Depth must be 0"));
        }

        let result =
            self.principal_search(&report.search_properties, &report.properties, path.as_deref(), &report.test).await?;

        let minimal = self.http_prefer().return_.as_deref() == Some("minimal");
        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        self.response.set_header("Vary", "Brief,Prefer");
        self.response.body = Some(self.generate_multi_status(result, minimal));
        Ok(())
    }

    /// `principalSearch($searchProperties, $requestedProperties, $collectionUri, $test)`.
    async fn principal_search(
        &mut self,
        search_properties: &[(String, Option<String>)],
        requested_properties: &[String],
        collection_uri: Option<&str>,
        test: &str,
    ) -> DavResult<Vec<PropFindResult>> {
        let uris: Vec<String> = match collection_uri {
            Some(uri) => vec![uri.to_owned()],
            None => PRINCIPAL_COLLECTION_SET.iter().map(|u| (*u).to_owned()).collect(),
        };

        let mut lookup_results = Vec::new();
        for uri in uris {
            let principal_collection = self.tree.node_for_path(self.ctx, &uri).await?;
            if !principal_collection.is_principal_collection() {
                // Not a principal collection, we're simply going to ignore this.
                continue;
            }

            // AbstractPrincipalCollection::searchPrincipals()
            for row in backend::search_principals(self.ctx, "principals", search_properties, test)? {
                let (_, name) = split(&row);
                lookup_results.push(format!("{}/{name}", uri.trim_end_matches('/')));
            }
        }

        let mut matches = Vec::new();
        for lookup_result in lookup_results {
            if let Some(first) =
                self.get_properties_for_path(&lookup_result, requested_properties, 0).await?.into_iter().next()
            {
                matches.push(first);
            }
        }
        Ok(matches)
    }

    /// `aclPrincipalPropSetReport($path, $report)`.
    async fn acl_principal_prop_set_report(&mut self, path: &str, report: &Element) -> DavResult<()> {
        // AclPrincipalPropSetReport::xmlDeserialize()
        let properties = report
            .key_value()
            .into_iter()
            .find(|(k, _)| *k == "{DAV:}prop")
            .map(|(_, v)| v.enumeration())
            .unwrap_or_default();

        if self.http_depth(0) != 0 {
            return Err(DavError::bad_request("The {DAV:}acl-principal-prop-set REPORT only supports Depth 0"));
        }

        // Fetching ACL rules for the given path.
        let acl = self.get_properties(path, &["{DAV:}acl".to_owned()]).await?;
        let Some(PropValue::Acl(aces)) = acl.into_iter().find(|(n, _)| n == "{DAV:}acl").map(|(_, v)| v) else {
            return Err(DavError::forbidden("Could not fetch ACL rules for this path"));
        };

        let mut principals = Vec::new();
        for ace in aces {
            if ace.principal.starts_with('{') {
                // It's not a principal, it's one of the special rules such as {DAV:}authenticated
                continue;
            }
            principals.push(ace.principal);
        }

        let properties = self.get_properties_for_multiple_paths(&principals, &properties).await?;

        self.response.status = Some(207);
        self.response.set_header("Content-Type", "application/xml; charset=utf-8");
        let body = self.generate_multi_status(properties.into_iter().map(|(_, r)| r).collect(), false);
        self.response.body = Some(body);
        Ok(())
    }
}

/// A property of the expand-property report, with the ones to expand below it.
#[derive(Debug, Clone)]
struct ExpandProperty {
    name: String,
    children: Option<Vec<ExpandProperty>>,
}

/// `ExpandPropertyReport::traverse()`.
fn expand_property_request(element: &Element) -> DavResult<Vec<ExpandProperty>> {
    let mut result: Vec<ExpandProperty> = Vec::new();
    for elem in element.elements() {
        if elem.clark != "{DAV:}property" {
            continue;
        }
        let namespace = elem.attribute("namespace").unwrap_or("DAV:");
        let name = elem
            .attribute("name")
            .ok_or_else(|| DavError::internal("ErrorException", "Undefined array key \"name\""))?;
        let prop_name = format!("{{{namespace}}}{name}");
        let children = match elem.inner() {
            Inner::Elements(_) => Some(expand_property_request(elem)?),
            _ => None,
        };
        match result.iter_mut().find(|p| p.name == prop_name) {
            Some(existing) => existing.children = children,
            None => result.push(ExpandProperty { name: prop_name, children }),
        }
    }
    Ok(result)
}

/// `PrincipalPropertySearchReport`.
struct PrincipalPropertySearchRequest {
    properties: Vec<String>,
    /// The properties and the value to match (`None` when it isn't text).
    search_properties: Vec<(String, Option<String>)>,
    apply_to_principal_collection_set: bool,
    test: String,
}

/// `PrincipalPropertySearchReport::xmlDeserialize()`.
fn principal_property_search_request(root: &Element) -> DavResult<PrincipalPropertySearchRequest> {
    let mut report = PrincipalPropertySearchRequest {
        properties: Vec::new(),
        search_properties: Vec::new(),
        apply_to_principal_collection_set: false,
        test: if root.attribute("test") == Some("anyof") { "anyof".to_owned() } else { "allof".to_owned() },
    };
    let mut found_search_prop = false;

    for elem in root.elements() {
        match elem.clark.as_str() {
            "{DAV:}prop" => {
                report.properties = elem.key_value().into_iter().map(|(k, _)| k.to_owned()).collect();
            }
            "{DAV:}property-search" => {
                found_search_prop = true;
                // This property has two sub-elements:
                //   {DAV:}prop - The property to be searched on. This may also be more than one
                //   {DAV:}match - The value to match with
                let values = elem.key_value();
                let prop = values.iter().find(|(k, _)| *k == "{DAV:}prop").map(|(_, v)| *v);
                let matched = values.iter().find(|(k, _)| *k == "{DAV:}match").map(|(_, v)| v.inner());
                let (Some(prop), Some(matched)) = (prop, matched.filter(|m| *m != Inner::Null)) else {
                    return Err(DavError::bad_request(
                        "The {DAV:}property-search element must contain one {DAV:}match and one {DAV:}prop element",
                    ));
                };
                let value = match matched {
                    Inner::Text(t) => Some(t),
                    _ => None,
                };
                for (prop_name, _) in prop.key_value() {
                    match report.search_properties.iter_mut().find(|(k, _)| k == prop_name) {
                        Some(entry) => entry.1 = value.clone(),
                        None => report.search_properties.push((prop_name.to_owned(), value.clone())),
                    }
                }
            }
            "{DAV:}apply-to-principal-collection-set" => report.apply_to_principal_collection_set = true,
            _ => {}
        }
    }

    if !found_search_prop {
        return Err(DavError::bad_request(
            "The {DAV:}principal-property-search report must contain at least 1 {DAV:}property-search element",
        ));
    }

    Ok(report)
}

/// `DAVACL\Xml\Property\Acl::xmlDeserialize()`: the aces of the grants.
fn acl_request(root: &Element) -> DavResult<Vec<Ace>> {
    let mut privileges = Vec::new();
    for element in root.elements().filter(|e| e.clark == "{DAV:}ace") {
        let ace = element.key_value();
        let get = |name: &str| ace.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);

        let Some(principal) = get("{DAV:}principal").and_then(|p| p.elements().next()) else {
            return Err(DavError::bad_request("Each {DAV:}ace element must have one {DAV:}principal element"));
        };
        let principal = match principal.clark.as_str() {
            "{DAV:}unauthenticated" => "{DAV:}unauthenticated".to_owned(),
            "{DAV:}authenticated" => "{DAV:}authenticated".to_owned(),
            "{DAV:}all" => "{DAV:}all".to_owned(),
            "{DAV:}href" => format!("{}/", principal.text().unwrap_or_default().trim_end_matches('/')),
            name => return Err(DavError::bad_request(format!("Unknown or unsupported principal type: {name}"))),
        };

        let protected = get("{DAV:}protected").is_some();

        let Some(grant) = get("{DAV:}grant") else {
            return Err(DavError::not_implemented(
                "Every {DAV:}ace element must have a {DAV:}grant element. {DAV:}deny is not yet supported",
            ));
        };
        for elem in grant.elements().filter(|e| e.clark == "{DAV:}privilege") {
            for privilege in elem.enumeration() {
                privileges.push(Ace { principal: principal.clone(), protected, privilege });
            }
        }
    }
    Ok(privileges)
}
