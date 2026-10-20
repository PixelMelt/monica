//! The part of `sabre/xml` the DAV server uses: the `Writer` (PHP's
//! `XMLWriter`, i.e. libxml2's `xmlTextWriter`, with sabre's namespace
//! handling), the `Reader` (here a tree of the request body that the
//! deserializers walk), and the `DOMDocument` of `Server::start()`'s error
//! responses.

use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

pub const NS_DAV: &str = "DAV:";
pub const NS_SABREDAV: &str = "http://sabredav.org/ns";
pub const NS_CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";
pub const NS_CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
pub const NS_CALENDARSERVER: &str = "http://calendarserver.org/ns/";

/// `$server->xml->namespaceMap` once the plugins are loaded: `Xml\Service`,
/// then `CardDAV\Plugin`, then `CalDAV\Plugin`.
const NAMESPACE_MAP: [(&str, &str); 5] =
    [(NS_DAV, "d"), (NS_SABREDAV, "s"), (NS_CARDDAV, "card"), (NS_CALDAV, "cal"), (NS_CALENDARSERVER, "cs")];

/// `Service::parseClarkNotation()`: `{namespace}name`.
pub fn parse_clark(s: &str) -> Option<(&str, &str)> {
    let rest = s.strip_prefix('{')?;
    let end = rest.find('}')?;
    Some((&rest[..end], &rest[end + 1..]))
}

/// `xmlEncodeSpecialChars()`: the escaping of text content.
fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

/// `xmlBufAttrSerializeTxtContent()` without a document encoding: the
/// escaping of attribute values (non-ASCII as character references).
fn escape_attribute(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if !c.is_ascii() => out.push_str(&format!("&#x{:X};", u32::from(c))),
            c => out.push(c),
        }
    }
}

/// An open element of the writer (`xmlTextWriterStackEntry`).
struct Open {
    name: String,
    /// `XML_TEXTWRITER_TEXT`: the start tag is closed.
    text: bool,
    /// A namespace declaration of `startElementNS`, written when the start tag closes.
    ns_decl: Option<(String, String)>,
}

/// `Sabre\Xml\Writer`.
pub struct Writer {
    out: String,
    indent: bool,
    do_indent: bool,
    stack: Vec<Open>,
    adhoc_namespaces: Vec<String>,
    namespaces_written: bool,
    /// `$writer->contextUri`.
    pub context_uri: String,
}

impl Writer {
    /// `$service->getWriter()` then `openMemory()`.
    pub fn new(context_uri: &str) -> Writer {
        Writer {
            out: String::new(),
            indent: false,
            do_indent: false,
            stack: Vec::new(),
            adhoc_namespaces: Vec::new(),
            namespaces_written: false,
            context_uri: context_uri.to_owned(),
        }
    }

    /// `setIndent(true)` (one space per level).
    pub fn set_indent(&mut self) {
        self.indent = true;
        self.do_indent = true;
    }

    /// `startDocument()`.
    pub fn start_document(&mut self) {
        self.out.push_str("<?xml version=\"1.0\"?>\n");
    }

    /// `endDocument()`: closes what is open.
    pub fn end_document(&mut self) {
        while !self.stack.is_empty() {
            self.end_element();
        }
        if !self.indent {
            self.out.push('\n');
        }
    }

    /// `outputMemory()`.
    pub fn output(self) -> String {
        self.out
    }

    fn write_indent(&mut self) {
        for _ in 1..self.stack.len() {
            self.out.push(' ');
        }
    }

    fn output_ns_decl(&mut self) {
        if let Some((prefix, uri)) = self.stack.last_mut().and_then(|o| o.ns_decl.take()) {
            self.out.push_str(&format!(" xmlns:{prefix}=\""));
            escape_attribute(&uri, &mut self.out);
            self.out.push('"');
        }
    }

    /// Closes the parent's start tag before content (`xmlTextWriterHandleStateDependencies`).
    fn close_start_tag(&mut self, newline: bool) {
        if self.stack.last().is_some_and(|o| !o.text) {
            self.output_ns_decl();
            self.out.push('>');
            if newline && self.indent {
                self.out.push('\n');
            }
            if let Some(o) = self.stack.last_mut() {
                o.text = true;
            }
        }
    }

    fn start_raw(&mut self, qname: String, ns_decl: Option<(String, String)>) {
        self.close_start_tag(true);
        self.stack.push(Open { name: qname.clone(), text: false, ns_decl });
        if self.indent {
            self.write_indent();
        }
        self.out.push('<');
        self.out.push_str(&qname);
    }

    /// `startElement($name)`, with `$name` in clark notation.
    pub fn start_element(&mut self, name: &str) {
        match parse_clark(name) {
            Some((namespace, local)) => {
                if let Some((_, prefix)) = NAMESPACE_MAP.iter().find(|(ns, _)| *ns == namespace) {
                    self.start_raw(format!("{prefix}:{local}"), None);
                } else if namespace.is_empty() {
                    self.start_raw(local.to_owned(), None);
                    self.write_attribute("xmlns", "");
                } else {
                    let index = match self.adhoc_namespaces.iter().position(|ns| ns == namespace) {
                        Some(i) => i,
                        None => {
                            self.adhoc_namespaces.push(namespace.to_owned());
                            self.adhoc_namespaces.len() - 1
                        }
                    };
                    let prefix = format!("x{}", index + 1);
                    self.start_raw(format!("{prefix}:{local}"), Some((prefix, namespace.to_owned())));
                }
            }
            None => self.start_raw(name.to_owned(), None),
        }
        if !self.namespaces_written {
            for (namespace, prefix) in NAMESPACE_MAP {
                self.write_attribute(&format!("xmlns:{prefix}"), namespace);
            }
            self.namespaces_written = true;
        }
    }

    /// `writeAttribute($name, $value)` (clark names of the map get their prefix).
    pub fn write_attribute(&mut self, name: &str, value: &str) {
        let name = match parse_clark(name) {
            Some((namespace, local)) => match NAMESPACE_MAP.iter().find(|(ns, _)| *ns == namespace) {
                Some((_, prefix)) => format!("{prefix}:{local}"),
                None => local.to_owned(),
            },
            None => name.to_owned(),
        };
        self.out.push(' ');
        self.out.push_str(&name);
        self.out.push_str("=\"");
        escape_attribute(value, &mut self.out);
        self.out.push('"');
    }

    /// `text($value)`.
    pub fn text(&mut self, value: &str) {
        self.close_start_tag(false);
        if self.indent {
            self.do_indent = false;
        }
        escape_text(value, &mut self.out);
    }

    /// `endElement()`.
    pub fn end_element(&mut self) {
        let Some((text, name)) = self.stack.last().map(|o| (o.text, o.name.clone())) else { return };
        if text {
            if self.indent && self.do_indent {
                self.write_indent();
            }
            self.do_indent = true;
            self.out.push_str("</");
            self.out.push_str(&name);
            self.out.push('>');
        } else {
            self.output_ns_decl();
            if self.indent {
                self.do_indent = true;
            }
            self.out.push_str("/>");
        }
        if self.indent {
            self.out.push('\n');
        }
        self.stack.pop();
    }

    /// `writeElement($name)` of an empty element.
    pub fn write_empty(&mut self, name: &str) {
        self.start_element(name);
        self.end_element();
    }

    /// `writeElement($name, $text)`.
    pub fn write_text_element(&mut self, name: &str, value: &str) {
        self.start_element(name);
        self.text(value);
        self.end_element();
    }
}

/// A node of a parsed request body.
#[derive(Debug, Clone, PartialEq)]
pub enum XNode {
    Element(Element),
    Text(String),
}

/// An element of a parsed request body (what `XMLReader` sees of it).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Element {
    /// `{namespace}name`.
    pub clark: String,
    /// `parseAttributes()`: clark keys for namespaced attributes, local names otherwise.
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XNode>,
    /// `isEmptyElement`: written as `<name/>`.
    pub self_closing: bool,
}

/// `parseInnerTree()`: child elements, else the text, else nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Inner<'a> {
    Null,
    Text(String),
    Elements(Vec<&'a Element>),
}

impl Element {
    /// `$reader->getAttribute($name)` / `$elem['attributes'][$name]`.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// The child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            XNode::Element(e) => Some(e),
            XNode::Text(_) => None,
        })
    }

    /// `parseInnerTree()`.
    pub fn inner(&self) -> Inner<'_> {
        let elements: Vec<&Element> = self.elements().collect();
        if !elements.is_empty() {
            return Inner::Elements(elements);
        }
        let mut text: Option<String> = None;
        for child in &self.children {
            if let XNode::Text(t) = child {
                text.get_or_insert_with(String::new).push_str(t);
            }
        }
        match text {
            Some(t) => Inner::Text(t),
            None => Inner::Null,
        }
    }

    /// The text of `parseInnerTree()` (`null` for none or for child elements).
    pub fn text(&self) -> Option<String> {
        match self.inner() {
            Inner::Text(t) => Some(t),
            _ => None,
        }
    }

    /// `Deserializer\keyValue()`: the child elements by clark name (later
    /// ones win, in the order each name first appeared).
    pub fn key_value(&self) -> Vec<(&str, &Element)> {
        let mut values: Vec<(&str, &Element)> = Vec::new();
        for child in self.elements() {
            match values.iter_mut().find(|(k, _)| *k == child.clark) {
                Some(entry) => entry.1 = child,
                None => values.push((&child.clark, child)),
            }
        }
        values
    }

    /// `Deserializer\enum()`: the clark names of the child elements.
    pub fn enumeration(&self) -> Vec<String> {
        self.elements().map(|e| e.clark.clone()).collect()
    }
}

/// `ParseException` (and `LibXMLException`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError(pub String);

/// An element being read, with its text runs (`CDATA` runs are never blank).
struct Building {
    element: Element,
    text: Option<(String, bool)>,
}

impl Building {
    /// Ends the current text node: `XMLReader` reports a blank one as
    /// whitespace, which `parseInnerTree()` skips.
    fn flush_text(&mut self) {
        if let Some((text, cdata)) = self.text.take()
            && (cdata || !text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')))
        {
            self.element.children.push(XNode::Text(text));
        }
    }

    fn push_text(&mut self, text: &str, cdata: bool) {
        let entry = self.text.get_or_insert_with(|| (String::new(), false));
        entry.0.push_str(text);
        entry.1 |= cdata;
    }
}

fn err(e: impl std::fmt::Display) -> ParseError {
    ParseError(e.to_string())
}

/// Closes the innermost element; the root when it was the last one.
fn close(stack: &mut Vec<Building>) -> Option<Element> {
    let mut done = stack.pop()?;
    done.flush_text();
    match stack.last_mut() {
        Some(parent) => {
            parent.element.children.push(XNode::Element(done.element));
            None
        }
        None => Some(done.element),
    }
}

/// `Service::parse()`: the root element of `input`.
pub fn parse(input: &str) -> Result<Element, ParseError> {
    if input.is_empty() {
        return Err(ParseError("The input element to parse is empty. Do not attempt to parse".into()));
    }
    let mut reader = NsReader::from_str(input);
    let mut stack: Vec<Building> = Vec::new();
    loop {
        let (ns, event) = reader.read_resolved_event().map_err(err)?;
        let namespace = match ns {
            ResolveResult::Bound(ns) => String::from_utf8_lossy(ns.as_ref()).into_owned(),
            ResolveResult::Unbound => String::new(),
            ResolveResult::Unknown(p) => {
                return Err(ParseError(format!("Namespace prefix {} is not defined", String::from_utf8_lossy(&p))));
            }
        };
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                let mut element =
                    Element { clark: format!("{{{namespace}}}{local}"), self_closing: empty, ..Default::default() };
                for attr in e.attributes() {
                    let attr = attr.map_err(err)?;
                    if attr.key.as_namespace_binding().is_some() {
                        continue;
                    }
                    let (resolved, local) = reader.resolve_attribute(attr.key);
                    let local = String::from_utf8_lossy(local.as_ref()).into_owned();
                    let name = match resolved {
                        ResolveResult::Bound(ns) => format!("{{{}}}{local}", String::from_utf8_lossy(ns.as_ref())),
                        _ => local,
                    };
                    element.attributes.push((name, attr.unescape_value().map_err(err)?.into_owned()));
                }
                if let Some(parent) = stack.last_mut() {
                    parent.flush_text();
                }
                stack.push(Building { element, text: None });
                if empty && let Some(root) = close(&mut stack) {
                    return Ok(root);
                }
            }
            Event::End(_) => {
                if let Some(root) = close(&mut stack) {
                    return Ok(root);
                }
            }
            Event::Text(t) => {
                let text = t.xml10_content().map_err(err)?;
                if let Some(b) = stack.last_mut() {
                    b.push_text(&text, false);
                }
            }
            Event::CData(t) => {
                let text = String::from_utf8_lossy(&t.into_inner()).into_owned();
                if let Some(b) = stack.last_mut() {
                    b.push_text(&text, true);
                }
            }
            Event::GeneralRef(r) => {
                let text = match r.resolve_char_ref().map_err(err)? {
                    Some(c) => c.to_string(),
                    None => match r.decode().map_err(err)?.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        other => return Err(ParseError(format!("Entity '{other}' not defined"))),
                    },
                };
                if let Some(b) = stack.last_mut() {
                    b.push_text(&text, false);
                }
            }
            Event::Eof => return Err(ParseError("Premature end of data".into())),
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
}
