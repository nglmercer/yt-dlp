use fancy_regex::Regex;
use std::collections::HashMap;
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

/// Per-extractor configuration arguments (`--extractor-args IE_KEY:ARGS`).
/// Mirrors the `extractor_args` downloader param: the outer key is the
/// lowercased IE key, inner keys are normalized (`strip().lower().replace('-',
/// '_')` at parse time), and every value is the list of comma-separated
/// entries for that key.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ExtractorArgs {
    ie_args: HashMap<String, HashMap<String, Vec<String>>>,
}

impl ExtractorArgs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse one `--extractor-args IE_KEY:ARGS` value into its IE key and
    /// argument map, mirroring `_dict_from_options_callback` with
    /// `multiple_keys=False` plus the extractor-arg `process` step. Later
    /// pairs win on repeat keys, and repeating the flag replaces the whole
    /// per-IE map (handled by [`Self::insert_ie_args`]).
    pub fn parse_cli_value(value: &str) -> Result<(String, HashMap<String, Vec<String>>), String> {
        let (ie_key, args) = value.split_once(':').ok_or_else(|| {
            format!("wrong --extractor-args formatting; it should be IE_KEY:ARGS, not \"{value}\"")
        })?;
        if ie_key.is_empty()
            || !ie_key.chars().all(|character| {
                character.is_alphanumeric() || character == '_' || character == '-'
            })
        {
            return Err(format!(
                "wrong --extractor-args formatting; it should be IE_KEY:ARGS, not \"{value}\""
            ));
        }
        let mut parsed = HashMap::new();
        for entry in args.split(';') {
            let (key, values) = match entry.split_once('=') {
                Some((key, values)) => (key, values),
                None => (entry, ""),
            };
            let key = key.trim().to_ascii_lowercase().replace('-', "_");
            parsed.insert(key, split_extractor_arg_values(values));
        }
        Ok((ie_key.to_ascii_lowercase(), parsed))
    }

    /// Store one parsed flag value, replacing any previous map for the IE.
    pub fn insert_ie_args(&mut self, ie_key: String, args: HashMap<String, Vec<String>>) {
        self.ie_args.insert(ie_key, args);
    }

    /// Mirror `InfoExtractor._configuration_arg`: look up `key` under the
    /// lowercased `ie_key` (the key itself is matched exactly, as stored).
    /// Missing keys yield an empty list; with `casesense` false the values
    /// are lowercased.
    pub fn configuration_arg(&self, ie_key: &str, key: &str, casesense: bool) -> Vec<String> {
        let values = self
            .ie_args
            .get(&ie_key.to_ascii_lowercase())
            .and_then(|args| args.get(key))
            .cloned()
            .unwrap_or_default();
        if casesense {
            values
        } else {
            values
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect()
        }
    }
}

/// Split one extractor-arg value on commas that are not backslash-escaped,
/// unescape `\,` to `,`, and strip whitespace, mirroring the
/// `--extractor-args` value processing in `yt_dlp/options.py`.
fn split_extractor_arg_values(values: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut previous = '\0';
    for character in values.chars() {
        if character == ',' && previous != '\\' {
            items.push(std::mem::take(&mut current));
        } else {
            current.push(character);
        }
        previous = character;
    }
    items.push(current);
    items
        .into_iter()
        .map(|item| item.replace(r"\,", ","))
        .map(|item| item.trim().to_owned())
        .collect()
}

/// Shared native request context for extractors that need to fetch metadata or
/// manifests. It owns no Python state and all response errors are surfaced as
/// native extractor errors.
pub struct ExtractionContext {
    director: RequestDirector,
    cookie_jar: SharedCookieJar,
    extractor_args: ExtractorArgs,
}

impl ExtractionContext {
    pub fn new(director: RequestDirector, cookie_jar: SharedCookieJar) -> Self {
        Self {
            director,
            cookie_jar,
            extractor_args: ExtractorArgs::new(),
        }
    }

    pub fn with_extractor_args(mut self, extractor_args: ExtractorArgs) -> Self {
        self.extractor_args = extractor_args;
        self
    }

    pub fn native() -> Self {
        Self::new(RequestDirector::native(), CookieJar::new().shared())
    }

    pub fn cookie_jar(&self) -> &SharedCookieJar {
        &self.cookie_jar
    }

    pub fn configuration_arg(&self, ie_key: &str, key: &str, casesense: bool) -> Vec<String> {
        self.extractor_args
            .configuration_arg(ie_key, key, casesense)
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
