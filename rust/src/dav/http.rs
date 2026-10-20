//! The functions of `sabre/http` and `sabre/uri` the DAV server uses.

use chrono::{NaiveDateTime, TimeZone, Utc};

/// `Response::$statusCodes`.
pub fn status_text(code: u16) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version not supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        509 => "Bandwidth Limit Exceeded",
        510 => "Not extended",
        511 => "Network Authentication Required",
        _ => "",
    }
}

/// `HTTP\encodePath()`.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"_-.~()/:@".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02x}"));
        }
    }
    out
}

/// PHP's `rawurldecode()`.
pub fn rawurldecode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn hex(c: u8) -> Option<u8> {
    char::from(c).to_digit(16).and_then(|d| u8::try_from(d).ok())
}

/// PHP's `rawurlencode()`.
pub fn rawurlencode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// PHP's `urlencode()`.
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.".contains(&b) {
            out.push(char::from(b));
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `HTTP\decodePath()`: `rawurldecode`, read as ISO-8859-1 when it isn't UTF-8.
pub fn decode_path(path: &str) -> String {
    let bytes = rawurldecode(path);
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    }
}

/// `Uri\split()`: the parent path and the last segment.
pub fn split(path: &str) -> (String, String) {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    match trimmed.rfind('/') {
        Some(i) => {
            let name = &trimmed[i + 1..];
            if name.is_empty() {
                return (String::new(), String::new());
            }
            (trimmed[..i].to_owned(), name.to_owned())
        }
        None => (String::new(), trimmed.to_owned()),
    }
}

/// The parts of `Uri\parse()` the server reads.
#[derive(Debug, Clone, Default, PartialEq)]
struct UriParts {
    scheme: Option<String>,
    host: Option<String>,
    port: Option<String>,
    user: Option<String>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

/// `Uri\parse()` (non-ASCII characters percent-encoded first).
fn parse_uri(uri: &str) -> UriParts {
    let mut encoded = String::with_capacity(uri.len());
    for c in uri.chars() {
        if c.is_ascii() {
            encoded.push(c);
        } else {
            let mut buf = [0; 4];
            encoded.push_str(&rawurlencode(c.encode_utf8(&mut buf).as_bytes()));
        }
    }
    let mut rest = encoded.as_str();
    let mut parts = UriParts::default();
    if let Some(i) = rest.find(':') {
        let scheme = &rest[..i];
        if !scheme.is_empty()
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            parts.scheme = Some(scheme.to_owned());
            rest = &rest[i + 1..];
        }
    }
    if let Some(i) = rest.find('#') {
        parts.fragment = Some(rest[i + 1..].to_owned());
        rest = &rest[..i];
    }
    if let Some(i) = rest.find('?') {
        parts.query = Some(rest[i + 1..].to_owned());
        rest = &rest[..i];
    }
    if let Some(authority) = rest.strip_prefix("//") {
        let end = authority.find('/').unwrap_or(authority.len());
        let (auth, path) = authority.split_at(end);
        let host = match auth.rfind('@') {
            Some(i) => {
                parts.user = Some(auth[..i].split(':').next().unwrap_or_default().to_owned());
                &auth[i + 1..]
            }
            None => auth,
        };
        let (host, port) = match host.rfind(':') {
            Some(i) if host[i + 1..].bytes().all(|b| b.is_ascii_digit()) && !host[i + 1..].is_empty() => {
                (&host[..i], Some(host[i + 1..].to_owned()))
            }
            _ => (host, None),
        };
        if !host.is_empty() {
            parts.host = Some(host.to_owned());
        }
        parts.port = port;
        if !path.is_empty() {
            parts.path = Some(path.to_owned());
        }
    } else if !rest.is_empty() {
        parts.path = Some(rest.to_owned());
    }
    parts
}

/// `Uri\build()`.
fn build_uri(parts: &UriParts) -> String {
    let mut uri = String::new();
    let mut authority = String::new();
    if let Some(host) = &parts.host {
        authority = host.clone();
        if let Some(user) = &parts.user {
            authority = format!("{user}@{authority}");
        }
        if let Some(port) = &parts.port {
            authority = format!("{authority}:{port}");
        }
    }
    if let Some(scheme) = &parts.scheme {
        uri = format!("{scheme}:");
    }
    if !authority.is_empty() || parts.scheme.as_deref() == Some("file") {
        uri.push_str("//");
        uri.push_str(&authority);
    }
    if let Some(path) = &parts.path {
        uri.push_str(path);
    }
    if let Some(query) = &parts.query {
        uri.push('?');
        uri.push_str(query);
    }
    if let Some(fragment) = &parts.fragment {
        uri.push('#');
        uri.push_str(fragment);
    }
    uri
}

/// `Uri\resolve($basePath, $newPath)`.
pub fn resolve(base_path: &str, new_path: &str) -> String {
    let delta = parse_uri(new_path);
    if delta.scheme.is_some() {
        return build_uri(&delta);
    }
    let base = parse_uri(base_path);
    let pick = |d: &Option<String>, b: &Option<String>| d.clone().or_else(|| b.clone());
    let mut new = UriParts {
        scheme: pick(&delta.scheme, &base.scheme),
        host: pick(&delta.host, &base.host),
        port: pick(&delta.port, &base.port),
        ..Default::default()
    };
    let path = match &delta.path {
        Some(p) if !p.is_empty() => {
            if p.starts_with('/') {
                p.clone()
            } else {
                let mut path = base.path.clone().unwrap_or_default();
                if let Some(i) = path.rfind('/') {
                    path.truncate(i);
                }
                format!("{path}/{p}")
            }
        }
        _ => match base.path.clone() {
            Some(p) if !p.is_empty() => p,
            _ => "/".to_owned(),
        },
    };
    let mut new_parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "." => {}
            ".." => {
                new_parts.pop();
            }
            p => new_parts.push(p),
        }
    }
    let path = new_parts.join("/");
    new.path = Some(if path.starts_with('/') { path } else { format!("/{path}") });
    if delta.query.as_deref().is_some_and(|q| !q.is_empty()) {
        new.query = delta.query.clone();
    } else if base.query.is_some() && delta.host.is_none() && delta.path.is_none() {
        new.query = base.query.clone();
    }
    if delta.fragment.as_deref().is_some_and(|f| !f.is_empty()) {
        new.fragment = delta.fragment.clone();
    }
    build_uri(&new)
}

/// `Uri\normalize()`.
pub fn normalize(uri: &str) -> String {
    let mut parts = parse_uri(uri);
    if let Some(path) = &parts.path {
        let mut new_parts: Vec<String> = Vec::new();
        for part in path.trim_start_matches('/').split('/') {
            match part {
                "." => {}
                ".." => {
                    new_parts.pop();
                }
                p => new_parts.push(rawurlencode(&rawurldecode(p))),
            }
        }
        parts.path = Some(format!("/{}", new_parts.join("/")));
    }
    if let Some(scheme) = &parts.scheme {
        let scheme = scheme.to_lowercase();
        let default_port = match scheme.as_str() {
            "http" => Some("80"),
            "https" => Some("443"),
            _ => None,
        };
        if parts.port.is_some() && parts.port.as_deref() == default_port {
            parts.port = None;
        }
        if matches!(scheme.as_str(), "http" | "https") && parts.path.is_none() {
            parts.path = Some("/".to_owned());
        }
        parts.scheme = Some(scheme);
    }
    if let Some(host) = &parts.host {
        parts.host = Some(host.to_lowercase());
    }
    build_uri(&parts)
}

/// `HTTP\toDate()`: an RFC 7231 date.
pub fn to_date(timestamp: i64) -> String {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|d| d.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
        .unwrap_or_default()
}

/// `HTTP\parseDate()`: an RFC 1123, RFC 850 or asctime date, as a timestamp.
pub fn parse_date(date: &str) -> Option<i64> {
    let date = date.trim_matches(' ');
    let formats = ["%a, %d %b %Y %H:%M:%S GMT", "%A, %d-%b-%y %H:%M:%S GMT", "%a %b %e %H:%M:%S %Y"];
    for format in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(date, format)
            && strict_date_format(date)
        {
            return Some(dt.and_utc().timestamp());
        }
    }
    None
}

/// The format check of `parseDate()` (4-digit years can't start with 0).
fn strict_date_format(date: &str) -> bool {
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let wkdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let weekdays = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    let time_ok = |t: &str| {
        let b = t.as_bytes();
        b.len() == 8
            && b[2] == b':'
            && b[5] == b':'
            && b[0].is_ascii_digit()
            && b[1].is_ascii_digit()
            && (b[0] < b'2' || (b[0] == b'2' && b[1] <= b'3'))
            && (b'0'..=b'5').contains(&b[3])
            && b[4].is_ascii_digit()
            && (b'0'..=b'5').contains(&b[6])
            && b[7].is_ascii_digit()
    };
    let day2_ok = |d: &str| {
        let b = d.as_bytes();
        b.len() == 2 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && d != "00" && d <= "31"
    };
    let year4_ok = |y: &str| y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()) && !y.starts_with('0');
    let parts: Vec<&str> = date.split(' ').collect();
    // RFC 1123: "Sun, 06 Nov 1994 08:49:37 GMT"
    if parts.len() == 6 && parts[0].len() == 4 && parts[0].ends_with(',') && wkdays.contains(&&parts[0][..3]) {
        return day2_ok(parts[1])
            && months.contains(&parts[2])
            && year4_ok(parts[3])
            && time_ok(parts[4])
            && parts[5] == "GMT";
    }
    // RFC 850: "Sunday, 06-Nov-94 08:49:37 GMT"
    if parts.len() == 4 && parts[0].ends_with(',') && weekdays.contains(&parts[0].trim_end_matches(',')) {
        let d: Vec<&str> = parts[1].split('-').collect();
        return d.len() == 3
            && day2_ok(d[0])
            && months.contains(&d[1])
            && d[2].len() == 2
            && d[2].bytes().all(|b| b.is_ascii_digit())
            && time_ok(parts[2])
            && parts[3] == "GMT";
    }
    // asctime: "Sun Nov  6 08:49:37 1994"
    let compact: Vec<&str> = date.split(' ').filter(|p| !p.is_empty()).collect();
    compact.len() == 5
        && wkdays.contains(&compact[0])
        && months.contains(&compact[1])
        && time_ok(compact[3])
        && year4_ok(compact[4])
}

/// A proposal or option of `negotiateContentType()` (`parseMimeType()`).
struct MimeType {
    r#type: String,
    sub_type: String,
    quality: f64,
    parameters: Vec<(String, String)>,
}

/// `HTTP\parseMimeType()`; `None` for a value without a `/`.
fn parse_mime_type(s: &str) -> Option<MimeType> {
    let mut parts = s.split(';');
    let mut mime = parts.next().unwrap_or_default().trim().to_owned();
    if mime == "*" {
        mime = "*/*".to_owned();
    }
    let segments: Vec<&str> = mime.split('/').collect();
    if segments.len() != 2 {
        return None;
    }
    let mut parameters = Vec::new();
    let mut quality = 1.0;
    for part in parts {
        let part = part.trim();
        let (name, value) = match part.find('=') {
            Some(i) if i > 0 => (&part[..i], Some(&part[i + 1..])),
            _ => (part, None),
        };
        if name != "q" {
            parameters.push((name.to_owned(), part.to_owned()));
        } else {
            quality = value.and_then(php_floatval).unwrap_or(0.0);
            break;
        }
    }
    Some(MimeType { r#type: segments[0].to_owned(), sub_type: segments[1].to_owned(), quality, parameters })
}

/// PHP's `(float)` of a numeric prefix.
fn php_floatval(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .take_while(|(i, c)| c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    s[..end].parse().ok().or(Some(0.0))
}

/// `HTTP\negotiateContentType()`.
pub fn negotiate_content_type<'a>(accept: Option<&str>, options: &[&'a str]) -> Option<&'a str> {
    let Some(accept) = accept.filter(|a| !a.is_empty()) else {
        return options.first().copied();
    };
    let proposals: Vec<Option<MimeType>> = accept.split(',').map(parse_mime_type).collect();
    let parsed: Vec<Option<MimeType>> = options.iter().map(|o| parse_mime_type(o)).collect();
    let mut last_quality = 0.0;
    let mut last_specificity = 0;
    let mut last_option_index = 0;
    let mut last_choice = None;
    for proposal in proposals.iter().flatten() {
        if proposal.quality < last_quality {
            continue;
        }
        for (option_index, option) in parsed.iter().enumerate() {
            let Some(option) = option else { continue };
            if proposal.r#type != "*" && proposal.r#type != option.r#type {
                continue;
            }
            if proposal.sub_type != "*" && proposal.sub_type != option.sub_type {
                continue;
            }
            let params_match = option.parameters.iter().all(|(name, value)| {
                proposal.parameters.iter().find(|(n, _)| n == name).is_some_and(|(_, v)| v == value)
            });
            if !params_match {
                continue;
            }
            let specificity = if proposal.r#type != "*" { 20 } else { 0 }
                + if proposal.sub_type != "*" { 10 } else { 0 }
                + option.parameters.len();
            if proposal.quality > last_quality
                || (proposal.quality == last_quality && specificity > last_specificity)
                || (proposal.quality == last_quality
                    && specificity == last_specificity
                    && option_index < last_option_index)
            {
                last_quality = proposal.quality;
                last_specificity = specificity;
                last_option_index = option_index;
                last_choice = Some(options[option_index]);
            }
        }
    }
    last_choice
}

/// `Server::getHTTPPrefer()`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prefer {
    pub return_: Option<String>,
    pub handling: Option<String>,
}

/// `getHTTPPrefer()`: the `Prefer` header (`parsePrefer()`), or `Brief: t`.
pub fn get_http_prefer(prefer: Option<&str>, brief: Option<&str>) -> Prefer {
    let mut result = Prefer::default();
    if let Some(prefer) = prefer.filter(|p| !p.is_empty()) {
        for value in prefer.split(',').map(str::trim) {
            let name_end = value
                .find(|c: char| !(c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`~".contains(c)))
                .unwrap_or(value.len());
            if name_end == 0 {
                continue;
            }
            let name = &value[..name_end];
            let rest = value[name_end..].trim_start();
            let value = rest.strip_prefix('=').map(|v| {
                let v = v.trim_start();
                let end = v.find(|c: char| c == ';' || c.is_whitespace()).unwrap_or(v.len());
                v[..end].trim_matches('"').to_owned()
            });
            match name {
                "return-representation" => result.return_ = Some("representation".into()),
                "return-minimal" => result.return_ = Some("minimal".into()),
                "strict" => result.handling = Some("strict".into()),
                "lenient" => result.handling = Some("lenient".into()),
                _ => match name.to_lowercase().as_str() {
                    "return" => result.return_ = value.filter(|v| !v.is_empty()).or(Some("1".into())),
                    "handling" => result.handling = value.filter(|v| !v.is_empty()).or(Some("1".into())),
                    _ => {}
                },
            }
        }
    } else if brief == Some("t") {
        result.return_ = Some("minimal".into());
    }
    result
}
