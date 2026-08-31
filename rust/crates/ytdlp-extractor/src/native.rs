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

/// Native Ku6 page/API extractor. Ku6 publishes the page title in the HTML
/// document and the playable F4V URL in a small JSON endpoint; both are
/// consumed directly through the Rust request context.
pub struct Ku6Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Ku6Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Ku6Extractor {
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
                "Ku6 URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Ku6 URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(r#"(?is)<h1\b[^>]*\btitle\s*=\s*["'][^"']*["'][^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_owned())
            .or_else(|| html_title_value(&html))
            .unwrap_or_else(|| video_id.clone());
        let response = context.get_json(&format!(
            "http://v.ku6.com/fetchVideo4Player/{video_id}.html"
        ))?;
        let media_url = response
            .get("data")
            .and_then(|data| json_string(data, "f"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Ku6 video {video_id} has no playable URL"),
                )
            })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "f4v");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Graspop festival extractor. The festival API returns the HLS asset
/// and poster metadata in one JSON response; the native downloader consumes
/// the returned manifest URL.
pub struct GraspopExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GraspopExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GraspopExtractor {
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
                "Graspop URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Graspop URL has no ID")
            })?;
        let metadata = context.get_json(&format!(
            "https://tv.proximus.be/MWC/videocenter/festivals/{video_id}/stream"
        ))?;
        let asset_url = metadata
            .get("source")
            .and_then(|source| json_string(source, "assetUri"))
            .filter(|value| !value.is_empty())
            .map(|value| url_with_scheme(value, "http"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Graspop video {video_id} has no HLS asset"),
                )
            })?;
        let extension = "mp4";
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(&metadata, "name")
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&video_id)
            ),
        );
        info.insert("url", serde_json::json!(asset_url.clone()));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": asset_url,
                "format_id": "hls",
                "ext": extension,
                "protocol": "m3u8_native",
            }]),
        );
        info.insert_if_some(
            "thumbnail",
            metadata
                .get("source")
                .and_then(|source| json_string(source, "poster")),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native ScreenRec page extractor. The player configuration embeds an HLS
/// URL and the page supplies OpenGraph metadata.
pub struct ScreenRecExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ScreenRecExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ScreenRecExtractor {
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
                "ScreenRec URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "ScreenRec URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let matcher = Regex::new(r#"(?is)\bcustomUrl\s*:\s*(["'])(?P<url>(?:(?!\1).)+)\1"#)
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid ScreenRec player matcher: {error}"),
                )
            })?;
        let captures = matcher.captures(&html).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("ScreenRec video {video_id} has no HLS URL"),
            )
        })?;
        let media_url = captures
            .name("url")
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.is_empty())
            .map(|value| proto_relative_url(&value, "https:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("ScreenRec video {video_id} has an empty HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Match TV live-channel extractor. Both the public on-air URL and
/// the iframe URL share the same channel configuration endpoint.
pub struct MatchTvExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl MatchTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid MatchTV URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for MatchTvExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        _url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = "matchtv-live";
        let page_url = "https://video.matchtv.ru/iframe/channel/106";
        let webpage = context.get(page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let matcher = Regex::new(r#"(?is)\bdata-config\s*=\s*"config=(https?://[^?"]+)[?"]"#)
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid MatchTV player matcher: {error}"),
                )
            })?;
        let source_url = matcher
            .captures(&html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "MatchTV player has no stream configuration URL",
                )
            })?;
        let media_url = format!("{}.m3u8", source_url.replace("/feed/", "/media/"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!("Матч ТВ - Прямой эфир"));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native JWPlatform extractor. JWPlatform's v2 endpoint is a stable JSON
/// contract used by the service's player URLs and by several site-specific
/// wrapper extractors.
pub struct JwPlatformExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl JwPlatformExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let matchers = descriptor
            .valid_urls
            .iter()
            .map(|pattern| {
                compile_source_pattern(pattern).map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid JWPlatform URL pattern: {error}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for JwPlatformExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "JWPlatform URL did not contain a media ID",
                )
            })?;
        let response =
            context.get_json(&format!("https://cdn.jwplayer.com/v2/media/{video_id}"))?;
        let base_url = format!("https://cdn.jwplayer.com/v2/media/{video_id}");
        let playlist = response
            .get("playlist")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().collect::<Vec<_>>())
            .unwrap_or_else(|| vec![&response]);
        let mut entries = Vec::new();
        for item in playlist {
            let item_id = json_string(item, "mediaid").unwrap_or(&video_id).to_owned();
            let sources = item
                .get("sources")
                .and_then(serde_json::Value::as_array)
                .map(|sources| sources.iter().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![item]);
            let mut formats = Vec::new();
            for (index, source) in sources.into_iter().enumerate() {
                let Some(raw_url) = json_string(source, "file").filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if raw_url.starts_with("rtmp:") {
                    continue;
                }
                let source_url = resolve_url(&base_url, &proto_relative_url(raw_url, "https:"));
                let source_type = json_string(source, "type").unwrap_or("");
                let source_ext = source_type
                    .split(';')
                    .next()
                    .and_then(|value| mimetype_extension(Some(value)))
                    .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&source_url), "mp4"));
                let url_ext = yt_dlp_core::determine_ext(Some(&source_url), &source_ext);
                let is_hls = source_type.eq_ignore_ascii_case("hls")
                    || url_ext == "m3u8"
                    || source_url.contains("format=m3u8-aapl");
                let is_dash = source_type.eq_ignore_ascii_case("dash")
                    || url_ext == "mpd"
                    || source_url.contains("format=mpd-time-csf");
                let format_id = json_string(source, "label")
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("http-{index}"));
                let mut format = serde_json::json!({
                    "url": source_url,
                    "format_id": format_id,
                    "ext": if is_hls || is_dash { "mp4" } else { url_ext.as_str() },
                });
                if is_hls {
                    format["protocol"] = serde_json::json!("m3u8_native");
                } else if is_dash {
                    format["protocol"] = serde_json::json!("http_dash_segments");
                } else if source_type.starts_with("audio/")
                    || source_ext.starts_with("mp3")
                    || matches!(url_ext.as_str(), "mp3" | "m4a" | "aac" | "ogg" | "oga")
                {
                    format["vcodec"] = serde_json::json!("none");
                }
                if let Some(value) = json_i64(source, "width") {
                    format["width"] = serde_json::json!(value);
                }
                if let Some(value) = json_i64(source, "height")
                    .or_else(|| json_string(source, "label").and_then(jwplatform_label_height))
                {
                    format["height"] = serde_json::json!(value);
                }
                if let Some(value) = json_i64(source, "bitrate") {
                    format["tbr"] = serde_json::json!(value as f64 / 1000.0);
                }
                if let Some(value) = json_i64(source, "filesize") {
                    format["filesize"] = serde_json::json!(value);
                }
                formats.push(format);
            }
            if formats.is_empty() {
                continue;
            }
            let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(item_id));
            info.insert_if_some(
                "title",
                json_string(item, "title").map(unescape_html_attribute),
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
                    .unwrap_or_else(|| serde_json::json!("mp4")),
            );
            info.insert("formats", serde_json::Value::Array(formats));
            info.insert_if_some(
                "description",
                json_string(item, "description").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "thumbnail",
                json_string(item, "image")
                    .map(|value| resolve_url(&base_url, &proto_relative_url(value, "https:"))),
            );
            info.insert_if_some(
                "timestamp",
                json_i64(item, "pubdate").or_else(|| {
                    json_string(item, "pubdate")
                        .map(str::to_owned)
                        .and_then(parse_timestamp)
                }),
            );
            info.insert_if_some(
                "duration",
                json_f64(&response, "duration").or_else(|| json_f64(item, "duration")),
            );
            info.insert_if_some(
                "alt_title",
                json_string(item, "subtitle").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "genre",
                json_string(item, "genre").map(|value| html_text_fragment(value)),
            );
            info.insert_if_some(
                "channel",
                json_string(item, "channel")
                    .or_else(|| json_string(item, "category"))
                    .map(str::to_owned),
            );
            info.insert_if_some("season_number", json_i64(item, "season"));
            info.insert_if_some("episode_number", json_i64(item, "episode"));
            info.insert_if_some("release_year", json_i64(item, "releasedate"));
            info.insert_if_some("age_limit", json_i64(item, "age_restriction"));
            if let Some(tracks) = item.get("tracks").and_then(serde_json::Value::as_array) {
                let mut subtitles = serde_json::Map::new();
                for track in tracks {
                    let Some(kind) = json_string(track, "kind") else {
                        continue;
                    };
                    if !matches!(kind.to_ascii_lowercase().as_str(), "captions" | "subtitles") {
                        continue;
                    }
                    let Some(raw_url) =
                        json_string(track, "file").filter(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    let language = json_string(track, "label").unwrap_or("en");
                    let subtitle_url =
                        resolve_url(&base_url, &proto_relative_url(raw_url, "https:"));
                    subtitles
                        .entry(language.to_owned())
                        .or_insert_with(|| serde_json::json!([]))
                        .as_array_mut()
                        .expect("JWPlatform subtitle list")
                        .push(serde_json::json!({"url": subtitle_url}));
                }
                if !subtitles.is_empty() {
                    info.insert("subtitles", serde_json::Value::Object(subtitles));
                }
            }
            entries.push(info);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("JWPlatform media {video_id} has no playable sources"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.pop().expect("one JWPlatform entry"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&response, "title"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn jwplatform_label_height(label: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?i)\b(\d{3,4})p\b"#).ok()?;
    matcher
        .captures(label)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<i64>().ok())
}

/// Native wrapper for Bundesliga pages that expose a JWPlatform media ID.
pub struct BundesligaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BundesligaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BundesligaExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Bundesliga URL did not contain a media ID",
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{video_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}

/// Native wrapper for OutsideTV pages that encode a JWPlatform media ID.
pub struct OutsideTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl OutsideTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for OutsideTvExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "OutsideTV URL did not contain a media ID",
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{video_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}

/// Native wrapper for TeachingChannel pages that expose a JWPlatform media
/// ID in either the player data attribute or element ID.
pub struct TeachingChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TeachingChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TeachingChannelExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "TeachingChannel URL did not contain a display ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let player_matcher = Regex::new(
            r#"(?is)(?:data-mid\s*=\s*["']|id\s*=\s*["']jw-video-player-)([a-zA-Z0-9]{8})"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid TeachingChannel player matcher: {error}"),
            )
        })?;
        let media_id = player_matcher
            .captures(&html)
            .ok()
            .flatten()
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("TeachingChannel page {display_id} has no JWPlatform media ID"),
                )
            })?;
        Ok(ExtractorResult::Redirect {
            url: format!("jwplatform:{media_id}"),
            ie_key: Some("JWPlatform".to_owned()),
        })
    }
}

/// Native AtScale conference event playlist extractor. Event pages expose
/// canonical video URLs in data-url attributes; each entry is expanded by
/// the native Generic extractor so OpenGraph/HTML5 media is preserved.
pub struct AtScaleConfEventExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AtScaleConfEventExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AtScaleConfEventExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "AtScale event URL did not contain a playlist ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let link_matcher =
            Regex::new(r#"(?is)\bdata-url\s*=\s*"((?:https?://)(?:www\.)?atscaleconference\.com/videos/[^"]+)""#)
                .map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("invalid AtScale video link matcher: {error}"),
                    )
                })?;
        let generic =
            GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "Generic", "", true));
        let mut entries = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(entry_url) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let entry = generic.extract_with_context(entry_url, context)?;
            match entry {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        "AtScale video entry did not resolve to a single native result",
                    ));
                }
            }
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("AtScale event {playlist_id} has no video entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", html_meta_value(&html, "og:title"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native NZZ article/video playlist extractor. NZZ embeds one or more
/// JWPlayer settings objects in page scripts; these are parsed as data and
/// never evaluated as JavaScript.
pub struct NzzExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NzzExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NzzExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "NZZ URL did not contain a page ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let script_matcher = Regex::new(
            r#"(?is)<script\b[^>]*\bdata-hid\s*=\s*"jw-video-jw[^"]*"[^>]*>(.*?)</script>"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid NZZ JWPlayer script matcher: {error}"),
            )
        })?;
        let mut entries = Vec::new();
        for captures in script_matcher.captures_iter(&html).flatten() {
            let Some(script) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(settings) = json_object_after_marker(script, "var settings") else {
                continue;
            };
            let items = settings
                .get("playlist")
                .and_then(serde_json::Value::as_array)
                .map(|items| items.iter().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![&settings]);
            for item in items {
                if let Some(entry) = nzz_jw_entry(item, &page_id) {
                    entries.push(entry);
                }
            }
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("NZZ page {page_id} has no playable JWPlayer entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(page_id));
        info.insert_if_some("title", html_meta_value(&html, "og:title"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn nzz_jw_entry(item: &serde_json::Value, fallback_id: &str) -> Option<InfoDict> {
    let sources = item
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| sources.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![item]);
    let mut formats = Vec::new();
    for (index, source) in sources.into_iter().enumerate() {
        let raw_url = json_string(source, "file")
            .or_else(|| json_string(source, "url"))
            .filter(|value| !value.is_empty())?;
        if raw_url.starts_with("rtmp:") {
            continue;
        }
        let source_type = json_string(source, "type").unwrap_or("");
        let source_ext = source_type
            .split(';')
            .next()
            .and_then(|value| mimetype_extension(Some(value)))
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(raw_url), "mp4"));
        let source_ext = yt_dlp_core::determine_ext(Some(raw_url), &source_ext);
        let is_hls = source_type.eq_ignore_ascii_case("hls") || source_ext == "m3u8";
        let is_dash = source_type.eq_ignore_ascii_case("dash") || source_ext == "mpd";
        let mut format = serde_json::json!({
            "url": proto_relative_url(raw_url, "https:"),
            "format_id": json_string(source, "label")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("http-{index}")),
            "ext": if is_hls || is_dash { "mp4" } else { source_ext.as_str() },
        });
        if is_hls {
            format["protocol"] = serde_json::json!("m3u8_native");
        } else if is_dash {
            format["protocol"] = serde_json::json!("http_dash_segments");
        }
        if let Some(value) = json_i64(source, "width") {
            format["width"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(source, "height") {
            format["height"] = serde_json::json!(value);
        }
        if let Some(value) = json_i64(source, "bitrate") {
            format["tbr"] = serde_json::json!(value as f64 / 1000.0);
        }
        formats.push(format);
    }
    if formats.is_empty() {
        return None;
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let id = json_string(item, "mediaid")
        .or_else(|| json_string(item, "id"))
        .unwrap_or(fallback_id);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(id));
    info.insert(
        "title",
        serde_json::json!(
            json_string(item, "title")
                .map(unescape_html_attribute)
                .unwrap_or_else(|| fallback_id.to_owned())
        ),
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
            .unwrap_or_else(|| serde_json::json!("mp4")),
    );
    info.insert("formats", serde_json::Value::Array(formats));
    info.insert_if_some(
        "description",
        json_string(item, "description").map(html_text_fragment),
    );
    info.insert_if_some("thumbnail", json_string(item, "image"));
    info.insert_if_some("timestamp", json_i64(item, "pubdate"));
    info.insert_if_some("duration", json_f64(item, "duration"));
    if let Some(tracks) = item.get("tracks").and_then(serde_json::Value::as_array) {
        let mut subtitles = serde_json::Map::new();
        for track in tracks {
            let Some(raw_url) = json_string(track, "file").filter(|value| !value.is_empty()) else {
                continue;
            };
            let language = json_string(track, "label").unwrap_or("en");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("NZZ subtitle list")
                .push(serde_json::json!({
                    "url": proto_relative_url(raw_url, "https:")
                }));
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    Some(info)
}

/// Native BehindKink HTML5 video extractor.
pub struct BehindKinkExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BehindKinkExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BehindKinkExtractor {
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
                "BehindKink URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "BehindKink URL has no ID")
            })?;
        let upload_date = ["year", "month", "day"]
            .iter()
            .map(|name| {
                captures
                    .name(name)
                    .map(|value| value.as_str())
                    .unwrap_or_default()
            })
            .collect::<String>();
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let video_url = Regex::new(r#"(?is)<source\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, &unescape_html_attribute(value.as_str())))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("BehindKink page {display_id} has no video source"),
                )
            })?;
        let media_id = video_url
            .split('?')
            .next()
            .and_then(|value| value.rsplit('/').next())
            .unwrap_or(&display_id)
            .split('_')
            .next()
            .unwrap_or(&display_id)
            .trim_end_matches(".mp4")
            .trim_end_matches(".mov")
            .to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(video_url.clone()));
        info.insert(
            "ext",
            serde_json::json!(yt_dlp_core::determine_ext(Some(&video_url), "mp4")),
        );
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| media_id.clone())
            ),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert("upload_date", serde_json::json!(upload_date));
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": video_url,
                "format_id": "source",
                "ext": yt_dlp_core::determine_ext(
                    info.get_str("url"),
                    "mp4",
                ),
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Historic Films page extractor. The page supplies the tape ID and
/// descriptive metadata while the media URL follows a stable service path.
pub struct HistoricFilmsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HistoricFilmsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HistoricFilmsExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Historic Films URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let tape_id = Regex::new(
            r#"(?is)(?:class\s*=\s*["']tapeId["'][^>]*>|["']tapeId["']\s*:\s*["'])([^<"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_owned())
        .map(|value| {
            value
                .rsplit_once(':')
                .filter(|(_, suffix)| !suffix.contains('/'))
                .map_or(value.clone(), |(_, suffix)| suffix.to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Historic Films page {video_id} has no tape ID"),
            )
        })?;
        let media_url = format!("http://www.historicfilms.com/video/{tape_id}_{video_id}_web.mov");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mov"));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some(
            "thumbnail",
            html_meta_value(&html, "thumbnailUrl").or_else(|| html_meta_value(&html, "og:image")),
        );
        info.insert_if_some(
            "duration",
            html_meta_value(&html, "duration")
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mov",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native OnePlace podcast episode extractor.
pub struct OnePlacePodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl OnePlacePodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for OnePlacePodcastExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "OnePlace URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = [
            r#"(?is)\bmp3-url\s*=\s*"([^"]+)"#,
            r#"(?is)<div[^>]+\bid\s*=\s*"player"[^>]+\bdata-media-url\s*=\s*"([^"]+)"#,
        ]
        .iter()
        .find_map(|pattern| {
            Regex::new(pattern)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| resolve_url(url, &unescape_html_attribute(value.as_str())))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("OnePlace episode {video_id} has no audio URL"),
            )
        })?;
        let title = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*"[^"]*\bdetails\b[^"]*"[^>]*>.*?<h2\b[^>]*>(.*?)</h2>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .or_else(|| html_meta_value(&html, "og:title"))
        .or_else(|| html_title_value(&html));
        let description =
            Regex::new(r#"(?is)<div[^>]*\bclass\s*=\s*"[^"]*\bepDesc\b[^"]*"[^>]*>(.*?)</div>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "mp3",
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Megaphone embedded podcast player extractor.
pub struct MegaphoneExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MegaphoneExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MegaphoneExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Megaphone URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let episode = json_object_after_marker(&html, "var episode").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Megaphone episode {video_id} has no embedded JSON"),
            )
        })?;
        let raw_url = json_string(&episode, "mediaUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Megaphone episode {video_id} has no media URL"),
            )
        })?;
        let media_url = proto_relative_url(raw_url, "https:");
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp3");
        let title = html_meta_value(&html, "audio:title")
            .or_else(|| html_meta_value(&html, "og:title"))
            .unwrap_or_else(|| video_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some(
            "creators",
            html_meta_value(&html, "audio:artist").map(|value| vec![value]),
        );
        info.insert_if_some("duration", json_f64(&episode, "duration"));
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

/// Native Hypem track extractor. Track metadata is embedded in the page and
/// the service's source endpoint returns the final audio URL.
pub struct HypemExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HypemExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HypemExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Hypem URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let display_data = html_script_json(&html, "displayList-data")?;
        let track = display_data
            .get("tracks")
            .and_then(serde_json::Value::as_array)
            .and_then(|tracks| tracks.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Hypem track {page_id} has no embedded track data"),
                )
            })?;
        let track_id = json_value_string(track.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem track {page_id} has no source ID"),
            )
        })?;
        let key = json_string(track, "key").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem track {track_id} has no source key"),
            )
        })?;
        let source = native_get_json_with_headers(
            context,
            &format!("http://hypem.com/serve/source/{track_id}/{key}"),
            &[("Content-Type", "application/json")],
        )?;
        let media_url = json_string(&source, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Hypem source {track_id} has no audio URL"),
            )
        })?;
        let title = json_string(track, "song").unwrap_or(&track_id).to_owned();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(track_id));
        info.insert("title", serde_json::json!(title.clone()));
        info.insert("track", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp3"));
        info.insert_if_some("uploader", json_string(track, "artist"));
        info.insert_if_some("duration", json_i64(track, "time"));
        info.insert_if_some("timestamp", json_i64(track, "ts"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "mp3",
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native QingTing podcast program extractor.
pub struct QingTingExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl QingTingExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for QingTingExtractor {
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
                "QingTing URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("channel")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "QingTing URL has no channel",
                )
            })?;
        let program_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "QingTing URL has no program",
                )
            })?;
        let page_url = format!("https://m.qtfm.cn/vchannels/{channel_id}/programs/{program_id}/");
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let stores = json_object_after_marker(&html, "window.__initStores").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("QingTing program {program_id} has no store data"),
            )
        })?;
        let program_store = stores
            .get("ProgramStore")
            .unwrap_or(&serde_json::Value::Null);
        let program_info = program_store
            .get("programInfo")
            .unwrap_or(&serde_json::Value::Null);
        let channel_info = program_store
            .get("channelInfo")
            .unwrap_or(&serde_json::Value::Null);
        let podcaster = program_store
            .get("podcasterInfo")
            .and_then(|value| value.get("podcaster"))
            .unwrap_or(&serde_json::Value::Null);
        let media_url = json_string(program_info, "audioUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("QingTing program {program_id} has no audio URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(program_id));
        info.insert_if_some("title", json_string(program_info, "title"));
        info.insert("channel_id", serde_json::json!(channel_id));
        info.insert_if_some("channel", json_string(channel_info, "title"));
        info.insert_if_some("uploader", json_string(podcaster, "nickname"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("m4a"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("acodec", serde_json::json!("m4a"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": "m4a",
                "vcodec": "none",
                "acodec": "m4a",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Skyline Webcams live HLS extractor.
pub struct SkylineWebcamsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SkylineWebcamsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SkylineWebcamsExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Skyline Webcams URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(
            r#"(?is)(?:\burl|\bsource)\s*:\s*["']((?:https?:)?//[^"']+?\.m3u8[^"']*)["']"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| proto_relative_url(value.as_str(), "https:"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Skyline Webcams stream {video_id} has no HLS URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Webcamera.pl live extractor. The service obfuscates its HLS URL
/// with ROT13 in the page, which is decoded locally in Rust.
pub struct WebcameraplExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WebcameraplExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WebcameraplExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Webcamera.pl URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let encoded_url = Regex::new(r#"(?is)\bdata-src\s*=\s*"([^"]+\.z3h8)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Webcamera.pl stream {video_id} has no encoded HLS URL"),
                )
            })?;
        let media_url = rot13_ascii(&encoded_url);
        let title = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Alibaba product video extractor. Product pages expose their media
/// records in the detailData object; the selected video is returned directly.
pub struct AlibabaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AlibabaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AlibabaExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Alibaba URL has no product ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let detail = json_object_after_marker(&html, "window.detailData").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Alibaba product {display_id} has no detailData"),
            )
        })?;
        let product = detail
            .get("globalData")
            .and_then(|value| value.get("product"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Alibaba product {display_id} has no media product"),
                )
            })?;
        let media = product
            .get("mediaItems")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    json_string(item, "type") == Some("video")
                        && item.get("videoId").is_some()
                        && json_string(item, "videoUrl").is_some()
                })
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Alibaba product {display_id} has no playable video"),
                )
            })?;
        let video_id = json_value_string(media.get("videoId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Alibaba video record has no video ID",
            )
        })?;
        let media_url = json_string(media, "videoUrl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Alibaba video record has no video URL",
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut format = serde_json::json!({
            "url": media_url,
            "format_id": json_string(media, "definition").unwrap_or("source"),
            "ext": ext,
        });
        for (source, target) in [
            ("bitrate", "tbr"),
            ("width", "width"),
            ("height", "height"),
            ("length", "filesize"),
        ] {
            if let Some(value) = json_i64(media, source) {
                format[target] = serde_json::json!(value);
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(product, "subject"));
        info.insert_if_some("duration", json_f64(media, "duration"));
        info.insert_if_some("thumbnail", json_string(media, "videoCoverUrl"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext));
        info.insert("formats", serde_json::json!([format]));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Moving Image archive extractor. Archive pages expose one HLS
/// manifest and a small set of labelled metadata fields.
pub struct MovingImageExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MovingImageExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MovingImageExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Moving Image URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bfile\s*:\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Moving Image film {video_id} has no HLS URL"),
                )
            })?;
        let title = html_field_value(&html, "Title")
            .map(|value| value.trim_matches(['(', ')', '[', ']']).trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| html_title_value(&html))
            .unwrap_or_else(|| video_id.clone());
        let description = html_field_value(&html, "Description");
        let duration = html_field_value(&html, "Running time").and_then(|value| {
            yt_dlp_core::parse_duration(value.trim_matches(['(', ')', '[', ']']))
        });
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*'([^']+)'"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("duration", duration);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Tweakers video API extractor.
pub struct TweakersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TweakersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TweakersExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Tweakers URL has no ID")
            })?;
        let data = context.get_json(&format!(
            "https://tweakers.net/video/s1playlist/{video_id}/1920/1080/playlist.json"
        ))?;
        let item = data
            .get("items")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Tweakers video {video_id} has no API item"),
                )
            })?;
        let mut formats = Vec::new();
        if let Some(locations) = item
            .get("locations")
            .and_then(|value| value.get("progressive"))
            .and_then(serde_json::Value::as_array)
        {
            for location in locations {
                let format_id = json_string(location, "label");
                for source in location
                    .get("sources")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(media_url) = json_string(source, "src") else {
                        continue;
                    };
                    let ext = json_string(source, "type")
                        .and_then(|value| mimetype_extension(value.split(';').next()))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4"));
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": format_id,
                        "ext": ext,
                    });
                    if let Some(value) = json_i64(location, "width") {
                        format["width"] = serde_json::json!(value);
                    }
                    if let Some(value) = json_i64(location, "height") {
                        format["height"] = serde_json::json!(value);
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Tweakers video {video_id} has no progressive formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(item, "title"));
        info.insert_if_some("description", json_string(item, "description"));
        info.insert_if_some("thumbnail", json_string(item, "poster"));
        info.insert_if_some("duration", json_i64(item, "duration"));
        info.insert_if_some("uploader_id", json_string(item, "account"));
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

/// Native KrasView page extractor.
pub struct KrasViewExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KrasViewExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KrasViewExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "KrasView URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let flashvars = json_object_after_marker(&html, "video_Init(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no player data"),
            )
        })?;
        let media_url = json_string(&flashvars, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KrasView video {video_id} has no media URL"),
            )
        })?;
        let ext = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some(
            "thumbnail",
            json_string(&flashvars, "image")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "og:image")),
        );
        info.insert_if_some("duration", json_i64(&flashvars, "duration"));
        info.insert_if_some(
            "width",
            html_meta_value(&html, "video:width").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some(
            "height",
            html_meta_value(&html, "video:height").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native 56.com page/API extractor. The legacy Sohu redirect variant is
/// surfaced as an explicit TODO because its target extractor is not yet
/// native; the direct XML API path is fully handled here.
pub struct C56Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl C56Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for C56Extractor {
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
                "56.com URL did not match its native pattern",
            )
        })?;
        let text_id = captures
            .name("textid")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "56.com URL has no text ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        if let Some(sohu_info) = json_object_after_marker(&html, "var sohuVideoInfo") {
            if let Some(sohu_url) = json_string(&sohu_info, "url") {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: 56.com Sohu wrapper requires native Sohu extraction ({sohu_url})"
                    ),
                ));
            }
        }
        let page = context.get_json(&format!("http://vxml.56.com/json/{text_id}/"))?;
        let info_data = page.get("info").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("56.com API response for {text_id} has no info"),
            )
        })?;
        let video_id = json_value_string(info_data.get("vid")).unwrap_or_else(|| text_id.clone());
        let mut formats = Vec::new();
        for file in info_data
            .get("rfiles")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(media_url) = json_string(file, "url") else {
                continue;
            };
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": json_string(file, "type").unwrap_or("source"),
                "ext": yt_dlp_core::determine_ext(Some(media_url), "flv"),
            });
            if let Some(value) = json_i64(file, "filesize") {
                format["filesize"] = serde_json::json!(value);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("56.com video {video_id} has no media files"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let duration = json_f64(info_data, "duration").map(|value| value / 1000.0);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(info_data, "Subject"));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "thumbnail",
            json_string(info_data, "bimg").or_else(|| json_string(info_data, "img")),
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
                .unwrap_or_else(|| serde_json::json!("flv")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native TASS page extractor. JW-style source records embedded in the page
/// are parsed as JSON data and filtered to HTTP MP4 renditions.
pub struct TassExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl TassExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for TassExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "TASS URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let sources = json_array_after_marker(&html, "sources").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("TASS video {video_id} has no source list"),
            )
        })?;
        let mut formats = Vec::new();
        for source in sources.as_array().into_iter().flatten() {
            let Some(media_url) = json_string(source, "file") else {
                continue;
            };
            if !media_url.starts_with("http") || !media_url.ends_with(".mp4") {
                continue;
            }
            let format_id = json_string(source, "label").unwrap_or("source");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "ext": "mp4",
                "quality": if format_id == "hd" { 1 } else { 0 },
            });
            if let Some(value) = json_i64(source, "width") {
                format["width"] = serde_json::json!(value);
            }
            if let Some(value) = json_i64(source, "height") {
                format["height"] = serde_json::json!(value);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("TASS video {video_id} has no HTTP MP4 sources"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_title_value(&html)),
        );
        info.insert_if_some("description", html_meta_value(&html, "og:description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "url",
            first.get("url").cloned().unwrap_or(serde_json::Value::Null),
        );
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Photobucket page/API extractor.
pub struct PhotobucketExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PhotobucketExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PhotobucketExtractor {
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
                "Photobucket URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Photobucket URL has no ID")
            })?;
        let extension = captures
            .name("ext")
            .map(|value| value.as_str().to_ascii_lowercase())
            .unwrap_or_else(|| "mp4".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "Pb.Data.Shared.MEDIA").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Photobucket media {video_id} has no shared metadata"),
            )
        })?;
        let html_code = data
            .get("linkcodes")
            .and_then(|value| json_string(value, "html"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no HTML link code"),
                )
            })?;
        let media_url = Regex::new(r#"(?is)\bfile=([^&\s]+?\.mp4)"#)
            .ok()
            .and_then(|matcher| matcher.captures(html_code).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, &percent_decode(value.as_str())))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Photobucket media {video_id} has no file URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert_if_some("uploader", json_string(&data, "username"));
        info.insert_if_some("timestamp", json_i64(&data, "creationDate"));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert_if_some("thumbnail", json_string(&data, "thumbUrl"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": extension,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Nobel Prize media-page extractor. Video JSON-LD and metadata are
/// read directly; query aliases id and qid are both supported.
pub struct NobelPrizeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NobelPrizeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NobelPrizeExtractor {
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
        if !self.suitable(url) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Nobel Prize URL did not match its native pattern",
            ));
        }
        let parsed = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Nobel Prize URL: {error}"),
            )
        })?;
        let video_id = parsed
            .query_pairs()
            .find(|(key, _)| key == "id" || key == "qid")
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Nobel Prize URL requires id or qid",
                )
            })?;
        let page_url = format!(
            "https://mediaplayer.nobelprize.org{}",
            parsed
                .path()
                .is_empty()
                .then_some("/mediaplayer/")
                .unwrap_or(parsed.path())
        );
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = html_json_ld(&html).unwrap_or(serde_json::Value::Null);
        let media_url = json_string(&data, "contentUrl")
            .or_else(|| json_string(&data, "url"))
            .map(str::to_owned)
            .or_else(|| html_meta_value(&html, "contentUrl"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Nobel Prize media {video_id} has no content URL"),
                )
            })?;
        let media_url = proto_relative_url(&media_url, "https:");
        let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "caption")
                    .or_else(|| json_string(&data, "name").map(str::to_owned))
                    .unwrap_or(video_id.clone())
            ),
        );
        info.insert_if_some(
            "description",
            json_string(&data, "description")
                .map(str::to_owned)
                .or_else(|| html_meta_value(&html, "description")),
        );
        info.insert_if_some("thumbnail", json_string(&data, "thumbnailUrl"));
        info.insert_if_some(
            "duration",
            json_string(&data, "duration").and_then(yt_dlp_core::parse_duration),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "uploadDate")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(ext.clone()));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": ext,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Caltrans traffic-camera live HLS extractor.
pub struct CaltransExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CaltransExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CaltransExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Caltrans URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let media_url = Regex::new(r#"(?is)\bvideoStreamURL\s*=\s*"([^"]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Caltrans camera {video_id} has no stream URL"),
                )
            })?;
        let route_place = Regex::new(r#"(?is)\broutePlace\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned());
        let location = Regex::new(r#"(?is)\blocationName\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let title = route_place
            .map(|place| format!("{place} : {location}"))
            .unwrap_or(location);
        let thumbnail = Regex::new(r#"(?is)\bposterURL\s*=\s*"([^"]*)""#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| proto_relative_url(value.as_str(), "https:"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("ts"));
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "ts",
                "protocol": "m3u8_native",
                "is_live": true,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native CozyTV replay extractor.
pub struct CozyTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CozyTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CozyTvExtractor {
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
                "CozyTV URL did not match its native pattern",
            )
        })?;
        let uploader = captures
            .name("uploader")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CozyTV URL has no uploader")
            })?;
        let date = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "CozyTV URL has no replay ID",
                )
            })?;
        let video_id = format!("{uploader}-{date}");
        let data = context.get_json(&format!(
            "https://api.cozy.tv/cache/{uploader}/replay/{date}"
        ))?;
        let media_url =
            format!("https://cozycdn.foxtrotstream.xyz/replays/{uploader}/{date}/index.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert(
            "uploader",
            serde_json::json!(json_string(&data, "user").unwrap_or(&uploader)),
        );
        info.insert(
            "upload_date",
            serde_json::json!(
                json_string(&data, "date")
                    .map(|value| {
                        value
                            .chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
                    .filter(|value| value.len() == 8)
                    .unwrap_or_else(|| {
                        date.chars()
                            .filter(|character| character.is_ascii_digit())
                            .take(8)
                            .collect::<String>()
                    })
            ),
        );
        info.insert("was_live", serde_json::json!(true));
        info.insert_if_some("duration", json_i64(&data, "duration"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Livestreamfails API/direct-media extractor.
pub struct LivestreamfailsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LivestreamfailsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for LivestreamfailsExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Livestreamfails URL has no ID",
                )
            })?;
        let data = context.get_json(&format!("https://api.livestreamfails.com/clip/{video_id}"))?;
        let source_id = json_string(&data, "sourceId").map(str::to_owned);
        let remote_id = json_string(&data, "videoId").unwrap_or(&video_id);
        let media_url = format!("https://livestreamfails-video-prod.b-cdn.net/video/{remote_id}");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("display_id", source_id);
        info.insert_if_some("title", json_string(&data, "label").map(str::to_owned));
        info.insert_if_some(
            "creator",
            data.get("streamer")
                .and_then(|value| json_string(value, "label")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&data, "imageId")
                .map(|value| format!("https://livestreamfails-image-prod.b-cdn.net/image/{value}")),
        );
        info.insert_if_some(
            "timestamp",
            json_string(&data, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Masters tournament video API extractor.
pub struct MastersExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MastersExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MastersExtractor {
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
                "Masters URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Masters URL has no ID")
            })?;
        let date = captures
            .name("date")
            .map(|value| value.as_str().replace('-', ""))
            .unwrap_or_default();
        let data = context.get_json(&format!(
            "https://www.masters.com/relatedcontent/rest/v2/masters_v1/en/content/masters_v1_{video_id}_en"
        ))?;
        let media_url = data
            .get("media")
            .and_then(|value| json_string(value, "m3u8"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Masters video {video_id} has no HLS URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&data, "title"));
        info.insert("upload_date", serde_json::json!(date));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        if let Some(images) = data
            .get("images")
            .and_then(|value| value.get(0))
            .and_then(serde_json::Value::as_object)
        {
            let thumbnails = images
                .iter()
                .filter_map(|(id, value)| {
                    Some(serde_json::json!({
                        "id": id,
                        "url": value.as_str()?
                    }))
                })
                .collect::<Vec<_>>();
            if !thumbnails.is_empty() {
                info.insert("thumbnails", serde_json::Value::Array(thumbnails));
            }
        }
        Ok(ExtractorResult::single(info))
    }
}

/// Native Mir24 article iframe/HLS extractor.
pub struct Mir24TvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Mir24TvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Mir24TvExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mir24 URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url =
            Regex::new(r#"(?is)<iframe\b[^>]+\bsrc\s*=\s*["'](https?://mir24\.tv/players/[^"']+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .ok_or_else(|| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Mir24 article {video_id} has no player iframe"),
                    )
                })?;
        let player = url::Url::parse(&iframe_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Mir24 player URL: {error}"),
            )
        })?;
        let media_url = player
            .query_pairs()
            .find_map(|(key, value)| {
                (key == "source").then(|| proto_relative_url(value.as_ref(), "https:"))
            })
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Mir24 player {video_id} has no source URL"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "og:title")
                    .or_else(|| html_title_value(&html))
                    .unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Blogger video configuration extractor.
pub struct BloggerExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BloggerExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for BloggerExtractor {
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
        let token_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Blogger URL has no token")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let data = json_object_after_marker(&html, "var VIDEO_CONFIG").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no VIDEO_CONFIG object"),
            )
        })?;
        let streams = data
            .get("streams")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Blogger video {token_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in streams {
            let Some(play_url) = json_string(stream, "play_url") else {
                continue;
            };
            let ext = url_query_value(play_url, "mime")
                .and_then(|mime| mimetype_extension(Some(&mime)))
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(play_url), "mp4"));
            formats.push(serde_json::json!({
                "url": play_url,
                "format_id": json_value_string(stream.get("format_id")),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Blogger video {token_id} has no playable streams"),
            )
        })?;
        let video_id = json_string(&data, "iframe_id").unwrap_or(&token_id);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(video_id));
        info.insert_if_some("thumbnail", json_string(&data, "thumbnail"));
        info.insert_if_some(
            "duration",
            first
                .get("url")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| url_query_value(value, "dur"))
                .and_then(|value| yt_dlp_core::parse_duration(&value)),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native radio.de station-page extractor. The source marks this service as
/// non-working today, but its historical contract is still represented here
/// without a compatibility runtime.
pub struct RadioDeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioDeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioDeExtractor {
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
        let radio_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "radio.de URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let broadcast = json_object_after_marker(&html, "stationService")
            .and_then(|service| service.get("station").cloned())
            .or_else(|| json_object_after_marker(&html, "station"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no broadcast data"),
                )
            })?;
        let stream_urls = broadcast
            .get("streamUrls")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("radio.de station {radio_id} has no streams"),
                )
            })?;
        let mut formats = Vec::new();
        for stream in stream_urls {
            let Some(stream_url) = json_string(stream, "streamUrl") else {
                continue;
            };
            let codec = json_string(stream, "streamContentFormat").unwrap_or("mp3");
            formats.push(serde_json::json!({
                "url": stream_url,
                "ext": codec.to_ascii_lowercase(),
                "acodec": codec,
                "abr": json_f64(stream, "bitRate"),
                "asr": json_f64(stream, "sampleRate"),
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("radio.de station {radio_id} has no playable streams"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(radio_id));
        info.insert(
            "title",
            serde_json::json!(json_string(&broadcast, "name").unwrap_or("radio.de station")),
        );
        info.insert_if_some(
            "description",
            json_string(&broadcast, "description")
                .or_else(|| json_string(&broadcast, "shortDescription")),
        );
        info.insert_if_some(
            "thumbnail",
            json_string(&broadcast, "picture4Url")
                .or_else(|| json_string(&broadcast, "picture4TransUrl"))
                .or_else(|| json_string(&broadcast, "logo100x100")),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native RadioZET podcast API extractor.
pub struct RadioZetPodcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RadioZetPodcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RadioZetPodcastExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "RadioZET URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let podcast_id = Regex::new(
            r#"(?is)<div\b[^>]*\bid\s*=\s*["']player["'][^>]*\bdata-id\s*=\s*["']([^"']+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("RadioZET podcast {display_id} has no player ID"),
            )
        })?;
        let data_url = format!(
            "https://player.radiozet.pl/api/podcasts/getPodcast/(node)/{podcast_id}/(station)/radiozet"
        );
        let response = context.get_json(&data_url)?;
        let data = response
            .get("data")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no API record"),
                )
            })?;
        let stream_url = data
            .get("player")
            .and_then(|player| json_string(player, "stream"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("RadioZET podcast {podcast_id} has no audio stream"),
                )
            })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(podcast_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some(
            "title",
            data.get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "description",
            data.get("program")
                .and_then(|program| json_string(program, "desc"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "release_timestamp",
            data.get("published_date").and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            }),
        );
        info.insert_if_some(
            "thumbnail",
            data.get("program")
                .and_then(|program| program.get("image"))
                .and_then(|image| json_string(image, "original")),
        );
        info.insert_if_some(
            "duration",
            data.get("player")
                .and_then(|player| player.get("duration"))
                .cloned(),
        );
        info.insert_if_some(
            "series",
            data.get("program")
                .and_then(|program| json_string(program, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        info.insert_if_some(
            "creator",
            data.get("presenter")
                .and_then(serde_json::Value::as_array)
                .and_then(|presenters| presenters.first())
                .and_then(|presenter| json_string(presenter, "title"))
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
        let ext = yt_dlp_core::determine_ext(Some(stream_url), "mp3");
        info.insert("url", serde_json::json!(stream_url));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{"url": stream_url, "format_id": "source", "ext": ext}]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native WorldStarHipHop HTML5 media extractor.
pub struct WorldStarHipHopExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WorldStarHipHopExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WorldStarHipHopExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "WorldStar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(r#"(?is)<div\b[^>]*class\s*=\s*["'][^"']*content-heading[^"']*["'][^>]*>\s*<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<span\b[^>]*class\s*=\s*["'][^"']*tc-sp-pinned-title[^"']*["'][^>]*>(.*?)</span>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| html_meta_value(&html, "og:title"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("WorldStar video {video_id} has no title"),
                )
            })?;
        let formats = html5_media_formats(url, &html);
        if formats.is_empty() {
            let generic =
                GenericExtractor::new(ExtractorDescriptor::new("GenericIE", "Generic", "", true));
            let fallback = generic.extract_with_context(url, context)?;
            if let ExtractorResult::Single(mut info) = fallback {
                info.insert("id", serde_json::json!(video_id));
                info.insert("title", serde_json::json!(title));
                return Ok(ExtractorResult::single(info));
            }
            return Ok(fallback);
        }
        let first = formats.first().cloned().expect("WorldStar format");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native This American Life archive/audio extractor.
pub struct ThisAmericanLifeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ThisAmericanLifeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ThisAmericanLifeExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "This American Life URL has no ID",
                )
            })?;
        let page_url = format!("http://www.thisamericanlife.org/radio-archives/episode/{video_id}");
        let webpage = context.get(&page_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let stream_url =
            format!("http://stream.thisamericanlife.org/{video_id}/stream/{video_id}_64k.m3u8");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(stream_url.clone()));
        info.insert("protocol", serde_json::json!("m3u8_native"));
        info.insert("ext", serde_json::json!("m4a"));
        info.insert("acodec", serde_json::json!("aac"));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert("abr", serde_json::json!(64));
        info.insert(
            "title",
            serde_json::json!(
                html_meta_value(&html, "twitter:title").unwrap_or_else(|| video_id.clone())
            ),
        );
        info.insert_if_some("description", html_meta_value(&html, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": stream_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "m4a",
                "acodec": "aac",
                "vcodec": "none",
                "abr": 64,
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Academic Earth course playlist extractor.
pub struct AcademicEarthCourseExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcademicEarthCourseExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AcademicEarthCourseExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Academic Earth playlist URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*playlist-name[^"']*["'][^>]*>(.*?)</h1>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Academic Earth playlist {playlist_id} has no title"),
            )
        })?;
        let description =
            Regex::new(r#"(?is)<p\b[^>]*\bclass\s*=\s*["'][^"']*excerpt[^"']*["'][^>]*>(.*?)</p>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
        let link_matcher = Regex::new(
            r#"(?is)<li\b[^>]*\bclass\s*=\s*["'][^"']*lecture-preview[^"']*["'][^>]*>\s*<a\b[^>]*\btarget\s*=\s*["']_blank["'][^>]*\bhref\s*=\s*["']([^"']+)"#,
        )
        .map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Academic Earth lecture matcher: {error}"),
            )
        })?;
        let base_url = url::Url::parse(url).ok();
        let mut entries = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str().trim()) else {
                continue;
            };
            let entry_url = base_url
                .as_ref()
                .and_then(|base| base.join(raw_url).ok())
                .map_or_else(
                    || proto_relative_url(raw_url, "https:"),
                    |value| value.to_string(),
                );
            entries.push(native_url_result(&entry_url));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Premiership Rugby article/JWPlatform HLS extractor.
pub struct PremiershipRugbyExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PremiershipRugbyExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PremiershipRugbyExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Premiership Rugby URL has no article slug",
                )
            })?;
        let data_url = format!(
            "https://article-cms-api.incrowdsports.com/v2/articles/slug/{display_id}?clientId=PRL"
        );
        let response = context.get_json(&data_url)?;
        let article = response
            .get("data")
            .and_then(|data| data.get("article"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Premiership Rugby article {display_id} has no article object"),
                )
            })?;
        let hero = article.get("heroMedia").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no hero media"),
            )
        })?;
        let content = hero.get("content").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no media content"),
            )
        })?;
        let media_url = json_string(content, "videoLink").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Premiership Rugby article {display_id} has no video link"),
            )
        })?;
        let video_id = json_string(content, "sourceSystemId").unwrap_or(&display_id);
        let duration = content
            .get("metadata")
            .and_then(|metadata| json_f64(metadata, "msDuration"))
            .map(|milliseconds| milliseconds / 1000.0);
        let categories = article
            .get("categories")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                serde_json::Value::Array(
                    items
                        .iter()
                        .filter_map(|item| json_string(item, "text").map(str::to_owned))
                        .map(serde_json::Value::String)
                        .collect(),
                )
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(hero, "title"));
        info.insert_if_some("thumbnail", json_string(content, "videoThumbnail"));
        info.insert_if_some("duration", duration);
        info.insert_if_some("tags", article.get("tags").cloned());
        info.insert_if_some("categories", categories);
        info.insert_if_some(
            "subtitles",
            content
                .get("subtitles")
                .cloned()
                .or_else(|| content.get("captions").cloned()),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native MatchiTV Next.js/HLS extractor.
pub struct MatchiTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MatchiTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MatchiTvExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MatchiTV URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let loaded_media = html_script_json(&html, "__NEXT_DATA__")
            .ok()
            .and_then(|data| data.get("props").cloned())
            .and_then(|props| props.get("pageProps").cloned())
            .and_then(|page_props| page_props.get("loadedMedia").cloned())
            .unwrap_or(serde_json::Value::Null);
        let court = json_string(&loaded_media, "courtDescription");
        let start = json_string(&loaded_media, "startDateTime");
        let title = match (court, start) {
            (Some(court), Some(start)) => format!("{court} {start}"),
            (Some(court), None) => court.to_owned(),
            (None, Some(start)) => start.to_owned(),
            (None, None) => video_id.clone(),
        };
        let media_url = format!(
            "https://streams.padelgo.tv/v2/streams/m3u8/{video_id}/anonymous/playlist.m3u8"
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert(
            "thumbnail",
            serde_json::json!(format!("https://thumbnails.padelgo.tv/{video_id}.jpg")),
        );
        info.insert_if_some("upload_date", start.and_then(date_digits));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native SZTV.hu VOD extractor.
pub struct SztvHuExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SztvHuExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SztvHuExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "SZTV URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let video_file = Regex::new(r#"(?is)\bfile\s*:\s*["'][^"']*?:([^"']+)["']\s*,"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().trim().to_owned())
            .map(|value| {
                value
                    .rsplit_once(':')
                    .filter(|(_, suffix)| !suffix.contains('/'))
                    .map_or(value.clone(), |(_, suffix)| suffix.to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("SZTV video {video_id} has no media file"),
                )
            })?;
        let title = html_meta_value(&html, "title")
            .map(|value| {
                value
                    .split(" - ")
                    .next()
                    .unwrap_or(&value)
                    .trim()
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let media_url = format!(
            "http://media.sztv.hu/vod/{}",
            video_file.trim_start_matches('/')
        );
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", html_meta_value(&html, "description"));
        info.insert_if_some("thumbnail", html_meta_value(&html, "og:image"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "source",
                "ext": "mp4",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Arnes Video public-media API extractor.
pub struct ArnesExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ArnesExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ArnesExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Arnes URL has no ID")
            })?;
        let response = context.get_json(&format!(
            "https://video.arnes.si/api/public/video/{video_id}"
        ))?;
        let video = response.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no data object"),
            )
        })?;
        let title = json_string(video, "title").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no title"),
            )
        })?;
        let media = video
            .get("media")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Arnes video {video_id} has no media records"),
                )
            })?;
        let mut formats = Vec::new();
        for item in media {
            let Some(raw_url) = json_string(item, "url") else {
                continue;
            };
            let media_url = resolve_url("https://video.arnes.si", raw_url);
            let format_id = json_string(item, "format")
                .and_then(|value| value.strip_prefix("FORMAT_"))
                .map(str::to_owned);
            let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "format_note": json_string(item, "formatTranslation"),
                "width": json_i64(item, "width"),
                "height": json_i64(item, "height"),
                "ext": ext,
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Arnes video {video_id} has no playable media"),
            )
        })?;
        let channel = video.get("channel").unwrap_or(&serde_json::Value::Null);
        let channel_id = json_string(channel, "url");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some(
            "thumbnail",
            json_string(video, "thumbnailUrl")
                .map(|value| resolve_url("https://video.arnes.si", value)),
        );
        info.insert_if_some("description", json_string(video, "description"));
        info.insert_if_some("license", json_string(video, "license"));
        info.insert_if_some("creator", json_string(video, "author"));
        info.insert_if_some(
            "timestamp",
            json_string(video, "creationTime")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("channel", json_string(channel, "name"));
        info.insert_if_some("channel_id", channel_id);
        info.insert_if_some(
            "channel_url",
            channel_id.map(|value| format!("https://video.arnes.si/?channel={value}")),
        );
        info.insert_if_some(
            "duration",
            json_f64(video, "duration").map(|milliseconds| milliseconds / 1000.0),
        );
        info.insert_if_some("view_count", json_i64(video, "views"));
        info.insert_if_some("tags", video.get("hashtags").cloned());
        info.insert_if_some(
            "start_time",
            url_query_value(url, "t").and_then(|value| value.parse::<i64>().ok()),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native CJSW episode audio-page extractor.
pub struct CjswExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl CjswExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for CjswExtractor {
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
                "CJSW URL did not match its native pattern",
            )
        })?;
        let program = captures
            .name("program")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no program")
            })?;
        let episode_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CJSW URL has no episode ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let title = Regex::new(
            r#"(?is)<h1\b[^>]*\bclass\s*=\s*["'][^"']*episode-header__title[^"']*["'][^>]*>([^<]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str().trim()))
        .or_else(|| {
            Regex::new(r#"(?is)\bdata-audio-title\s*=\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| unescape_html_attribute(value.as_str().trim()))
        })
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CJSW episode {episode_id} has no title"),
            )
        })?;
        let audio_url = Regex::new(r#"(?is)<button\b[^>]*\bdata-audio-src\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("CJSW episode {episode_id} has no audio URL"),
                )
            })?;
        let audio_id =
            Regex::new(r#"(?i)/([\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12})\.mp3"#)
                .ok()
                .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
                .unwrap_or_else(|| format!("{program}/{episode_id}"));
        let ext = yt_dlp_core::determine_ext(Some(&audio_url), "mp3");
        let description = Regex::new(r#"(?is)<p\b[^>]*>(.*?)</p>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let series = Regex::new(r#"(?is)\bdata-showname\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .unwrap_or(program);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("series", serde_json::json!(series));
        info.insert("episode_id", serde_json::json!(episode_id));
        info.insert("url", serde_json::json!(audio_url.clone()));
        info.insert("ext", serde_json::json!(ext));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "source",
                "ext": ext,
                "vcodec": "none",
            }]),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Daystar Lightcast configuration/HLS extractor.
pub struct DaystarClipExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DaystarClipExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DaystarClipExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Daystar URL has no ID")
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let iframe_url = Regex::new(r#"(?is)<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(url, value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no iframe"),
                )
            })?;
        let config_url = iframe_url.replace("player.php", "config2.php");
        let config_response = context.get(&config_url)?;
        let config_html = String::from_utf8_lossy(config_response.body());
        let sources = json_array_after_marker(&config_html, "sources")
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Daystar clip {video_id} has no source list"),
                )
            })?;
        let mut formats = Vec::new();
        for source in sources {
            let Some(raw_url) = json_string(&source, "file") else {
                continue;
            };
            if json_string(&source, "type").map(|value| value.eq_ignore_ascii_case("m3u8"))
                != Some(true)
            {
                continue;
            }
            let media_url = resolve_url("https://www.lightcast.com/embed/", raw_url);
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Daystar clip {video_id} has no HLS source"),
            )
        })?;
        let thumbnail = Regex::new(r#"(?is)\bimage\s*:\s*["']([^"']+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&config_html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| resolve_url(&config_url, value.as_str()));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            html_meta_value(&html, "og:title").or_else(|| html_meta_value(&html, "twitter:title")),
        );
        info.insert_if_some(
            "description",
            html_meta_value(&html, "og:description")
                .or_else(|| html_meta_value(&html, "twitter:description")),
        );
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native DCTP versioned REST/API extractor.
pub struct DctpTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DctpTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DctpTvExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "DCTP URL has no slug")
            })?;
        let base_url = "http://dctp-ivms2-restapi.s3.amazonaws.com";
        let version = context.get_json(&format!("{base_url}/version.json"))?;
        let version_name = json_string(&version, "version_name").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "DCTP version response has no version_name",
            )
        })?;
        let restapi_base = format!("{base_url}/{version_name}/restapi");
        let info = context.get_json(&format!("{restapi_base}/slugs/{display_id}.json"))?;
        let object_id = json_value_string(info.get("object_id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no object ID"),
            )
        })?;
        let media = context.get_json(&format!("{restapi_base}/media/{object_id}.json"))?;
        let uuid = json_string(&media, "uuid").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("DCTP film {display_id} has no media UUID"),
            )
        })?;
        let title = json_string(&media, "title").unwrap_or(&display_id);
        let is_wide = json_bool(&media, "is_wide").unwrap_or(false);
        let mut formats = Vec::new();
        let mut add_formats = |suffix: &str| {
            let filename = format!("{uuid}_dctp_{suffix}.m4v");
            formats.push(serde_json::json!({
                "format_id": format!("hls-{suffix}"),
                "url": format!("https://cdn-segments.dctp.tv/{filename}/playlist.m3u8"),
                "protocol": "m3u8_native",
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("s3-{suffix}"),
                "url": format!("https://completed-media.s3.amazonaws.com/{filename}"),
                "ext": "m4v",
            }));
            formats.push(serde_json::json!({
                "format_id": format!("http-{suffix}"),
                "url": format!("https://cdn-media.dctp.tv/{filename}"),
                "ext": "m4v",
            }));
        };
        add_formats(&format!("0500_{}", if is_wide { "16x9" } else { "4x3" }));
        if is_wide {
            add_formats("720p");
        }
        let thumbnails = media
            .get("images")
            .and_then(serde_json::Value::as_array)
            .map(|images| {
                serde_json::Value::Array(
                    images
                        .iter()
                        .filter_map(|image| {
                            let image_url = json_string(image, "url")?;
                            let mut thumbnail = serde_json::Map::new();
                            thumbnail.insert(
                                "url".to_owned(),
                                serde_json::Value::String(image_url.to_owned()),
                            );
                            if let Some(width) = json_i64(image, "width") {
                                thumbnail.insert("width".to_owned(), serde_json::json!(width));
                            }
                            if let Some(height) = json_i64(image, "height") {
                                thumbnail.insert("height".to_owned(), serde_json::json!(height));
                            }
                            Some(serde_json::Value::Object(thumbnail))
                        })
                        .collect(),
                )
            })
            .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()));
        let first = formats.first().cloned().expect("DCTP format");
        let mut result = InfoDict::new();
        result.insert("id", serde_json::json!(uuid));
        result.insert("display_id", serde_json::json!(display_id));
        result.insert("title", serde_json::json!(title));
        result.insert_if_some("alt_title", json_string(&media, "subtitle"));
        result.insert_if_some(
            "description",
            json_string(&media, "description").or_else(|| json_string(&media, "teaser")),
        );
        result.insert_if_some(
            "timestamp",
            json_string(&media, "created")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        result.insert_if_some(
            "duration",
            json_f64(&media, "duration_in_ms").map(|milliseconds| milliseconds / 1000.0),
        );
        result.insert_if_some("thumbnails", thumbnails);
        result.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        result.insert("ext", serde_json::json!("m4v"));
        result.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(result))
    }
}

/// Native APA embed/player extractor. JWPlatform-backed pages return an
/// explicit native redirect; older players expose direct HLS/progressive URLs.
pub struct ApaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ApaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ApaExtractor {
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
                "APA URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("base_url")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "APA URL has no base URL")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "APA URL has no ID")
            })?;
        let player_url = format!("{base_url}/player/{video_id}");
        let webpage = context.get(&player_url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let field = |name: &str| {
            let pattern = format!(r#"(?is)\b{}\s*:\s*["']([^"']+)["']"#, regex::escape(name));
            Regex::new(&pattern)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| value.as_str().to_owned())
        };
        if let Some(jwplatform_id) = Regex::new(r#"(?i)\bmedia[iI]d\s*:\s*["']([a-zA-Z0-9]{8})"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
        {
            return Ok(ExtractorResult::Redirect {
                url: format!("jwplatform:{jwplatform_id}"),
                ie_key: Some("JWPlatform".to_owned()),
            });
        }
        let title = field("title").unwrap_or_else(|| video_id.clone());
        let mut formats = Vec::new();
        if let Some(source_url) = field("hls").or_else(|| field("hlsUrl")) {
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": "hls",
                "protocol": "m3u8_native",
                "ext": "mp4",
            }));
        }
        if let Some(source_url) = field("progressive") {
            let height = Regex::new(r#"(?i)(\d+)\.mp4(?:$|[?#])"#)
                .ok()
                .and_then(|matcher| matcher.captures(&source_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok());
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": "progressive",
                "height": height,
                "ext": "mp4",
            }));
        }
        let first = formats.first().cloned().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("APA video {video_id} has no playable sources"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", field("description"));
        info.insert_if_some("thumbnail", field("poster"));
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        Ok(ExtractorResult::single(info))
    }
}

/// Native href.li redirect extractor. URL results are represented explicitly
/// so the Rust CLI can follow them without a compatibility runtime.
pub struct HrefLiRedirectExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl HrefLiRedirectExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for HrefLiRedirectExtractor {
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
        _context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let captures = self.matcher.captures(url).ok().flatten().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "href.li URL did not match its native pattern",
            )
        })?;
        let target = captures
            .name("url")
            .map(|value| percent_decode(value.as_str()))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "href.li URL has no target")
            })?;
        Ok(ExtractorResult::Redirect {
            url: target,
            ie_key: None,
        })
    }
}

/// Native Streamable AJAX extractor. Streamable's public API exposes the
/// complete media inventory, including the older records that do not have
/// video dimensions or codec metadata, so no browser runtime is needed.
pub struct StreamableExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl StreamableExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for StreamableExtractor {
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
                "Streamable URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Streamable URL has no ID")
            })?;
        let video = context.get_json(&format!("https://ajax.streamable.com/videos/{video_id}"))?;
        if json_i64(&video, "status") != Some(2) {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Streamable video {video_id} is unavailable or still processing"),
            ));
        }

        let files = video
            .get("files")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Streamable video {video_id} has no media files"),
                )
            })?;
        let mut formats = Vec::new();
        for (format_id, file) in files {
            let Some(raw_url) = json_string(file, "url") else {
                continue;
            };
            let media_url = proto_relative_url(raw_url, "https:");
            let mut format = serde_json::Map::new();
            format.insert("format_id".to_owned(), serde_json::json!(format_id));
            format.insert("url".to_owned(), serde_json::json!(media_url));
            format.insert(
                "ext".to_owned(),
                serde_json::json!(yt_dlp_core::determine_ext(Some(raw_url), "mp4")),
            );
            if let Some(value) = json_i64(file, "width") {
                format.insert("width".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "height") {
                format.insert("height".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "size") {
                format.insert("filesize".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(file, "framerate") {
                format.insert("fps".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_f64(file, "bitrate") {
                format.insert("vbr".to_owned(), serde_json::json!(value / 1000.0));
            }
            if let Some(metadata) = file.get("input_metadata") {
                if let Some(value) = json_string(metadata, "video_codec_name") {
                    format.insert("vcodec".to_owned(), serde_json::json!(value));
                }
                if let Some(value) = json_string(metadata, "audio_codec_name") {
                    format.insert("acodec".to_owned(), serde_json::json!(value));
                }
            }
            formats.push(serde_json::Value::Object(format));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Streamable video {video_id} has no playable media files"),
            ));
        }

        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(
                json_string(&video, "reddit_title")
                    .or_else(|| json_string(&video, "title"))
                    .unwrap_or(video_id)
            ),
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
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("description", json_string(&video, "description"));
        info.insert_if_some(
            "thumbnail",
            json_string(&video, "thumbnail_url").map(|value| proto_relative_url(value, "https:")),
        );
        info.insert_if_some(
            "uploader",
            video
                .get("owner")
                .and_then(|owner| json_string(owner, "user_name")),
        );
        info.insert_if_some("timestamp", json_f64(&video, "date_added"));
        info.insert_if_some("duration", json_f64(&video, "duration"));
        info.insert_if_some("view_count", json_i64(&video, "plays"));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Newgrounds media extractor. Newgrounds exposes either a direct
/// embedController URL or a JSON source list for the media page; both paths
/// are handled through the native request stack.
pub struct NewgroundsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsExtractor {
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
                "Newgrounds URL did not match its native pattern",
            )
        })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Newgrounds URL has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());

        let direct_media_url =
            Regex::new(r#"(?is)embedController\(\s*\[\s*\{\s*"url"\s*:\s*("(?:\\.|[^"\\])*")"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str()))
                .and_then(decode_json_string);

        let mut formats = Vec::new();
        let mut uploader = None;
        if let Some(media_url) = direct_media_url {
            formats.push(serde_json::json!({
                "url": proto_relative_url(&media_url, "https:"),
                "format_id": "source",
                "quality": 1,
                "ext": yt_dlp_core::determine_ext(Some(&media_url), "mp4"),
            }));
        } else {
            let json_video = native_get_json_with_headers(
                context,
                &format!("https://www.newgrounds.com/portal/video/{media_id}"),
                &[
                    ("Accept", "application/json"),
                    ("Referer", url),
                    ("X-Requested-With", "XMLHttpRequest"),
                ],
            )?;
            uploader = json_string(&json_video, "author").map(str::to_owned);
            if let Some(sources) = json_video
                .get("sources")
                .and_then(serde_json::Value::as_object)
            {
                for (format_id, source_list) in sources {
                    let quality = format_id
                        .trim_end_matches(|character: char| character == 'p' || character == 'P')
                        .parse::<i64>()
                        .ok();
                    for media_url in json_media_urls(source_list) {
                        formats.push(serde_json::json!({
                            "url": proto_relative_url(&media_url, "https:"),
                            "format_id": format_id,
                            "quality": quality,
                            "ext": yt_dlp_core::determine_ext(Some(&media_url), "mp4"),
                        }));
                    }
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds media {media_id} has no playable formats"),
            ));
        }
        if uploader.is_none() {
            uploader = Regex::new(r#"(?is)<h4[^>]*>(.*?)</h4>.*?<em>\s*(?:Author|Artist)\s*</em>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|value| !value.is_empty());
        }
        if uploader.is_none() {
            uploader = Regex::new(r#"(?is)(?:Author|Writer)\s*<a[^>]*>(.*?)</a>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| {
                    captures
                        .get(1)
                        .map(|value| html_text_fragment(value.as_str()))
                })
                .filter(|value| !value.is_empty());
        }

        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(media_id));
        info.insert(
            "title",
            serde_json::json!(html_title_value(&webpage).unwrap_or_else(|| media_id.to_owned())),
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
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("uploader", uploader);
        info.insert_if_some(
            "timestamp",
            html_attribute_value(&webpage, "itemprop", "uploadDate")
                .or_else(|| html_attribute_value(&webpage, "itemprop", "datePublished"))
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "duration",
            html_json_number(&webpage, "duration").and_then(|value| value.parse::<f64>().ok()),
        );
        info.insert_if_some("thumbnail", html_meta_value(&webpage, "og:image"));
        let description = html_element_by_id(&webpage, "author_comments")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| html_meta_value(&webpage, "og:description"));
        info.insert_if_some("description", description);
        let age_limit = Regex::new(r#"(?is)<h2\s+class=["']rated-([etma])["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| match value.as_str() {
                "e" => Some(0),
                "t" => Some(13),
                "m" => Some(17),
                "a" => Some(18),
                _ => None,
            });
        info.insert_if_some("age_limit", age_limit);
        info.insert_if_some(
            "view_count",
            Regex::new(r#"(?is)<dt>\s*(?:Views|Listens)\s*</dt>\s*<dd>\s*([\d\.,]+)\s*</dd>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| {
                    value
                        .as_str()
                        .replace(',', "")
                        .replace('.', "")
                        .parse::<i64>()
                        .ok()
                }),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Newgrounds collection/search extractor. Entries are materialized
/// through NewgroundsExtractor so playlist selection can operate entirely on
/// Rust InfoDict values.
pub struct NewgroundsPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsPlaylistExtractor {
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
                "Newgrounds collection URL did not match its native pattern",
            )
        })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Newgrounds collection URL has no ID",
                )
            })?;
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let links = newgrounds_media_links(&html);
        let entries = extract_newgrounds_entries(context, &links)?;
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds collection {playlist_id} has no media entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", html_title_value(&html));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Newgrounds user listing extractor. The JSON page endpoint is
/// paginated, so pages are fetched until the service returns an empty page.
pub struct NewgroundsUserExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NewgroundsUserExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NewgroundsUserExtractor {
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
                "Newgrounds user URL did not match its native pattern",
            )
        })?;
        let user_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Newgrounds user URL has no ID",
                )
            })?;
        let mut links = Vec::new();
        for page in 1..=1_000 {
            let page_url = if url.contains('?') {
                format!("{url}&page={page}")
            } else {
                format!("{url}?page={page}")
            };
            let response = native_get_json_with_headers(
                context,
                &page_url,
                &[
                    ("Accept", "application/json, text/javascript, */*; q=0.01"),
                    ("X-Requested-With", "XMLHttpRequest"),
                ],
            )?;
            let Some(items) = response.get("items").and_then(serde_json::Value::as_array) else {
                break;
            };
            if items.is_empty() {
                break;
            }
            for item in items {
                for fragment in json_text_values(item) {
                    for link in newgrounds_media_links(fragment) {
                        if !links.contains(&link) {
                            links.push(link);
                        }
                    }
                }
            }
        }
        let entries = extract_newgrounds_entries(context, &links)?;
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Newgrounds user {user_id} has no media entries"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(user_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn newgrounds_media_extractor() -> Result<NewgroundsExtractor, ExtractorError> {
    NewgroundsExtractor::new(ExtractorDescriptor::new(
        "NewgroundsIE",
        "Newgrounds",
        r"https?://(?:www\.)?newgrounds\.com/(?:audio/listen|portal/view)/(?P<id>\d+)(?:/format/flash)?",
        true,
    ))
}

fn newgrounds_media_links(value: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(
        r#"(?is)href\s*=\s*["'](?:https?://(?:www\.)?newgrounds\.com)?/?((?:portal/view|audio/listen)/(\d+))"#,
    ) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for captures in matcher.captures_iter(value).flatten() {
        let Some(path) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let link = format!("https://www.newgrounds.com/{path}");
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

fn json_text_values(value: &serde_json::Value) -> Vec<&str> {
    match value {
        serde_json::Value::String(value) => vec![value.as_str()],
        serde_json::Value::Array(values) => values.iter().flat_map(json_text_values).collect(),
        serde_json::Value::Object(values) => values.values().flat_map(json_text_values).collect(),
        _ => Vec::new(),
    }
}

fn extract_newgrounds_entries(
    context: &ExtractionContext,
    links: &[String],
) -> Result<Vec<InfoDict>, ExtractorError> {
    let extractor = newgrounds_media_extractor()?;
    links
        .iter()
        .map(
            |link| match extractor.extract_with_context(link, context)? {
                ExtractorResult::Single(info) => Ok(info),
                ExtractorResult::Redirect { .. } => Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Newgrounds media entry unexpectedly returned a redirect",
                )),
                ExtractorResult::Playlist { .. } => Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Newgrounds media entry unexpectedly returned a playlist",
                )),
            },
        )
        .collect()
}

/// Native Wistia media extractor backed by the embed JSON endpoint.
pub struct WistiaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaExtractor {
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
                "Wistia URL did not match its native pattern",
            )
        })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Wistia URL has no ID")
            })?;
        let config = wistia_embed_config(context, "medias", media_id, url)?;
        Ok(ExtractorResult::single(wistia_media_info(&config)?))
    }
}

/// Native Wistia playlist extractor. Wistia playlist responses embed media
/// configs, which are materialized so the Rust CLI can select entries without
/// lazy Python callbacks.
pub struct WistiaPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaPlaylistExtractor {
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
                "Wistia playlist URL did not match its native pattern",
            )
        })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Wistia playlist URL has no ID",
                )
            })?;
        let config = wistia_embed_config(context, "playlists", playlist_id, url)?;
        let media_values = config
            .as_array()
            .and_then(|values| values.first())
            .and_then(|value| value.get("medias"))
            .and_then(serde_json::Value::as_array)
            .or_else(|| config.get("medias").and_then(serde_json::Value::as_array))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Wistia playlist {playlist_id} has no media list"),
                )
            })?;
        let mut entries = Vec::new();
        for media in media_values {
            let Some(embed_config) = media.get("embed_config") else {
                continue;
            };
            entries.push(wistia_media_info(embed_config)?);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Wistia playlist {playlist_id} has no playable media"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Wistia channel extractor. API-backed channels are materialized into
/// media InfoDicts; webpage JSONP fallback and password prompts are explicit
/// TODOs because they need additional native configuration inputs.
pub struct WistiaChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl WistiaChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for WistiaChannelExtractor {
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
                "Wistia channel URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Wistia channel URL has no ID",
                )
            })?;
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(media_id) = parsed
                .query_pairs()
                .find_map(|(key, value)| {
                    matches!(key.as_ref(), "wmediaid" | "wvideoid" | "wvideo")
                        .then(|| value.into_owned())
                })
                .filter(|media_id| !media_id.is_empty())
            {
                let extractor = WistiaExtractor::new(ExtractorDescriptor::new(
                    "WistiaIE",
                    "Wistia",
                    r"(?:wistia:|https?://(?:\w+\.)?wistia\.(?:net|com)/(?:embed/)?(?:iframe|medias)/)(?P<id>[a-z0-9]{10})",
                    true,
                ))?;
                return extractor.extract_with_context(&format!("wistia:{media_id}"), context);
            }
        }
        let config = wistia_embed_config(context, "channel", channel_id, url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Wistia channel webpage JSONP fallback is not ported ({error})"),
            )
        })?;
        let series = config
            .get("series")
            .and_then(serde_json::Value::as_array)
            .and_then(|series| series.first())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Wistia channel {channel_id} has no series"),
                )
            })?;
        let mut media_ids: Vec<String> = Vec::new();
        if let Some(sections) = series.get("sections").and_then(serde_json::Value::as_array) {
            for section in sections {
                for key in ["videos", "episodes"] {
                    let Some(values) = section.get(key).and_then(serde_json::Value::as_array)
                    else {
                        continue;
                    };
                    for value in values {
                        let Some(media_id) = json_string(value, "hashedId") else {
                            continue;
                        };
                        if !media_ids.iter().any(|existing| existing == media_id) {
                            media_ids.push(media_id.to_owned());
                        }
                    }
                }
            }
        }
        let mut entries = Vec::new();
        for media_id in media_ids {
            let media_config = wistia_embed_config(context, "medias", &media_id, url)?;
            entries.push(wistia_media_info(&media_config)?);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Wistia channel {channel_id} has no playable media"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert_if_some("title", json_string(series, "title"));
        info.insert_if_some("description", json_string(series, "description"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn wistia_embed_config(
    context: &ExtractionContext,
    config_type: &str,
    config_id: &str,
    referer: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let config = native_get_json_with_headers(
        context,
        &format!("https://fast.wistia.net/embed/{config_type}/{config_id}.json"),
        &[("Referer", referer)],
    )?;
    if let Some(error) = json_string(&config, "error") {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Wistia {config_type} {config_id}: {error}"),
        ));
    }
    if config
        .get("media")
        .and_then(|media| {
            media
                .get("embed_options")
                .or_else(|| media.get("embedOptions"))
        })
        .and_then(|options| options.get("plugin"))
        .and_then(|plugin| plugin.get("passwordProtectedVideo"))
        .and_then(|password| password.get("on"))
        .and_then(serde_json::Value::as_str)
        == Some("true")
    {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            "TODO: Wistia password-protected media requires a native video-password option",
        ));
    }
    Ok(config)
}

fn wistia_media_info(config: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let media = config.get("media").unwrap_or(config);
    let media_id = json_string(media, "hashedId").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Wistia media config has no hashedId",
        )
    })?;
    let title = json_string(media, "name").unwrap_or(media_id);
    let mut formats = Vec::new();
    let mut thumbnails = Vec::new();
    if let Some(assets) = media.get("assets").and_then(serde_json::Value::as_array) {
        for asset in assets {
            let Some(raw_url) = json_string(asset, "url") else {
                continue;
            };
            if json_i64(asset, "status").is_some_and(|status| status != 2) {
                continue;
            }
            let asset_type = json_string(asset, "type").unwrap_or("unknown");
            if matches!(asset_type, "preview" | "storyboard") {
                continue;
            }
            if matches!(asset_type, "still" | "still_image") {
                thumbnails.push(serde_json::json!({
                    "url": wistia_replace_bin_extension(raw_url, asset),
                    "width": json_i64(asset, "width"),
                    "height": json_i64(asset, "height"),
                    "filesize": json_i64(asset, "size"),
                }));
                continue;
            }
            let display_name = json_string(asset, "display_name");
            let format_id = if asset_type.ends_with("_video") {
                display_name
                    .map(|display_name| {
                        format!("{}-{display_name}", asset_type.trim_end_matches("_video"))
                    })
                    .unwrap_or_else(|| asset_type.to_owned())
            } else {
                asset_type.to_owned()
            };
            let container = json_string(asset, "container");
            let asset_ext = json_string(asset, "ext")
                .map(str::to_owned)
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(raw_url), "bin"));
            let is_hls = container == Some("m3u8") || asset_ext == "m3u8";
            let mut format = serde_json::json!({
                "format_id": format_id,
                "url": raw_url,
                "quality": if asset_type == "original" { 1 } else { 0 },
                "tbr": json_i64(asset, "bitrate"),
            });
            if display_name == Some("Audio") {
                format["vcodec"] = serde_json::json!("none");
            } else {
                format["width"] = json_i64(asset, "width")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["height"] = json_i64(asset, "height")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["vcodec"] = json_string(asset, "codec")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
            }
            if is_hls {
                let mut hls = format.clone();
                hls["ext"] = serde_json::json!("mp4");
                hls["protocol"] = serde_json::json!("m3u8_native");
                formats.push(hls);
                let mut ts = format;
                ts["format_id"] = serde_json::json!(format_id.replace("hls-", "ts-"));
                ts["url"] = serde_json::json!(raw_url.replace(".bin", ".ts"));
                ts["ext"] = serde_json::json!("ts");
                formats.push(ts);
            } else {
                format["container"] =
                    container.map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                format["ext"] = serde_json::json!(asset_ext);
                format["filesize"] = json_i64(asset, "size")
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value));
                formats.push(format);
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Wistia media {media_id} has no playable assets"),
        ));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(media_id));
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
    info.insert_if_some("description", json_string(media, "seoDescription"));
    info.insert_if_some("duration", json_f64(media, "duration"));
    info.insert_if_some("timestamp", json_i64(media, "createdAt"));
    if !thumbnails.is_empty() {
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
    }
    if let Some(captions) = media.get("captions").and_then(serde_json::Value::as_array) {
        let mut subtitles = serde_json::Map::new();
        for caption in captions {
            let Some(language) = json_string(caption, "language") else {
                continue;
            };
            subtitles.insert(
                language.to_owned(),
                serde_json::json!([{
                    "url": format!("https://fast.wistia.net/embed/captions/{media_id}.vtt?language={language}")
                }]),
            );
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    Ok(info)
}

fn wistia_replace_bin_extension(url: &str, asset: &serde_json::Value) -> String {
    let extension = json_string(asset, "ext")
        .map(str::to_owned)
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(url), "jpg"));
    url.replace(".bin", &format!(".{extension}"))
}

/// Native VidLii page extractor. Media URLs are embedded in the player
/// configuration and are checked with native HEAD requests before exposure.
pub struct VidLiiExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl VidLiiExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for VidLiiExtractor {
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
                "VidLii URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "VidLii URL has no ID")
            })?;
        let page_url = format!("https://www.vidlii.com/watch?v={video_id}");
        let webpage_response = context.get(&page_url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let parsed_page = url::Url::parse(&page_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid VidLii page URL: {error}"),
            )
        })?;

        let source_matcher =
            Regex::new(r#"(?is)\bsrc\s*:\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid VidLii source matcher: {error}"),
                )
            })?;
        let height_matcher = Regex::new(r#"(?i)(\d+)\.mp4"#).ok();
        let mut formats = Vec::new();
        for captures in source_matcher.captures_iter(&webpage).flatten() {
            let Some(raw_url) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let source_url = parsed_page
                .join(&proto_relative_url(raw_url, "https:"))
                .map(|value| value.to_string())
                .unwrap_or_else(|_| raw_url.to_owned());
            let height = height_matcher
                .as_ref()
                .and_then(|matcher| matcher.captures(&source_url).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok())
                .unwrap_or(360);
            let mut request = Request::new(&source_url);
            request.set_method("HEAD").map_err(map_request_error)?;
            if context.request(&request).is_err() {
                continue;
            }
            formats.push(serde_json::json!({
                "url": source_url,
                "format_id": format!("{height}p"),
                "height": height,
                "ext": "mp4",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("VidLii video {video_id} has no playable source URLs"),
            ));
        }

        let title = Regex::new(r#"(?is)<h1\b[^>]*>(.*?)</h1>"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| html_text_fragment(value.as_str()))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                html_title_value(&webpage)
                    .map(|value| value.trim_end_matches(" - VidLii").trim().to_owned())
            })
            .unwrap_or_else(|| video_id.to_owned());
        let description = html_meta_value(&webpage, "description")
            .or_else(|| html_meta_value(&webpage, "twitter:description"))
            .or_else(|| {
                html_element_by_id(&webpage, "des_text")
                    .map(|value| html_text_fragment(&value))
                    .filter(|value| !value.is_empty())
            });
        let thumbnail = html_meta_value(&webpage, "twitter:image").or_else(|| {
            Regex::new(r#"(?is)\bimg\s*:\s*["']([^"']+)["']"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1).map(|value| value.as_str()))
                .and_then(|value| {
                    parsed_page
                        .join(&proto_relative_url(value, "https:"))
                        .ok()
                        .map(|value| value.to_string())
                })
        });
        let (uploader_id, uploader) = Regex::new(
            r#"(?is)<div[^>]*class=["'][^"']*\bwt_person\b[^"']*["'][^>]*>\s*<a[^>]*href=["']/user/([^"'/?#]+)["'][^>]*>(.*?)</a>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .map(|captures| {
            let uploader_id = captures.get(1).map(|value| value.as_str().to_owned());
            let uploader = captures
                .get(2)
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
            (uploader_id, uploader)
        })
        .unwrap_or((None, None));
        let upload_date = html_meta_value(&webpage, "datePublished")
            .or_else(|| {
                Regex::new(r#"(?is)<date\b[^>]*>([^<]+)"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                    .and_then(|captures| {
                        captures
                            .get(1)
                            .map(|value| value.as_str().trim().to_owned())
                    })
            })
            .and_then(parse_timestamp);
        let duration = html_meta_value(&webpage, "video:duration")
            .or_else(|| html_json_number(&webpage, "duration"))
            .and_then(|value| value.parse::<f64>().ok());
        let view_count = Regex::new(
            r#"(?is)(?:<strong>\s*([0-9,]+)\s*</strong>\s*views|Views\s*:\s*<strong>\s*([0-9,]+)\s*</strong>)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().replace(',', "").parse::<i64>().ok());
        let comment_count = Regex::new(
            r#"(?is)(?:<span[^>]*id=["']cmt_num["'][^>]*>\s*(\d+)|Comments\s*:\s*<strong>\s*(\d+))"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .and_then(|value| value.as_str().parse::<i64>().ok());
        let average_rating = Regex::new(r#"(?is)\brating\s*:\s*([0-9.]+)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .and_then(|value| value.as_str().parse::<f64>().ok());
        let category =
            Regex::new(r#"(?is)<div>\s*Category\s*:\s*</div>\s*<div>\s*<a[^>]*>([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty());
        let tags =
            Regex::new(r#"(?is)<a[^>]*\bhref=["']/results\?[^"']*\bq=[^"']*["'][^>]*>([^<]+)</a>"#)
                .ok()
                .map(|matcher| {
                    matcher
                        .captures_iter(&webpage)
                        .flatten()
                        .filter_map(|captures| captures.get(1))
                        .map(|value| html_text_fragment(value.as_str()))
                        .filter(|value| !value.is_empty())
                        .map(serde_json::Value::String)
                        .collect::<Vec<_>>()
                })
                .filter(|values| !values.is_empty());
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
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "uploader_url",
            uploader_id
                .as_deref()
                .map(|value| format!("https://www.vidlii.com/user/{value}")),
        );
        info.insert_if_some("uploader_id", uploader_id);
        info.insert_if_some("uploader", uploader);
        info.insert_if_some("timestamp", upload_date);
        info.insert_if_some("duration", duration);
        info.insert_if_some("view_count", view_count);
        info.insert_if_some("comment_count", comment_count);
        info.insert_if_some("average_rating", average_rating);
        info.insert_if_some("categories", category.map(|value| vec![value]));
        info.insert_if_some("tags", tags);
        Ok(ExtractorResult::single(info))
    }
}

/// Native PeerTube v1 video API extractor. PeerTube instances share one
/// metadata contract, so the generated URL matcher supplies the instance
/// host and this implementation handles files, streaming playlists, captions,
/// and common account/channel metadata without browser code.
pub struct PeerTubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PeerTubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PeerTubeExtractor {
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
                "PeerTube URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .or_else(|| captures.name("host_2"))
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no host")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no ID")
            })?;
        let api_base = format!("https://{host}/api/v1/videos/{video_id}");
        let video = context.get_json(&api_base)?;
        if let Some(error) = json_string(&video, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube API rejected {video_id}: {error}"),
            ));
        }
        let title = json_string(&video, "name").unwrap_or(video_id).to_owned();
        let mut formats = Vec::new();
        let mut is_live = false;
        if let Some(playlists) = video
            .get("streamingPlaylists")
            .and_then(serde_json::Value::as_array)
        {
            for playlist in playlists {
                let Some(playlist_url) = json_string(playlist, "playlistUrl") else {
                    continue;
                };
                is_live = true;
                formats.push(serde_json::json!({
                    "url": playlist_url,
                    "format_id": "hls",
                    "ext": "mp4",
                    "protocol": "m3u8_native",
                }));
                if let Some(playlist_files) =
                    playlist.get("files").and_then(serde_json::Value::as_array)
                {
                    for file in playlist_files {
                        add_peertube_file_format(file, &mut formats);
                    }
                }
            }
        }
        if let Some(files) = video.get("files").and_then(serde_json::Value::as_array) {
            for file in files {
                add_peertube_file_format(file, &mut formats);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube video {video_id} has no playable formats"),
            ));
        }

        let parsed_page = url::Url::parse(&format!("https://{host}")).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid PeerTube host {host}: {error}"),
            )
        })?;
        let webpage_url = format!("https://{host}/videos/watch/{video_id}");
        let thumbnail = json_string(&video, "thumbnailPath")
            .and_then(|path| parsed_page.join(path).ok().map(|value| value.to_string()));
        let description = if json_string(&video, "description")
            .is_some_and(|description| description.len() >= 250)
        {
            context
                .get_json(&format!("{api_base}/description"))
                .ok()
                .and_then(|value| json_string(&value, "description").map(str::to_owned))
                .or_else(|| json_string(&video, "description").map(str::to_owned))
        } else {
            json_string(&video, "description").map(str::to_owned)
        };
        let account = video.get("account").unwrap_or(&serde_json::Value::Null);
        let channel = video.get("channel").unwrap_or(&serde_json::Value::Null);
        let category = video
            .get("category")
            .and_then(|value| json_string(value, "label"))
            .map(|value| vec![serde_json::json!(value)]);
        let subtitles = peertube_subtitles(host, video_id, context);
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
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some(
            "timestamp",
            json_string(&video, "publishedAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some("uploader", json_string(account, "displayName"));
        info.insert_if_some(
            "uploader_id",
            json_i64(account, "id").map(|value| value.to_string()),
        );
        info.insert_if_some("uploader_url", json_string(account, "url"));
        info.insert_if_some("channel", json_string(channel, "displayName"));
        info.insert_if_some(
            "channel_id",
            json_i64(channel, "id").map(|value| value.to_string()),
        );
        info.insert_if_some("channel_url", json_string(channel, "url"));
        info.insert_if_some(
            "language",
            video
                .get("language")
                .and_then(|language| json_string(language, "id")),
        );
        info.insert_if_some(
            "license",
            video
                .get("licence")
                .or_else(|| video.get("license"))
                .and_then(|license| json_string(license, "label")),
        );
        info.insert_if_some("duration", json_i64(&video, "duration"));
        info.insert_if_some("view_count", json_i64(&video, "views"));
        info.insert_if_some("like_count", json_i64(&video, "likes"));
        info.insert_if_some("dislike_count", json_i64(&video, "dislikes"));
        info.insert_if_some(
            "age_limit",
            json_bool(&video, "nsfw").map(|value| i64::from(value) * 18),
        );
        info.insert_if_some("tags", video.get("tags").cloned());
        info.insert_if_some("categories", category);
        info.insert_if_some("subtitles", subtitles);
        info.insert("is_live", serde_json::json!(is_live));
        info.insert("webpage_url", serde_json::json!(webpage_url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native PeerTube account/channel/playlist extractor. The API is paginated
/// with stable offsets; entries are expanded through the native video
/// extractor so playlist downloads never need a Python callback or URL-result
/// compatibility layer.
pub struct PeerTubePlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl PeerTubePlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for PeerTubePlaylistExtractor {
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
                "PeerTube playlist URL did not match its native pattern",
            )
        })?;
        let host = captures
            .name("host")
            .or_else(|| captures.name("host_2"))
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "PeerTube URL has no host")
            })?;
        let playlist_type = captures
            .name("type")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "PeerTube playlist URL has no resource type",
                )
            })?;
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "PeerTube playlist URL has no resource ID",
                )
            })?;
        let api_resource = match playlist_type {
            "a" => "accounts",
            "c" => "video-channels",
            "w/p" => "video-playlists",
            _ => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!("TODO: unsupported PeerTube resource type {playlist_type}"),
                ));
            }
        };
        let api_base = format!("https://{host}/api/v1/{api_resource}/{playlist_id}");
        let playlist = context.get_json(&api_base)?;
        if let Some(error) = json_string(&playlist, "error") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("PeerTube API rejected {playlist_id}: {error}"),
            ));
        }

        let video_extractor = PeerTubeExtractor::new(ExtractorDescriptor::new(
            "PeerTubeIE",
            "PeerTube",
            r"https?://(?P<host>[^/]+)/w/(?P<id>[^/?#]+)",
            true,
        ))?;
        const PAGE_SIZE: usize = 30;
        let mut entries = Vec::new();
        for page in 0usize.. {
            let start = page.checked_mul(PAGE_SIZE).ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    "TODO: PeerTube playlist pagination exceeded native bounds",
                )
            })?;
            let response = context.get_json(&format!(
                "{api_base}/videos?sort=-createdAt&start={start}&count={PAGE_SIZE}&nsfw=both"
            ))?;
            let data = response
                .get("data")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let page_len = data.len();
            for video in data {
                let short_uuid = json_string(&video, "shortUUID").or_else(|| {
                    video
                        .get("video")
                        .and_then(|nested| json_string(nested, "shortUUID"))
                });
                let Some(short_uuid) = short_uuid else {
                    continue;
                };
                let entry_url = format!("https://{host}/w/{short_uuid}");
                let entry = video_extractor
                    .extract_with_context(&entry_url, context)
                    .map_err(|error| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("PeerTube playlist entry {short_uuid}: {error}"),
                        )
                    })?;
                match entry {
                    ExtractorResult::Single(info) => entries.push(info),
                    ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("PeerTube entry {short_uuid} returned a non-media result"),
                        ));
                    }
                }
            }
            if page_len < PAGE_SIZE {
                break;
            }
        }

        let thumbnail = json_string(&playlist, "thumbnailPath").and_then(|path| {
            url::Url::parse(&format!("https://{host}"))
                .ok()?
                .join(path)
                .ok()
                .map(|value| value.to_string())
        });
        let owner = playlist
            .get("ownerAccount")
            .or_else(|| playlist.get("owner"))
            .unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some(
            "title",
            json_string(&playlist, "displayName").or_else(|| json_string(&playlist, "name")),
        );
        info.insert_if_some("description", json_string(&playlist, "description"));
        info.insert_if_some(
            "timestamp",
            json_string(&playlist, "createdAt")
                .map(str::to_owned)
                .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "channel",
            json_string(owner, "name").or_else(|| json_string(&playlist, "displayName")),
        );
        info.insert_if_some(
            "channel_id",
            json_value_string(owner.get("id").or_else(|| playlist.get("id"))),
        );
        info.insert_if_some("thumbnail", thumbnail);
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

/// Native Rumble page wrapper. Canonical pages embed the same u3 player
/// endpoint used by RumbleEmbedExtractor; page-level counters and description
/// are merged after extracting that native media record.
pub struct RumbleExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleExtractor {
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
        let page_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble page has no ID")
            })?;
        let webpage_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(webpage_response.body());
        let embed_id = Regex::new(
            r#"(?is)(?:rumble\.com/embed/|["']embedUrl["']\s*:\s*["'](?:https?:)?//rumble\.com/embed/|<iframe[^>]+\bsrc=["'](?:https?:)?//rumble\.com/embed/|Rumble\(\s*["']play["']\s*,\s*\{[^}]*["']?video["']?\s*:\s*["'])([0-9a-z]+)"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Rumble page {page_id} has no native embed URL"),
            )
        })?;
        let embed_extractor = RumbleEmbedExtractor::new(ExtractorDescriptor::new(
            "RumbleEmbedIE",
            "RumbleEmbed",
            r"https?://(?:www\.)?rumble\.com/embed/(?:[0-9a-z]+\.)?(?P<id>[0-9a-z]+)",
            true,
        ))?;
        let mut info = match embed_extractor
            .extract_with_context(&format!("https://rumble.com/embed/{embed_id}"), context)?
        {
            ExtractorResult::Single(info) => info,
            ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Rumble embed unexpectedly returned a non-media result",
                ));
            }
        };
        info.insert_if_some(
            "release_timestamp",
            Regex::new(
                r#"(?is)(?:Livestream begins|Streamed on):\s*<time[^>]*datetime=["']([^"']+)"#,
            )
            .ok()
            .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .and_then(parse_timestamp),
        );
        info.insert_if_some(
            "view_count",
            Regex::new(r#"(?is)"userInteractionCount"\s*:\s*(\d+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<i64>().ok()),
        );
        info.insert_if_some(
            "like_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_up_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "dislike_count",
            Regex::new(r#"(?is)<span[^>]*data-js=["']rumbles_down_votes["'][^>]*>\s*([^<]+)"#)
                .ok()
                .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                .and_then(|captures| captures.get(1))
                .and_then(|value| parse_compact_count(value.as_str())),
        );
        info.insert_if_some(
            "description",
            html_element_by_class(&webpage, "media-description")
                .map(|value| html_text_fragment(&value))
                .filter(|value| !value.is_empty()),
        );
        Ok(ExtractorResult::single(info))
    }
}

/// Native Rumble channel/user listing extractor. Rumble exposes the same
/// video-card markup on both channel and user pages; pagination ends with an
/// empty page or a native HTTP 404, matching the source extractor's behavior.
pub struct RumbleChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleChannelExtractor {
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
                "Rumble channel URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("url")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| url.to_owned());
        let playlist_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble channel has no ID")
            })?;
        let video_extractor = RumbleExtractor::new(ExtractorDescriptor::new(
            "RumbleIE",
            "Rumble",
            r"https?://(?:www\.)?rumble\.com/(?P<id>v[\w.-]+)[^/]*$",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for page in 1..=10_000usize {
            let page_url = format!("{base_url}?page={page}");
            let response = match context.get(&page_url) {
                Ok(response) => response,
                Err(error) if error.message.contains("HTTP 404") => break,
                Err(error) => return Err(error),
            };
            let html = String::from_utf8_lossy(response.body());
            let links = rumble_channel_video_links(&html);
            if links.is_empty() {
                break;
            }
            for link in links {
                if seen_links.contains(&link) {
                    continue;
                }
                seen_links.push(link.clone());
                let entry = video_extractor
                    .extract_with_context(&link, context)
                    .map_err(|error| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link}: {error}"),
                        )
                    })?;
                match entry {
                    ExtractorResult::Single(info) => entries.push(info),
                    ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                        return Err(ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Rumble channel entry {link} returned a non-media result"),
                        ));
                    }
                }
            }
        }
        Ok(ExtractorResult::Playlist {
            info: {
                let mut info = InfoDict::new();
                info.insert("id", serde_json::json!(playlist_id));
                info
            },
            entries,
        })
    }
}

fn rumble_channel_video_links(html: &str) -> Vec<String> {
    let Ok(anchor_matcher) = Regex::new(r"(?is)<a\b([^>]+)>") else {
        return Vec::new();
    };
    let Ok(href_matcher) = Regex::new(r#"(?is)\bhref\s*=\s*[\"']([^\"']+)"#) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for captures in anchor_matcher.captures_iter(html).flatten() {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let class_attributes = attributes.to_ascii_lowercase();
        if !class_attributes.contains("videostream__link")
            && !class_attributes.contains("video-item--a")
        {
            continue;
        }
        let Some(href) = href_matcher
            .captures(attributes)
            .ok()
            .flatten()
            .and_then(|value| {
                value
                    .get(1)
                    .map(|value| unescape_html_attribute(value.as_str()))
            })
        else {
            continue;
        };
        let Some(link) = url::Url::parse("https://rumble.com/")
            .ok()
            .and_then(|base| base.join(&href).ok())
            .map(|value| value.to_string())
        else {
            continue;
        };
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

/// Native Slideshare video extractor. The legacy page contains a JSON object
/// assigned to slideshare_object; extracting that object directly avoids a
/// browser or embedded interpreter.
pub struct SlideshareExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SlideshareExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SlideshareExtractor {
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
                "Slideshare URL did not match its native pattern",
            )
        })?;
        let page_title = captures
            .name("title")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| "slideshare".to_owned());
        let webpage = context.get(url)?;
        let html = String::from_utf8_lossy(webpage.body());
        let payload = json_object_after_marker(&html, "slideshare_object,").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Slideshare page {page_title} has no slideshare_object JSON"),
            )
        })?;
        let slideshow = payload.get("slideshow").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no slideshow metadata",
            )
        })?;
        let slideshow_type = json_string(slideshow, "type").unwrap_or("unknown");
        if slideshow_type != "video" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: Slideshare slideshow type {slideshow_type:?} is not a video"),
            ));
        }
        let player = payload.get("jsplayer").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no jsplayer metadata",
            )
        })?;
        let document = json_string(&payload, "doc").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no document name",
            )
        })?;
        let bucket = json_string(player, "video_bucket").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare object has no video bucket",
            )
        })?;
        let extension = json_string(player, "video_extension").unwrap_or("mp4");
        let bucket_url =
            url::Url::parse(&proto_relative_url(bucket, "https:")).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video bucket {bucket:?}: {error}"),
                )
            })?;
        let video_url = bucket_url
            .join(&format!("{document}-SD.{extension}"))
            .map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Slideshare video path: {error}"),
                )
            })?
            .to_string();
        let slideshow_id = json_value_string(slideshow.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Slideshare slideshow has no ID",
            )
        })?;
        let title = json_string(slideshow, "title")
            .map(str::to_owned)
            .unwrap_or(page_title);
        let description = html_element_by_id(&html, "slideshow-description-paragraph")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                Regex::new(r#"(?is)<p[^>]*\bitemprop\s*=\s*["']description["'][^>]*>(.*?)</p>"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&html).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.is_empty())
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(slideshow_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(video_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": video_url,
                "format_id": "sd",
                "ext": extension,
            }]),
        );
        info.insert_if_some("thumbnail", json_string(slideshow, "pin_image_url"));
        info.insert_if_some("description", description);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Soundgasm single-audio extractor. Audio URLs and metadata are
/// embedded in the page's jPlayer markup and require no JavaScript execution.
pub struct SoundgasmExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmExtractor {
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
                "Soundgasm URL did not match its native pattern",
            )
        })?;
        let user = captures
            .name("user")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no user")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Soundgasm URL has no title")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let audio_url = Regex::new(r#"\bm4a\s*:\s*["']([^"']+)["']"#)
            .ok()
            .and_then(|matcher| matcher.captures(&html).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Soundgasm audio {display_id} has no m4a URL"),
                )
            })?;
        let title = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-title\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| display_id.clone());
        let description = Regex::new(
            r#"(?is)<div[^>]*\bclass\s*=\s*["'][^"']*\bjp-description\b[^"']*["'][^>]*>(.*?)</div>"#,
        )
        .ok()
        .and_then(|matcher| matcher.captures(&html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
        .or_else(|| {
            Regex::new(r#"(?is)<li>\s*Description:\s*(.*?)</li>"#)
                .ok()
                .and_then(|matcher| matcher.captures(&html).ok().flatten())
                .and_then(|captures| captures.get(1))
                .map(|value| html_text_fragment(value.as_str()))
                .filter(|value| !value.is_empty())
        });
        let audio_id = Regex::new(r#"/([^/]+)\.m4a(?:[?#]|$)"#)
            .ok()
            .and_then(|matcher| matcher.captures(&audio_url).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| display_id.clone());
        let extension = yt_dlp_core::determine_ext(Some(&audio_url), "m4a");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(audio_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("url", serde_json::json!(audio_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert("vcodec", serde_json::json!("none"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": audio_url,
                "format_id": "audio",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert("uploader", serde_json::json!(user));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Soundgasm profile playlist extractor. Profile pages expose links
/// to the same native audio pages, which are expanded in Rust for consistent
/// playlist selection and JSON output.
pub struct SoundgasmProfileExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl SoundgasmProfileExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for SoundgasmProfileExtractor {
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
                "Soundgasm profile URL did not match its native pattern",
            )
        })?;
        let profile_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Soundgasm profile has no ID",
                )
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let link_matcher =
            Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Soundgasm profile link matcher: {error}"),
                )
            })?;
        let base = url::Url::parse(url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("invalid Soundgasm profile URL: {error}"),
            )
        })?;
        let audio_extractor = SoundgasmExtractor::new(ExtractorDescriptor::new(
            "SoundgasmIE",
            "soundgasm",
            r"https?://(?:www\.)?soundgasm\.net/u/(?P<user>[0-9a-zA-Z_-]+)/(?P<display_id>[0-9a-zA-Z_-]+)",
            true,
        ))?;
        let mut entries = Vec::new();
        let mut seen_links = Vec::new();
        for captures in link_matcher.captures_iter(&html).flatten() {
            let Some(raw_link) = captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(link) = base.join(raw_link).ok().map(|value| value.to_string()) else {
                continue;
            };
            if !link.contains(&format!("/u/{profile_id}/")) || seen_links.contains(&link) {
                continue;
            }
            seen_links.push(link.clone());
            let entry = audio_extractor
                .extract_with_context(&link, context)
                .map_err(|error| {
                    ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link}: {error}"),
                    )
                })?;
            match entry {
                ExtractorResult::Single(info) => entries.push(info),
                ExtractorResult::Redirect { .. } | ExtractorResult::Playlist { .. } => {
                    return Err(ExtractorError::new(
                        ExtractorErrorKind::Extraction,
                        format!("Soundgasm profile entry {link} returned a non-audio result"),
                    ));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(profile_id));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn parse_compact_count(value: &str) -> Option<i64> {
    let value = value.trim().replace(',', "");
    let (number, multiplier) = match value.chars().last()? {
        'K' | 'k' => (&value[..value.len() - 1], 1_000.0),
        'M' | 'm' => (&value[..value.len() - 1], 1_000_000.0),
        'B' | 'b' => (&value[..value.len() - 1], 1_000_000_000.0),
        _ => (value.as_str(), 1.0),
    };
    number
        .parse::<f64>()
        .ok()
        .map(|value| (value * multiplier) as i64)
}

const IMGUR_CLIENT_ID: &str = "546c25a59c58ad7";

/// Native Imgur animated-media extractor. Imgur's post API contains direct
/// media URLs and account metadata; the optional GIFV page contributes
/// additional source variants and Open Graph fields.
pub struct ImgurExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl ImgurExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for ImgurExtractor {
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
                "Imgur URL did not match its native pattern",
            )
        })?;
        let media_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Imgur URL has no ID")
            })?;
        imgur_media_info(context, media_id, url).map(ExtractorResult::single)
    }
}

/// Native Imgur gallery and album extractor. Playable animated media is
/// expanded eagerly into native InfoDict entries so dump-json, ranges, and
/// downloads all stay on the Rust path.
pub struct ImgurGalleryExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
    gallery_mode: bool,
}

impl ImgurGalleryExtractor {
    pub fn new(
        descriptor: ExtractorDescriptor,
        gallery_mode: bool,
    ) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
            gallery_mode,
        })
    }
}

impl InfoExtractor for ImgurGalleryExtractor {
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
                "Imgur gallery URL did not match its native pattern",
            )
        })?;
        let gallery_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Imgur gallery has no ID")
            })?;
        let data = imgur_api(context, "albums", &gallery_id)?;
        let title = imgur_clean_description(json_string(&data, "title"));
        let description = imgur_clean_description(json_string(&data, "description"));
        let is_album = json_bool(&data, "is_album").unwrap_or(false);
        let media_ids = data
            .get("media")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|media| {
                json_string(media, "type") == Some("video")
                    || media
                        .get("metadata")
                        .and_then(|metadata| json_bool(metadata, "is_animated"))
                        .unwrap_or(false)
            })
            .filter_map(|media| json_value_string(media.get("id")))
            .collect::<Vec<_>>();

        if is_album && self.gallery_mode && media_ids.len() == 1 {
            let mut info = imgur_media_info(context, &media_ids[0], url)?;
            info.insert_if_some("title", title);
            info.insert_if_some("description", description);
            return Ok(ExtractorResult::single(info));
        }
        if is_album {
            let mut entries = Vec::new();
            for media_id in media_ids {
                entries.push(imgur_media_info(context, &media_id, url)?);
            }
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(gallery_id));
            info.insert_if_some("title", title);
            info.insert_if_some("description", description);
            return Ok(ExtractorResult::Playlist { info, entries });
        }

        let mut info = imgur_media_info(context, &gallery_id, url)?;
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        Ok(ExtractorResult::single(info))
    }
}

fn imgur_api(
    context: &ExtractionContext,
    endpoint: &str,
    media_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    native_get_json_with_headers(
        context,
        &format!(
            "https://api.imgur.com/post/v1/{endpoint}/{media_id}?client_id={IMGUR_CLIENT_ID}&include=media,account"
        ),
        &[("Accept", "application/json")],
    )
}

fn imgur_clean_description(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.contains("Discover the magic of the internet at Imgur"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn imgur_media_info(
    context: &ExtractionContext,
    media_id: &str,
    page_url: &str,
) -> Result<InfoDict, ExtractorError> {
    let data = imgur_api(context, "media", media_id)?;
    let media = data
        .get("media")
        .and_then(serde_json::Value::as_array)
        .and_then(|media| media.first())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Imgur media {media_id} has no media record"),
            )
        })?;
    let is_playable = json_string(media, "type") == Some("video")
        || media
            .get("metadata")
            .and_then(|metadata| json_bool(metadata, "is_animated"))
            .unwrap_or(false);
    if !is_playable {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Unsupported,
            format!("TODO: Imgur media {media_id} is a static image"),
        ));
    }
    let media_url = json_string(media, "url").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Imgur media {media_id} has no URL"),
        )
    })?;
    let metadata = media.get("metadata").unwrap_or(&serde_json::Value::Null);
    let webpage = context
        .get(&format!("https://i.imgur.com/{media_id}.gifv"))
        .ok()
        .map(|response| String::from_utf8_lossy(response.body()).into_owned())
        .unwrap_or_default();
    let api_ext = json_string(media, "ext")
        .map(str::to_owned)
        .or_else(|| mimetype_extension(json_string(media, "mime_type")))
        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4"));
    let mut formats = Vec::new();
    let mut media_format = serde_json::json!({
        "url": media_url,
        "format_id": "original",
        "ext": api_ext,
        "width": json_i64(media, "width"),
        "height": json_i64(media, "height"),
        "filesize": json_i64(media, "size"),
    });
    if json_bool(metadata, "has_sound") == Some(false) {
        media_format["acodec"] = serde_json::json!("none");
    }
    if json_string(media, "type") == Some("image") {
        media_format["acodec"] = serde_json::json!("none");
        media_format["preference"] = serde_json::json!(-10);
    }
    formats.push(media_format);

    if let Some(video_elements) = html_element_by_class(&webpage, "video-elements") {
        if let Ok(source_matcher) =
            Regex::new(r#"(?is)<source\s+src=["']([^"']+)["']\s+type=["']([^"']+)["']"#)
        {
            for captures in source_matcher.captures_iter(&video_elements).flatten() {
                let Some(source_url) = captures.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                let Some(mimetype) = captures.get(2).map(|value| value.as_str()) else {
                    continue;
                };
                let source_url = proto_relative_url(source_url, "https:");
                let extension = mimetype_extension(Some(mimetype))
                    .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&source_url), "mp4"));
                formats.push(serde_json::json!({
                    "url": source_url,
                    "format_id": mimetype.split('/').nth(1).unwrap_or("source"),
                    "ext": extension,
                    "width": html_meta_value(&webpage, "video:width")
                        .and_then(|value| value.parse::<i64>().ok()),
                    "height": html_meta_value(&webpage, "video:height")
                        .and_then(|value| value.parse::<i64>().ok()),
                }));
            }
        }
    }
    if let Some(twitter_url) = html_meta_value(&webpage, "twitter:player:stream") {
        let content_type = html_meta_value(&webpage, "twitter:player:stream:content_type");
        let extension = mimetype_extension(content_type.as_deref())
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&twitter_url), "mp4"));
        formats.push(serde_json::json!({
            "url": twitter_url,
            "format_id": "twitter",
            "ext": extension,
            "width": html_meta_value(&webpage, "twitter:width")
                .and_then(|value| value.parse::<i64>().ok()),
            "height": html_meta_value(&webpage, "twitter:height")
                .and_then(|value| value.parse::<i64>().ok()),
        }));
    }

    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let title = json_string(metadata, "title")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| html_meta_value(&webpage, "og:title"))
        .unwrap_or_else(|| media_id.to_owned());
    let description = imgur_clean_description(json_string(metadata, "description")).or_else(|| {
        imgur_clean_description(html_meta_value(&webpage, "og:description").as_deref())
    });
    let account = data.get("account").unwrap_or(&serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(media_id));
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
    info.insert_if_some("description", description);
    info.insert_if_some("uploader_id", json_value_string(data.get("account_id")));
    info.insert_if_some(
        "uploader",
        json_string(account, "username").map(str::to_owned),
    );
    info.insert_if_some("uploader_url", json_string(account, "avatar_url"));
    info.insert_if_some("like_count", json_i64(&data, "upvote_count"));
    info.insert_if_some("dislike_count", json_i64(&data, "downvote_count"));
    info.insert_if_some("comment_count", json_i64(&data, "comment_count"));
    info.insert_if_some(
        "age_limit",
        json_bool(&data, "is_mature").and_then(|value| value.then_some(18)),
    );
    info.insert_if_some(
        "timestamp",
        json_string(metadata, "updated_at")
            .or_else(|| json_string(metadata, "created_at"))
            .or_else(|| json_string(&data, "updated_at"))
            .or_else(|| json_string(&data, "created_at"))
            .map(str::to_owned)
            .and_then(parse_timestamp),
    );
    info.insert_if_some(
        "release_timestamp",
        json_string(metadata, "created_at")
            .or_else(|| json_string(&data, "created_at"))
            .map(str::to_owned)
            .and_then(parse_timestamp),
    );
    info.insert_if_some(
        "duration",
        json_f64(metadata, "duration").or_else(|| json_f64(media, "duration")),
    );
    let thumbnail = html_meta_value(&webpage, "thumbnailUrl")
        .or_else(|| html_meta_value(&webpage, "twitter:image"))
        .or_else(|| html_meta_value(&webpage, "og:image"))
        .unwrap_or_else(|| format!("https://i.imgur.com/{media_id}h.jpg"));
    info.insert(
        "thumbnails",
        serde_json::json!([{
            "url": thumbnail,
            "http_headers": {"Accept": "*/*"}
        }]),
    );
    info.insert("http_headers", serde_json::json!({"Accept": "*/*"}));
    info.insert("webpage_url", serde_json::json!(page_url));
    Ok(info)
}

/// Native EbaumsWorld XML player extractor.
pub struct EbaumsWorldExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EbaumsWorldExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EbaumsWorldExtractor {
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
                "EbaumsWorld URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "EbaumsWorld URL has no ID")
            })?;
        let response = context.get(&format!(
            "http://www.ebaumsworld.com/video/player/{video_id}"
        ))?;
        let xml = String::from_utf8_lossy(response.body());
        let media_url = xml_element_text(&xml, "file").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EbaumsWorld video {video_id} has no media URL"),
            )
        })?;
        let extension = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert(
            "title",
            serde_json::json!(xml_element_text(&xml, "title").unwrap_or_else(|| video_id.clone())),
        );
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some("description", xml_element_text(&xml, "description"));
        info.insert_if_some("thumbnail", xml_element_text(&xml, "image"));
        info.insert_if_some("uploader", xml_element_text(&xml, "username"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Fuyin TV API extractor.
pub struct FuyinTvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FuyinTvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FuyinTvExtractor {
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
                "Fuyin TV URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Fuyin TV URL has no ID")
            })?;
        let api = native_get_json_with_headers(
            context,
            &format!("https://www.fuyin.tv/api/api/tv.movie/url?urlid={video_id}"),
            &[("Accept", "application/json")],
        )?;
        let data = api.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Fuyin TV API response has no data object",
            )
        })?;
        let media_url = json_string(data, "url").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fuyin TV video {video_id} has no media URL"),
            )
        })?;
        let webpage = context
            .get(url)
            .ok()
            .map(|response| String::from_utf8_lossy(response.body()).into_owned())
            .unwrap_or_default();
        let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(data, "title"));
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!(extension));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "http",
                "ext": extension,
            }]),
        );
        info.insert_if_some("description", html_meta_value(&webpage, "description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native CAM4 live HLS extractor.
pub struct Cam4Extractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl Cam4Extractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for Cam4Extractor {
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
                "CAM4 URL did not match its native pattern",
            )
        })?;
        let channel_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "CAM4 URL has no ID")
            })?;
        let data = native_get_json_with_headers(
            context,
            &format!("https://www.cam4.com/rest/v1.0/profile/{channel_id}/streamInfo"),
            &[("Accept", "application/json")],
        )?;
        let playlist_url = json_string(&data, "cdnURL").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("CAM4 channel {channel_id} has no live stream URL"),
            )
        })?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(channel_id));
        info.insert("title", serde_json::json!(channel_id));
        info.insert("url", serde_json::json!(playlist_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": playlist_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert("is_live", serde_json::json!(true));
        info.insert("live_status", serde_json::json!("is_live"));
        info.insert("age_limit", serde_json::json!(18));
        info.insert(
            "thumbnail",
            serde_json::json!(format!(
                "https://snapshots.xcdnpro.com/thumbnails/{channel_id}"
            )),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Kommunetv stream API extractor.
pub struct KommunetvExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KommunetvExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KommunetvExtractor {
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
                "Kommunetv URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Kommunetv URL has no ID")
            })?;
        let host = url::Url::parse(url)
            .ok()
            .and_then(|value| value.host_str().map(str::to_owned))
            .unwrap_or_else(|| "oslo.kommunetv.no".to_owned());
        let data = native_get_json_with_headers(
            context,
            &format!("https://{host}/api/streams?streamType=1&id={video_id}"),
            &[("Accept", "application/json")],
        )?;
        let title = data
            .get("stream")
            .and_then(|stream| json_string(stream, "title"))
            .unwrap_or(video_id.as_str());
        let playlist_url = data
            .get("playlist")
            .and_then(serde_json::Value::as_array)
            .and_then(|playlist| playlist.first())
            .and_then(|playlist| playlist.get("playlist"))
            .and_then(serde_json::Value::as_array)
            .and_then(|playlist| playlist.first())
            .and_then(|playlist| json_string(playlist, "file"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kommunetv stream {video_id} has no playlist URL"),
                )
            })?;
        let mut parsed_playlist = url::Url::parse(playlist_url).map_err(|error| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Kommunetv playlist URL: {error}"),
            )
        })?;
        parsed_playlist.set_query(None);
        parsed_playlist.set_fragment(None);
        let playlist_url = parsed_playlist.to_string();
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(playlist_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert(
            "formats",
            serde_json::json!([{
                "url": playlist_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }]),
        );
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

/// Native Stream.cz/Televize Seznam GraphQL and playlist extractor.
pub struct StreamCzExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl StreamCzExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for StreamCzExtractor {
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
                "Stream.cz URL did not match its native pattern",
            )
        })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Stream.cz URL has no slug")
            })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Stream.cz URL has no ID")
            })?;
        let graphql_payload = serde_json::json!({
            "variables": {"urlName": video_id},
            "query": "query LoadEpisode($urlName : String){ episode(urlName: $urlName){ id spl urlName name perex duration views } }"
        });
        let graphql = native_post_json(
            context,
            "https://www.televizeseznam.cz/api/graphql",
            &graphql_payload,
        )?;
        let episode = graphql
            .get("data")
            .and_then(|data| data.get("episode"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Stream.cz episode {video_id} is missing from GraphQL response"),
                )
            })?;
        let playlist_base = json_string(episode, "spl").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz episode {video_id} has no playlist URL"),
            )
        })?;
        let playlist_url = format!("{playlist_base}spl2,3");
        let mut playlist = context.get_json(&playlist_url)?;
        if playlist.get("data").is_none() {
            if let Some(location) = json_string(&playlist, "Location") {
                playlist = context.get_json(location)?;
            }
        }
        let video = playlist.get("data").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz playlist {playlist_url} has no data"),
            )
        })?;
        let mut formats = Vec::new();
        if let Some(qualities) = video
            .get("http_stream")
            .and_then(|stream| stream.get("qualities"))
            .and_then(serde_json::Value::as_object)
        {
            for (format_id, stream) in qualities {
                add_stream_cz_format(&playlist_url, format_id, stream, "ts", -1, &mut formats);
            }
        }
        if let Some(qualities) = video.get("mp4").and_then(serde_json::Value::as_object) {
            for (format_id, stream) in qualities {
                add_stream_cz_format(&playlist_url, format_id, stream, "mp4", 1, &mut formats);
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Stream.cz episode {video_id} has no playable formats"),
            ));
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut subtitles = serde_json::Map::new();
        if let Some(values) = video
            .get("subtitles")
            .and_then(serde_json::Value::as_object)
        {
            for subtitle in values.values() {
                let Some(language) = json_string(subtitle, "language") else {
                    continue;
                };
                let Some(urls) = subtitle.get("urls").and_then(serde_json::Value::as_object) else {
                    continue;
                };
                let entries = urls
                    .iter()
                    .filter_map(|(extension, value)| {
                        let media_url = value.as_str()?;
                        Some(serde_json::json!({
                            "ext": extension,
                            "url": resolve_url(&playlist_url, media_url),
                        }))
                    })
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    subtitles.insert(language.to_owned(), serde_json::Value::Array(entries));
                }
            }
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(episode, "name"));
        info.insert_if_some("description", json_string(episode, "perex"));
        info.insert_if_some("duration", json_f64(episode, "duration"));
        info.insert_if_some("view_count", json_i64(episode, "views"));
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
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn add_stream_cz_format(
    playlist_url: &str,
    format_id: &str,
    stream: &serde_json::Value,
    extension: &str,
    source_preference: i64,
    formats: &mut Vec<serde_json::Value>,
) {
    let Some(raw_url) = json_string(stream, "url") else {
        return;
    };
    let mut format = serde_json::json!({
        "format_id": format!("{format_id}-{extension}"),
        "ext": extension,
        "source_preference": source_preference,
        "url": resolve_url(playlist_url, raw_url),
    });
    if let Some(value) = json_f64(stream, "bandwidth") {
        format["tbr"] = serde_json::json!(value / 1000.0);
    }
    if let Some(value) = json_f64(stream, "duration") {
        format["duration"] = serde_json::json!(value / 1000.0);
    }
    if let Some(resolution) = stream
        .get("resolution")
        .and_then(serde_json::Value::as_array)
    {
        if let Some(width) = resolution.first().and_then(serde_json::Value::as_i64) {
            format["width"] = serde_json::json!(width);
        }
        if let Some(height) = resolution.get(1).and_then(serde_json::Value::as_i64) {
            format["height"] = serde_json::json!(height);
        }
    }
    if format.get("height").is_none() {
        if let Ok(height) = format_id.trim_end_matches('p').parse::<i64>() {
            format["height"] = serde_json::json!(height);
        }
    }
    if let Some(codec) = json_string(stream, "codec") {
        let codec = codec.to_ascii_lowercase();
        if codec.contains("avc") || codec.contains("h264") || codec.contains("vp8") {
            format["vcodec"] = serde_json::json!(codec);
        }
        if codec.contains("aac") || codec.contains("mp4a") || codec.contains("opus") {
            format["acodec"] = serde_json::json!(codec);
        }
    }
    formats.push(format);
}

fn resolve_url(base: &str, value: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|base| base.join(value).ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| value.to_owned())
}

fn xml_element_text(xml: &str, element: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<{}\b[^>]*>(.*?)</{}\s*>"#,
        regex::escape(element),
        regex::escape(element)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(xml)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn add_peertube_file_format(file: &serde_json::Value, formats: &mut Vec<serde_json::Value>) {
    let Some(file_url) = json_string(file, "fileUrl") else {
        return;
    };
    let label = file
        .get("resolution")
        .and_then(|resolution| json_string(resolution, "label"));
    let mut format = serde_json::json!({
        "url": file_url,
        "format_id": label,
        "filesize": json_i64(file, "size"),
        "ext": yt_dlp_core::determine_ext(Some(file_url), "mp4"),
    });
    if let Some(label) = label {
        if let Some((width, height)) = parse_resolution_label(label) {
            format["width"] = serde_json::json!(width);
            format["height"] = serde_json::json!(height);
        } else if label.ends_with('p') {
            if let Ok(height) = label.trim_end_matches('p').parse::<i64>() {
                format["height"] = serde_json::json!(height);
            }
        }
        if label == "0p" {
            format["vcodec"] = serde_json::json!("none");
        } else if let Some(fps) = json_i64(file, "fps") {
            format["fps"] = serde_json::json!(fps);
        }
    }
    if format.get("ext").and_then(serde_json::Value::as_str) == Some("m3u8") {
        format["ext"] = serde_json::json!("mp4");
        format["protocol"] = serde_json::json!("m3u8_native");
    }
    formats.push(format);
}

fn parse_resolution_label(label: &str) -> Option<(i64, i64)> {
    let matcher = Regex::new(r#"(?i)^(\d+)x(\d+)$"#).ok()?;
    let captures = matcher.captures(label).ok().flatten()?;
    Some((
        captures.get(1)?.as_str().parse().ok()?,
        captures.get(2)?.as_str().parse().ok()?,
    ))
}

fn html_element_by_class(html: &str, class: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<([a-z0-9]+)\b[^>]*\bclass\s*=\s*["'][^"']*\b{}\b[^"']*["'][^>]*>(.*?)</\1\s*>"#,
        regex::escape(class)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(2).map(|value| value.as_str().to_owned()))
}

fn html_field_value(html: &str, field_name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<span\s+class\s*=\s*["']field_title["'][^>]*>\s*{}\s*:\s*</span>\s*<span\s+class\s*=\s*["']field_content["'][^>]*>([^<]+)"#,
        regex::escape(field_name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|matcher| matcher.captures(html).ok().flatten())
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn peertube_subtitles(
    host: &str,
    video_id: &str,
    context: &ExtractionContext,
) -> Option<serde_json::Value> {
    let captions = context
        .get_json(&format!("https://{host}/api/v1/videos/{video_id}/captions"))
        .ok()?;
    let data = captions.get("data").and_then(serde_json::Value::as_array)?;
    let mut subtitles = serde_json::Map::new();
    for caption in data {
        let Some(path) = json_string(caption, "captionPath") else {
            continue;
        };
        let language = caption
            .get("language")
            .and_then(|language| json_string(language, "id"))
            .unwrap_or("en");
        subtitles.insert(
            language.to_owned(),
            serde_json::json!([{"url": format!("https://{host}{path}")}]),
        );
    }
    (!subtitles.is_empty()).then_some(serde_json::Value::Object(subtitles))
}

/// Native Vidyard player JSON extractor. The player endpoint exposes direct
/// media, HLS, captions, chapter metadata, and optional additional metadata;
/// multi-chapter players become native playlists.
pub struct VidyardExtractor {
    descriptor: ExtractorDescriptor,
    matchers: Vec<Regex>,
}

impl VidyardExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        let mut matchers = Vec::new();
        for pattern in &descriptor.valid_urls {
            matchers.push(compile_source_pattern(pattern).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid Vidyard URL pattern: {error}"),
                )
            })?);
        }
        Ok(Self {
            descriptor,
            matchers,
        })
    }
}

impl InfoExtractor for VidyardExtractor {
    fn descriptor(&self) -> &ExtractorDescriptor {
        &self.descriptor
    }

    fn suitable(&self, url: &str) -> bool {
        self.matchers
            .iter()
            .any(|matcher| matcher.is_match(url).unwrap_or(false))
    }

    fn is_native(&self) -> bool {
        true
    }

    fn native_matcher_count(&self) -> usize {
        self.matchers.len()
    }

    fn extract_with_context(
        &self,
        url: &str,
        context: &ExtractionContext,
    ) -> Result<ExtractorResult, ExtractorError> {
        let video_id = self
            .matchers
            .iter()
            .find_map(|matcher| {
                matcher
                    .captures(url)
                    .ok()
                    .flatten()
                    .and_then(|captures| captures.name("id"))
                    .map(|value| value.as_str().to_owned())
            })
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Vidyard URL has no ID")
            })?;
        let response =
            context.get_json(&format!("https://play.vidyard.com/player/{video_id}.json"))?;
        let payload = response.get("payload").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard player response has no payload",
            )
        })?;
        let chapters = payload
            .get("chapters")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    "Vidyard player payload has no chapters",
                )
            })?;
        let mut entries = Vec::new();
        for chapter in chapters {
            let mut entry = vidyard_chapter_info(chapter)?;
            if let Some(facade_id) = json_string(chapter, "facadeUuid") {
                if let Ok(additional) =
                    context.get_json(&format!("https://play.vidyard.com/video/{facade_id}"))
                {
                    merge_vidyard_additional_metadata(&mut entry, &additional);
                }
            }
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Vidyard player {video_id} has no chapters"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.pop().expect("one Vidyard chapter"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert(
            "id",
            serde_json::json!(
                json_string(payload, "playerUuid")
                    .or_else(|| json_string(payload, "playerUUID"))
                    .unwrap_or(&video_id)
            ),
        );
        info.insert_if_some("title", json_string(payload, "name"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn vidyard_chapter_info(chapter: &serde_json::Value) -> Result<InfoDict, ExtractorError> {
    let facade_id = json_string(chapter, "facadeUuid")
        .or_else(|| json_string(chapter, "id"))
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Vidyard chapter has no facadeUuid",
            )
        })?;
    let mut formats = Vec::new();
    let sources = chapter.get("sources").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            "Vidyard chapter has no sources",
        )
    })?;
    if let Some(hls) = sources.get("hls") {
        for source in json_object_values(hls) {
            let Some(media_url) = json_string(source, "url") else {
                continue;
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
    }
    if let Some(sources) = sources.as_object() {
        for (source_type, source_list) in sources {
            if source_type == "hls" {
                continue;
            }
            for source in json_object_values(source_list) {
                let Some(media_url) = json_string(source, "url") else {
                    continue;
                };
                let profile = json_string(source, "profile");
                let mut format = serde_json::json!({
                    "url": media_url,
                    "format_id": format!("http-{source_type}{}", profile.map_or_else(String::new, |profile| format!("-{profile}"))),
                    "ext": mimetype_extension(json_string(source, "mimeType"))
                        .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(media_url), "mp4")),
                });
                if let Some(profile) = profile {
                    if let Some((width, height)) = parse_resolution_label(profile) {
                        format["width"] = serde_json::json!(width);
                        format["height"] = serde_json::json!(height);
                    } else if let Some(height) = profile
                        .strip_suffix('p')
                        .and_then(|value| value.parse::<i64>().ok())
                    {
                        format["height"] = serde_json::json!(height);
                    }
                }
                formats.push(format);
            }
        }
    }
    if formats.is_empty() {
        return Err(ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Vidyard chapter {facade_id} has no playable sources"),
        ));
    }
    let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(facade_id));
    info.insert_if_some(
        "display_id",
        json_i64(chapter, "videoId").map(|value| value.to_string()),
    );
    info.insert_if_some("title", json_string(chapter, "name"));
    info.insert_if_some(
        "description",
        json_string(chapter, "description").map(unescape_html_attribute),
    );
    info.insert_if_some(
        "duration",
        json_f64(chapter, "milliseconds")
            .map(|value| value / 1000.0)
            .or_else(|| json_f64(chapter, "seconds")),
    );
    if let Some(thumbnails) = chapter
        .get("thumbnailUrls")
        .and_then(serde_json::Value::as_object)
    {
        let values = thumbnails
            .values()
            .filter_map(|thumbnail| {
                let url = thumbnail
                    .as_str()
                    .or_else(|| json_string(thumbnail, "url"))?;
                Some(serde_json::json!({"url": url}))
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(values));
        }
    }
    if let Some(captions) = chapter
        .get("captions")
        .and_then(serde_json::Value::as_array)
    {
        let mut subtitles = serde_json::Map::new();
        for caption in captions {
            let Some(url) = json_string(caption, "vttUrl") else {
                continue;
            };
            let language = json_string(caption, "language").unwrap_or("und");
            subtitles
                .entry(language.to_owned())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("subtitle value is an array")
                .push(serde_json::json!({
                    "url": url,
                    "name": json_string(caption, "name"),
                }));
        }
        if !subtitles.is_empty() {
            info.insert("subtitles", serde_json::Value::Object(subtitles));
        }
    }
    if let Some(tags) = chapter.get("tags").and_then(serde_json::Value::as_array) {
        info.insert(
            "tags",
            serde_json::Value::Array(
                tags.iter()
                    .filter_map(|tag| json_string(tag, "name"))
                    .map(|tag| serde_json::json!(tag))
                    .collect(),
            ),
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
    info.insert(
        "http_headers",
        serde_json::json!({"Referer": "https://play.vidyard.com/"}),
    );
    Ok(info)
}

fn merge_vidyard_additional_metadata(info: &mut InfoDict, metadata: &serde_json::Value) {
    info.insert_if_some(
        "title",
        json_string(metadata, "title").or_else(|| json_string(metadata, "name")),
    );
    info.insert_if_some("duration", json_f64(metadata, "seconds"));
    if let Some(thumbnails) = metadata
        .get("thumbnailUrl")
        .and_then(serde_json::Value::as_object)
        .and_then(|value| value.get("url"))
        .and_then(serde_json::Value::as_str)
    {
        info.insert("thumbnails", serde_json::json!([{"url": thumbnails}]));
    }
    if let Some(sections) = metadata
        .get("videoSections")
        .and_then(serde_json::Value::as_array)
    {
        let chapters = sections
            .iter()
            .filter_map(|section| {
                Some(serde_json::json!({
                    "title": json_string(section, "title")?,
                    "start_time": json_f64(section, "milliseconds").map(|value| value / 1000.0)?,
                }))
            })
            .collect::<Vec<_>>();
        if !chapters.is_empty() {
            info.insert("chapters", serde_json::Value::Array(chapters));
        }
    }
}

fn json_object_values(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    match value {
        serde_json::Value::Array(values) => values.iter().collect(),
        serde_json::Value::Object(values) => values.values().collect(),
        _ => Vec::new(),
    }
}

fn mimetype_extension(mimetype: Option<&str>) -> Option<String> {
    Some(
        match mimetype? {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/ogg" => "ogv",
            "audio/mpeg" => "mp3",
            "audio/mp4" => "m4a",
            "audio/webm" => "webm",
            "audio/ogg" => "ogg",
            "audio/flac" => "flac",
            _ => return None,
        }
        .to_owned(),
    )
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

fn proto_relative_url(value: &str, scheme: &str) -> String {
    value
        .strip_prefix("//")
        .map_or_else(|| value.to_owned(), |rest| format!("{scheme}//{rest}"))
}

fn url_query_value(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn date_digits(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .take(8)
        .collect::<String>();
    (digits.len() == 8).then_some(digits)
}

fn native_url_result(url: &str) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("_type", serde_json::json!("url"));
    info.insert("url", serde_json::json!(url));
    info
}

fn html5_media_formats(page_url: &str, html: &str) -> Vec<serde_json::Value> {
    let Ok(matcher) = Regex::new(r#"(?is)<(?:source|video|audio)\b[^>]*\bsrc\s*=\s*["']([^"']+)"#)
    else {
        return Vec::new();
    };
    let base_url = url::Url::parse(page_url).ok();
    let mut urls = Vec::new();
    for captures in matcher.captures_iter(html).flatten() {
        let Some(raw_url) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if raw_url.is_empty() {
            continue;
        }
        let raw_url = proto_relative_url(raw_url, "https:");
        let media_url = base_url
            .as_ref()
            .and_then(|base| base.join(&raw_url).ok())
            .map_or(raw_url, |value| value.to_string());
        if !urls.contains(&media_url) {
            urls.push(media_url);
        }
    }
    urls.into_iter()
        .enumerate()
        .map(|(index, media_url)| {
            let ext = yt_dlp_core::determine_ext(Some(&media_url), "mp4");
            serde_json::json!({
                "format_id": format!("html5-{index}"),
                "url": media_url,
                "ext": ext,
                "protocol": if ext == "m3u8" { "m3u8_native" } else { "http" },
            })
        })
        .collect()
}

fn url_with_scheme(value: &str, scheme: &str) -> String {
    if let Ok(mut parsed) = url::Url::parse(value) {
        if parsed.set_scheme(scheme).is_ok() {
            return parsed.to_string();
        }
    }
    value.split_once("://").map_or_else(
        || value.to_owned(),
        |(_, rest)| format!("{scheme}://{rest}"),
    )
}

fn percent_decode(value: &str) -> String {
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
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn rot13_ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' => {
                let offset = character as u8 - b'a';
                (b'a' + (offset + 13) % 26) as char
            }
            'A'..='Z' => {
                let offset = character as u8 - b'A';
                (b'A' + (offset + 13) % 26) as char
            }
            _ => character,
        })
        .collect()
}

fn native_get_json_with_headers(
    context: &ExtractionContext,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<serde_json::Value, ExtractorError> {
    let mut request = Request::new(url);
    for (name, value) in headers {
        request.headers_mut().set(*name, *value);
    }
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid JSON from {}: {error}", response.url()),
        )
    })
}

fn decode_json_string(value: &str) -> Option<String> {
    serde_json::from_str(value).ok()
}

fn json_media_urls(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(values) => values.iter().flat_map(json_media_urls).collect(),
        serde_json::Value::Object(values) => {
            let mut urls = Vec::new();
            for key in ["src", "url"] {
                if let Some(value) = values.get(key).and_then(serde_json::Value::as_str) {
                    urls.push(value.to_owned());
                }
            }
            if urls.is_empty() {
                urls.extend(values.values().flat_map(json_media_urls));
            }
            urls
        }
        _ => Vec::new(),
    }
}

fn html_title_value(html: &str) -> Option<String> {
    let matcher = Regex::new(r#"(?is)<title\b[^>]*>(.*?)</title>"#).ok()?;
    let captures = matcher.captures(html).ok().flatten()?;
    let title = captures
        .get(1)
        .map(|value| html_text_fragment(value.as_str()))?;
    let title = title
        .trim_end_matches(" - Newgrounds")
        .trim_end_matches(" | Newgrounds")
        .trim();
    (!title.is_empty()).then(|| title.to_owned())
}

fn html_attribute_value(html: &str, attribute: &str, expected: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<[^>]+\b{}\s*=\s*["']{}\s*["'][^>]*\bcontent\s*=\s*["']([^"']+)""#,
        regex::escape(attribute),
        regex::escape(expected),
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn parse_timestamp(value: String) -> Option<i64> {
    yt_dlp_core::parse_iso8601(&value)
        .or_else(|| yt_dlp_core::parse_iso8601(&format!("{value}T00:00:00Z")))
}

fn json_object_after_marker(text: &str, marker: &str) -> Option<serde_json::Value> {
    let marker_start = text.find(marker)?;
    let remainder = &text[marker_start + marker.len()..];
    let open_offset = remainder.find('{')?;
    let bytes = remainder.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(open_offset) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'\"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'\"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return parse_common_javascript_value(&String::from_utf8_lossy(
                        &bytes[open_offset..=offset],
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

fn json_array_after_marker(text: &str, marker: &str) -> Option<serde_json::Value> {
    let marker_start = text.find(marker)?;
    let remainder = &text[marker_start + marker.len()..];
    let open_offset = remainder.find('[')?;
    let bytes = remainder.as_bytes();
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(open_offset) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => stack.push(*byte),
            b'}' => {
                if stack.pop() != Some(b'{') {
                    return None;
                }
            }
            b']' => {
                if stack.pop() != Some(b'[') {
                    return None;
                }
                if stack.is_empty() {
                    return parse_common_javascript_value(&String::from_utf8_lossy(
                        &bytes[open_offset..=offset],
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_common_javascript_value(value: &str) -> Option<serde_json::Value> {
    if let Ok(parsed) = serde_json::from_str(value) {
        return Some(parsed);
    }
    let matcher = Regex::new(r#"([,{]\s*)([A-Za-z_$][A-Za-z0-9_$-]*)\s*:"#).ok()?;
    let normalized = matcher.replace_all(value, "$1\"$2\":");
    serde_json::from_str(&normalized).ok()
}

fn html_json_ld(html: &str) -> Option<serde_json::Value> {
    let matcher = Regex::new(
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#,
    )
    .ok()?;
    matcher.captures_iter(html).flatten().find_map(|captures| {
        captures
            .get(1)
            .and_then(|value| serde_json::from_str(value.as_str().trim()).ok())
    })
}

fn html_json_number(html: &str, key: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)["']{}\s*["']\s*:\s*["']?([0-9]+(?:\.[0-9]+)?)"#,
        regex::escape(key)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
}

fn html_element_by_id(html: &str, id: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<([a-z0-9]+)\b[^>]*\bid\s*=\s*["']{}\s*["'][^>]*>(.*?)</\1\s*>"#,
        regex::escape(id)
    );
    let matcher = Regex::new(&pattern).ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(2).map(|value| value.as_str().to_owned()))
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

/// Native Rumble embed API extractor. The embed JSON exposes direct, audio,
/// HLS, captions, live state, and author metadata without executing its player.
pub struct RumbleEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl RumbleEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for RumbleEmbedExtractor {
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
                "Rumble embed URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Rumble embed URL has no ID")
            })?;
        let mut request = Request::new("https://rumble.com/embedJS/u3/");
        request.update_query(&[
            ("request".to_owned(), "video".to_owned()),
            ("ver".to_owned(), "2".to_owned()),
            ("v".to_owned(), video_id.to_owned()),
        ]);
        let video = context.get_json(request.url())?;
        let live_status = match (
            json_i64(&video, "live"),
            json_bool(&video, "livestream_has_dvr"),
        ) {
            (Some(0), Some(true)) => "was_live",
            (Some(0), _) => "not_live",
            (Some(1), Some(false)) => "was_live",
            (Some(1), _) => "is_upcoming",
            (Some(2), _) => "is_live",
            _ => "",
        };
        let mut formats = Vec::new();
        if let Some(format_groups) = video.get("ua").and_then(serde_json::Value::as_object) {
            for (format_type, format_info) in format_groups {
                let candidates = match format_info {
                    serde_json::Value::Array(values) => {
                        values.iter().map(|value| (None, value)).collect::<Vec<_>>()
                    }
                    serde_json::Value::Object(values) => values
                        .iter()
                        .map(|(height, value)| (Some(height.as_str()), value))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                for (height_hint, video_info) in candidates {
                    let Some(media_url) = json_string(video_info, "url") else {
                        continue;
                    };
                    if format_type == "tar" {
                        continue;
                    }
                    let meta = video_info.get("meta").unwrap_or(&serde_json::Value::Null);
                    let height = json_i64(meta, "h")
                        .or_else(|| height_hint.and_then(|height| height.parse::<i64>().ok()));
                    if format_type == "hls" {
                        formats.push(serde_json::json!({
                            "url": media_url,
                            "format_id": "hls",
                            "ext": "mp4",
                            "protocol": "m3u8_native",
                        }));
                        continue;
                    }
                    let is_timeline = format_type == "timeline";
                    let is_audio = format_type == "audio";
                    let mut format = serde_json::json!({
                        "url": media_url,
                        "format_id": height.map_or_else(
                            || format_type.to_owned(),
                            |height| format!("{format_type}-{height}p")
                        ),
                        "format_note": if is_timeline { "Timeline" } else { "" },
                        "vcodec": if is_audio { "none" } else { "unknown" },
                        "acodec": if is_timeline { "none" } else { "unknown" },
                        "fps": if is_timeline || is_audio {
                            serde_json::Value::Null
                        } else {
                            video.get("fps").cloned().unwrap_or(serde_json::Value::Null)
                        },
                    });
                    for (source, target) in [
                        ("bitrate", "tbr"),
                        ("size", "filesize"),
                        ("w", "width"),
                        ("h", "height"),
                    ] {
                        if let Some(value) = meta.get(source) {
                            format[target] = value.clone();
                        }
                    }
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Rumble video {video_id} has no playable formats"),
            ));
        }
        let author = video.get("author").unwrap_or(&serde_json::Value::Null);
        let mut info = InfoDict::new();
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some(
            "title",
            json_string(&video, "title").map(unescape_html_attribute),
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
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some(
            "timestamp",
            json_string(&video, "pubDate").and_then(yt_dlp_core::parse_iso8601),
        );
        info.insert_if_some("channel", json_string(author, "name"));
        info.insert_if_some("channel_url", json_string(author, "url"));
        info.insert_if_some("uploader", json_string(author, "name"));
        if !live_status.is_empty() {
            info.insert("live_status", serde_json::json!(live_status));
        }
        if live_status != "is_live" && live_status != "post_live" {
            info.insert_if_some("duration", json_i64(&video, "duration"));
        }
        let mut thumbnails = Vec::new();
        if let Some(values) = video.get("t").and_then(serde_json::Value::as_array) {
            thumbnails.extend(values.iter().filter_map(|thumbnail| {
                let url = json_string(thumbnail, "i")?;
                Some(serde_json::json!({
                    "url": url,
                    "width": json_i64(thumbnail, "w"),
                    "height": json_i64(thumbnail, "h"),
                }))
            }));
        }
        if thumbnails.is_empty() {
            if let Some(thumbnail) = json_string(&video, "i") {
                thumbnails.push(serde_json::json!({"url": thumbnail}));
            }
        }
        if !thumbnails.is_empty() {
            info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        }
        if let Some(captions) = video.get("cc").and_then(serde_json::Value::as_object) {
            let subtitles = captions
                .iter()
                .filter_map(|(language, caption)| {
                    let path = json_string(caption, "path")?;
                    Some((
                        language.clone(),
                        serde_json::json!([{
                            "url": path,
                            "name": json_string(caption, "language").unwrap_or("")
                        }]),
                    ))
                })
                .collect::<serde_json::Map<_, _>>();
            if !subtitles.is_empty() {
                info.insert("subtitles", serde_json::Value::Object(subtitles));
            }
        }
        Ok(ExtractorResult::single(info))
    }
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
