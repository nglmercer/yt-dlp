use fancy_regex::Regex;
use yt_dlp_core::InfoDict;
use yt_dlp_networking::{
    CookieJar, Request, RequestDirector, RequestError, Response, SharedCookieJar,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractorErrorKind {
    InvalidUrl,
    Unsupported,
    Extraction,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractorError {
    pub kind: ExtractorErrorKind,
    pub message: String,
}

impl ExtractorError {
    pub fn new(kind: ExtractorErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ExtractorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for ExtractorError {}

/// Result of a native extraction. Playlists retain their container metadata
/// separately from entries so the CLI can later apply playlist and entry
/// selection without flattening the result prematurely.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractorResult {
    Single(InfoDict),
    Redirect {
        url: String,
        ie_key: Option<String>,
    },
    Playlist {
        info: InfoDict,
        entries: Vec<InfoDict>,
    },
}

impl ExtractorResult {
    pub fn single(info: InfoDict) -> Self {
        Self::Single(info)
    }

    pub fn into_info_dict(self) -> InfoDict {
        match self {
            Self::Single(info) => info,
            Self::Redirect { url, ie_key } => {
                let mut info = InfoDict::new();
                info.insert("_type", serde_json::json!("url"));
                info.insert("url", serde_json::json!(url));
                info.insert_if_some("ie_key", ie_key);
                info
            }
            Self::Playlist { mut info, entries } => {
                info.insert("_type", serde_json::json!("playlist"));
                info.insert(
                    "entries",
                    serde_json::to_value(entries).unwrap_or(serde_json::Value::Null),
                );
                info
            }
        }
    }

    pub fn as_single(&self) -> Option<&InfoDict> {
        match self {
            Self::Single(info) => Some(info),
            Self::Redirect { .. } | Self::Playlist { .. } => None,
        }
    }
}

/// Shared native request context for extractors that need to fetch metadata or
/// manifests. It owns no Python state and all response errors are surfaced as
/// native extractor errors.
pub struct ExtractionContext {
    director: RequestDirector,
    cookie_jar: SharedCookieJar,
}

impl ExtractionContext {
    pub fn new(director: RequestDirector, cookie_jar: SharedCookieJar) -> Self {
        Self {
            director,
            cookie_jar,
        }
    }

    pub fn native() -> Self {
        Self::new(RequestDirector::native(), CookieJar::new().shared())
    }

    pub fn cookie_jar(&self) -> &SharedCookieJar {
        &self.cookie_jar
    }

    pub fn request(&self, request: &Request) -> Result<Response, ExtractorError> {
        self.request_with_status(request, &[])
    }

    pub fn request_with_status(
        &self,
        request: &Request,
        accepted_statuses: &[u16],
    ) -> Result<Response, ExtractorError> {
        let mut request = request.clone();
        if request.cookie_jar().is_none() {
            request.set_cookie_jar(self.cookie_jar.clone());
        }
        let response = self.director.send(&request).map_err(map_request_error)?;
        if response.status() >= 400 && !accepted_statuses.contains(&response.status()) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Network,
                format!(
                    "HTTP {} while extracting {}",
                    response.status(),
                    response.url()
                ),
            ));
        }
        Ok(response)
    }

    pub fn get(&self, url: &str) -> Result<Response, ExtractorError> {
        self.request(&Request::new(url))
    }

    pub fn get_json(&self, url: &str) -> Result<serde_json::Value, ExtractorError> {
        let response = self.get(url)?;
        serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid JSON from {}: {error}", response.url()),
            )
        })
    }
}

pub(crate) fn map_request_error(error: RequestError) -> ExtractorError {
    ExtractorError::new(ExtractorErrorKind::Network, error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractorDescriptor {
    pub key: String,
    pub name: String,
    pub valid_url: String,
    pub valid_urls: Vec<String>,
    pub working: bool,
    pub source_module: Option<String>,
    pub source_class: Option<String>,
}

impl ExtractorDescriptor {
    pub fn new(
        key: impl Into<String>,
        name: impl Into<String>,
        valid_url: impl Into<String>,
        working: bool,
    ) -> Self {
        let valid_url = valid_url.into();
        Self {
            key: key.into(),
            name: name.into(),
            valid_urls: if valid_url.is_empty() {
                Vec::new()
            } else {
                vec![valid_url.clone()]
            },
            valid_url,
            working,
            source_module: None,
            source_class: None,
        }
    }

    pub fn with_valid_urls(
        key: impl Into<String>,
        name: impl Into<String>,
        valid_urls: Vec<String>,
        working: bool,
    ) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            valid_url: valid_urls.first().cloned().unwrap_or_default(),
            valid_urls,
            working,
            source_module: None,
            source_class: None,
        }
    }

    pub fn with_source(mut self, module: impl Into<String>, class: impl Into<String>) -> Self {
        self.source_module = Some(module.into());
        self.source_class = Some(class.into());
        self
    }
}

pub trait InfoExtractor: Send + Sync {
    fn descriptor(&self) -> &ExtractorDescriptor;

    fn suitable(&self, url: &str) -> bool;

    fn is_native(&self) -> bool {
        false
    }

    fn pattern_count(&self) -> usize {
        self.descriptor().valid_urls.len()
    }

    fn native_matcher_count(&self) -> usize {
        0
    }

    fn matcher_error_count(&self) -> usize {
        0
    }

    fn matcher_errors(&self) -> &[String] {
        &[]
    }

    fn extract_with_context(
        &self,
        url: &str,
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        self.extract(url).map(ExtractorResult::single)
    }

    fn extract(&self, _url: &str) -> Result<InfoDict, ExtractorError> {
        Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!(
                "TODO: extractor {} is not ported to Rust yet",
                self.descriptor().key
            ),
        ))
    }
}

pub struct DescriptorExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
    matcher_errors: Vec<String>,
}

pub(crate) fn compile_source_pattern(pattern: &str) -> Result<Regex, fancy_regex::Error> {
    match Regex::new(pattern) {
        Ok(matcher) => Ok(matcher),
        Err(error) => {
            // The source accepts an unescaped `[` inside a character class in
            // a legacy-compatible way. Rust's regex parser requires it to be
            // escaped; this is the only such pattern in the current
            // generated inventory, so retain the source pattern and repair
            // only its native compilation form.
            let repaired = pattern.replace("[/+[\\w-]", "[/+\\[\\w-]");
            if repaired == pattern {
                Err(error)
            } else {
                Regex::new(&repaired)
            }
        }
    }
}

impl DescriptorExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Self::from_patterns(descriptor, None)
    }

    fn from_patterns(
        descriptor: ExtractorDescriptor,
        patterns: Option<&[String]>,
    ) -> Result<Self, ExtractorError> {
        let patterns = patterns.unwrap_or(&descriptor.valid_urls);
        let mut matchers = Vec::with_capacity(patterns.len());
        for pattern in patterns {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid valid_url for {}: {error}", descriptor.key),
                )
            })?);
        }
        Ok(Self {
            descriptor,
            matchers,
            matcher_errors: Vec::new(),
        })
    }

    pub fn from_patterns_lossy(descriptor: ExtractorDescriptor, patterns: &[String]) -> Self {
        let mut matchers = Vec::with_capacity(patterns.len());
        let mut matcher_errors = Vec::new();
        for pattern in patterns {
            match compile_source_pattern(pattern) {
                Ok(matcher) => matchers.push(matcher),
                Err(error) => matcher_errors.push(format!("{pattern}: {error}")),
            }
        }
        Self {
            descriptor,
            matchers,
            matcher_errors,
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.descriptor.valid_urls.len()
    }

    pub fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    pub fn matcher_errors(&self) -> &[String] {
        &self.matcher_errors
    }

    pub fn fully_native_matchable(&self) -> bool {
        self.matcher_errors.is_empty()
    }
}

impl InfoExtractor for DescriptorExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn native_matcher_count(&self) -> usize {
        DescriptorExtractor::native_matcher_count(self)
    }

    fn matcher_error_count(&self) -> usize {
        self.matcher_errors.len()
    }

    fn matcher_errors(&self) -> &[String] {
        &self.matcher_errors
    }
}
