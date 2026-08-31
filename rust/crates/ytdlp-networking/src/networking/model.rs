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
