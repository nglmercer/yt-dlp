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
