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
