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

fn html_script_json(html: &str, script_id: &str) -> Result<serde_json::Value, ExtractorError> {
    let pattern = format!(
        r#"(?is)<script\b[^>]*\bid\s*=\s*["']{}["'][^>]*>(.*?)</script>"#,
        regex::escape(script_id)
    );
    let matcher = Regex::new(&pattern).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid script-data matcher: {error}"),
        )
    })?;
    let captures = matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("HTML has no {script_id} JSON script"),
            )
        })?;
    serde_json::from_str(captures.trim()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid {script_id} JSON: {error}"),
        )
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

fn json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn native_post_json(
    context: &ExtractionContext,
    url: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(url);
    request.set_method("POST").map_err(map_request_error)?;
    request.headers_mut().set("Accept", "application/json");
    request
        .headers_mut()
        .set("Content-Type", "application/json");
    request.set_data(Some(serde_json::to_vec(payload).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("could not encode native JSON request: {error}"),
        )
    })?));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JSON from {}: {error}", response.url()),
        )
    })
}

fn unescape_html_attribute(value: &str) -> String {
    [
        ("&quot;", "\""),
        ("&#34;", "\""),
        ("&#x22;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&apos;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&nbsp;", " "),
        ("&amp;", "&"),
    ]
    .into_iter()
    .fold(value.to_owned(), |value, (from, to)| {
        value.replace(from, to)
    })
}

fn html_data_json_attribute(html: &str, attribute: &str) -> Option<serde_json::Value> {
    let attribute = regex::escape(attribute);
    for pattern in [
        format!(r#"(?is)\bdata-{attribute}\s*=\s*"([^"]*)"#),
        format!(r#"(?is)\bdata-{attribute}\s*=\s*'([^']*)"#),
    ] {
        let Ok(matcher) = Regex::new(&pattern) else {
            continue;
        };
        let Some(captures) = matcher.captures(html).ok().flatten() else {
            continue;
        };
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str(&unescape_html_attribute(raw)) {
            return Some(value);
        }
    }
    None
}

fn audio_boom_clip_store(html: &str) -> Option<serde_json::Value> {
    for pattern in [
        r#"(?is)data-react-class\s*=\s*["']V5DetailPagePlayer["'][^>]*data-react-props\s*=\s*["']([^"']*)"#,
        r#"(?is)data-react-props\s*=\s*["']([^"']*)[^>]*data-react-class\s*=\s*["']V5DetailPagePlayer["']"#,
    ] {
        let Ok(matcher) = Regex::new(pattern) else {
            continue;
        };
        let Some(captures) = matcher.captures(html).ok().flatten() else {
            continue;
        };
        let Some(raw) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Ok(store) = serde_json::from_str(&unescape_html_attribute(raw)) {
            return Some(store);
        }
    }
    None
}

fn html_text_fragment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    unescape_html_attribute(output.trim())
}

/// Native AudioBoom HTML/API extractor. The page embeds the same clip store
/// used by the source implementation; Rust reads that JSON directly and
/// falls back to Open Graph/audio metadata when the player attributes change.
pub struct AudioBoomExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudioBoomExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AudioBoomExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "AudioBoom URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "AudioBoom URL has no ID")
            })?;
        let webpage = context.get(&format!("https://audioboom.com/posts/{audio_id}"))?;
        let html = String::from_utf8_lossy(webpage.body());
        let clip_store = audio_boom_clip_store(&html);
        let clip = clip_store
            .as_ref()
            .and_then(|store| store.get("clips"))
            .and_then(serde_json::Value::as_array)
            .and_then(|clips| clips.first());

        let media_url = clip
            .and_then(|clip| json_string(clip, "clipURLPriorToLoading"))
            .map(str::to_owned)
            .or_else(|| {
                html_meta_value(&html, "og:audio").map(|value| unescape_html_attribute(&value))
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("AudioBoom page has no playable audio for {audio_id}"),
                )
            })?;
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp3");
        let title = clip
            .and_then(|clip| json_string(clip, "title"))
            .map(str::to_owned)
            .or_else(|| {
                ["og:title", "og:audio:title", "audio_title"]
                    .iter()
                    .find_map(|key| html_meta_value(&html, key))
            })
            .unwrap_or_else(|| audio_id.to_owned());
        let description = clip
            .and_then(|clip| json_string(clip, "description"))
            .map(str::to_owned)
            .or_else(|| {
                clip.and_then(|clip| json_string(clip, "formattedDescription"))
                    .map(html_text_fragment)
            })
            .or_else(|| html_meta_value(&html, "og:description"));
        let duration = clip
            .and_then(|clip| json_f64(clip, "duration"))
            .or_else(|| {
                html_meta_value(&html, "weibo:audio:duration")
                    .and_then(|value| value.parse::<f64>().ok())
            });
        let uploader = clip
            .and_then(|clip| json_string(clip, "author"))
            .map(str::to_owned)
            .or_else(|| {
                [
                    "og:audio:artist",
                    "twitter:audio:artist_name",
                    "audio_artist",
                ]
                .iter()
                .find_map(|key| html_meta_value(&html, key))
            });
        let uploader_url = Regex::new(
            r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*\bavatar\b[^"']*["'][^>]*>.*?<a\b[^>]*href\s*=\s*["'](https?://[^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()));

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("uploader_url", uploader_url);
        Ok(ExtractorResult::single(info))
    }
}

/// Native BitChute API extractor. Video media and metadata are obtained from
/// the public JSON endpoints; HLS URLs are handed to the native downloader.
pub struct BitChuteExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BitChuteExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BitChuteExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "BitChute URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BitChute URL has no ID")
            })?;
        let payload = serde_json::json!({"video_id": video_id});
        let media = native_post_json(
            context,
            "https://api.bitchute.com/api/beta/video/media",
            &payload,
        )?;
        let media_url = json_string(&media, "media_url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BitChute media response has no media_url",
            )
        })?;
        let detected_ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let is_hls = detected_ext == "m3u8";
        let output_ext = if is_hls {
            "mp4".to_owned()
        } else {
            detected_ext
        };
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(output_ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": if is_hls { "hls" } else { "direct" },
                "ext": output_ext,
                "protocol": if is_hls { "m3u8_native" } else { "http" },
            }]),
        );

        let video =
            native_post_json(context, "https://api.bitchute.com/api/beta/video", &payload).ok();
        if let Some(video) = video.as_ref() {
            info.insert_if_some("title", json_string(video, "video_name"));
            info.insert_if_some("description", json_string(video, "description"));
            info.insert_if_some("thumbnail", json_string(video, "thumbnail_url"));
            info.insert_if_some("view_count", json_i64(video, "view_count"));
            let duration = json_f64(video, "duration")
                .or_else(|| json_string(video, "duration").and_then(yt_dlp_core::parse_duration));
            info.insert_if_some("duration", duration);
            if let Some(value) = video.get("date_published") {
                info.insert("date_published", value.clone());
            }
            if let Some(value) = video.get("state_id").and_then(serde_json::Value::as_str) {
                info.insert("is_live", serde_json::json!(value == "live"));
            }
            if let Some(tags) = video.get("hashtags").and_then(serde_json::Value::as_array) {
                let tags = tags
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();
                info.insert("tags", serde_json::Value::Array(tags));
            }
            if let Some(profile_id) = json_string(video, "profile_id") {
                info.insert("uploader_id", serde_json::json!(profile_id));
                info.insert(
                    "uploader_url",
                    serde_json::json!(format!("https://www.bitchute.com/profile/{profile_id}/")),
                );
            }
            if let Some(channel) = video.get("channel") {
                info.insert_if_some("channel", json_string(channel, "channel_name"));
                info.insert_if_some("channel_id", json_string(channel, "channel_id"));
                if let Some(channel_url) = json_string(channel, "channel_url") {
                    info.insert("channel_url", serde_json::json!(channel_url));
                }
                if let Some(channel_id) = json_string(channel, "channel_id") {
                    if let Ok(channel_data) = native_post_json(
                        context,
                        "https://api.bitchute.com/api/beta/channel",
                        &serde_json::json!({"channel_id": channel_id}),
                    ) {
                        info.insert_if_some("uploader", json_string(&channel_data, "profile_name"));
                        info.insert_if_some(
                            "uploader_id",
                            json_string(&channel_data, "profile_id"),
                        );
                        if let Some(profile_id) = json_string(&channel_data, "profile_id") {
                            info.insert(
                                "uploader_url",
                                serde_json::json!(format!(
                                    "https://www.bitchute.com/profile/{profile_id}/"
                                )),
                            );
                        }
                        info.insert_if_some("channel", json_string(&channel_data, "channel_name"));
                        if let Some(slug) = json_string(&channel_data, "url_slug") {
                            info.insert(
                                "channel_url",
                                serde_json::json!(format!(
                                    "https://www.bitchute.com/channel/{slug}/"
                                )),
                            );
                        }
                    }
                }
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

fn archive_download_url(identifier: &str, name: &str) -> String {
    let mut url = url::Url::parse("https://archive.org/download").expect("static Archive.org URL");
    {
        let mut segments = url
            .path_segments_mut()
            .expect("Archive.org URL has mutable path segments");
        segments.push(identifier);
        segments.push(name);
    }
    url.to_string()
}

fn decode_url_component(value: &str) -> String {
    fn hex_digit(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn archive_text_value(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| {
            value.as_str().map(str::to_owned).or_else(|| {
                value.as_array().map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            })
        })
        .filter(|value| !value.is_empty())
}

fn archive_file_extension(name: &str) -> Option<String> {
    let extension = name.rsplit_once('.')?.1.trim().to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "3gp"
            | "aac"
            | "aiff"
            | "ape"
            | "avi"
            | "flac"
            | "flv"
            | "m4a"
            | "m4v"
            | "mka"
            | "mkv"
            | "mov"
            | "mp3"
            | "mp4"
            | "mpa"
            | "mpeg"
            | "mpg"
            | "oga"
            | "ogg"
            | "ogv"
            | "opus"
            | "wav"
            | "webm"
            | "wmv"
    )
    .then_some(extension)
}

/// Native Archive.org metadata extractor. Archive items are represented from
/// the public metadata JSON, with files sharing their 'original' name grouped
/// into one entry and multiple media entries returned as a native playlist.
pub struct ArchiveOrgExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArchiveOrgExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArchiveOrgExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Archive.org URL did not match its native pattern",
            )
        })?;
        let requested_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Archive.org URL has no ID")
            })?;
        let requested_id = decode_url_component(requested_id);
        let (requested_identifier, requested_entry) = requested_id
            .split_once('/')
            .map_or((requested_id.clone(), None), |(identifier, entry)| {
                (identifier.to_owned(), Some(entry.to_owned()))
            });
        let metadata = context.get_json(&format!(
            "https://archive.org/metadata/{requested_identifier}"
        ))?;
        let metadata_info = metadata.get("metadata").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Archive.org metadata has no metadata object",
            )
        })?;
        let identifier = json_string(metadata_info, "identifier")
            .unwrap_or(requested_identifier.as_str())
            .to_owned();

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(identifier));
        info.insert(
            "webpage_url",
            serde_json::json!(format!("https://archive.org/details/{identifier}")),
        );
        info.insert_if_some("title", archive_text_value(metadata_info.get("title")));
        info.insert_if_some(
            "description",
            archive_text_value(metadata_info.get("description")),
        );
        info.insert_if_some(
            "uploader",
            archive_text_value(
                metadata_info
                    .get("uploader")
                    .or_else(|| metadata_info.get("adder")),
            ),
        );
        info.insert_if_some("license", json_string(metadata_info, "licenseurl"));
        info.insert_if_some("location", json_string(metadata_info, "venue"));
        info.insert_if_some("release_year", json_i64(metadata_info, "year"));
        info.insert_if_some("release_date", json_string(metadata_info, "date"));
        if let Some(value) = metadata_info.get("creator") {
            info.insert("creators", value.clone());
        }

        let files = metadata
            .get("files")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Archive.org metadata has no files array",
                )
            })?;
        let mut entries = Vec::<InfoDict>::new();
        for file in files {
            if json_string(file, "format") == Some("Thumbnail") {
                continue;
            }
            let Some(name) = json_string(file, "name") else {
                continue;
            };
            let Some(extension) = archive_file_extension(name) else {
                continue;
            };
            let group = json_string(file, "original").unwrap_or(name);
            if let Some(requested_entry) = requested_entry.as_deref()
                && requested_entry != name
                && requested_entry != group
            {
                continue;
            }
            let entry_index = entries
                .iter()
                .position(|entry| entry.get_str("_archive_group") == Some(group))
                .unwrap_or_else(|| {
                    let mut entry = InfoDict::new();
                    entry.insert("_archive_group", serde_json::json!(group));
                    entry.insert("id", serde_json::json!(format!("{identifier}/{group}")));
                    entry.insert("display_id", serde_json::json!(group));
                    entry.insert(
                        "title",
                        serde_json::json!(json_string(file, "title").unwrap_or(group)),
                    );
                    entry.insert("formats", serde_json::json!([]));
                    entries.push(entry);
                    entries.len() - 1
                });
            let entry = &mut entries[entry_index];
            if let Some(value) = json_string(file, "description") {
                if !entry.contains_key("description") {
                    entry.insert("description", serde_json::json!(value));
                }
            }
            if let Some(value) = json_string(file, "creator") {
                if !entry.contains_key("creators") {
                    entry.insert("creators", serde_json::json!([value]));
                }
            }
            entry.insert_if_some(
                "duration",
                json_f64(file, "length")
                    .or_else(|| json_string(file, "length").and_then(yt_dlp_core::parse_duration)),
            );
            entry.insert_if_some("track_number", json_i64(file, "track"));
            entry.insert_if_some("album", json_string(file, "album"));
            entry.insert_if_some("discnumber", json_i64(file, "disc"));
            let file_url = archive_download_url(&identifier, name);
            let format = serde_json::json!({
                "url": file_url,
                "format": file.get("format").cloned().unwrap_or(serde_json::Value::Null),
                "ext": extension,
                "width": json_i64(file, "width"),
                "height": json_i64(file, "height"),
                "filesize": json_i64(file, "size"),
                "protocol": "https",
                "format_note": file.get("source").cloned().unwrap_or(serde_json::Value::Null),
            });
            let mut formats = entry
                .remove("formats")
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default();
            formats.push(format);
            entry.insert("formats", serde_json::Value::Array(formats));
            if !entry.contains_key("url") {
                entry.insert("url", serde_json::json!(file_url));
                entry.insert("ext", serde_json::json!(extension));
            }
        }
        for entry in &mut entries {
            entry.remove("_archive_group");
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Archive.org item {identifier} has no playable media files"),
            ));
        }

        if let Some(requested_entry) = requested_entry.as_deref() {
            let selected = entries
                .into_iter()
                .find(|entry| entry.get_str("display_id") == Some(requested_entry))
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Archive.org item has no requested file {requested_entry}"),
                    )
                })?;
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        if entries.len() == 1 {
            let selected = entries.pop().expect("one Archive.org entry");
            let mut merged = info;
            for (key, value) in selected.iter() {
                merged.insert(key, value.clone());
            }
            return Ok(ExtractorResult::single(merged));
        }
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn google_drive_mime_extension(mime_type: Option<&str>) -> Option<&'static str> {
    match mime_type {
        Some("video/mp4") => Some("mp4"),
        Some("video/webm") => Some("webm"),
        Some("video/ogg") => Some("ogv"),
        Some("audio/mpeg") => Some("mp3"),
        Some("audio/mp4") => Some("m4a"),
        Some("audio/webm") => Some("webm"),
        Some("audio/ogg") => Some("ogg"),
        Some("audio/flac") => Some("flac"),
        _ => None,
    }
}

fn google_drive_filename(content_disposition: Option<&str>) -> Option<String> {
    let matcher = Regex::new(r#"(?i)\bfilename\s*=\s*(?:["']([^"']+)["']|([^;\s]+))"#).ok()?;
    let captures = matcher.captures(content_disposition?).ok().flatten()?;
    captures
        .get(1)
        .or_else(|| captures.get(2))
        .map(|value| value.as_str().to_owned())
}

/// Native Google Drive playback extractor. Playback JSON formats and the
/// source-download response are handled with the Rust request stack.
pub struct GoogleDriveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoogleDriveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoogleDriveExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Google Drive URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Google Drive URL has no ID")
            })?;
        let mut playback_request = Request::new(format!(
            "https://content-workspacevideo-pa.googleapis.com/v1/drive/media/{video_id}/playback"
        ));
        playback_request.update_query(&[(
            "key".to_owned(),
            "AIzaSyDVQw45DwoYh632gvsP5vPDqEKvb-Ywnb8".to_owned(),
        )]);
        playback_request
            .headers_mut()
            .set("Referer", "https://drive.google.com/");
        let playback_response = context.request(&playback_request)?;
        let video_info: serde_json::Value = serde_json::from_slice(playback_response.body())
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Google Drive playback JSON: {error}"),
                )
            })?;

        let streaming_data = video_info
            .get("mediaStreamingData")
            .and_then(|value| value.get("formatStreamingData"));
        let mut formats = Vec::new();
        for group in ["adaptiveTranscodes", "progressiveTranscodes"] {
            let Some(transcodes) = streaming_data
                .and_then(|value| value.get(group))
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for transcode in transcodes {
                let Some(media_url) = json_string(transcode, "url") else {
                    continue;
                };
                let metadata = transcode.get("transcodeMetadata");
                let ext = google_drive_mime_extension(
                    metadata.and_then(|value| json_string(value, "mimeType")),
                )
                .unwrap_or("mp4");
                let mut format = serde_json::Map::new();
                format.insert("url".to_owned(), serde_json::json!(media_url));
                format.insert(
                    "format_id".to_owned(),
                    serde_json::json!(
                        json_value_string(transcode.get("itag"))
                            .unwrap_or_else(|| group.to_owned())
                    ),
                );
                format.insert("ext".to_owned(), serde_json::json!(ext));
                for (source, target) in [
                    ("width", "width"),
                    ("height", "height"),
                    ("videoFps", "fps"),
                    ("contentLength", "filesize"),
                ] {
                    if let Some(value) = metadata.and_then(|value| value.get(source)) {
                        format.insert(target.to_owned(), value.clone());
                    }
                }
                if let Some(value) =
                    metadata.and_then(|value| json_string(value, "videoCodecString"))
                {
                    format.insert("vcodec".to_owned(), serde_json::json!(value));
                }
                if let Some(value) =
                    metadata.and_then(|value| json_string(value, "audioCodecString"))
                {
                    format.insert("acodec".to_owned(), serde_json::json!(value));
                }
                format.insert(
                    "downloader_options".to_owned(),
                    serde_json::json!({"http_chunk_size": 10 << 20}),
                );
                formats.push(serde_json::Value::Object(format));
            }
        }

        let mut title = video_info
            .get("mediaMetadata")
            .and_then(|value| json_string(value, "title"))
            .map(str::to_owned);
        let source_response = {
            let mut request = Request::new("https://drive.usercontent.google.com/download");
            request.update_query(&[
                ("id".to_owned(), video_id.to_owned()),
                ("export".to_owned(), "download".to_owned()),
                ("confirm".to_owned(), "t".to_owned()),
            ]);
            request
                .headers_mut()
                .set("Referer", "https://drive.google.com/");
            context.request(&request).ok()
        };
        if let Some(response) = source_response {
            if let Some(filename) =
                google_drive_filename(response.headers().get("Content-Disposition"))
            {
                title.get_or_insert(filename);
                let ext = title
                    .as_deref()
                    .map(|value| yt_dlp_core::determine_ext(Some(value), "mp4"))
                    .unwrap_or_else(|| "mp4".to_owned());
                formats.push(serde_json::json!({
                    "url": response.url(),
                    "format_id": "source",
                    "ext": ext,
                    "quality": 1,
                    "protocol": "https",
                }));
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Google Drive file {video_id} has no playable formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", title);
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(duration) = video_info.get("mediaMetadata").and_then(|value| {
            json_f64(value, "duration")
                .or_else(|| json_string(value, "duration").and_then(yt_dlp_core::parse_duration))
        }) {
            info.insert("duration", serde_json::json!(duration));
        }
        if let Some(thumbnails) = video_info
            .get("thumbnails")
            .and_then(serde_json::Value::as_array)
        {
            let thumbnails = thumbnails
                .iter()
                .filter_map(|thumbnail| {
                    let url = json_string(thumbnail, "url")?;
                    let mut value = serde_json::json!({"url": url});
                    for key in ["width", "height"] {
                        if let Some(number) = thumbnail.get(key) {
                            value[key] = number.clone();
                        }
                    }
                    Some(value)
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Bandcamp track extractor. Track metadata and playable encodings are
/// read from the page's tralbum/embed JSON attributes without executing the
/// Bandcamp player.
pub struct BandcampTrackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BandcampTrackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BandcampTrackExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Bandcamp track URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no uploader",
                )
            })?;
        let page_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bandcamp URL has no track slug",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let tralbum = html_data_json_attribute(&html, "tralbum").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Bandcamp page has no tralbum JSON",
            )
        })?;
        let track_info = tralbum
            .get("trackinfo")
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Bandcamp page has no track information",
                )
            })?;
        let mut formats = Vec::new();
        if let Some(files) = track_info
            .get("file")
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, value) in files {
                let Some(raw_url) = value.as_str() else {
                    continue;
                };
                let Some((extension, bitrate)) = format_id.split_once('-') else {
                    continue;
                };
                let media_url = raw_url
                    .strip_prefix("//")
                    .map_or_else(|| raw_url.to_owned(), |url| format!("https://{url}"));
                let mut format = serde_json::json!({
                    "format_id": format_id,
                    "url": media_url,
                    "ext": extension,
                    "vcodec": "none",
                    "acodec": extension,
                });
                if let Ok(bitrate) = bitrate.parse::<i64>() {
                    format["abr"] = serde_json::json!(bitrate);
                }
                formats.push(format);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Bandcamp track {page_id} has no playable encodings"),
            ));
        }
        let embed = html_data_json_attribute(&html, "embed").unwrap_or(serde_json::Value::Null);
        let current = tralbum.get("current").unwrap_or(&serde_json::Value::Null);
        let track = json_string(track_info, "title").map(str::to_owned);
        let artist = json_string(&embed, "artist")
            .or_else(|| json_string(current, "artist"))
            .or_else(|| json_string(&tralbum, "artist"))
            .map(str::to_owned);
        let title = match (artist.as_deref(), track.as_deref()) {
            (Some(artist), Some(track)) => format!("{artist} - {track}"),
            (None, Some(track)) => track.to_owned(),
            (_, None) => page_id.to_owned(),
        };
        let track_id =
            json_value_string(track_info.get("track_id").or_else(|| track_info.get("id")))
                .or_else(|| json_value_string(tralbum.get("id")))
                .unwrap_or_else(|| page_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id.clone()));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp3")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("track", track);
        info.insert_if_some("artist", artist.clone());
        info.insert_if_some("uploader", artist);
        info.insert("uploader_id", serde_json::json!(uploader));
        info.insert(
            "uploader_url",
            serde_json::json!(format!("https://{uploader}.bandcamp.com")),
        );
        info.insert_if_some("album", json_string(&embed, "album_title"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("duration", json_f64(track_info, "duration"));
        info.insert_if_some("track_number", json_i64(track_info, "track_num"));
        info.insert("track_id", serde_json::json!(track_id));
        if let Ok(tag_matcher) = Regex::new(
            r#"(?is)<(?:a|span)\b[^>]*class\s*=\s*["'][^"']*\btag\b[^"']*["'][^>]*>(.*?)</(?:a|span)>"#,
        ) {
            let tags = tag_matcher
                .captures_iter(&html)
                .flatten()
                .filter_map(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|tag| !tag.is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                info.insert("tags", serde_json::Value::Array(tags));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

const BANNED_VIDEO_QUERY: &str = r#"
query GetVideoAndComments($id: String!) {
    getVideo(id: $id) {
        streamUrl
        directUrl
        unlisted
        live
        tags { name }
        title
        summary
        playCount
        largeImage
        videoDuration
        channel { _id title }
        createdAt
    }
    getVideoComments(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

const BANNED_COMMENT_REPLIES_QUERY: &str = r#"
query GetCommentReplies($id: String!) {
    getCommentReplies(id: $id, limit: 999999, offset: 0) {
        _id
        content
        user { _id username }
        voteCount { positive }
        createdAt
        replyCount
    }
}"#;

fn banned_video_call(
    context: &ExtractionContext,
    id: &str,
    operation: &str,
    query: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let response = native_post_json(
        context,
        "https://api.infowarsmedia.com/graphql",
        &serde_json::json!({
            "variables": {"id": id},
            "operationName": operation,
            "query": query,
        }),
    )?;
    response.get("data").cloned().ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "BannedVideo GraphQL response has no data",
        )
    })
}

fn banned_comment_value(comment: &serde_json::Value, parent: &str) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert(
        "id".to_owned(),
        json_value_string(comment.get("_id"))
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    value.insert(
        "text".to_owned(),
        json_string(comment, "content")
            .map_or(serde_json::Value::Null, |text| serde_json::json!(text)),
    );
    if let Some(user) = comment.get("user") {
        if let Some(username) = json_string(user, "username") {
            value.insert("author".to_owned(), serde_json::json!(username));
        }
        if let Some(user_id) = json_value_string(user.get("_id")) {
            value.insert("author_id".to_owned(), serde_json::json!(user_id));
        }
    }
    if let Some(timestamp) = comment.get("createdAt") {
        value.insert("timestamp".to_owned(), timestamp.clone());
    }
    value.insert("parent".to_owned(), serde_json::json!(parent));
    if let Some(likes) = comment
        .get("voteCount")
        .and_then(|votes| json_i64(votes, "positive"))
    {
        value.insert("like_count".to_owned(), serde_json::json!(likes));
    }
    serde_json::Value::Object(value)
}

/// Native BannedVideo GraphQL extractor. Metadata, media variants, and
/// available comments are fetched with typed Rust requests and no scripting
/// runtime.
pub struct BannedVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BannedVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BannedVideoExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "BannedVideo URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BannedVideo URL has no ID")
            })?;
        let data = banned_video_call(context, video_id, "GetVideoAndComments", BANNED_VIDEO_QUERY)?;
        let video = data.get("getVideo").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no video object",
            )
        })?;
        let mut formats = Vec::new();
        if let Some(media_url) = json_string(video, "directUrl") {
            formats.push(serde_json::json!({
                "format_id": "direct",
                "quality": 1,
                "url": media_url,
                "ext": "mp4",
                "protocol": "http",
            }));
        }
        if let Some(media_url) = json_string(video, "streamUrl") {
            formats.push(serde_json::json!({
                "format_id": "hls",
                "url": media_url,
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "BannedVideo response has no playable media URLs",
            ));
        }
        let title = json_string(video, "title")
            .map(|title| title.strip_suffix('.').unwrap_or(title).to_owned())
            .unwrap_or_else(|| video_id.to_owned());
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "is_live",
            video.get("live").and_then(serde_json::Value::as_bool),
        );
        info.insert_if_some("description", json_string(video, "summary"));
        if let Some(channel) = video.get("channel") {
            info.insert_if_some("channel", json_string(channel, "title"));
            info.insert_if_some("channel_id", json_value_string(channel.get("_id")));
        }
        info.insert_if_some("view_count", json_i64(video, "playCount"));
        info.insert_if_some("thumbnail", json_string(video, "largeImage"));
        info.insert_if_some("duration", json_f64(video, "videoDuration"));
        if let Some(tags) = video.get("tags").and_then(serde_json::Value::as_array) {
            let tags = tags
                .iter()
                .filter_map(|tag| json_string(tag, "name"))
                .map(|tag| serde_json::json!(tag))
                .collect::<Vec<_>>();
            info.insert("tags", serde_json::Value::Array(tags));
        }
        if let Some(comments) = data
            .get("getVideoComments")
            .and_then(serde_json::Value::as_array)
        {
            let mut all_comments = Vec::new();
            for comment in comments {
                let comment_id = json_value_string(comment.get("_id")).unwrap_or_default();
                all_comments.push(banned_comment_value(comment, "root"));
                if json_i64(comment, "replyCount").unwrap_or_default() > 0 && !comment_id.is_empty()
                {
                    if let Ok(reply_data) = banned_video_call(
                        context,
                        &comment_id,
                        "GetCommentReplies",
                        BANNED_COMMENT_REPLIES_QUERY,
                    ) {
                        if let Some(replies) = reply_data
                            .get("getCommentReplies")
                            .and_then(serde_json::Value::as_array)
                        {
                            all_comments.extend(
                                replies
                                    .iter()
                                    .map(|reply| banned_comment_value(reply, &comment_id)),
                            );
                        }
                    }
                }
            }
            info.insert("comments", serde_json::Value::Array(all_comments));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Coub API extractor. All media variants and counters are read from
/// the Coub JSON response and represented as ordinary Rust format records.
pub struct CoubExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CoubExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CoubExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Coub URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Coub URL has no ID")
            })?;
        let coub = context.get_json(&format!("http://coub.com/api/v2/coubs/{video_id}.json"))?;
        if let Some(error) = json_string(&coub, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("Coub API error: {error}"),
            ));
        }
        let file_versions = coub
            .get("file_versions")
            .and_then(serde_json::Value::as_object);
        let mut formats = Vec::new();
        if let Some(html5) = file_versions
            .and_then(|versions| versions.get("html5"))
            .and_then(serde_json::Value::as_object)
        {
            for (kind, media_type) in [("video", "video"), ("audio", "audio")] {
                let Some(qualities) = html5.get(kind).and_then(serde_json::Value::as_object) else {
                    continue;
                };
                for (quality, item) in qualities {
                    let Some(media_url) = item.get("url").and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let default_ext = if media_type == "audio" { "mp3" } else { "mp4" };
                    let ext = yt_dlp_core::determine_ext(Some(media_url), default_ext);
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format!("html5-{media_type}-{quality}"),
                        "ext": ext,
                        "quality": match quality.as_str() {
                            "low" => 0,
                            "med" => 1,
                            "high" => 2,
                            "higher" => 3,
                            _ => -1,
                        },
                        "vcodec": if media_type == "audio" { "none" } else { "unknown" },
                        "acodec": if media_type == "video" { "none" } else { "unknown" },
                    });
                    if let Some(size) = json_i64(item, "size") {
                        format["filesize"] = serde_json::json!(size);
                    }
                    formats.push(format);
                }
            }
        }
        if let Some(item) = file_versions
            .and_then(|versions| versions.get("iphone"))
            .and_then(serde_json::Value::as_object)
        {
            if let Some(media_url) = json_string(&serde_json::Value::Object(item.clone()), "url") {
                formats.push(serde_json::json!({
                    "url": media_url,
                    "format_id": "iphone",
                    "ext": yt_dlp_core::determine_ext(Some(media_url), "mp4"),
                }));
            }
        }
        if let Some(media_url) = file_versions
            .and_then(|versions| versions.get("mobile"))
            .and_then(|mobile| json_string(mobile, "audio_url"))
        {
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "mobile-audio",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp3"),
                "vcodec": "none",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Coub API returned no playable formats for {video_id}"),
            ));
        }
        let channel = coub.get("channel");
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&coub, "title"));
        info.insert_if_some("thumbnail", json_string(&coub, "picture"));
        info.insert_if_some("duration", json_f64(&coub, "duration"));
        info.insert_if_some(
            "timestamp",
            json_string(&coub, "published_at")
                .or_else(|| json_string(&coub, "created_at"))
                .and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some(
            "uploader",
            channel.and_then(|value| json_string(value, "title")),
        );
        info.insert_if_some(
            "uploader_id",
            channel.and_then(|value| json_string(value, "permalink")),
        );
        info.insert_if_some(
            "view_count",
            json_i64(&coub, "views_count").or_else(|| json_i64(&coub, "views_increase_count")),
        );
        info.insert_if_some("like_count", json_i64(&coub, "likes_count"));
        info.insert_if_some("repost_count", json_i64(&coub, "recoubs_count"));
        if let Some(age_restricted) = json_bool(&coub, "age_restricted")
            .or_else(|| json_bool(&coub, "age_restricted_by_admin"))
        {
            info.insert(
                "age_limit",
                serde_json::json!(if age_restricted { 18 } else { 0 }),
            );
        }
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Vocaroo direct-audio extractor. The media host is selected from the
/// ID shape and a Rust HEAD request preserves the upload timestamp header.
pub struct VocarooExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VocarooExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VocarooExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Vocaroo URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vocaroo URL has no ID")
            })?;
        let media_subdomain =
            if audio_id.len() == 10 || (audio_id.len() == 12 && audio_id.starts_with('1')) {
                "media1"
            } else {
                "media"
            };
        let media_url = format!("https://{media_subdomain}.vocaroo.com/mp3/{audio_id}");
        let mut request = Request::new(&media_url);
        request.set_method("HEAD").map_err(map_request_error)?;
        request.headers_mut().set("Referer", "https://vocaroo.com/");
        let response = context.request(&request)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(""));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "http_headers",
            serde_json::json!({"Referer": "https://vocaroo.com/"}),
        );
        if let Some(timestamp) = response
            .headers()
            .get("x-bz-upload-timestamp")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| value / 1000.0)
        {
            info.insert("timestamp", serde_json::json!(timestamp));
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Freesound HTML/Open Graph extractor. The page metadata is enough to
/// build the same low/high audio format set without browser execution.
pub struct FreesoundExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FreesoundExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FreesoundExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Freesound URL did not match its native pattern",
            )
        })?;
        let audio_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Freesound URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let audio_url = html_meta_value(&html, "og:audio").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Freesound page {audio_id} has no audio URL"),
            )
        })?;
        let audio_url = audio_url
            .strip_prefix("https://freesound.org")
            .filter(|value| value.starts_with("http"))
            .unwrap_or(&audio_url)
            .to_owned();
        let mut audio_urls = vec![audio_url.clone()];
        if audio_url.contains("-lq.mp3") {
            audio_urls.push(audio_url.replace("-lq.mp3", "-hq.mp3"));
        }
        let channels = Regex::new(r#"(?is)Channels\s*</dt>\s*<dd[^>]*>(.*?)</dd>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| {
                captures
                    .get(1)
                    .map(|value| html_text_fragment(value.as_str()))
            });
        let formats = audio_urls
            .into_iter()
            .enumerate()
            .map(|(quality, media_url)| {
                serde_json::json!({
                    "url": media_url,
                    "format_id": if quality == 0 { "lq" } else { "hq" },
                    "ext": "mp3",
                    "format_note": channels.as_deref(),
                    "quality": quality,
                    "vcodec": "none",
                })
            })
            .collect::<Vec<_>>();
        let duration =
            Regex::new(r#"(?is)class\s*=\s*["'][^"']*\bduration\b[^"']*["'][^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| value.as_str().trim().to_owned())
                })
                .and_then(|value| {
                    value
                        .parse::<f64>()
                        .map(|value| value / 1000.0)
                        .ok()
                        .or_else(|| yt_dlp_core::parse_duration(&value))
                });
        let description =
            Regex::new(r#"(?is)\bid\s*=\s*["']sound_description["'][^>]*>(.*?)</div>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                });
        let tags = Regex::new(r#"(?is)<a\b[^>]*>([^<]+)</a>"#)
            .ok()
            .and_then(|matcher| {
                let container = Regex::new(
                    r#"(?is)class\s*=\s*["'][^"']*\btags\b[^"']*["'][^>]*>(.*?)</(?:div|section)>"#,
                )
                .ok()?;
                let captures = container.captures(&html).ok().flatten()?;
                let body = captures.get(1)?.as_str();
                let values = matcher
                    .captures_iter(body)
                    .flatten()
                    .filter_map(|captures| {
                        captures
                            .get(1)
                            .map(|value| html_text_fragment(value.as_str()))
                    })
                    .filter(|tag| !tag.is_empty())
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();
                (!values.is_empty()).then_some(serde_json::Value::Array(values))
            });
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:audio:title").or_else(|| html_meta_value(&html, "og:title")),
        );
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("uploader", html_meta_value(&html, "og:audio:artist"));
        info.insert_if_some("tags", tags);
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Yandex Disk extractor. The page store, public download URL, and
/// server-provided video streams are consumed directly by Rust.
pub struct YandexDiskExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl YandexDiskExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for YandexDiskExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Yandex Disk URL did not match its native pattern",
            )
        })?;
        let mut video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Yandex Disk URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let store = html_script_json(&html, "store-prefetch")?;
        let resource_id = json_value_string(store.get("rootResourceId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Yandex Disk store has no root resource ID",
            )
        })?;
        let resource = store
            .get("resources")
            .and_then(serde_json::Value::as_object)
            .and_then(|resources| resources.get(&resource_id))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Yandex Disk store has no root resource",
                )
            })?;
        let title = json_string(resource, "name")
            .map(str::to_owned)
            .unwrap_or_else(|| video_id.clone());
        if let Some(public_key) = resource
            .get("meta")
            .and_then(|meta| json_string(meta, "short_url"))
        {
            if let Some(public_id) = self
                .matcher
                .captures(public_key)
                .ok()
                .flatten()
                .and_then(|captures| captures.name("id"))
                .map(|value| value.as_str().to_owned())
            {
                video_id = public_id;
            }
        }
        let meta = resource.get("meta").unwrap_or(&serde_json::Value::Null);
        let mut formats = Vec::new();
        let mut source_request =
            Request::new("https://cloud-api.yandex.net/v1/disk/public/resources/download");
        source_request.update_query(&[("public_key".to_owned(), url.to_owned())]);
        if let Ok(source) = context.request(&source_request) {
            if let Ok(source_json) = serde_json::from_slice::<serde_json::Value>(source.body()) {
                if let Some(source_url) = json_string(&source_json, "href") {
                    let ext = yt_dlp_core::determine_ext(
                        Some(&title),
                        json_string(meta, "ext")
                            .or_else(|| json_string(meta, "mime_type"))
                            .unwrap_or("mp4"),
                    );
                    formats.push(serde_json::json!({
                        "url": source_url,
                        "format_id": "source",
                        "ext": ext,
                        "quality": 1,
                        "filesize": json_i64(meta, "size"),
                    }));
                }
            }
        }
        if let Some(video_streams) = resource.get("videoStreams") {
            if let Some(videos) = video_streams
                .get("videos")
                .and_then(serde_json::Value::as_array)
            {
                for video in videos {
                    let Some(stream_url) = json_string(video, "url") else {
                        continue;
                    };
                    let size = video.get("size");
                    let height = json_i64(size.unwrap_or(&serde_json::Value::Null), "height");
                    let format_id =
                        height.map_or_else(|| "hls".to_owned(), |height| format!("hls-{height}p"));
                    formats.push(serde_json::json!({
                        "url": stream_url,
                        "format_id": format_id,
                        "ext": "mp4",
                        "height": height,
                        "width": json_i64(size.unwrap_or(&serde_json::Value::Null), "width"),
                        "protocol": "m3u8_native",
                    }));
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Yandex Disk resource {video_id} has no native media formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(duration) = video_streams_duration(resource) {
            info.insert("duration", serde_json::json!(duration));
        }
        if let Some(uid) = json_string(resource, "uid") {
            info.insert("uploader_id", serde_json::json!(uid));
            if let Some(display_name) = store
                .get("users")
                .and_then(serde_json::Value::as_object)
                .and_then(|users| users.get(uid))
                .and_then(|user| json_string(user, "displayName"))
            {
                info.insert("uploader", serde_json::json!(display_name));
            }
        }
        info.insert_if_some("view_count", json_i64(meta, "views_counter"));
        Ok(ExtractorResult::single(info))
    }
}

fn video_streams_duration(resource: &serde_json::Value) -> Option<f64> {
    resource
        .get("videoStreams")
        .and_then(|streams| json_f64(streams, "duration"))
        .map(|duration| duration / 1000.0)
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

/// Native Audius extractor. Host discovery, URL resolution, and stream URL
/// construction are performed through the Rust request context; the service's
/// JavaScript frontend is not needed.
pub struct AudiusExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiusExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Blerp GraphQL extractor. The query is intentionally limited to the
/// fields needed for a downloadable audio result, which keeps the Rust port
/// deterministic and avoids the web application's JavaScript bundle.
pub struct BlerpExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BlerpExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast episode extractor. Acast exposes episode metadata through a
/// small JSON endpoint, so the Rust port can preserve the audio result without
/// scraping or executing the embed player.
pub struct AcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast show/playlist extractor. Playlist entry construction is fully
/// native; selecting and downloading entries is kept as an explicit CLI TODO
/// until the playlist scheduler is ported.
pub struct AcastChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Dumpert JSON extractor. Media variants are represented as ordinary
/// Rust format records; HLS variants are handed to the native HLS downloader
/// by URL detection in the CLI.
pub struct DumpertExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DumpertExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiodraft entry extractor for contest URLs that already expose the
/// numeric entry ID. The custom-domain page-discovery variant remains an
/// explicit TODO because it requires a second HTML player parser.
pub struct AudiodraftExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiodraftExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiomack song extractor. The song endpoint provides a final media
/// URL and canonical metadata; wrapper URLs for another service are surfaced
/// as TODO instead of being delegated to a different runtime.
pub struct AudiomackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiomackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Aitube.kz extractor. The page's Next.js data and the service's HLS
/// endpoint are both consumed directly by Rust.
pub struct AitubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AitubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AitubeExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Aitube URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(name, _)| name == "id")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Aitube URL has no id query")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let next_data = html_script_json(&html, "__NEXT_DATA__")?;
        let video_info = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .and_then(|page_props| page_props.get("videoInfo"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Aitube page has no videoInfo data",
                )
            })?;
        let hls_url = format!(
            "https://api-http.aitube.kz/kz.aitudala.aitube.staticaccess/video/{video_id}/video"
        );
        let fallback_title = html_meta_value(&html, "og:title");
        let title = json_string(video_info, "title")
            .or(fallback_title.as_deref())
            .unwrap_or(&video_id)
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(hls_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": hls_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert_if_some("description", json_string(video_info, "description"));
        for (source, target) in [
            ("viewCount", "view_count"),
            ("likeCount", "like_count"),
            ("commentCount", "comment_count"),
            ("channelSubscriberCount", "channel_follower_count"),
        ] {
            if let Some(value) = video_info.get(source) {
                info.insert(target, value.clone());
            }
        }
        for (source, target) in [
            ("channelTitle", "channel"),
            ("channelId", "channel_id"),
            ("coverUrl", "thumbnail"),
        ] {
            info.insert_if_some(target, json_string(video_info, source));
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AudiomackExtractor {
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
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Audiomack URL: {error}"),
            )
        })?;
        let path = parsed.path().trim_matches('/');
        let song_tag = path
            .split_once("song/")
            .map(|(_, tag)| tag)
            .filter(|tag| !tag.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Audiomack URL has no song path",
                )
            })?;
        let mut request = Request::new(format!(
            "http://www.audiomack.com/api/music/url/song/{song_tag}"
        ));
        request.update_query(&[("extended".to_owned(), "1".to_owned())]);
        let response = context.get_json(request.url())?;
        let media_url = json_string(&response, "url")
            .filter(|url| !url.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audiomack API returned no song URL",
                )
            })?;
        if media_url.contains("soundcloud.com/") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                "TODO: native SoundCloud wrapper extraction is not implemented",
            ));
        }
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
        let id = json_value_string(response.get("id")).unwrap_or_else(|| {
            media_url
                .rsplit('/')
                .next()
                .unwrap_or(song_tag)
                .split('?')
                .next()
                .unwrap_or(song_tag)
                .trim_end_matches(&format!(".{ext}"))
                .to_owned()
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(id));
        info.insert_if_some("uploader", json_string(&response, "artist"));
        info.insert_if_some("title", json_string(&response, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AudiodraftExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Audiodraft URL did not match its native pattern",
            )
        })?;
        let entry_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Audiodraft URL has no ID")
            })?;
        let mut request =
            Request::new("https://www.audiodraft.com/scripts/general/player/getPlayerInfoNew.php");
        request.set_method("POST").map_err(map_request_error)?;
        request.headers_mut().set(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        );
        request
            .headers_mut()
            .set("X-Requested-With", "XMLHttpRequest");
        request.set_data(Some(format!("id=player_entry_{entry_id}").into_bytes()));
        let response = context.request(&request)?;
        let data: serde_json::Value = serde_json::from_slice(response.body()).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Audiodraft response: {error}"),
            )
        })?;
        let media_url = json_string(&data, "path").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audiodraft response has no media path",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_value_string(data.get("entry_id")).unwrap_or_else(|| entry_id.to_owned())
            ),
        );
        info.insert_if_some("title", json_string(&data, "entry_title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("uploader", json_string(&data, "designer_name"));
        info.insert_if_some("uploader_id", json_value_string(data.get("designer_id")));
        info.insert_if_some("webpage_url", json_string(&data, "entry_url"));
        info.insert_if_some("like_count", json_i64(&data, "entry_likes"));
        info.insert_if_some("average_rating", json_i64(&data, "entry_rating"));
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for DumpertExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Dumpert URL did not match its native pattern",
            )
        })?;
        let normalized_id = captures
            .name("id")
            .map(|value| value.as_str().replace('_', "/"))
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Dumpert URL has no ID")
            })?;
        let api_id = normalized_id.replace('/', "_");
        let response = context.get_json(&format!(
            "http://api-live.dumpert.nl/mobile_api/json/info/{api_id}"
        ))?;
        let item = response
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert API returned no item",
                )
            })?;
        let media = item
            .get("media")
            .and_then(serde_json::Value::as_array)
            .and_then(|media| {
                media.iter().find(|media| {
                    media.get("mediatype").and_then(serde_json::Value::as_str) == Some("VIDEO")
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Dumpert item has no VIDEO media",
                )
            })?;
        let formats = media
            .get("variants")
            .and_then(serde_json::Value::as_array)
            .map(|variants| {
                variants
                    .iter()
                    .filter_map(|variant| {
                        let url = variant.get("uri").and_then(serde_json::Value::as_str)?;
                        let version = variant
                            .get("version")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("source");
                        let detected_ext = yt_dlp_core::determine_ext(Some(url), "mp4");
                        let ext = if detected_ext == "m3u8" {
                            "mp4".to_owned()
                        } else {
                            detected_ext
                        };
                        Some(serde_json::json!({
                            "url": url,
                            "format_id": version,
                            "ext": ext,
                            "protocol": if url.split('?').next().is_some_and(|url| url.ends_with(".m3u8")) {
                                "m3u8_native"
                            } else {
                                "http"
                            },
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Dumpert media has no playable variants",
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(normalized_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some(
            "duration",
            media.get("duration").and_then(serde_json::Value::as_f64),
        );
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or(serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        if let Some(stills) = item.get("stills").and_then(serde_json::Value::as_object) {
            let thumbnails = stills
                .iter()
                .filter_map(|(id, value)| {
                    value
                        .as_str()
                        .map(|url| serde_json::json!({"id": id, "url": url}))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        if let Some(stats) = item.get("stats") {
            info.insert_if_some(
                "like_count",
                stats.get("kudos_total").and_then(|value| value.as_i64()),
            );
            info.insert_if_some(
                "view_count",
                stats.get("views_total").and_then(|value| value.as_i64()),
            );
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for AcastChannelExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Acast channel URL did not match its native pattern",
            )
        })?;
        let show_slug = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast show has no ID")
            })?;
        let show = context.get_json(&format!(
            "https://feeder.acast.com/api/v1/shows/{show_slug}"
        ))?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&show, "id").unwrap_or(show_slug)),
        );
        info.insert_if_some("title", json_string(&show, "title"));
        info.insert_if_some("description", json_string(&show, "description"));
        let show_info = show
            .as_object()
            .map(|show| {
                serde_json::json!({
                    "creator": show.get("author"),
                    "series": show.get("title"),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let entries = show
            .get("episodes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Acast show response has no episodes array",
                )
            })?
            .iter()
            .filter_map(|episode| {
                let media_url = json_string(episode, "url")?;
                let episode_id =
                    json_string(episode, "id").or_else(|| json_string(episode, "episodeUrl"))?;
                let title = json_string(episode, "title").unwrap_or(episode_id);
                let ext = yt_dlp_core::determine_ext(Some(media_url), "mp3");
                let mut entry = InfoDict::new();
                entry.insert("id", serde_json::json!(episode_id));
                entry.insert("title", serde_json::json!(title));
                entry.insert("url", serde_json::json!(media_url));
                entry.insert("ext", serde_json::json!(ext.clone()));
                entry.insert(
                    "formats",
                    serde_json::json!([{
                        "url": media_url,
                        "format_id": "audio",
                        "ext": ext,
                        "vcodec": "none",
                    }]),
                );
                entry.insert_if_some("description", json_string(episode, "description"));
                entry.insert_if_some("thumbnail", json_string(episode, "image"));
                if let Some(value) = episode.get("duration").and_then(|value| value.as_f64()) {
                    entry.insert("duration", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("creator").and_then(|value| value.as_str()) {
                    entry.insert("creator", serde_json::json!(value));
                }
                if let Some(value) = show_info.get("series").and_then(|value| value.as_str()) {
                    entry.insert("series", serde_json::json!(value));
                }
                Some(entry)
            })
            .collect::<Vec<_>>();
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

impl InfoExtractor for AcastExtractor {
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
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Acast URL did not match its native pattern",
            )
        })?;
        let channel = captures
            .name("channel")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Acast URL has no channel")
            })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Acast URL has no episode ID",
                )
            })?;
        let mut api_request = Request::new(format!(
            "https://feeder.acast.com/api/v1/shows/{channel}/episodes/{display_id}"
        ));
        api_request.update_query(&[("showInfo".to_owned(), "true".to_owned())]);
        let episode = context.get_json(api_request.url())?;
        let episode_url = json_string(&episode, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Acast episode has no media URL",
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(episode_url), "mp3");
        let title = json_string(&episode, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| display_id.to_owned());
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(&episode, "id").unwrap_or(display_id)),
        );
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("episode", serde_json::json!(title));
        info.insert("url", serde_json::json!(episode_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": episode_url,
                "format_id": "audio",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        info.insert_if_some("description", json_string(&episode, "description"));
        info.insert_if_some("thumbnail", json_string(&episode, "image"));
        info.insert_if_some("duration", json_f64(&episode, "duration"));
        info.insert_if_some("filesize", json_f64(&episode, "contentLength"));
        if let Some(show) = episode.get("show") {
            info.insert_if_some("creator", json_string(show, "author"));
            info.insert_if_some("series", json_string(show, "title"));
        }
        for (source, target) in [("season", "season_number"), ("episode", "episode_number")] {
            if let Some(value) = episode.get(source).and_then(|value| value.as_i64()) {
                info.insert(target, serde_json::json!(value));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

impl InfoExtractor for BlerpExtractor {
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
        let payload = serde_json::json!({
            "operationName": "webBitePageGetBite",
            "variables": {"_id": audio_id},
            "query": "query webBitePageGetBite($_id: MongoID!) { web { biteById(_id: $_id) { _id title userKeywords ownerObject { _id username } audio { mp3 { url } } } } }",
        });
        let mut request = Request::new("https://api.blerp.com/graphql");
        request.set_method("POST").map_err(map_request_error)?;
        request
            .headers_mut()
            .set("Content-Type", "application/json");
        request.set_data(Some(serde_json::to_vec(&payload).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("could not encode Blerp GraphQL request: {error}"),
            )
        })?));
        let response = context.request(&request)?;
        let response: serde_json::Value =
            serde_json::from_slice(response.body()).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Blerp GraphQL response: {error}"),
                )
            })?;
        let bite = response
            .get("data")
            .and_then(|data| data.get("web"))
            .and_then(|web| web.get("biteById"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp GraphQL response has no bite",
                )
            })?;
        let media_url = bite
            .get("audio")
            .and_then(|audio| audio.get("mp3"))
            .and_then(|mp3| mp3.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Blerp response has no MP3 URL",
                )
            })?;
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(json_string(bite, "_id").unwrap_or(&audio_id)),
        );
        info.insert(
            "title",
            serde_json::json!(json_string(bite, "title").unwrap_or(&audio_id)),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "mp3",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        if let Some(owner) = bite.get("ownerObject") {
            info.insert_if_some("uploader", json_string(owner, "username"));
            info.insert_if_some("uploader_id", json_string(owner, "_id"));
        }
        if let Some(tags) = bite
            .get("userKeywords")
            .and_then(serde_json::Value::as_array)
        {
            info.insert("tags", serde_json::Value::Array(tags.clone()));
        }
        Ok(ExtractorResult::single(info))
    }
}

fn audius_data<'a>(
    response: &'a serde_json::Value,
) -> Result<&'a serde_json::Value, ExtractorError> {
    response.get("data").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Audius API response has no data field",
        )
    })
}

fn json_value_string(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .unwrap_or_else(|| value.to_string())
    })
}

impl InfoExtractor for AudiusExtractor {
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
        let hosts_response = context.get_json("https://api.audius.co/")?;
        let hosts = audius_data(&hosts_response)?;
        let host = hosts
            .as_array()
            .and_then(|hosts| hosts.iter().find_map(|host| host.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Audius host discovery returned no API hosts",
                )
            })?
            .trim_end_matches('/')
            .to_owned();
        let track_response = if self.descriptor.key == "AudiusTrackIE" {
            let track_id = url
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::InvalidUrl,
                        "Audius track URL has no track ID",
                    )
                })?;
            context.get_json(&format!("{host}/v1/tracks/{track_id}"))?
        } else {
            let mut resolve_request = Request::new(format!("{host}/v1/resolve"));
            resolve_request.update_query(&[("url".to_owned(), url.to_owned())]);
            context.get_json(resolve_request.url())?
        };
        let track_data = audius_data(&track_response)?;
        let track_id = json_value_string(track_data.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Audius response has no track ID",
            )
        })?;
        let title = json_string(track_data, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| track_id.clone());
        let stream_url = format!("{host}/v1/tracks/{track_id}/stream");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("track", serde_json::json!(title));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "stream",
                "ext": "mp3",
                "vcodec": "none",
                "acodec": "mp3",
            }]),
        );
        info.insert_if_some("description", json_string(track_data, "description"));
        info.insert_if_some("duration", json_f64(track_data, "duration"));
        info.insert_if_some("genre", json_string(track_data, "genre"));
        for (name, source) in [
            ("view_count", "play_count"),
            ("like_count", "favorite_count"),
            ("repost_count", "repost_count"),
        ] {
            if let Some(value) = track_data.get(source) {
                info.insert(name, value.clone());
            }
        }
        if let Some(artist) = track_data
            .get("user")
            .and_then(|user| user.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            info.insert("artist", serde_json::json!(artist));
        }
        if let Some(artwork) = track_data
            .get("artwork")
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = artwork
                .iter()
                .filter_map(|(quality, value)| {
                    value.as_str().map(|url| {
                        serde_json::json!({
                            "id": quality,
                            "url": url,
                        })
                    })
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
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
            } else if descriptor.key == "ArchiveOrgIE" {
                registry.register(ArchiveOrgExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BandcampIE" {
                registry.register(BandcampTrackExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BannedVideoIE" {
                registry.register(BannedVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CoubIE" {
                registry.register(CoubExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoogleDriveIE" {
                registry.register(GoogleDriveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VocarooIE" {
                registry.register(VocarooExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreesoundIE" {
                registry.register(FreesoundExtractor::new(descriptor)?)?;
            } else if descriptor.key == "YandexDiskIE" {
                registry.register(YandexDiskExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudioBoomIE" {
                registry.register(AudioBoomExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BitChuteIE" {
                registry.register(BitChuteExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ClypIE" {
                registry.register(ClypExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BreitBartIE" {
                registry.register(BreitbartExtractor::new(descriptor)?)?;
            } else if matches!(descriptor.key.as_str(), "AudiusIE" | "AudiusTrackIE") {
                registry.register(AudiusExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BlerpIE" {
                registry.register(BlerpExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ACastIE" {
                registry.register(AcastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ACastChannelIE" {
                registry.register(AcastChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DumpertIE" {
                registry.register(DumpertExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudiodraftGenericIE" {
                registry.register(AudiodraftExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudiomackIE" {
                registry.register(AudiomackExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AitubeKZVideoIE" {
                registry.register(AitubeExtractor::new(descriptor)?)?;
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
    use yt_dlp_networking::{ErrorKind, RequestHandler};

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

    struct RoutedHandler {
        routes: Vec<(String, Vec<u8>)>,
    }

    impl RequestHandler for RoutedHandler {
        fn name(&self) -> &str {
            "extractor-route-test"
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            Ok(())
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            let body = self
                .routes
                .iter()
                .find(|(needle, _)| request.url().contains(needle))
                .map(|(_, body)| body.clone())
                .ok_or_else(|| {
                    RequestError::new(
                        ErrorKind::Transport,
                        format!("no test route for {}", request.url()),
                    )
                })?;
            Ok(Response::new(request.url(), 200, "OK", body))
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
        assert_eq!(registry.native_implementation_count(), 22);
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
    fn audioboom_native_extractor_reads_embedded_clip_store() {
        let extractor = AudioBoomExtractor::new(ExtractorDescriptor::new(
            "AudioBoomIE",
            "AudioBoom",
            r"https?://(?:www\.)?audioboom\.com/(?:boos|posts)/(?P<id>[0-9]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:description" content="fallback description">
                <meta property="weibo:audio:duration" content="12.5">
            </head><body>
                <div data-react-class="V5DetailPagePlayer"
                  data-react-props="{&quot;clips&quot;:[{&quot;clipURLPriorToLoading&quot;:&quot;https://cdn.example/audio.mp3&quot;,&quot;title&quot;:&quot;Native audio&quot;,&quot;description&quot;:&quot;Clip description&quot;,&quot;duration&quot;:12.25,&quot;author&quot;:&quot;Native host&quot;}]}"></div>
            </body></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://audioboom.com/posts/12345-title", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("12345"));
        assert_eq!(result.get_str("title"), Some("Native audio"));
        assert_eq!(result.get_str("url"), Some("https://cdn.example/audio.mp3"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(12.25)));
        assert_eq!(result.get_str("uploader"), Some("Native host"));
    }

    #[test]
    fn bitchute_native_extractor_reads_media_and_metadata_apis() {
        let extractor = BitChuteExtractor::new(ExtractorDescriptor::new(
            "BitChuteIE",
            "BitChute",
            r"https?://(?:(?:www|old)\.)?bitchute\.com/(?:video|embed|torrent/[^/?#]+)/(?P<id>[^/?#&]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "video/media".to_owned(),
                    br#"{"media_url":"https://cdn.example/video.mp4"}"#.to_vec(),
                ),
                (
                    "api/beta/video".to_owned(),
                    br#"{
                        "video_name":"Native BitChute",
                        "description":"Description",
                        "thumbnail_url":"https://cdn.example/thumb.jpg",
                        "view_count":7,
                        "duration":"00:00:16",
                        "hashtags":["bitchute"],
                        "profile_id":"profile1",
                        "channel":{"channel_id":"channel1","channel_name":"Channel"}
                    }"#
                    .to_vec(),
                ),
                (
                    "api/beta/channel".to_owned(),
                    br#"{
                        "profile_name":"Native uploader",
                        "profile_id":"profile1",
                        "channel_name":"Channel",
                        "url_slug":"channel"
                    }"#
                    .to_vec(),
                ),
            ],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://www.bitchute.com/video/abc123/", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("abc123"));
        assert_eq!(result.get_str("title"), Some("Native BitChute"));
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(16.0)));
        assert_eq!(result.get_str("uploader"), Some("Native uploader"));
        assert_eq!(
            result.get_str("channel_url"),
            Some("https://www.bitchute.com/channel/channel/")
        );
    }

    #[test]
    fn archive_org_native_extractor_maps_metadata_files_and_entry_selection() {
        let extractor = ArchiveOrgExtractor::new(ExtractorDescriptor::new(
            "ArchiveOrgIE",
            "archive.org",
            r"https?://(?:www\.)?archive\.org/(?:details|embed)/(?P<id>[^?#]+)(?:[?].*)?$",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"{
                "metadata": {
                    "identifier":"demo-item",
                    "title":"Demo archive",
                    "description":"A native archive",
                    "creator":"Archive author",
                    "uploader":"uploader@example.test",
                    "licenseurl":"https://creativecommons.org/publicdomain/zero/1.0/"
                },
                "files": [{
                    "name":"sample video.mp4",
                    "title":"Sample video",
                    "format":"MPEG4",
                    "size":"42",
                    "length":"00:01:02.5",
                    "source":"original"
                }]
            }"#
            .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://archive.org/details/demo-item/sample%20video.mp4",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("demo-item/sample video.mp4"));
        assert_eq!(result.get_str("title"), Some("Sample video"));
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(62.5)));
        assert_eq!(
            result.get_str("url"),
            Some("https://archive.org/download/demo-item/sample%20video.mp4")
        );
        assert_eq!(result.get_str("uploader"), Some("uploader@example.test"));
    }

    #[test]
    fn google_drive_native_extractor_maps_playback_transcodes() {
        let extractor = GoogleDriveExtractor::new(ExtractorDescriptor::new(
            "GoogleDriveIE",
            "GoogleDrive",
            r#"(?x)https?://(?:docs|drive|drive\.usercontent)\.google\.com/(?:file/d/|(?:uc|open|download)\?.*?id=)(?P<id>[a-zA-Z0-9_-]{28,})"#,
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"{
                "mediaMetadata":{"title":"drive video.mp4","duration":9.5},
                "mediaStreamingData":{"formatStreamingData":{
                    "adaptiveTranscodes":[{
                        "url":"https://cdn.example/drive.mp4",
                        "itag":18,
                        "transcodeMetadata":{
                            "mimeType":"video/mp4",
                            "width":640,
                            "height":360,
                            "videoFps":30,
                            "contentLength":"42",
                            "videoCodecString":"h264",
                            "audioCodecString":"aac"
                        }
                    }]
                }},
                "thumbnails":[{"url":"https://cdn.example/thumb.jpg","width":640,"height":360}]
            }"#
            .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://drive.google.com/file/d/0ByeS4oOUV-49Zzh4R1J6R09zazQ/view",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("0ByeS4oOUV-49Zzh4R1J6R09zazQ"));
        assert_eq!(result.get_str("title"), Some("drive video.mp4"));
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(9.5)));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn bandcamp_native_extractor_reads_track_json_attributes() {
        let extractor = BandcampTrackExtractor::new(ExtractorDescriptor::new(
            "BandcampIE",
            "Bandcamp",
            r"https?://(?P<uploader>[^/]+)\.bandcamp\.com/track/(?P<id>[^/?#&]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:image" content="https://cdn.example/art.jpg">
            </head><body>
                <div data-tralbum='{"id":12345,"artist":"Native Artist","current":{"artist":"Native Artist"},"trackinfo":[{"id":12345,"title":"Native Track","duration":42.5,"track_num":2,"file":{"mp3-128":"//cdn.example/track.mp3","flac-999":"https://cdn.example/track.flac"}}]}' data-embed='{"artist":"Native Artist","album_title":"Native Album"}'></div>
                <a class="tag">ambient</a>
            </body></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://artist.bandcamp.com/track/native-track", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("12345"));
        assert_eq!(
            result.get_str("title"),
            Some("Native Artist - Native Track")
        );
        assert_eq!(result.get_str("album"), Some("Native Album"));
        assert_eq!(result.get_str("ext"), Some("flac"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(42.5)));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn banned_video_native_extractor_reads_graphql_metadata_and_comments() {
        let extractor = BannedVideoExtractor::new(ExtractorDescriptor::new(
            "BannedVideoIE",
            "BannedVideo",
            r"https?://(?:www\.)?banned\.video/watch\?id=(?P<id>[0-f]{24})",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"{
                "data":{
                    "getVideo":{
                        "directUrl":"https://cdn.example/video.mp4",
                        "streamUrl":"https://cdn.example/master.m3u8",
                        "live":false,
                        "title":"Native title.",
                        "summary":"Summary",
                        "playCount":12,
                        "largeImage":"https://cdn.example/thumb.jpg",
                        "videoDuration":30.5,
                        "channel":{"_id":"channel1","title":"Channel"},
                        "tags":[{"name":"news"}]
                    },
                    "getVideoComments":[{
                        "_id":"comment1",
                        "content":"Hello",
                        "user":{"_id":"user1","username":"commenter"},
                        "voteCount":{"positive":3},
                        "replyCount":0
                    }]
                }
            }"#
            .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://banned.video/watch?id=5e7a859644e02200c6ef5f11",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("5e7a859644e02200c6ef5f11"));
        assert_eq!(result.get_str("title"), Some("Native title"));
        assert_eq!(result.get_str("channel"), Some("Channel"));
        assert_eq!(result.get("view_count"), Some(&serde_json::json!(12)));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            result
                .get("comments")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn coub_native_extractor_maps_api_versions_and_counters() {
        let extractor = CoubExtractor::new(ExtractorDescriptor::new(
            "CoubIE",
            "Coub",
            r#"(?:coub:|https?://(?:coub\.com/(?:view|embed|coubs)/|c-cdn\.coub\.com/fb-player\.swf\?.*\bcoub(?:ID|id)=))(?P<id>[\da-z]+)"#,
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"{
                "title":"Native Coub",
                "picture":"https://cdn.example/coub.jpg",
                "duration":4.6,
                "published_at":"2015-04-08T00:00:00Z",
                "views_count":10,
                "likes_count":4,
                "recoubs_count":2,
                "age_restricted":false,
                "channel":{"title":"Native uploader","permalink":"native.uploader"},
                "file_versions":{
                    "html5":{
                        "video":{"low":{"url":"https://cdn.example/low.mp4","size":100}},
                        "audio":{"high":{"url":"https://cdn.example/high.mp3","size":20}}
                    },
                    "iphone":{"url":"https://cdn.example/iphone.mp4"},
                    "mobile":{"audio_url":"https://cdn.example/mobile.mp3"}
                }
            }"#
            .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("http://coub.com/view/5u5n1", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("5u5n1"));
        assert_eq!(result.get_str("title"), Some("Native Coub"));
        assert_eq!(
            result.get("timestamp"),
            Some(&serde_json::json!(1_428_451_200))
        );
        assert_eq!(result.get("age_limit"), Some(&serde_json::json!(0)));
        assert_eq!(result.get_str("uploader_id"), Some("native.uploader"));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(4)
        );
    }

    #[test]
    fn vocaroo_native_extractor_builds_head_checked_audio_url() {
        let extractor = VocarooExtractor::new(ExtractorDescriptor::new(
            "VocarooIE",
            "Vocaroo",
            r"https?://(?:www\.)?(?:vocaroo\.com|voca\.ro)/(?:embed/)?(?P<id>\w+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler { body: Vec::new() });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://vocaroo.com/1de8yA3LNe77", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("1de8yA3LNe77"));
        assert_eq!(
            result.get_str("url"),
            Some("https://media1.vocaroo.com/mp3/1de8yA3LNe77")
        );
        assert_eq!(result.get_str("ext"), Some("mp3"));
        assert_eq!(result.get_str("vcodec"), Some("none"));
    }

    #[test]
    fn freesound_native_extractor_maps_html_audio_metadata() {
        let extractor = FreesoundExtractor::new(ExtractorDescriptor::new(
            "FreesoundIE",
            "Freesound",
            r"https?://(?:www\.)?freesound\.org/people/[^/]+/sounds/(?P<id>[^/]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head>
                <meta property="og:audio" content="https://freesound.orghttps://cdn.example/sound-lq.mp3">
                <meta property="og:audio:title" content="Native sound">
                <meta property="og:audio:artist" content="Native artist">
            </head><body>
                <div id="sound_description"><p>Description</p></div>
                <span class="duration">12500</span>
                <div class="tags"><a href="/tag/one">one</a><a href="/tag/two">two</a></div>
            </body></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://www.freesound.org/people/native/sounds/12345/",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("12345"));
        assert_eq!(result.get_str("title"), Some("Native sound"));
        assert_eq!(result.get_str("uploader"), Some("Native artist"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn yandex_disk_native_extractor_reads_store_and_public_media() {
        let extractor = YandexDiskExtractor::new(ExtractorDescriptor::new(
            "YandexDiskIE",
            "YandexDisk",
            r#"(?x)https?://(?P<domain>yadi\.sk|disk\.(?:360\.)?yandex\.(?:ru|com))/(?:[di]/|public.*?\bhash=)(?P<id>[^/?#&]+)"#,
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "cloud-api.yandex.net".to_owned(),
                    br#"{"href":"https://cdn.example/source.mp4"}"#.to_vec(),
                ),
                (
                    "yadi.sk".to_owned(),
                    br#"<script id="store-prefetch">{
                        "rootResourceId":"r1",
                        "resources":{"r1":{
                            "name":"native.mp4",
                            "uid":"u1",
                            "meta":{"ext":"mp4","size":"42","views_counter":"7"},
                            "videoStreams":{
                                "duration":12500,
                                "videos":[{
                                    "url":"https://cdn.example/stream.m3u8",
                                    "dimension":"720p",
                                    "size":{"width":1280,"height":720}
                                }]
                            }
                        }},
                        "users":{"u1":{"displayName":"Native user"}}
                    }</script>"#
                        .to_vec(),
                ),
            ],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://yadi.sk/i/VdOeDou8eZs6Y", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("VdOeDou8eZs6Y"));
        assert_eq!(result.get_str("title"), Some("native.mp4"));
        assert_eq!(result.get_str("uploader"), Some("Native user"));
        assert_eq!(result.get("duration"), Some(&serde_json::json!(12.5)));
        assert_eq!(
            result
                .get("formats")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
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

    #[test]
    fn audius_native_extractor_resolves_track_and_builds_stream_url() {
        let extractor = AudiusExtractor::new(ExtractorDescriptor::new(
            "AudiusIE",
            "Audius",
            r"(?x)https?://(?:www\.)?(?:audius\.co/(?P<uploader>[\w\d-]+)(?!/album|/playlist)/(?P<title>\S+))",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![
                (
                    "api.audius.co/".to_owned(),
                    br#"{"data":["https://api.audius.test"]}"#.to_vec(),
                ),
                (
                    "/v1/resolve?".to_owned(),
                    br#"{
                        "data": {
                            "id": "track1",
                            "title": "Native track",
                            "description": "Description",
                            "duration": 30,
                            "genre": "Electronic",
                            "play_count": 4,
                            "favorite_count": 2,
                            "repost_count": 1,
                            "user": {"name": "artist"},
                            "artwork": {"150x150": "https://cdn.example/art.jpg"}
                        }
                    }"#
                    .to_vec(),
                ),
            ],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://audius.co/artist/native-track", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("track1"));
        assert_eq!(result.get_str("artist"), Some("artist"));
        assert_eq!(
            result.get_str("url"),
            Some("https://api.audius.test/v1/tracks/track1/stream")
        );
        assert_eq!(result.get("view_count"), Some(&serde_json::json!(4)));
    }

    #[test]
    fn blerp_native_extractor_reads_graphql_audio_result() {
        let extractor = BlerpExtractor::new(ExtractorDescriptor::new(
            "BlerpIE",
            "blerp",
            r"https?://(?:www\.)?blerp\.com/soundbites/(?P<id>[0-9a-zA-Z]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "api.blerp.com/graphql".to_owned(),
                br#"{
                    "data": {
                        "web": {
                            "biteById": {
                                "_id": "bite1",
                                "title": "Native sound",
                                "userKeywords": ["native", "rust"],
                                "ownerObject": {"_id": "user1", "username": "tester"},
                                "audio": {"mp3": {"url": "https://cdn.example/bite.mp3"}}
                            }
                        }
                    }
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://blerp.com/soundbites/6320fe8745636cb4dd677a5a",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("bite1"));
        assert_eq!(result.get_str("uploader"), Some("tester"));
        assert_eq!(result.get_str("url"), Some("https://cdn.example/bite.mp3"));
        assert_eq!(
            result
                .get("tags")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn acast_native_extractor_maps_episode_api_response() {
        let extractor = AcastExtractor::new(ExtractorDescriptor::new(
            "ACastIE",
            "acast",
            r#"(?x:https?://(?:(?:(?:embed|www|shows)\.)?acast\.com/|play\.acast\.com/s/)(?P<channel>[^/?#]+)/(?:episodes/)?(?P<id>[^/#?"]+))"#,
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "feeder.acast.com/api/v1/shows".to_owned(),
                br#"{
                    "id": "episode1",
                    "episodeUrl": "episode-slug",
                    "url": "https://cdn.example/episode.mp3",
                    "title": "Native episode",
                    "description": "Description",
                    "duration": 120,
                    "show": {"author": "Creator", "title": "Series"},
                    "season": 2,
                    "episode": 4
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://shows.acast.com/channel/episodes/episode-slug",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("episode1"));
        assert_eq!(result.get_str("series"), Some("Series"));
        assert_eq!(result.get_str("creator"), Some("Creator"));
        assert_eq!(result.get("episode_number"), Some(&serde_json::json!(4)));
    }

    #[test]
    fn acast_channel_native_extractor_builds_playlist_entries() {
        let extractor = AcastChannelExtractor::new(ExtractorDescriptor::new(
            "ACastChannelIE",
            "acast:channel",
            r"(?x)https?://(?:(?:(?:www|shows)\.)?acast\.com/|play\.acast\.com/s/)(?P<id>[^/#?]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "feeder.acast.com/api/v1/shows".to_owned(),
                br#"{
                    "id": "show1",
                    "title": "Native show",
                    "description": "Show description",
                    "author": "Creator",
                    "episodes": [
                        {"id": "episode1", "title": "One", "url": "https://cdn.example/one.mp3"},
                        {"id": "episode2", "title": "Two", "url": "https://cdn.example/two.mp3"}
                    ]
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://shows.acast.com/native-show", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("_type"), Some("playlist"));
        assert_eq!(result.get_str("title"), Some("Native show"));
        assert_eq!(
            result
                .get("entries")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn dumpert_native_extractor_maps_variants_and_stats() {
        let extractor = DumpertExtractor::new(ExtractorDescriptor::new(
            "DumpertIE",
            "Dumpert",
            r"(?x)(?P<protocol>https?)://(?:(?:www|legacy)\.)?dumpert\.nl/(?:item/)(?P<id>[0-9]+[/_][0-9a-zA-Z]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "api-live.dumpert.nl/mobile_api/json/info".to_owned(),
                br#"{
                    "items": [{
                        "title": "Native Dumpert",
                        "description": "Description",
                        "media": [{
                            "mediatype": "VIDEO",
                            "duration": 9,
                            "variants": [
                                {"version": "mobile", "uri": "https://cdn.example/mobile.mp4"},
                                {"version": "hls", "uri": "https://cdn.example/master.m3u8"}
                            ]
                        }],
                        "stills": {"thumb": "https://cdn.example/thumb.jpg"},
                        "stats": {"kudos_total": 3, "views_total": 10}
                    }]
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context("https://www.dumpert.nl/item/6646981_951bc60f", &context)
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("6646981/951bc60f"));
        assert_eq!(result.get_str("title"), Some("Native Dumpert"));
        assert_eq!(result.get("view_count"), Some(&serde_json::json!(10)));
        assert_eq!(
            result
                .get("formats")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn audiodraft_native_extractor_posts_entry_lookup() {
        let extractor = AudiodraftExtractor::new(ExtractorDescriptor::new(
            "AudiodraftGenericIE",
            "Audiodraft:generic",
            r"https?://www\.audiodraft\.com/contests/[^/#]+#entries&eid=(?P<id>\d+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "audiodraft.com/scripts/general/player/getPlayerInfoNew.php".to_owned(),
                br#"{
                    "entry_id": 30138,
                    "entry_title": "Native sound",
                    "path": "https://cdn.example/sound.mp3",
                    "designer_name": "tester",
                    "designer_id": 19452,
                    "entry_url": "https://www.audiodraft.com/entry/30138",
                    "entry_likes": 7,
                    "entry_rating": 5
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://www.audiodraft.com/contests/contest#entries&eid=30138",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("30138"));
        assert_eq!(result.get_str("uploader"), Some("tester"));
        assert_eq!(result.get("average_rating"), Some(&serde_json::json!(5)));
    }

    #[test]
    fn audiomack_native_extractor_maps_song_api_response() {
        let extractor = AudiomackExtractor::new(ExtractorDescriptor::new(
            "AudiomackIE",
            "audiomack",
            r"https?://(?:www\.)?audiomack\.com/(?:song/|(?=.+/song/))(?P<id>[\w/-]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(RoutedHandler {
            routes: vec![(
                "audiomack.com/api/music/url/song".to_owned(),
                br#"{
                    "id": 310086,
                    "artist": "Native artist",
                    "title": "Native song",
                    "url": "https://cdn.example/song.mp3"
                }"#
                .to_vec(),
            )],
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://www.audiomack.com/song/native-artist/native-song",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("id"), Some("310086"));
        assert_eq!(result.get_str("uploader"), Some("Native artist"));
        assert_eq!(result.get_str("ext"), Some("mp3"));
    }

    #[test]
    fn aitube_native_extractor_reads_next_data_and_hls_result() {
        let extractor = AitubeExtractor::new(ExtractorDescriptor::new(
            "AitubeKZVideoIE",
            "AitubeKZVideo",
            r"https?://aitube\.kz/(?:video|embed/)\?(?:[^\?]+)?id=(?P<id>[\w-]+)",
            true,
        ))
        .unwrap();
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            body: br#"<html><head></head><body>
                <script id="__NEXT_DATA__" type="application/json">{
                    "props": {"pageProps": {"videoInfo": {
                        "title": "Native Aitube",
                        "description": "Description",
                        "viewCount": 12,
                        "channelTitle": "Channel",
                        "channelId": "channel1",
                        "coverUrl": "https://cdn.example/cover.jpg"
                    }}}
                }</script>
            </body></html>"#
                .to_vec(),
        });
        let context = ExtractionContext::new(director, CookieJar::new().shared());
        let result = extractor
            .extract_with_context(
                "https://aitube.kz/video?id=9291d29b-c038-49a1-ad42-3da2051d353c",
                &context,
            )
            .unwrap()
            .into_info_dict();

        assert_eq!(result.get_str("title"), Some("Native Aitube"));
        assert_eq!(result.get_str("channel"), Some("Channel"));
        assert_eq!(result.get_str("ext"), Some("mp4"));
        assert!(
            result
                .get_str("url")
                .is_some_and(|url| url.ends_with("/video"))
        );
    }
}
