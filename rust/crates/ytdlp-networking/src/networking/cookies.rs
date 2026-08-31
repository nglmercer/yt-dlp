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

    /// Load a Netscape/Mozilla cookie-jar file without involving a Python
    /// cookie implementation. Expired records are ignored at load time, as
    /// they are by the in-memory request path.
    pub fn load_netscape_file(&mut self, path: &Path) -> Result<usize, RequestError> {
        let file = File::open(path).map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("could not read cookie file {path:?}: {error}"),
            )
        })?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut loaded = 0;
        for (line_number, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|error| {
                RequestError::new(
                    ErrorKind::Transport,
                    format!("could not read cookie file {path:?}: {error}"),
                )
            })?;
            let (http_only, line) = line
                .strip_prefix("#HttpOnly_")
                .map_or((false, line.as_str()), |line| (true, line));
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.splitn(7, '\t').collect::<Vec<_>>();
            if fields.len() != 7 {
                return Err(RequestError::invalid(format!(
                    "invalid Netscape cookie at {path:?}:{}",
                    line_number + 1
                )));
            }
            let domain = fields[0].trim();
            let host = domain.trim_start_matches('.');
            let cookie_path = if fields[2].is_empty() { "/" } else { fields[2] };
            let expires = fields[4].parse::<u64>().unwrap_or(0);
            if expires > 0 && expires <= now {
                continue;
            }
            if host.is_empty() || fields[5].is_empty() {
                continue;
            }
            let request_url = Url::parse(&format!("https://{host}/")).map_err(|error| {
                RequestError::invalid(format!(
                    "invalid cookie domain {domain:?} at {path:?}:{}: {error}",
                    line_number + 1
                ))
            })?;
            let mut builder =
                cookie_store::RawCookie::build((fields[5].to_owned(), fields[6].to_owned()))
                    .path(cookie_path.to_owned())
                    .secure(fields[3].eq_ignore_ascii_case("TRUE"))
                    .http_only(http_only);
            if fields[1].eq_ignore_ascii_case("TRUE") {
                builder = builder.domain(domain.to_owned());
            }
            let cookie = builder.build();
            if self.store.insert_raw(&cookie, &request_url).is_ok() {
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    /// Save unexpired cookies in Netscape format for reuse by later native
    /// invocations. Session cookies use an expiry of zero.
    pub fn save_netscape_file(&self, path: &Path) -> Result<usize, RequestError> {
        let mut file = File::create(path).map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("could not write cookie file {path:?}: {error}"),
            )
        })?;
        writeln!(file, "# Netscape HTTP Cookie File").map_err(|error| {
            RequestError::new(
                ErrorKind::Transport,
                format!("could not write cookie file {path:?}: {error}"),
            )
        })?;
        let mut saved = 0;
        for cookie in self.store.iter_unexpired() {
            let domain = String::from(&cookie.domain);
            if domain.is_empty()
                || cookie
                    .name()
                    .chars()
                    .any(|character| matches!(character, '\t' | '\n' | '\r'))
            {
                continue;
            }
            let value = cookie.value();
            if value
                .chars()
                .any(|character| matches!(character, '\t' | '\n' | '\r'))
            {
                continue;
            }
            let include_subdomains =
                matches!(&cookie.domain, cookie_store::CookieDomain::Suffix(_));
            let http_only = cookie.http_only().unwrap_or(false);
            let domain_field = if http_only {
                format!("#HttpOnly_{domain}")
            } else {
                domain
            };
            let expires = match &cookie.expires {
                cookie_store::CookieExpiration::AtUtc(time) => time.unix_timestamp().max(0),
                cookie_store::CookieExpiration::SessionEnd => 0,
            };
            writeln!(
                file,
                "{domain_field}\t{}\t{}\t{}\t{expires}\t{}\t{value}",
                if include_subdomains { "TRUE" } else { "FALSE" },
                String::from(&cookie.path),
                if cookie.secure().unwrap_or(false) {
                    "TRUE"
                } else {
                    "FALSE"
                },
                cookie.name(),
            )
            .map_err(|error| {
                RequestError::new(
                    ErrorKind::Transport,
                    format!("could not write cookie file {path:?}: {error}"),
                )
            })?;
            saved += 1;
        }
        Ok(saved)
    }
}
