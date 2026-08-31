//! Ordered extractor registry for the Rust migration.
//!
//! Descriptors can be registered before their implementation is ported. Such
//! entries remain visible and return an explicit TODO error, which prevents a
//! missing extractor from being mistaken for a generic match.

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
            Self::Playlist { .. } => None,
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
        let mut request = request.clone();
        if request.cookie_jar().is_none() {
            request.set_cookie_jar(self.cookie_jar.clone());
        }
        let response = self.director.send(&request).map_err(map_request_error)?;
        if response.status() >= 400 {
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

fn map_request_error(error: RequestError) -> ExtractorError {
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
                "extractor {} is not ported to Rust yet",
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

fn compile_source_pattern(pattern: &str) -> Result<Regex, fancy_regex::Error> {
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

/// Minimal native equivalent of GenericIE for direct resources and simple
/// pages. It intentionally returns only URL-derived fields; richer HTML,
/// manifest, and playlist inspection belongs to the later generic extractor
/// stages.
pub struct GenericExtractor {
    descriptor: ExtractorDescriptor,
}

impl GenericExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Self {
        Self { descriptor }
    }
}

impl InfoExtractor for GenericExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, _url: &str) -> bool {
        true
    }

    fn is_native(&self) -> bool {
        true
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let mut info = self.extract(url)?;
        if info.get_bool("direct") == Some(true) {
            return Ok(ExtractorResult::single(info));
        }

        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid generic URL: {error}"),
            )
        })?;
        let mut media_urls = Vec::new();
        for key in [
            "og:video:secure_url",
            "og:video",
            "og:audio",
            "twitter:player:stream",
        ] {
            if let Some(value) = html_meta_value(&html, key) {
                if let Ok(media_url) = parsed.join(value.trim()) {
                    media_urls.push(media_url.to_string());
                }
            }
        }
        if let Ok(source_matcher) =
            Regex::new(r#"(?is)<(?:source|video|audio)\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
        {
            for captures in source_matcher.captures_iter(&html).flatten() {
                if let Some(value) = captures.get(1).map(|value| value.as_str()) {
                    if let Ok(media_url) = parsed.join(value.trim()) {
                        if !media_urls.contains(&media_url.to_string()) {
                            media_urls.push(media_url.to_string());
                        }
                    }
                }
            }
        }

        if let Some(title) = html_meta_value(&html, "og:title") {
            info.insert("title", serde_json::json!(title));
        }
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        if let Some(thumbnail) = html_meta_value(&html, "og:image") {
            info.insert(
                "thumbnail",
                serde_json::json!(
                    parsed
                        .join(thumbnail.trim())
                        .map_or(thumbnail, |url| url.to_string())
                ),
            );
        }
        let formats = media_urls
            .iter()
            .enumerate()
            .map(|(index, media_url)| {
                let ext = yt_dlp_core::determine_ext(Some(media_url), "unknown_video");
                serde_json::json!({
                    "format_id": format!("generic-{index}"),
                    "url": media_url,
                    "ext": ext,
                    "protocol": if ext == "m3u8" { "m3u8_native" } else { "http" },
                })
            })
            .collect::<Vec<_>>();
        if let Some(first) = formats.first() {
            info.insert(
                "url",
                first.get("url").cloned().unwrap_or(serde_json::Value::Null),
            );
            info.insert(
                "ext",
                first.get("ext").cloned().unwrap_or(serde_json::Value::Null),
            );
            info.insert("formats", serde_json::Value::Array(formats));
        }
        Ok(ExtractorResult::single(info))
    }

    fn extract(&self, url: &str) -> Result<InfoDict, ExtractorError> {
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid generic URL: {error}"),
            )
        })?;
        let path_name = parsed
            .path_segments()
            .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
            .map(str::to_owned)
            .unwrap_or_else(|| parsed.host_str().unwrap_or("download").to_owned());
        let (id, extension) = path_name.rsplit_once('.').map_or_else(
            || (path_name.clone(), None),
            |(stem, extension)| {
                let extension = (!extension.is_empty()
                    && extension.len() <= 10
                    && extension
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric()))
                .then(|| extension.to_ascii_lowercase());
                (stem.to_owned(), extension)
            },
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(id));
        info.insert("title", serde_json::json!(id));
        info.insert("url", serde_json::json!(url));
        info.insert("direct", serde_json::json!(extension.is_some()));
        if let Some(extension) = extension {
            info.insert("ext", serde_json::json!(extension));
        }
        Ok(info)
    }
}

fn descriptor_matcher(descriptor: &ExtractorDescriptor) -> Result<Regex, ExtractorError> {
    let pattern = descriptor.valid_urls.first().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("native extractor {} has no URL pattern", descriptor.key),
        )
    })?;
    compile_source_pattern(pattern).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid native URL pattern for {}: {error}", descriptor.key),
        )
    })
}

fn path_segment_after(url: &str, marker: &str) -> Result<String, ExtractorError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid extractor URL: {error}"),
        )
    })?;
    let segments = parsed
        .path_segments()
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "URL has no path"))?
        .collect::<Vec<_>>();
    let position = segments
        .iter()
        .position(|segment| *segment == marker)
        .and_then(|position| segments.get(position + 1))
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("URL has no path segment after {marker}"),
            )
        })?;
    Ok((*position).to_owned())
}

fn last_path_segment(url: &str) -> Result<String, ExtractorError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("invalid extractor URL: {error}"),
        )
    })?;
    parsed
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| ExtractorError::new(ExtractorErrorKind::InvalidUrl, "URL has no ID"))
}

fn html_meta_value(html: &str, key: &str) -> Option<String> {
    let key = regex::escape(key);
    let patterns = [
        format!(
            r#"(?is)<meta\b[^>]*(?:property|name)\s*=\s*["']{key}["'][^>]*content\s*=\s*["']([^"']*)"#,
        ),
        format!(
            r#"(?is)<meta\b[^>]*content\s*=\s*["']([^"']*)["'][^>]*(?:property|name)\s*=\s*["']{key}["']"#,
        ),
    ];
    patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(html).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
    })
}

fn json_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

/// Native Clyp API extractor. The API response already contains stable media
/// URLs, so this port does not depend on browser JavaScript or an embedded
/// interpreter.
pub struct ClypExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ClypExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ClypExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let audio_id = last_path_segment(url)?;
        let mut api_request = Request::new(format!("https://api.clyp.it/{audio_id}"));
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(token) = parsed
                .query_pairs()
                .find(|(name, _)| name == "token")
                .map(|(_, value)| value.into_owned())
            {
                api_request.update_query(&[("token".to_owned(), token)]);
            }
        }
        let metadata = context.get_json(api_request.url())?;
        let mut formats = Vec::new();
        for secure in ["", "Secure"] {
            for extension in ["Ogg", "Mp3"] {
                let key = format!("{secure}{extension}Url");
                let Some(format_url) = json_string(&metadata, &key) else {
                    continue;
                };
                formats.push(serde_json::json!({
                    "url": format_url,
                    "format_id": format!("{secure}{extension}"),
                    "ext": extension.to_ascii_lowercase(),
                    "vcodec": "none",
                    "acodec": extension.to_ascii_lowercase(),
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Clyp API returned no playable formats for {audio_id}"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&metadata, "Title").unwrap_or(&audio_id)),
        );
        info.insert_if_some("description", json_string(&metadata, "Description"));
        info.insert_if_some("duration", json_f64(&metadata, "Duration"));
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(value) = first.get("url") {
            info.insert("url", value.clone());
        }
        if let Some(value) = first.get("ext") {
            info.insert("ext", value.clone());
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Breitbart extractor. Breitbart exposes a JWPlayer HLS manifest whose
/// URL is derived from the video ID; page metadata is read with the native HTTP
/// stack and the existing Rust HLS downloader handles the media.
pub struct BreitbartExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BreitbartExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BreitbartExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matcher.is_match(url).unwrap_or(false)
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        1
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = path_segment_after(url, "v")?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let manifest_url = format!("https://cdn.jwplayer.com/manifests/{video_id}.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("url", serde_json::json!(manifest_url));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": manifest_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

#[derive(Default)]
pub struct ExtractorRegistry {
    extractors: Vec<Box<dyn InfoExtractor>>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestRecord {
    key: String,
    name: String,
    #[allow(dead_code)]
    module: String,
    #[allow(dead_code)]
    class: String,
    working: bool,
    patterns: Vec<String>,
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the ordered extractor inventory generated from the source
    /// `gen_extractors()` registry. Patterns that use source-only regular
    /// expression features remain visible in the inventory and are reported
    /// through `DescriptorExtractor::matcher_errors` instead of disappearing.
    pub fn generated() -> Result<Self, ExtractorError> {
        let records: Vec<ManifestRecord> =
            serde_json::from_str(include_str!("../data/extractors.json")).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid generated extractor manifest: {error}"),
                )
            })?;
        let mut registry = Self::new();
        for record in records {
            let descriptor = ExtractorDescriptor::with_valid_urls(
                record.key,
                record.name,
                record.patterns.clone(),
                record.working,
            )
            .with_source(record.module, record.class);
            if descriptor.key == "GenericIE" {
                registry.register(GenericExtractor::new(descriptor))?;
            } else if descriptor.key == "ClypIE" {
                registry.register(ClypExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BreitBartIE" {
                registry.register(BreitbartExtractor::new(descriptor)?)?;
            } else {
                registry.register(DescriptorExtractor::from_patterns_lossy(
                    descriptor,
                    &record.patterns,
                ))?;
            }
        }
        Ok(registry)
    }

    pub fn register<E>(&mut self, extractor: E) -> Result<(), ExtractorError>
    where
        E: InfoExtractor + 'static,
    {
        let key = extractor.descriptor().key.as_str();
        if self
            .extractors
            .iter()
            .any(|registered| registered.descriptor().key == key)
        {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("duplicate extractor key: {key}"),
            ));
        }
        self.extractors.push(Box::new(extractor));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.extractors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extractors.is_empty()
    }

    pub fn native_matchable_count(&self) -> usize {
        self.extractors
            .iter()
            .filter(|extractor| {
                extractor.pattern_count() > 0 && extractor.matcher_error_count() == 0
            })
            .count()
    }

    pub fn native_implementation_count(&self) -> usize {
        self.extractors
            .iter()
            .filter(|extractor| extractor.is_native())
            .count()
    }

    pub fn native_pattern_count(&self) -> usize {
        self.extractors
            .iter()
            .map(|extractor| extractor.native_matcher_count())
            .sum()
    }

    pub fn pattern_error_count(&self) -> usize {
        self.extractors
            .iter()
            .map(|extractor| extractor.matcher_error_count())
            .sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn InfoExtractor> {
        self.extractors.iter().map(Box::as_ref)
    }

    pub fn find(&self, url: &str) -> Option<&dyn InfoExtractor> {
        self.extractors
            .iter()
            .find(|extractor| extractor.suitable(url))
            .map(Box::as_ref)
    }

    pub fn extract(&self, url: &str) -> Result<InfoDict, ExtractorError> {
        let extractor = self.find(url).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("no extractor found for URL: {url}"),
            )
        })?;
        extractor.extract(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yt_dlp_networking::RequestHandler;

    struct FakeHandler {
        body: Vec<u8>,
    }

    impl RequestHandler for FakeHandler {
        fn name(&self) -> &str {
            "extractor-test"
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            Ok(())
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            Ok(Response::new(request.url(), 200, "OK", self.body.clone()))
        }
    }

    #[test]
    fn registry_preserves_registration_order() {
        let mut registry = ExtractorRegistry::new();
        registry
            .register(
                DescriptorExtractor::new(ExtractorDescriptor::new(
                    "first",
                    "First",
                    r"^https://example\.com/.*$",
                    true,
                ))
                .unwrap(),
            )
            .unwrap();
        registry
            .register(
                DescriptorExtractor::new(ExtractorDescriptor::new(
                    "second",
                    "Second",
                    r"^https://example\.com/video$",
                    true,
                ))
                .unwrap(),
            )
            .unwrap();

        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry
                .find("https://example.com/video")
                .unwrap()
                .descriptor()
                .key,
            "first"
        );
    }

    #[test]
    fn unported_descriptor_is_explicitly_unsupported() {
        let extractor = DescriptorExtractor::new(ExtractorDescriptor::new(
            "test",
            "Test",
            r"^https://test\.example/",
            false,
        ))
        .unwrap();
        let error = extractor.extract("https://test.example/video").unwrap_err();
        assert_eq!(error.kind, ExtractorErrorKind::Unsupported);
        assert!(error.message.contains("not ported"));
    }

    #[test]
    fn duplicate_keys_and_invalid_patterns_are_rejected() {
        let mut registry = ExtractorRegistry::new();
        let first = DescriptorExtractor::new(ExtractorDescriptor::new(
            "same",
            "Same",
            r"^https://example\.com",
            true,
        ))
        .unwrap();
        registry.register(first).unwrap();
        let duplicate = DescriptorExtractor::new(ExtractorDescriptor::new(
            "same",
            "Same again",
            r"^https://other\.example",
            true,
        ))
        .unwrap();
        assert!(registry.register(duplicate).is_err());
        assert!(
            DescriptorExtractor::new(ExtractorDescriptor::new("broken", "Broken", "[", true,))
                .is_err()
        );
    }

    #[test]
    fn generated_manifest_preserves_extractor_inventory_and_order() {
        let registry = ExtractorRegistry::generated().unwrap();

        // Refresh this snapshot whenever the source extractor registry is
        // intentionally regenerated.
        assert_eq!(registry.len(), 1_752);
        assert!(registry.native_matchable_count() > 1_000);
        assert!(registry.native_pattern_count() > 1_000);
        assert_eq!(registry.pattern_error_count(), 0);
        assert_eq!(
            registry.iter().last().unwrap().descriptor().key,
            "GenericIE"
        );
        assert_eq!(registry.native_implementation_count(), 3);
    }

    #[test]
    fn generic_extractor_returns_stable_url_metadata() {
        let registry = ExtractorRegistry::generated().unwrap();
        let info = registry
            .extract("https://media.example.test/path/sample-video.MP4?token=1")
            .unwrap();
        assert_eq!(info.get("id"), Some(&serde_json::json!("sample-video")));
        assert_eq!(info.get("title"), Some(&serde_json::json!("sample-video")));
        assert_eq!(info.get("ext"), Some(&serde_json::json!("mp4")));
        assert_eq!(info.get("direct"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn clyp_native_extractor_maps_api_formats_in_rust() {
        let extractor = ClypExtractor::new(ExtractorDescriptor::new(
            "ClypIE",
            "Clyp",
            r"https?://(?:www\.)?clyp\.it/(?P<id>[a-z0-9]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br##"{
                "Title": "research",
                "Description": "#Research",
                "Duration": 51.278,
                "OggUrl": "https://cdn.example/research.ogg",
                "Mp3Url": "https://cdn.example/research.mp3"
            }"##
            .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://clyp.it/iynkjk4b", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("iynkjk4b"));
        assert_eq!(result.get_str("title"), Some("research"));
        assert_eq!(result.get_str("ext"), Some("ogg"));
        assert_eq!(
            result
                .get("formats")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn breitbart_native_extractor_reads_page_metadata_and_manifest() {
        let extractor = BreitbartExtractor::new(ExtractorDescriptor::new(
            "BreitBartIE",
            "BreitBart",
            r"https?://(?:www\.)?breitbart\.com/videos/v/(?P<id>[^/?#]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:title" content="Example title">
                <meta property="og:description" content="Example description">
                <meta property="og:image" content="https://cdn.example/thumb.jpg">
            </head></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://www.breitbart.com/videos/v/abc123/", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("title"), Some("Example title"));
        assert_eq!(
            result.get_str("url"),
            Some("https://cdn.jwplayer.com/manifests/abc123.m3u8")
        );
        assert_eq!(result.get_str("ext"), Some("mp4"));
    }

    #[test]
    fn generic_native_extractor_reads_open_graph_and_html5_media() {
        let extractor =
            GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "generic", ".*", true));
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:title" content="Native page">
                <meta property="og:description" content="Native description">
                <meta property="og:image" content="/thumb.jpg">
            </head><body>
                <video><source src="/media/video.mp4" type="video/mp4"></video>
            </body></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://example.test/watch/page", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("title"), Some("Native page"));
        assert_eq!(
            result.get_str("url"),
            Some("https://example.test/media/video.mp4")
        );
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert_eq!(
            result.get_str("thumbnail"),
            Some("https://example.test/thumb.jpg")
        );
    }
}
