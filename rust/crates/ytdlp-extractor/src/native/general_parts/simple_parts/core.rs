use fancy_regex::Regex;
use yt_dlp_core::InfoDict;
use yt_dlp_networking::Request;

use super::common::*;
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
