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
