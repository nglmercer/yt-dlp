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
