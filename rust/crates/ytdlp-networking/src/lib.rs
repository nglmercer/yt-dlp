//! Request-side networking primitives for the experimental Rust migration.
//!
//! This crate defines the data and error contracts shared by native request
//! handlers, including the first direct HTTP/1.1 transport implementation.

use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

fn remove_dot_segments(path: &str) -> String {
    let leading_slash = path.starts_with('/');
    let trailing_slash = path.ends_with('/');
    let mut output = Vec::new();

    for segment in path.split('/') {
        match segment {
            "." | "" if leading_slash && output.is_empty() => {}
            "." | "" => output.push(segment),
            ".." => {
                if !output.is_empty() {
                    output.pop();
                }
            }
            segment => output.push(segment),
        }
    }

    let mut normalized = output.join("/");
    if leading_slash && !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    if trailing_slash && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

/// Normalize a URL using the RFC 3986 transformations used by yt-dlp.
pub fn normalize_url(input: &str) -> String {
    let input = input
        .strip_prefix("//")
        .map_or_else(|| input.to_owned(), |rest| format!("http://{rest}"));
    let Ok(mut parsed) = Url::parse(&input) else {
        return input;
    };

    let path = parsed.path().to_owned();
    let normalized_path = remove_dot_segments(&path);
    if normalized_path != path {
        parsed.set_path(&normalized_path);
    }

    remove_empty_authority_path(parsed.to_string(), &input, &path)
}

fn has_explicit_path(url: &str) -> bool {
    let Some(authority_start) = url.find("//") else {
        return true;
    };
    let after_authority = &url[authority_start + 2..];
    let authority_end = after_authority
        .find(|character| matches!(character, '/' | '?' | '#'))
        .unwrap_or(after_authority.len());
    after_authority[authority_end..].starts_with('/')
}

fn remove_empty_authority_path(mut url: String, original: &str, path: &str) -> String {
    // `url::Url` materializes `/` for an empty authority path, while Python's
    // urlunparse preserves `https://example.com` without that slash.
    if path != "/" || has_explicit_path(original) {
        return url;
    }

    let authority_start = url.find("://").map_or(0, |index| index + 3);
    if let Some(relative_end) =
        url[authority_start..].find(|character| matches!(character, '/' | '?' | '#'))
    {
        let path_index = authority_start + relative_end;
        if url.as_bytes().get(path_index) == Some(&b'/') {
            url.remove(path_index);
        }
    }
    url
}

/// Update existing query keys and append new keys in insertion order.
pub fn update_url_query(url: &str, query: &[(String, String)]) -> String {
    if query.is_empty() {
        return url.to_owned();
    }
    let normalized_url = normalize_url(url);
    let Ok(mut parsed) = Url::parse(&normalized_url) else {
        return normalized_url;
    };

    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    for (key, value) in query {
        if let Some(index) = pairs
            .iter()
            .position(|(existing_key, _)| existing_key == key)
        {
            pairs.retain(|(existing_key, _)| existing_key != key);
            pairs.insert(index, (key.clone(), value.clone()));
        } else {
            pairs.push((key.clone(), value.clone()));
        }
    }

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(&key, &value);
    }
    let encoded_query = serializer.finish();
    parsed.set_query((!encoded_query.is_empty()).then_some(encoded_query.as_str()));
    remove_empty_authority_path(parsed.to_string(), &normalized_url, parsed.path())
}

/// Canonicalize a header name the same way Python's `str.title()` does for
/// the ASCII header names used by yt-dlp.
fn canonical_header_name(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Case-insensitive headers that retain the spelling supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers {
    values: IndexMap<String, String>,
    original_names: IndexMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {
        let name = name.as_ref();
        let canonical = canonical_header_name(name);
        self.original_names
            .insert(canonical.clone(), name.to_owned());
        self.values
            .insert(canonical, value.as_ref().trim().to_owned());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values
            .get(&canonical_header_name(name))
            .map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(&canonical_header_name(name))
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        let canonical = canonical_header_name(name);
        self.original_names.shift_remove(&canonical);
        self.values.shift_remove(&canonical)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Return the case-sensitive view used when adapting to another client.
    pub fn sensitive(&self) -> IndexMap<String, String> {
        self.values
            .iter()
            .map(|(canonical, value)| {
                (
                    self.original_names
                        .get(canonical)
                        .cloned()
                        .unwrap_or_else(|| canonical.clone()),
                    value.clone(),
                )
            })
            .collect()
    }
}

/// Case-insensitive response headers that retain repeated field values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseHeaders {
    values: IndexMap<String, Vec<String>>,
    original_names: IndexMap<String, String>,
}

impl ResponseHeaders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {
        let name = name.as_ref();
        let canonical = canonical_header_name(name);
        self.original_names
            .entry(canonical.clone())
            .or_insert_with(|| name.to_owned());
        self.values
            .entry(canonical)
            .or_default()
            .push(value.as_ref().trim().to_owned());
    }

    pub fn get_all(&self, name: &str) -> Vec<&str> {
        self.values
            .get(&canonical_header_name(name))
            .map(|values| values.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.get_all(name).into_iter().next()
    }

    /// Match yt-dlp's `Response.get_header` behavior.
    pub fn get_header(&self, name: &str) -> Option<String> {
        let values = self.get_all(name);
        if values.is_empty() {
            return None;
        }
        if canonical_header_name(name) == "Set-Cookie" {
            Some(values[0].to_owned())
        } else {
            Some(values.join(", "))
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(&canonical_header_name(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().flat_map(|(canonical, values)| {
            let name = self
                .original_names
                .get(canonical)
                .map(String::as_str)
                .unwrap_or(canonical.as_str());
            values.iter().map(move |value| (name, value.as_str()))
        })
    }
}

/// RFC 6265 cookie state shared by requests and redirect hops.
///
/// The underlying store owns domain, path, secure, expiry, and replacement
/// semantics. A shared handle is used because a redirecting request must
/// update the same jar that the caller supplied.
#[derive(Debug, Default, Clone)]
pub struct CookieJar {
    store: cookie_store::CookieStore,
}

pub type SharedCookieJar = Arc<Mutex<CookieJar>>;

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared(self) -> SharedCookieJar {
        Arc::new(Mutex::new(self))
    }

    pub fn len(&self) -> usize {
        self.store.iter_unexpired().count()
    }

    pub fn cookie_header(&self, url: &str) -> Result<Option<String>, RequestError> {
        let url = Url::parse(url)
            .map_err(|error| RequestError::invalid(format!("invalid cookie URL: {error}")))?;
        let values = self
            .store
            .get_request_values(&url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>();
        Ok((!values.is_empty()).then(|| values.join("; ")))
    }

    pub fn store_response(
        &mut self,
        url: &str,
        headers: &ResponseHeaders,
    ) -> Result<(), RequestError> {
        let url = Url::parse(url)
            .map_err(|error| RequestError::invalid(format!("invalid cookie URL: {error}")))?;
        for value in headers.get_all("Set-Cookie") {
            // Invalid Set-Cookie fields are ignored by the Python cookie jar.
            let _ = self.store.parse(value, &url);
        }
        Ok(())
    }
}

fn no_proxy_matches(url: &Url, no_proxy: &str) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    let port = url.port_or_known_default();

    no_proxy.split(',').any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        if entry.is_empty() {
            return false;
        }
        if entry == "*" {
            return true;
        }

        let (entry_host, entry_port) = if let Some(rest) = entry.strip_prefix('[') {
            rest.find(']').map_or((entry.as_str(), None), |end| {
                let host_end = end + 1;
                let port = rest[host_end..]
                    .strip_prefix(':')
                    .and_then(|value| value.parse::<u16>().ok());
                (&entry[..host_end + 1], port)
            })
        } else if entry.matches(':').count() == 1 {
            let (host, port) = entry.rsplit_once(':').unwrap();
            (host, port.parse::<u16>().ok())
        } else {
            (entry.as_str(), None)
        };
        let entry_host = entry_host.trim_matches(['[', ']']);
        let host_matches = host == entry_host
            || host
                .strip_suffix(entry_host)
                .is_some_and(|prefix| prefix.ends_with('.'))
            || (entry_host.starts_with('.') && host.ends_with(entry_host));
        host_matches && entry_port.is_none_or(|entry_port| Some(entry_port) == port)
    })
}

/// Select the proxy for a URL using yt-dlp's per-scheme, `all`, and `no`
/// mapping semantics. Environment proxies are intentionally outside this
/// function so callers can make the result deterministic.
pub fn select_proxy(
    url: &str,
    proxies: &IndexMap<String, Option<String>>,
) -> Result<Option<String>, RequestError> {
    let url = Url::parse(url)
        .map_err(|error| RequestError::invalid(format!("invalid proxy URL: {error}")))?;
    if proxies
        .get("no")
        .and_then(Option::as_deref)
        .is_some_and(|no_proxy| no_proxy_matches(&url, no_proxy))
    {
        return Ok(None);
    }
    if let Some(proxy) = proxies.get(url.scheme()) {
        return Ok(proxy.clone());
    }
    Ok(proxies.get("all").cloned().flatten())
}

/// Request failures that handlers and the director can exchange without
/// exposing a particular HTTP implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidRequest,
    Unsupported,
    Transport,
    Http { status: u16, reason: String },
    NoSupportingHandlers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    pub kind: ErrorKind,
    pub message: String,
}

impl RequestError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidRequest, message)
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for RequestError {}

/// Response metadata and an in-memory body returned by a request handler.
///
/// Streaming response bodies will replace `body` when the native transport
/// layer is added. Keeping the metadata contract separate makes that change
/// possible without changing the director API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    url: String,
    status: u16,
    reason: String,
    headers: ResponseHeaders,
    body: Vec<u8>,
}

impl Response {
    pub fn new(
        url: impl Into<String>,
        status: u16,
        reason: impl Into<String>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            url: url.into(),
            status,
            reason: reason.into(),
            headers: ResponseHeaders::new(),
            body: body.into(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn headers(&self) -> &ResponseHeaders {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut ResponseHeaders {
        &mut self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// A request contract corresponding to yt-dlp's networking `Request`.
///
/// The native representation currently supports in-memory byte payloads.
/// Streaming/file-like payloads are an explicit TODO until a native
/// `RequestBody` abstraction is implemented.
#[derive(Debug, Clone)]
pub struct Request {
    url: String,
    explicit_method: Option<String>,
    data: Option<Vec<u8>>,
    headers: Headers,
    proxies: IndexMap<String, Option<String>>,
    extensions: IndexMap<String, Value>,
    cookie_jar: Option<SharedCookieJar>,
}

impl Request {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: normalize_url(&url.into()),
            explicit_method: None,
            data: None,
            headers: Headers::new(),
            proxies: IndexMap::new(),
            extensions: IndexMap::new(),
            cookie_jar: None,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = normalize_url(&url.into());
    }

    pub fn update_query(&mut self, query: &[(String, String)]) {
        self.url = update_url_query(&self.url, query);
    }

    pub fn method(&self) -> &str {
        self.explicit_method
            .as_deref()
            .unwrap_or(if self.data.is_some() { "POST" } else { "GET" })
    }

    pub fn set_method(&mut self, method: impl AsRef<str>) -> Result<(), RequestError> {
        let method = method.as_ref();
        if method.is_empty() || method.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
            return Err(RequestError::invalid(
                "method must not contain control characters",
            ));
        }
        self.explicit_method = Some(method.to_ascii_uppercase());
        Ok(())
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn set_data(&mut self, data: Option<Vec<u8>>) {
        if self.data.is_none() && data.is_none() {
            self.headers.remove("Content-Length");
        }
        if self.data != data {
            if self.data.is_some() {
                self.headers.remove("Content-Length");
            }
            self.data = data;
        }
        if self.data.is_none() {
            self.headers.remove("Content-Type");
        } else if !self.headers.contains("Content-Type") {
            self.headers
                .set("Content-Type", "application/x-www-form-urlencoded");
        }
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    pub fn headers_mut(&mut self) -> &mut Headers {
        &mut self.headers
    }

    pub fn proxies(&self) -> &IndexMap<String, Option<String>> {
        &self.proxies
    }

    pub fn proxies_mut(&mut self) -> &mut IndexMap<String, Option<String>> {
        &mut self.proxies
    }

    pub fn extensions(&self) -> &IndexMap<String, Value> {
        &self.extensions
    }

    pub fn extensions_mut(&mut self) -> &mut IndexMap<String, Value> {
        &mut self.extensions
    }

    pub fn cookie_jar(&self) -> Option<&SharedCookieJar> {
        self.cookie_jar.as_ref()
    }

    pub fn set_cookie_jar(&mut self, cookie_jar: SharedCookieJar) {
        self.cookie_jar = Some(cookie_jar);
    }

    pub fn with_cookie_jar(mut self, cookie_jar: SharedCookieJar) -> Self {
        self.set_cookie_jar(cookie_jar);
        self
    }
}

impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
            && self.explicit_method == other.explicit_method
            && self.data == other.data
            && self.headers == other.headers
            && self.proxies == other.proxies
            && self.extensions == other.extensions
            && match (&self.cookie_jar, &other.cookie_jar) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }
}

impl Eq for Request {}

/// A request handler implemented by a concrete transport backend.
pub trait RequestHandler: Send + Sync {
    fn name(&self) -> &str;

    fn preference(&self, _request: &Request) -> i32 {
        0
    }

    fn supports(&self, request: &Request) -> Result<(), RequestError>;

    fn send(&self, request: &Request) -> Result<Response, RequestError>;
}

/// Minimal native HTTP/1.1 transport for deterministic local and direct HTTP
/// requests. HTTPS, proxies, compression, and cookies remain separate
/// capabilities; redirect policy is handled here for direct HTTP requests.
#[derive(Debug, Clone, Copy, Default)]
pub struct HttpHandler;

impl HttpHandler {
    fn timeout(request: &Request) -> Duration {
        request
            .extensions()
            .get("timeout")
            .and_then(Value::as_f64)
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .map(Duration::from_secs_f64)
            .unwrap_or(DEFAULT_TIMEOUT)
    }

    fn connect(request: &Request, url: &Url) -> Result<TcpStream, RequestError> {
        let host = url
            .host_str()
            .ok_or_else(|| RequestError::invalid("HTTP URL has no host"))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| RequestError::invalid("HTTP URL has no port"))?;
        let stream = TcpStream::connect((host, port)).map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("failed to connect to {host}:{port}: {error}"),
            )
        })?;
        let timeout = Self::timeout(request);
        stream
            .set_read_timeout(Some(timeout))
            .and_then(|_| stream.set_write_timeout(Some(timeout)))
            .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
        Ok(stream)
    }

    fn write_request(
        &self,
        request: &Request,
        url: &Url,
        stream: &mut TcpStream,
    ) -> Result<(), RequestError> {
        let cookie_header = if request.headers().contains("Cookie") {
            None
        } else if let Some(jar) = request.cookie_jar() {
            jar.lock()
                .map_err(|_| RequestError::new(ErrorKind::Transport, "cookie jar poisoned"))?
                .cookie_header(request.url())?
        } else {
            None
        };
        let mut target = url.path().to_owned();
        if target.is_empty() {
            target.push('/');
        }
        if let Some(query) = url.query() {
            target.push('?');
            target.push_str(query);
        }

        let host = url
            .host_str()
            .ok_or_else(|| RequestError::invalid("HTTP URL has no host"))?;
        let host_header = if let Some(port) = url.port() {
            if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            }
        } else if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };

        let mut serialized = format!("{} {target} HTTP/1.1\r\n", request.method());
        if !request.headers().contains("Host") {
            serialized.push_str("Host: ");
            serialized.push_str(&host_header);
            serialized.push_str("\r\n");
        }
        if let Some(cookie_header) = cookie_header {
            serialized.push_str("Cookie: ");
            serialized.push_str(&cookie_header);
            serialized.push_str("\r\n");
        }

        for (name, value) in request.headers().iter() {
            if name.bytes().any(|byte| byte == b'\r' || byte == b'\n')
                || value.bytes().any(|byte| byte == b'\r' || byte == b'\n')
            {
                return Err(RequestError::invalid(
                    "HTTP headers cannot contain CR or LF",
                ));
            }
            serialized.push_str(name);
            serialized.push_str(": ");
            serialized.push_str(value);
            serialized.push_str("\r\n");
        }
        if !request.headers().contains("Connection") {
            serialized.push_str("Connection: close\r\n");
        }
        if let Some(data) = request.data() {
            if !request.headers().contains("Content-Length") {
                serialized.push_str(&format!("Content-Length: {}\r\n", data.len()));
            }
        }
        serialized.push_str("\r\n");

        stream
            .write_all(serialized.as_bytes())
            .and_then(|_| request.data().map_or(Ok(()), |data| stream.write_all(data)))
            .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))
    }

    fn read_response(request: &Request, stream: &mut TcpStream) -> Result<Response, RequestError> {
        let mut raw = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let count = stream
                .read(&mut buffer)
                .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
            if count == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..count]);
            if raw.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let header_end = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::Transport,
                    "HTTP response has no header terminator",
                )
            })?;
        let header_text = std::str::from_utf8(&raw[..header_end]).map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("invalid HTTP headers: {error}"),
            )
        })?;
        let is_chunked = header_text.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("Transfer-Encoding")
                    && value.trim().eq_ignore_ascii_case("chunked")
            })
        });
        let content_length = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        let body_start = header_end + 4;
        if is_chunked {
            while chunked_body_end(&raw[body_start..])?.is_none() {
                let count = stream
                    .read(&mut buffer)
                    .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
                if count == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..count]);
            }
        } else if let Some(content_length) = content_length {
            let expected_length = body_start.checked_add(content_length).ok_or_else(|| {
                RequestError::new(ErrorKind::Transport, "HTTP response body is too large")
            })?;
            while raw.len() < expected_length {
                let count = stream
                    .read(&mut buffer)
                    .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
                if count == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..count]);
            }
        } else {
            stream
                .read_to_end(&mut raw)
                .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
        }
        parse_http_response(request.url(), &raw)
    }

    fn send_once(&self, request: &Request) -> Result<Response, RequestError> {
        let url = Url::parse(request.url())
            .map_err(|error| RequestError::invalid(format!("invalid HTTP URL: {error}")))?;
        let mut stream = Self::connect(request, &url)?;
        self.write_request(request, &url, &mut stream)?;
        let response = Self::read_response(request, &mut stream)?;
        if let Some(jar) = request.cookie_jar() {
            jar.lock()
                .map_err(|_| RequestError::new(ErrorKind::Transport, "cookie jar poisoned"))?
                .store_response(request.url(), response.headers())?;
        }
        Ok(response)
    }
}

impl RequestHandler for HttpHandler {
    fn name(&self) -> &str {
        "HTTP"
    }

    fn preference(&self, _request: &Request) -> i32 {
        0
    }

    fn supports(&self, request: &Request) -> Result<(), RequestError> {
        let url = Url::parse(request.url())
            .map_err(|error| RequestError::invalid(format!("invalid HTTP URL: {error}")))?;
        if url.scheme() != "http" {
            return Err(RequestError::new(
                ErrorKind::Unsupported,
                "HTTP handler only supports the http scheme",
            ));
        }
        if select_proxy(request.url(), request.proxies())?.is_some() {
            return Err(RequestError::new(
                ErrorKind::Unsupported,
                "HTTP handler does not support proxies",
            ));
        }
        if url.host_str().is_none() {
            return Err(RequestError::invalid("HTTP URL has no host"));
        }
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        self.supports(request)?;
        let mut current = request.clone();
        let mut visited = HashSet::new();

        for redirect_count in 0..=10 {
            if !visited.insert(current.url().to_owned()) {
                return Err(RequestError::new(
                    ErrorKind::Http {
                        status: 0,
                        reason: "redirect loop detected".to_owned(),
                    },
                    "HTTP redirect loop detected",
                ));
            }

            let response = self.send_once(&current)?;
            if !matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
                return Ok(response);
            }
            let Some(location) = response.headers().get("Location") else {
                return Ok(response);
            };
            if redirect_count == 10 {
                return Err(RequestError::new(
                    ErrorKind::Http {
                        status: response.status(),
                        reason: "too many redirects".to_owned(),
                    },
                    "HTTP redirect limit exceeded",
                ));
            }

            let next_url = Url::parse(current.url())
                .and_then(|url| url.join(location))
                .map_err(|error| {
                    RequestError::new(
                        ErrorKind::Transport,
                        format!("invalid redirect URL: {error}"),
                    )
                })?;
            let old_method = current.method().to_owned();
            let next_method = redirect_method(&old_method, response.status());
            current.headers_mut().remove("Cookie");
            if next_method != old_method {
                current.set_data(None);
                current.set_method(next_method)?;
            }
            current.set_url(next_url.to_string());
        }

        unreachable!("redirect loop always returns from the bounded loop")
    }
}

fn redirect_method(method: &str, status: u16) -> &str {
    if status == 303 && method != "HEAD" {
        return "GET";
    }
    if matches!(status, 301 | 302) && method == "POST" {
        return "GET";
    }
    method
}

/// HTTPS-capable client transport used for TLS, proxy, and content-coding
/// support. Redirects are kept under yt-dlp's control so method rewriting and
/// cookie handling remain identical to the direct handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReqwestHandler;

impl ReqwestHandler {
    fn proxy_url(request: &Request) -> Result<Option<String>, RequestError> {
        let proxy = select_proxy(request.url(), request.proxies())?;
        let Some(proxy) = proxy else {
            return Ok(None);
        };
        let parsed = Url::parse(&proxy).map_err(|error| {
            RequestError::new(
                ErrorKind::Unsupported,
                format!("invalid proxy URL {proxy:?}: {error}"),
            )
        })?;
        if !matches!(
            parsed.scheme(),
            "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
        ) {
            return Err(RequestError::new(
                ErrorKind::Unsupported,
                format!("unsupported proxy type: {:?}", parsed.scheme()),
            ));
        }
        if parsed.host_str().is_none() {
            return Err(RequestError::new(
                ErrorKind::Unsupported,
                format!("proxy URL has no host: {proxy:?}"),
            ));
        }
        Ok(Some(proxy))
    }

    fn client(request: &Request) -> Result<reqwest::blocking::Client, RequestError> {
        let mut builder = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(HttpHandler::timeout(request))
            .no_proxy()
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .zstd(true);
        if request.extensions().get("verify").and_then(Value::as_bool) == Some(false) {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if let Some(proxy) = Self::proxy_url(request)? {
            let proxy = reqwest::Proxy::all(proxy).map_err(|error| {
                RequestError::new(ErrorKind::Unsupported, format!("invalid proxy: {error}"))
            })?;
            builder = builder.proxy(proxy);
        }
        builder
            .build()
            .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))
    }

    fn send_once(&self, request: &Request) -> Result<Response, RequestError> {
        let client = Self::client(request)?;
        let method = reqwest::Method::from_bytes(request.method().as_bytes())
            .map_err(|error| RequestError::invalid(format!("invalid HTTP method: {error}")))?;
        let mut builder = client.request(method, request.url());
        let cookie_header = if request.headers().contains("Cookie") {
            None
        } else if let Some(jar) = request.cookie_jar() {
            jar.lock()
                .map_err(|_| RequestError::new(ErrorKind::Transport, "cookie jar poisoned"))?
                .cookie_header(request.url())?
        } else {
            None
        };

        for (name, value) in request.headers().iter() {
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| RequestError::invalid(format!("invalid header name: {error}")))?;
            let value = reqwest::header::HeaderValue::from_str(value)
                .map_err(|error| RequestError::invalid(format!("invalid header value: {error}")))?;
            builder = builder.header(name, value);
        }
        if let Some(cookie_header) = cookie_header {
            builder = builder.header(reqwest::header::COOKIE, cookie_header);
        }
        if let Some(data) = request.data() {
            builder = builder.body(data.to_vec());
        }

        let response = builder
            .send()
            .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
        let response_url = response.url().to_string();
        let status = response.status();
        let mut headers = ResponseHeaders::new();
        for (name, value) in response.headers() {
            headers.add(name.as_str(), String::from_utf8_lossy(value.as_bytes()));
        }
        if let Some(jar) = request.cookie_jar() {
            jar.lock()
                .map_err(|_| RequestError::new(ErrorKind::Transport, "cookie jar poisoned"))?
                .store_response(&response_url, &headers)?;
        }
        let body = response
            .bytes()
            .map_err(|error| RequestError::new(ErrorKind::Transport, error.to_string()))?;
        let reason = status.canonical_reason().unwrap_or_default();
        let mut result = Response::new(response_url, status.as_u16(), reason, body.to_vec());
        result.headers = headers;
        Ok(result)
    }
}

impl RequestHandler for ReqwestHandler {
    fn name(&self) -> &str {
        "Reqwest"
    }

    fn preference(&self, request: &Request) -> i32 {
        let is_https = Url::parse(request.url())
            .ok()
            .is_some_and(|url| url.scheme() == "https");
        let has_proxy = select_proxy(request.url(), request.proxies())
            .ok()
            .flatten()
            .is_some();
        if is_https || has_proxy { 10 } else { -10 }
    }

    fn supports(&self, request: &Request) -> Result<(), RequestError> {
        let url = Url::parse(request.url())
            .map_err(|error| RequestError::invalid(format!("invalid HTTP URL: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(RequestError::new(
                ErrorKind::Unsupported,
                "Reqwest handler only supports http and https schemes",
            ));
        }
        if url.host_str().is_none() {
            return Err(RequestError::invalid("HTTP URL has no host"));
        }
        Self::proxy_url(request)?;
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        self.supports(request)?;
        let mut current = request.clone();
        let mut visited = HashSet::new();

        for redirect_count in 0..=10 {
            if !visited.insert(current.url().to_owned()) {
                return Err(RequestError::new(
                    ErrorKind::Http {
                        status: 0,
                        reason: "redirect loop detected".to_owned(),
                    },
                    "HTTP redirect loop detected",
                ));
            }
            let response = self.send_once(&current)?;
            if !matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
                return Ok(response);
            }
            let Some(location) = response.headers().get("Location") else {
                return Ok(response);
            };
            if redirect_count == 10 {
                return Err(RequestError::new(
                    ErrorKind::Http {
                        status: response.status(),
                        reason: "too many redirects".to_owned(),
                    },
                    "HTTP redirect limit exceeded",
                ));
            }
            let next_url = Url::parse(current.url())
                .and_then(|url| url.join(location))
                .map_err(|error| {
                    RequestError::new(
                        ErrorKind::Transport,
                        format!("invalid redirect URL: {error}"),
                    )
                })?;
            let old_method = current.method().to_owned();
            let next_method = redirect_method(&old_method, response.status());
            current.headers_mut().remove("Cookie");
            if next_method != old_method {
                current.set_data(None);
                current.set_method(next_method)?;
            }
            current.set_url(next_url.to_string());
        }
        unreachable!("redirect loop always returns from the bounded loop")
    }
}

fn parse_http_response(request_url: &str, raw: &[u8]) -> Result<Response, RequestError> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            RequestError::new(
                ErrorKind::Transport,
                "HTTP response has no header terminator",
            )
        })?;
    let header_text = std::str::from_utf8(&raw[..header_end]).map_err(|error| {
        RequestError::new(
            ErrorKind::Transport,
            format!("invalid HTTP headers: {error}"),
        )
    })?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().ok_or_else(|| {
        RequestError::new(ErrorKind::Transport, "HTTP response has no status line")
    })?;
    let mut status_parts = status_line.splitn(3, ' ');
    let version = status_parts.next().unwrap_or_default();
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| {
            RequestError::new(ErrorKind::Transport, "HTTP response has an invalid status")
        })?;
    if !version.starts_with("HTTP/") {
        return Err(RequestError::new(
            ErrorKind::Transport,
            "HTTP response has an invalid version",
        ));
    }

    let mut headers = ResponseHeaders::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(|| {
            RequestError::new(ErrorKind::Transport, "HTTP response has an invalid header")
        })?;
        headers.add(name, value);
    }

    let encoded_body = &raw[header_end + 4..];
    let body = if headers
        .get("Transfer-Encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        decode_chunked_body(encoded_body)?
    } else if let Some(length) = headers.get("Content-Length") {
        let length = length.parse::<usize>().map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("invalid Content-Length: {error}"),
            )
        })?;
        if encoded_body.len() < length {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "HTTP response body is incomplete",
            ));
        }
        encoded_body[..length].to_vec()
    } else {
        encoded_body.to_vec()
    };

    let reason = status_parts_reason(status_line)
        .filter(|reason| !reason.is_empty())
        .unwrap_or_else(|| default_http_reason(status));
    let mut response = Response::new(request_url, status, reason, body);
    response.headers = headers;
    Ok(response)
}

fn status_parts_reason(status_line: &str) -> Option<&str> {
    status_line.splitn(3, ' ').nth(2)
}

fn default_http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

fn decode_chunked_body(mut body: &[u8]) -> Result<Vec<u8>, RequestError> {
    let mut decoded = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "invalid chunk size"))?;
        let size_text = std::str::from_utf8(&body[..line_end]).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(decoded);
        }
        let end = size
            .checked_add(2)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        if body.len() < end || &body[size..end] != b"\r\n" {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "invalid chunk body",
            ));
        }
        decoded.extend_from_slice(&body[..size]);
        body = &body[end..];
    }
}

fn chunked_body_end(body: &[u8]) -> Result<Option<usize>, RequestError> {
    let mut offset = 0;
    loop {
        let Some(line_end) = body[offset..]
            .windows(2)
            .position(|window| window == b"\r\n")
        else {
            return Ok(None);
        };
        let line_end = offset + line_end;
        let size_text = std::str::from_utf8(&body[offset..line_end]).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|error| {
            RequestError::new(ErrorKind::Transport, format!("invalid chunk size: {error}"))
        })?;
        let data_start = line_end + 2;
        if size == 0 {
            let trailers = &body[data_start..];
            if trailers.starts_with(b"\r\n") {
                return Ok(Some(data_start + 2));
            }
            return Ok(trailers
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|end| data_start + end + 4));
        }
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        let chunk_end = data_end
            .checked_add(2)
            .ok_or_else(|| RequestError::new(ErrorKind::Transport, "chunk body is too large"))?;
        if body.len() < chunk_end {
            return Ok(None);
        }
        if &body[data_end..chunk_end] != b"\r\n" {
            return Err(RequestError::new(
                ErrorKind::Transport,
                "invalid chunk body",
            ));
        }
        offset = chunk_end;
    }
}

/// Select and invoke the most-preferred handler for a request.
pub struct RequestDirector {
    handlers: Vec<Box<dyn RequestHandler>>,
}

impl RequestDirector {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Construct the native HTTP stack with direct HTTP first and the
    /// HTTPS/proxy/compression-capable native backend as the secondary handler.
    pub fn native() -> Self {
        let mut director = Self::new();
        director.add_handler(HttpHandler);
        director.add_handler(ReqwestHandler);
        director
    }

    pub fn add_handler<H>(&mut self, handler: H)
    where
        H: RequestHandler + 'static,
    {
        self.handlers.push(Box::new(handler));
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if self.handlers.is_empty() {
            return Err(RequestError::new(
                ErrorKind::NoSupportingHandlers,
                "No request handlers configured",
            ));
        }

        let mut handlers: Vec<&dyn RequestHandler> =
            self.handlers.iter().map(AsRef::as_ref).collect();
        handlers.sort_by_key(|handler| std::cmp::Reverse(handler.preference(request)));

        let mut unsupported = Vec::new();
        for handler in handlers {
            match handler.supports(request) {
                Ok(()) => {}
                Err(error) if error.kind == ErrorKind::Unsupported => {
                    unsupported.push(format!("{}: {}", handler.name(), error.message));
                    continue;
                }
                Err(error) => return Err(error),
            }

            return handler.send(request);
        }

        let message = if unsupported.is_empty() {
            "Unable to handle request".to_owned()
        } else {
            format!("Unable to handle request: {}", unsupported.join(", "))
        };
        Err(RequestError::new(ErrorKind::NoSupportingHandlers, message))
    }
}

impl Default for RequestDirector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn headers_are_case_insensitive_but_retain_original_spelling() {
        let mut headers = Headers::new();
        headers.set("x-dlp", " data ");
        headers.set("Ytdl-Test", "one");
        headers.set("ytdl-test", "two");

        assert_eq!(headers.get("X-DLP"), Some("data"));
        assert_eq!(headers.get("YTDL-TEST"), Some("two"));
        assert_eq!(
            headers.sensitive().get("ytdl-test"),
            Some(&"two".to_owned())
        );
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn response_headers_preserve_multiplicity_and_cookie_semantics() {
        let mut headers = ResponseHeaders::new();
        headers.add("Test", "test1");
        headers.add("test", "test2");
        headers.add("Set-Cookie", "cookie1");
        headers.add("Set-cookie", "cookie2");

        assert_eq!(headers.get_all("TEST"), vec!["test1", "test2"]);
        assert_eq!(headers.get_header("test"), Some("test1, test2".to_owned()));
        assert_eq!(headers.get_header("set-cookie"), Some("cookie1".to_owned()));
        assert_eq!(headers.get_header("missing"), None);
    }

    #[test]
    fn cookie_jar_applies_domain_path_and_secure_rules() {
        let mut jar = CookieJar::new();
        let mut headers = ResponseHeaders::new();
        headers.add("Set-Cookie", "sid=abc; Path=/account");
        headers.add("Set-Cookie", "secure=yes; Secure; Path=/");
        jar.store_response("https://example.com/login", &headers)
            .unwrap();

        assert_eq!(
            jar.cookie_header("https://example.com/account/page")
                .unwrap(),
            Some("sid=abc; secure=yes".to_owned())
        );
        assert_eq!(
            jar.cookie_header("http://example.com/account/page")
                .unwrap(),
            Some("sid=abc".to_owned())
        );
        assert_eq!(
            jar.cookie_header("https://example.com/other").unwrap(),
            Some("secure=yes".to_owned())
        );
    }

    #[test]
    fn proxy_selection_honors_scheme_all_and_no_proxy() {
        let proxies = IndexMap::from([
            ("all".to_owned(), Some("http://proxy.example".to_owned())),
            (
                "https".to_owned(),
                Some("http://secure-proxy.example".to_owned()),
            ),
            (
                "no".to_owned(),
                Some("localhost,example.com:8080".to_owned()),
            ),
        ]);
        assert_eq!(
            select_proxy("https://video.example", &proxies).unwrap(),
            Some("http://secure-proxy.example".to_owned())
        );
        assert_eq!(
            select_proxy("http://video.example", &proxies).unwrap(),
            Some("http://proxy.example".to_owned())
        );
        assert_eq!(
            select_proxy("http://localhost:8080", &proxies).unwrap(),
            None
        );
        assert_eq!(
            select_proxy("http://example.com:8080/path", &proxies).unwrap(),
            None
        );
    }

    #[test]
    fn request_defaults_method_and_content_headers_from_data() {
        let mut request = Request::new("https://example.com");
        assert_eq!(request.method(), "GET");
        assert!(request.data().is_none());
        assert!(!request.headers().contains("Content-Type"));

        request.set_data(Some(b"a=1".to_vec()));
        assert_eq!(request.method(), "POST");
        assert_eq!(
            request.headers().get("content-type"),
            Some("application/x-www-form-urlencoded")
        );

        request.headers_mut().set("Content-Length", "3");
        request.set_data(Some(b"a=2".to_vec()));
        assert!(!request.headers().contains("Content-Length"));

        request.set_method("put").unwrap();
        assert_eq!(request.method(), "PUT");
        request.set_data(None);
        assert_eq!(request.method(), "PUT");
        assert!(!request.headers().contains("Content-Type"));
    }

    #[test]
    fn request_rejects_control_characters_in_method() {
        let mut request = Request::new("https://example.com");
        assert!(request.set_method("GET\n").is_err());
    }

    #[test]
    fn normalize_url_matches_reference_examples() {
        assert_eq!(normalize_url("//example.com"), "http://example.com");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(
            normalize_url("https://фtest.example.com/ some spaceв?ä=c"),
            "https://xn--test-z6d.example.com/%20some%20space%D0%B2?%C3%A4=c"
        );
        assert_eq!(
            normalize_url("https://example.com/a/../b"),
            "https://example.com/b"
        );
    }

    #[test]
    fn update_url_query_replaces_existing_keys_and_appends_new_keys() {
        assert_eq!(
            update_url_query(
                "http://example.com?q=something",
                &[("v".to_owned(), "xyz".to_owned())]
            ),
            "http://example.com?q=something&v=xyz"
        );
        assert_eq!(
            update_url_query(
                "http://example.com?q=something&v=old",
                &[("v".to_owned(), "123".to_owned())]
            ),
            "http://example.com?q=something&v=123"
        );
        assert_eq!(
            update_url_query(
                "http://example.com?a=1&a=2&blank=",
                &[
                    ("a".to_owned(), "3".to_owned()),
                    ("v".to_owned(), "hello world".to_owned())
                ]
            ),
            "http://example.com?a=3&v=hello+world"
        );
    }

    fn serve_once(response: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            stream.write_all(&response).unwrap();
        });
        (format!("http://{address}"), thread)
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
        }

        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            return request;
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let content_length = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        let expected_length = header_end + 4 + content_length.unwrap_or(0);
        while request.len() < expected_length {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
        }
        request
    }

    fn serve_sequence(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                stream.write_all(&response).unwrap();
            }
            requests
        });
        (format!("http://{address}"), thread)
    }

    #[test]
    fn http_handler_sends_request_and_reads_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello"
                .to_vec(),
        );
        let mut request = Request::new(format!("{url}/hello?x=1"));
        request.headers_mut().set("X-Test", "one");

        let mut director = RequestDirector::new();
        director.add_handler(HttpHandler);
        let response = director.send(&request).unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.reason(), "OK");
        assert_eq!(response.body(), b"hello");
        assert_eq!(response.headers().get("content-type"), Some("text/plain"));
        server.join().unwrap();
    }

    #[test]
    fn http_handler_decodes_chunked_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n".to_vec(),
        );
        let request = Request::new(url);
        let response = HttpHandler.send(&request).unwrap();

        assert_eq!(response.reason(), "OK");
        assert_eq!(response.body(), b"hello world");
        server.join().unwrap();
    }

    #[test]
    fn http_handler_does_not_claim_https() {
        let request = Request::new("https://example.com");
        let error = HttpHandler.supports(&request).unwrap_err();

        assert_eq!(error.kind, ErrorKind::Unsupported);
    }

    #[test]
    fn http_handler_follows_redirects_and_rewrites_post_to_get() {
        let (url, server) = serve_sequence(vec![
            b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
        ]);
        let mut request = Request::new(format!("{url}/start"));
        request.set_data(Some(b"a=1".to_vec()));

        let response = HttpHandler.send(&request).unwrap();
        let requests = server.join().unwrap();
        let first = String::from_utf8_lossy(&requests[0]);
        let second = String::from_utf8_lossy(&requests[1]);

        assert_eq!(response.url(), format!("{url}/final"));
        assert_eq!(response.body(), b"ok");
        assert!(first.starts_with("POST /start HTTP/1.1\r\n"));
        assert!(first.ends_with("a=1"));
        assert!(second.starts_with("GET /final HTTP/1.1\r\n"));
        assert!(!second.contains("Content-Length:"));
        assert!(!second.contains("Content-Type:"));
    }

    #[test]
    fn http_handler_reports_redirect_loops() {
        let (url, server) = serve_once(
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n"
                .to_vec(),
        );
        let error = HttpHandler
            .send(&Request::new(format!("{url}/loop")))
            .unwrap_err();
        server.join().unwrap();

        assert_eq!(
            error.kind,
            ErrorKind::Http {
                status: 0,
                reason: "redirect loop detected".to_owned(),
            }
        );
    }

    #[test]
    fn http_handler_persists_set_cookie_across_redirects() {
        let (url, server) = serve_sequence(vec![
            b"HTTP/1.1 302 Found\r\nSet-Cookie: sid=abc; Path=/\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n"
                .to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
        ]);
        let jar = CookieJar::new().shared();
        let request = Request::new(format!("{url}/start")).with_cookie_jar(jar);

        let response = HttpHandler.send(&request).unwrap();
        let requests = server.join().unwrap();
        let second = String::from_utf8_lossy(&requests[1]);

        assert_eq!(response.body(), b"ok");
        assert!(second.contains("Cookie: sid=abc\r\n"));
    }

    #[test]
    fn reqwest_handler_supports_https_and_decodes_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello"
                .to_vec(),
        );
        let request = Request::new(format!("{url}/hello"));
        let response = ReqwestHandler.send(&request).unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.body(), b"hello");
        server.join().unwrap();
        assert!(
            ReqwestHandler
                .supports(&Request::new("https://example.com"))
                .is_ok()
        );
    }

    #[test]
    fn redirect_method_matches_python_policy() {
        for (method, status, expected) in [
            ("GET", 303, "GET"),
            ("HEAD", 303, "HEAD"),
            ("PUT", 303, "GET"),
            ("POST", 301, "GET"),
            ("HEAD", 301, "HEAD"),
            ("POST", 302, "GET"),
            ("PUT", 302, "PUT"),
            ("POST", 307, "POST"),
            ("POST", 308, "POST"),
        ] {
            assert_eq!(redirect_method(method, status), expected);
        }
    }

    struct FakeHandler {
        name: &'static str,
        preference: i32,
        supported: bool,
        body: &'static [u8],
    }

    impl RequestHandler for FakeHandler {
        fn name(&self) -> &str {
            self.name
        }

        fn preference(&self, _request: &Request) -> i32 {
            self.preference
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            if self.supported {
                Ok(())
            } else {
                Err(RequestError::new(ErrorKind::Unsupported, "test scheme"))
            }
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            Ok(Response::new(request.url(), 200, "OK", self.body.to_vec()))
        }
    }

    #[test]
    fn director_prefers_supported_handler() {
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            name: "low",
            preference: 1,
            supported: true,
            body: b"low",
        });
        director.add_handler(FakeHandler {
            name: "high-but-unsupported",
            preference: 2,
            supported: false,
            body: b"unused",
        });
        director.add_handler(FakeHandler {
            name: "high",
            preference: 2,
            supported: true,
            body: b"high",
        });

        let response = director.send(&Request::new("test://example")).unwrap();
        assert_eq!(response.body(), b"high");
    }

    #[test]
    fn director_reports_unsupported_handlers() {
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            name: "fake",
            preference: 0,
            supported: false,
            body: b"unused",
        });

        let error = director.send(&Request::new("test://example")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::NoSupportingHandlers);
        assert_eq!(error.message, "Unable to handle request: fake: test scheme");
    }

    #[test]
    fn native_director_selects_direct_http_handler() {
        let (url, server) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec());
        let response = RequestDirector::native().send(&Request::new(url)).unwrap();
        server.join().unwrap();
        assert_eq!(response.body(), b"ok");
    }
}
