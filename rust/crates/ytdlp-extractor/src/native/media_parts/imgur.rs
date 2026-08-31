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
