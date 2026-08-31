//! Ordered extractor registry for the Rust migration.
//!
//! Descriptors can be registered before their implementation is ported. Such
//! entries remain visible and return an explicit compatibility error, which
//! prevents a missing extractor from being mistaken for a generic match.

use fancy_regex::Regex;
use yt_dlp_core::InfoDict;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractorErrorKind {
    InvalidUrl,
    Unsupported,
    Extraction,
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

fn compile_python_pattern(pattern: &str) -> Result<Regex, fancy_regex::Error> {
    match Regex::new(pattern) {
        Ok(matcher) => Ok(matcher),
        Err(error) => {
            // Python accepts an unescaped `[` inside a character class in a
            // legacy-compatible way. Rust's regex parser requires it to be
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
            matchers.push(compile_python_pattern(pattern).map_err(|error| {
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
            match compile_python_pattern(pattern) {
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

    /// Load the ordered extractor inventory generated from Python's
    /// `gen_extractors()` registry. Patterns that use Python-only regular
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

        // Refresh this snapshot whenever the Python extractor registry is
        // intentionally regenerated.
        assert_eq!(registry.len(), 1_752);
        assert!(registry.native_matchable_count() > 1_000);
        assert!(registry.native_pattern_count() > 1_000);
        assert_eq!(registry.pattern_error_count(), 0);
        assert_eq!(
            registry.iter().last().unwrap().descriptor().key,
            "GenericIE"
        );
        assert_eq!(registry.native_implementation_count(), 1);
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
}
