//! The node tree of `DAVServiceProvider::nodes()`: a `SimpleCollection`
//! root with `PrincipalCollection`, `AddressBookRoot` (Monica's, with its
//! `AddressBookHome` and `AddressBook`) and `CalendarRoot`, and the sabre
//! node classes below them (`Principal`, `Card`, `CalendarHome`, `Calendar`,
//! `CalendarObject`), plus `Sabre\DAV\Tree`.

use std::collections::HashMap;

use super::backend::{self, AddressBookInfo, CARD, CalendarInfo, CardData, Ctx, ObjectData, Principal};
use super::http::split;
use super::props::{Ace, PropValue};
use super::server::{DavError, DavResult};

/// A node of the tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// The `SimpleCollection('root', …)` of the nodes.
    Root,
    /// `DAVACL\PrincipalCollection`.
    PrincipalCollection,
    /// `DAVACL\Principal`.
    Principal(Principal),
    /// Monica's `AddressBookRoot`.
    AddressBookRoot,
    /// Monica's `AddressBookHome`.
    AddressBookHome { principal_uri: String },
    /// Monica's `AddressBook`.
    AddressBook(AddressBookInfo),
    /// `CardDAV\Card`.
    Card { address_book: AddressBookInfo, card: CardData },
    /// `CalDAV\CalendarRoot`.
    CalendarRoot,
    /// `CalDAV\CalendarHome`.
    CalendarHome { principal: Principal },
    /// `CalDAV\Calendar`.
    Calendar(CalendarInfo),
    /// `CalDAV\CalendarObject`.
    CalendarObject { calendar: CalendarInfo, object: ObjectData },
}

/// `AbstractPrincipalCollection::$principalPrefix`.
const PRINCIPAL_PREFIX: &str = "principals";

impl Node {
    /// `getName()`.
    pub fn name(&self) -> String {
        match self {
            Node::Root => "root".to_owned(),
            Node::PrincipalCollection => split(PRINCIPAL_PREFIX).1,
            Node::Principal(p) => split(&p.uri).1,
            Node::AddressBookRoot => "addressbooks".to_owned(),
            Node::AddressBookHome { principal_uri } => split(principal_uri).1,
            Node::AddressBook(info) => info.uri.clone(),
            Node::Card { card, .. } => card.uri.clone(),
            Node::CalendarRoot => "calendars".to_owned(),
            Node::CalendarHome { principal } => split(&principal.uri).1,
            Node::Calendar(info) => info.uri.clone(),
            Node::CalendarObject { object, .. } => object.uri.clone(),
        }
    }

    /// `instanceof ICollection`.
    pub fn is_collection(&self) -> bool {
        !matches!(self, Node::Principal(_) | Node::Card { .. } | Node::CalendarObject { .. })
    }

    /// `instanceof IFile`.
    pub fn is_file(&self) -> bool {
        matches!(self, Node::Card { .. } | Node::CalendarObject { .. })
    }

    /// `instanceof IPrincipal`.
    pub fn is_principal(&self) -> bool {
        matches!(self, Node::Principal(_))
    }

    /// `instanceof IPrincipalCollection`.
    pub fn is_principal_collection(&self) -> bool {
        matches!(self, Node::PrincipalCollection | Node::AddressBookRoot | Node::CalendarRoot)
    }

    /// `instanceof IACL`.
    pub fn is_acl(&self) -> bool {
        !matches!(self, Node::Root | Node::AddressBookRoot | Node::CalendarRoot)
    }

    /// `instanceof IProperties`.
    pub fn is_properties(&self) -> bool {
        matches!(self, Node::Principal(_) | Node::AddressBook(_) | Node::Calendar(_))
    }

    /// `instanceof IExtendedCollection`.
    pub fn is_extended_collection(&self) -> bool {
        matches!(self, Node::PrincipalCollection | Node::AddressBookHome { .. } | Node::CalendarHome { .. })
    }

    /// `instanceof IMultiGet`.
    pub fn is_multi_get(&self) -> bool {
        matches!(self, Node::AddressBook(_) | Node::Calendar(_))
    }

    /// `instanceof ISyncCollection`.
    pub fn is_sync_collection(&self) -> bool {
        matches!(self, Node::AddressBook(_) | Node::Calendar(_))
    }

    /// `getResourceTypeForNode()`: `$resourceTypeMapping`, in the order the
    /// server and the plugins add to it.
    pub fn resource_type(&self) -> Vec<String> {
        let mut result = Vec::new();
        if self.is_collection() {
            result.push("{DAV:}collection".to_owned());
        }
        if matches!(self, Node::AddressBook(_)) {
            result.push("{urn:ietf:params:xml:ns:carddav}addressbook".to_owned());
        }
        if matches!(self, Node::Calendar(_)) {
            result.push("{urn:ietf:params:xml:ns:caldav}calendar".to_owned());
        }
        if self.is_principal() {
            result.push("{DAV:}principal".to_owned());
        }
        result
    }

    /// `getOwner()` (`ACLTrait` returns `null`).
    pub fn owner(&self) -> Option<String> {
        match self {
            Node::Principal(p) => Some(p.uri.clone()),
            Node::AddressBookHome { principal_uri } => Some(principal_uri.clone()),
            Node::AddressBook(info) => Some(info.principaluri.clone()),
            Node::Card { address_book, .. } => Some(address_book.principaluri.clone()),
            Node::CalendarHome { principal } => Some(principal.uri.clone()),
            Node::Calendar(info) => Some(info.principaluri.clone()),
            Node::CalendarObject { calendar, .. } => Some(calendar.principaluri.clone()),
            _ => None,
        }
    }

    /// `getACL()` of an `IACL` node.
    pub fn acl(&self) -> Vec<Ace> {
        let ace = |privilege: &str, principal: &str| Ace {
            privilege: privilege.to_owned(),
            principal: principal.to_owned(),
            protected: true,
        };
        match self {
            Node::PrincipalCollection => vec![ace("{DAV:}read", "{DAV:}authenticated")],
            Node::Card { address_book, .. } => vec![ace("{DAV:}all", &address_book.principaluri)],
            Node::CalendarHome { principal } => {
                let uri = &principal.uri;
                vec![
                    ace("{DAV:}read", uri),
                    ace("{DAV:}write", uri),
                    ace("{DAV:}read", &format!("{uri}/calendar-proxy-write")),
                    ace("{DAV:}write", &format!("{uri}/calendar-proxy-write")),
                    ace("{DAV:}read", &format!("{uri}/calendar-proxy-read")),
                ]
            }
            Node::Calendar(info) => {
                let owner = &info.principaluri;
                vec![
                    ace("{DAV:}read", owner),
                    ace("{DAV:}read", &format!("{owner}/calendar-proxy-write")),
                    ace("{DAV:}read", &format!("{owner}/calendar-proxy-read")),
                    ace("{urn:ietf:params:xml:ns:caldav}read-free-busy", "{DAV:}authenticated"),
                    ace("{DAV:}write", owner),
                    ace("{DAV:}write", &format!("{owner}/calendar-proxy-write")),
                ]
            }
            Node::CalendarObject { calendar, .. } => {
                // `$obj['acl'] = $this->getChildACL()`.
                let owner = &calendar.principaluri;
                vec![
                    ace("{DAV:}read", owner),
                    ace("{DAV:}read", &format!("{owner}/calendar-proxy-write")),
                    ace("{DAV:}read", &format!("{owner}/calendar-proxy-read")),
                    ace("{DAV:}write", owner),
                    ace("{DAV:}write", &format!("{owner}/calendar-proxy-write")),
                ]
            }
            // `ACLTrait::getACL()`.
            _ => vec![ace("{DAV:}all", "{DAV:}owner")],
        }
    }

    /// `getLastModified()`.
    pub async fn last_modified(&self, ctx: &Ctx<'_>) -> DavResult<Option<i64>> {
        match self {
            Node::AddressBook(info) => {
                Ok(backend::get_last_modified(ctx, &CARD, Some(&info.id)).await?.map(|t| t.timestamp()))
            }
            Node::Card { card, .. } => Ok(Some(card.lastmodified)),
            Node::CalendarObject { object, .. } => Ok(Some(object.lastmodified)),
            _ => Ok(None),
        }
    }

    /// `get()` of a file.
    pub fn get(&self) -> String {
        match self {
            Node::Card { card, .. } => card.carddata.clone(),
            Node::CalendarObject { object, .. } => object.calendardata.clone(),
            _ => String::new(),
        }
    }

    /// `getETag()` of a file.
    pub fn etag(&self) -> Option<String> {
        match self {
            Node::Card { card, .. } => Some(card.etag.clone()),
            Node::CalendarObject { object, .. } => Some(object.etag.clone()),
            _ => None,
        }
    }

    /// `getSize()` of a file.
    pub fn size(&self) -> usize {
        self.get().len()
    }

    /// `getContentType()` of a file.
    pub fn content_type(&self) -> Option<String> {
        match self {
            Node::Card { .. } => Some("text/vcard; charset=utf-8".to_owned()),
            Node::CalendarObject { .. } => Some("text/calendar; charset=utf-8".to_owned()),
            _ => None,
        }
    }

    /// `getSyncToken()` of an `ISyncCollection` (Monica's `AddressBook`
    /// refreshes the token).
    pub async fn sync_token(&self, ctx: &Ctx<'_>) -> DavResult<Option<String>> {
        match self {
            Node::AddressBook(info) => {
                Ok(Some(backend::refresh_sync_token(ctx, &CARD, Some(&info.id)).await?.id.to_string()))
            }
            Node::Calendar(info) => Ok(info
                .get("{DAV:}sync-token")
                .or_else(|| info.get("{http://sabredav.org/ns}sync-token"))
                .and_then(PropValue::as_text)
                .map(str::to_owned)),
            _ => Ok(None),
        }
    }

    /// `getProperties($requestedProperties)` of an `IProperties` node.
    pub fn properties(&self, requested: &[String]) -> Vec<(String, PropValue)> {
        match self {
            Node::Principal(p) => requested
                .iter()
                .filter(|name| name.starts_with('{'))
                .filter_map(|name| p.get(name).map(|v| (name.clone(), PropValue::Text(v.to_owned()))))
                .collect(),
            Node::AddressBook(info) => {
                requested.iter().filter_map(|name| info.get(name).map(|v| (name.clone(), v.clone()))).collect()
            }
            Node::Calendar(info) => info.properties.clone(),
            _ => Vec::new(),
        }
    }

    /// `getChildren()`.
    pub async fn children(&self, ctx: &Ctx<'_>) -> DavResult<Vec<Node>> {
        match self {
            Node::Root => Ok(vec![Node::PrincipalCollection, Node::AddressBookRoot, Node::CalendarRoot]),
            Node::PrincipalCollection | Node::AddressBookRoot | Node::CalendarRoot => {
                Ok(backend::get_principals_by_prefix(ctx, PRINCIPAL_PREFIX)
                    .into_iter()
                    .map(|p| self.child_for_principal(p))
                    .collect())
            }
            Node::AddressBookHome { principal_uri } => Ok(backend::get_address_books_for_user(ctx, principal_uri)
                .await?
                .into_iter()
                .map(Node::AddressBook)
                .collect()),
            Node::AddressBook(info) => Ok(backend::get_cards(ctx, &info.id)
                .await?
                .into_iter()
                .map(|card| Node::Card { address_book: info.clone(), card })
                .collect()),
            Node::CalendarHome { principal } => Ok(backend::get_calendars_for_user(ctx, &principal.uri)
                .await?
                .into_iter()
                .map(Node::Calendar)
                .collect()),
            Node::Calendar(info) => Ok(backend::get_calendar_objects(ctx, &info.id)
                .await?
                .into_iter()
                .map(|object| Node::CalendarObject { calendar: info.clone(), object })
                .collect()),
            Node::Principal(_) | Node::Card { .. } | Node::CalendarObject { .. } => Ok(Vec::new()),
        }
    }

    /// `getChildForPrincipal($principal)` of the principal collections.
    fn child_for_principal(&self, principal: Principal) -> Node {
        match self {
            Node::AddressBookRoot => Node::AddressBookHome { principal_uri: principal.uri },
            Node::CalendarRoot => Node::CalendarHome { principal },
            _ => Node::Principal(principal),
        }
    }

    /// `getChild($name)`.
    pub async fn child(&self, ctx: &Ctx<'_>, name: &str) -> DavResult<Node> {
        match self {
            Node::Root => self
                .children(ctx)
                .await?
                .into_iter()
                .find(|c| c.name() == name)
                .ok_or_else(|| DavError::not_found(format!("File not found: {name} in 'root'"))),
            Node::PrincipalCollection | Node::AddressBookRoot | Node::CalendarRoot => {
                match backend::get_principal_by_path(ctx, &format!("{PRINCIPAL_PREFIX}/{name}")) {
                    Some(principal) => Ok(self.child_for_principal(principal)),
                    None => Err(DavError::not_found(format!("Principal with name {name} not found"))),
                }
            }
            Node::AddressBookHome { .. } => self
                .children(ctx)
                .await?
                .into_iter()
                .find(|c| c.name() == name)
                .ok_or_else(|| DavError::not_found(format!("Addressbook with name '{name}' could not be found"))),
            Node::AddressBook(info) => match backend::get_card(ctx, &info.id, name).await? {
                Some(card) => Ok(Node::Card { address_book: info.clone(), card }),
                None => Err(DavError::not_found("Card not found")),
            },
            Node::CalendarHome { principal } => {
                for calendar in backend::get_calendars_for_user(ctx, &principal.uri).await? {
                    if calendar.uri == name {
                        return Ok(Node::Calendar(calendar));
                    }
                }
                Err(DavError::not_found(format!("Node with name '{name}' could not be found")))
            }
            Node::Calendar(info) => match backend::get_calendar_object(ctx, &info.id, name).await? {
                Some(object) => Ok(Node::CalendarObject { calendar: info.clone(), object }),
                None => Err(DavError::not_found("Calendar object not found")),
            },
            Node::Principal(_) | Node::Card { .. } | Node::CalendarObject { .. } => {
                Err(DavError::not_found(format!("Could not find node at path: {name}")))
            }
        }
    }

    /// `childExists($name)`.
    pub async fn child_exists(&self, ctx: &Ctx<'_>, name: &str) -> DavResult<bool> {
        match self.child(ctx, name).await {
            Ok(_) => Ok(true),
            Err(e) if e.class == NOT_FOUND => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// `getMultipleChildren($paths)` of an `IMultiGet` node: the backend's
    /// `getMultipleCards()` / `getMultipleCalendarObjects()` maps each name
    /// to `getCard()` / `getCalendarObject()`, and a missing one becomes a
    /// node without a uri.
    pub async fn multiple_children(&self, ctx: &Ctx<'_>, names: &[String]) -> DavResult<Vec<Node>> {
        let mut children = Vec::new();
        for name in names {
            match self {
                Node::AddressBook(info) => match backend::get_card(ctx, &info.id, name).await? {
                    Some(card) => children.push(Node::Card { address_book: info.clone(), card }),
                    None => return Err(DavError::internal("ErrorException", "Undefined array key \"uri\"")),
                },
                Node::Calendar(info) => match backend::get_calendar_object(ctx, &info.id, name).await? {
                    Some(object) => children.push(Node::CalendarObject { calendar: info.clone(), object }),
                    None => {
                        return Err(DavError::internal(
                            "InvalidArgumentException",
                            "The objectData argument must contain an 'uri' property",
                        ));
                    }
                },
                _ => {}
            }
        }
        Ok(children)
    }

    /// `delete()`.
    pub async fn delete(&self, ctx: &Ctx<'_>) -> DavResult<()> {
        match self {
            Node::Card { address_book, card } => {
                backend::delete_card(ctx, &address_book.id, &card.uri).await?;
                Ok(())
            }
            Node::CalendarObject { calendar, object } => {
                backend::delete_calendar_object(ctx, &calendar.id, &object.uri).await
            }
            // `CardDAVBackend::deleteAddressBook()` and `CalDAVBackend::deleteCalendar()` do nothing.
            Node::AddressBook(_) | Node::Calendar(_) => Ok(()),
            Node::AddressBookHome { .. } => Err(DavError::method_not_allowed("")),
            Node::CalendarHome { .. } => Err(DavError::forbidden("")),
            _ => Err(DavError::forbidden("Permission denied to delete node")),
        }
    }

    /// `put($data)` of a file: the backend's ETag.
    pub async fn put(&self, ctx: &Ctx<'_>, data: &str) -> DavResult<Option<String>> {
        match self {
            Node::Card { address_book, card } => backend::update_card(ctx, &address_book.id, &card.uri, data).await,
            Node::CalendarObject { calendar, object } => {
                backend::update_calendar_object(ctx, &calendar.id, &object.uri, data).await
            }
            _ => Err(DavError::forbidden("Permission denied to change data")),
        }
    }

    /// `createFile($name, $data)` of a collection: the backend's ETag.
    pub async fn create_file(&self, ctx: &Ctx<'_>, name: &str, data: &str) -> DavResult<Option<String>> {
        match self {
            Node::AddressBook(info) => backend::create_card(ctx, &info.id, name, data).await,
            Node::Calendar(info) => backend::create_calendar_object(ctx, &info.id, name, data).await,
            Node::AddressBookHome { .. } => {
                Err(DavError::method_not_allowed("Creating new files in this collection is not supported"))
            }
            Node::CalendarHome { .. } => {
                Err(DavError::method_not_allowed("Creating new files in this collection is not supported"))
            }
            _ => Err(DavError::forbidden(format!("Permission denied to create file (filename {name})"))),
        }
    }

    /// `createDirectory($name)`: no node creates one.
    pub fn create_directory(&self) -> DavResult<()> {
        Err(match self {
            Node::AddressBook(_) => DavError::method_not_allowed("Creating collections in addressbooks is not allowed"),
            Node::Calendar(_) => {
                DavError::method_not_allowed("Creating collections in calendar objects is not allowed")
            }
            Node::AddressBookHome { .. } | Node::CalendarHome { .. } => {
                DavError::method_not_allowed("Creating new collections in this collection is not supported")
            }
            _ => DavError::forbidden("Permission denied to create directory"),
        })
    }

    /// `setName($name)`: no node can be renamed.
    pub fn set_name(&self) -> DavResult<()> {
        Err(match self {
            Node::AddressBookHome { .. } => DavError::method_not_allowed(""),
            Node::AddressBook(_) => DavError::method_not_allowed("Renaming addressbooks is not yet supported"),
            Node::CalendarHome { .. } => DavError::forbidden(""),
            Node::Calendar(_) => DavError::method_not_allowed("Renaming calendars is not yet supported"),
            _ => DavError::forbidden("Permission denied to rename file"),
        })
    }
}

/// `get_class()` of a `NotFound`.
pub const NOT_FOUND: &str = "Sabre\\DAV\\Exception\\NotFound";

/// `Sabre\DAV\Tree`.
#[derive(Default)]
pub struct Tree {
    cache: HashMap<String, Node>,
}

impl Tree {
    /// `getNodeForPath($path)`.
    pub async fn node_for_path(&mut self, ctx: &Ctx<'_>, path: &str) -> DavResult<Node> {
        let path = path.trim_matches('/');
        if let Some(node) = self.cache.get(path) {
            return Ok(node.clone());
        }

        // Is it the root node?
        if path.is_empty() {
            return Ok(Node::Root);
        }

        let mut node = Node::Root;
        // look for any cached parent and collect the parts below the parent
        let mut parts: Vec<String> = Vec::new();
        let mut remaining = path.to_owned();
        loop {
            let (rest, base) = split(&remaining);
            parts.insert(0, base);
            remaining = rest;
            if let Some(cached) = self.cache.get(&remaining) {
                node = cached.clone();
                break;
            }
            if remaining.is_empty() {
                break;
            }
        }

        for part in parts {
            if !node.is_collection() {
                return Err(DavError::not_found(format!("Could not find node at path: {path}")));
            }
            if !part.is_empty() {
                node = node.child(ctx, &part).await?;
            }
        }

        self.cache.insert(path.to_owned(), node.clone());
        Ok(node)
    }

    /// `nodeExists($path)`.
    pub async fn node_exists(&mut self, ctx: &Ctx<'_>, path: &str) -> DavResult<bool> {
        if path.is_empty() {
            return Ok(true);
        }
        let (parent, base) = split(path);
        let parent_node = match self.node_for_path(ctx, &parent).await {
            Ok(node) => node,
            Err(e) if e.class == NOT_FOUND => return Ok(false),
            Err(e) => return Err(e),
        };
        if !parent_node.is_collection() {
            return Ok(false);
        }
        match parent_node.child_exists(ctx, &base).await {
            Ok(exists) => Ok(exists),
            Err(e) if e.class == NOT_FOUND => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// `getChildren($path)`: the children, cached under their paths.
    pub async fn children(&mut self, ctx: &Ctx<'_>, path: &str) -> DavResult<Vec<Node>> {
        let node = self.node_for_path(ctx, path).await?;
        let mut base_path = path.trim_matches('/').to_owned();
        if !base_path.is_empty() {
            base_path.push('/');
        }
        let children = node.children(ctx).await?;
        for child in &children {
            self.cache.insert(format!("{base_path}{}", child.name()), child.clone());
        }
        Ok(children)
    }

    /// `markDirty($path)`.
    pub fn mark_dirty(&mut self, path: &str) {
        let path = path.trim_matches('/');
        self.cache.retain(|node_path, _| {
            !(path.is_empty() || node_path == path || node_path.starts_with(&format!("{path}/")))
        });
    }

    /// `delete($path)`.
    pub async fn delete(&mut self, ctx: &Ctx<'_>, path: &str) -> DavResult<()> {
        let node = self.node_for_path(ctx, path).await?;
        node.delete(ctx).await?;
        let (parent, _) = split(path);
        self.mark_dirty(&parent);
        Ok(())
    }

    /// `copy($sourcePath, $destinationPath)` (no node is an `ICopyTarget`).
    pub async fn copy(&mut self, ctx: &Ctx<'_>, source_path: &str, destination_path: &str) -> DavResult<()> {
        let source_node = self.node_for_path(ctx, source_path).await?;

        // grab the dirname and basename components
        let (destination_dir, destination_name) = split(destination_path);

        let destination_parent = self.node_for_path(ctx, &destination_dir).await?;
        self.copy_node(ctx, &source_node, &destination_parent, &destination_name).await?;

        self.mark_dirty(&destination_dir);
        Ok(())
    }

    /// `move($sourcePath, $destinationPath)` (no node is an `IMoveTarget`).
    pub async fn move_node(&mut self, ctx: &Ctx<'_>, source_path: &str, destination_path: &str) -> DavResult<()> {
        let (source_dir, _) = split(source_path);
        let (destination_dir, _) = split(destination_path);

        if source_dir == destination_dir {
            // If this is a 'local' rename, it means we can just trigger a rename.
            let source_node = self.node_for_path(ctx, source_path).await?;
            source_node.set_name()?;
        } else {
            self.node_for_path(ctx, &destination_dir).await?;
            self.copy(ctx, source_path, destination_path).await?;
            self.node_for_path(ctx, source_path).await?.delete(ctx).await?;
        }
        self.mark_dirty(&source_dir);
        self.mark_dirty(&destination_dir);
        Ok(())
    }

    /// `copyNode($source, $destinationParent, $destinationName)`: the
    /// properties the PHP copies next are ignored by the backends.
    async fn copy_node(
        &mut self,
        ctx: &Ctx<'_>,
        source: &Node,
        destination_parent: &Node,
        destination_name: &str,
    ) -> DavResult<()> {
        let destination_name = if destination_name.is_empty() { source.name() } else { destination_name.to_owned() };

        if source.is_file() {
            destination_parent.create_file(ctx, &destination_name, &source.get()).await?;
            destination_parent.child(ctx, &destination_name).await?;
        } else if source.is_collection() {
            destination_parent.create_directory()?;
        }
        Ok(())
    }

    /// `getMultipleNodes($paths)`.
    pub async fn multiple_nodes(&mut self, ctx: &Ctx<'_>, paths: &[String]) -> DavResult<Vec<(String, Node)>> {
        // Finding common parents
        let mut parents: Vec<(String, Vec<String>)> = Vec::new();
        for path in paths {
            let (parent, node) = split(path);
            match parents.iter_mut().find(|(p, _)| *p == parent) {
                Some((_, children)) => children.push(node),
                None => parents.push((parent, vec![node])),
            }
        }

        let mut result: Vec<(String, Node)> = Vec::new();
        for (parent, children) in parents {
            let parent_node = self.node_for_path(ctx, &parent).await?;
            if parent_node.is_multi_get() {
                for child_node in parent_node.multiple_children(ctx, &children).await? {
                    let full_path = format!("{parent}/{}", child_node.name());
                    self.cache.insert(full_path.clone(), child_node.clone());
                    put(&mut result, full_path, child_node);
                }
            } else {
                for child in children {
                    let full_path = format!("{parent}/{child}");
                    let node = self.node_for_path(ctx, &full_path).await?;
                    put(&mut result, full_path, node);
                }
            }
        }
        Ok(result)
    }
}

/// `$result[$path] = $node`: replaced in place, else appended.
fn put(result: &mut Vec<(String, Node)>, path: String, node: Node) {
    match result.iter_mut().find(|(p, _)| *p == path) {
        Some(entry) => entry.1 = node,
        None => result.push((path, node)),
    }
}
