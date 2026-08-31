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
