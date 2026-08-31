/// Native 9GAG API extractor for animated posts.
///
/// The API exposes the same image variants used by the Python extractor. JPG
/// and PNG records become thumbnails, while WebM/MP4 records become formats
/// and retain their codec-specific URLs when available.
pub struct NineGagExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl NineGagExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for NineGagExtractor {
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
                "9GAG URL did not match its native pattern",
            )
        })?;
        let post_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "9GAG URL has no post ID")
            })?;

        let response = context.get_json(&format!("https://9gag.com/v1/post?id={post_id}"))?;
        let post = response
            .get("data")
            .and_then(|data| data.get("post"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("9GAG post {post_id} is missing from the API response"),
                )
            })?;
        if json_string(post, "type") != Some("Animated") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!("TODO: 9GAG post {post_id} does not contain an animated video"),
            ));
        }

        let mut duration = None;
        let mut formats = Vec::new();
        let mut thumbnails = Vec::new();
        if let Some(images) = post.get("images").and_then(serde_json::Value::as_object) {
            for (key, image) in images {
                let Some(image_url) = ninegag_url(json_string(image, "url")) else {
                    continue;
                };
                let extension = yt_dlp_core::determine_ext(Some(&image_url), "mp4")
                    .to_ascii_lowercase();
                let image_id = key
                    .strip_prefix("image")
                    .unwrap_or(key.as_str())
                    .to_owned();

                if matches!(extension.as_str(), "jpg" | "png") {
                    if let Some(webp_url) = ninegag_url(json_string(image, "webpUrl")) {
                        let mut thumbnail = ninegag_media_fields(image, webp_url);
                        thumbnail.insert(
                            "id".to_owned(),
                            serde_json::json!(format!("{image_id}-webp")),
                        );
                        thumbnails.push(serde_json::Value::Object(thumbnail));
                    }
                    let mut thumbnail = ninegag_media_fields(image, image_url);
                    thumbnail.insert("id".to_owned(), serde_json::json!(image_id));
                    thumbnail.insert("ext".to_owned(), serde_json::json!(extension));
                    thumbnails.push(serde_json::Value::Object(thumbnail));
                } else if matches!(extension.as_str(), "webm" | "mp4") {
                    if duration.unwrap_or_default() == 0 {
                        duration = json_i64(image, "duration");
                    }
                    let mut common = ninegag_media_fields(image, image_url);
                    if image.get("hasAudio").and_then(serde_json::Value::as_i64) == Some(0) {
                        common.insert("acodec".to_owned(), serde_json::json!("none"));
                    }
                    for video_codec in ["vp8", "vp9", "h265"] {
                        let codec_key = format!("{video_codec}Url");
                        let Some(codec_url) = ninegag_url(json_string(image, &codec_key)) else {
                            continue;
                        };
                        let mut codec_format = common.clone();
                        codec_format.insert(
                            "format_id".to_owned(),
                            serde_json::json!(format!("{image_id}-{video_codec}")),
                        );
                        codec_format.insert("url".to_owned(), serde_json::json!(codec_url));
                        codec_format.insert(
                            "vcodec".to_owned(),
                            serde_json::json!(video_codec),
                        );
                        formats.push(serde_json::Value::Object(codec_format));
                    }
                    common.insert("ext".to_owned(), serde_json::json!(extension));
                    common.insert("format_id".to_owned(), serde_json::json!(image_id));
                    formats.push(serde_json::Value::Object(common));
                }
            }
        }

        let creator = post.get("creator");
        let categories = post
            .get("postSection")
            .and_then(|section| json_string(section, "name"))
            .filter(|name| !name.is_empty())
            .map(|name| vec![name.to_owned()]);
        let tags = post
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .filter(|values| !values.is_empty())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|tag| json_string(tag, "key"))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            });

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(post_id));
        info.insert_if_some(
            "title",
            json_string(post, "title").map(unescape_html_attribute),
        );
        info.insert_if_some("timestamp", json_i64(post, "creationTs"));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "uploader",
            creator.and_then(|creator| json_string(creator, "fullName")),
        );
        info.insert_if_some(
            "uploader_id",
            creator.and_then(|creator| json_string(creator, "username")),
        );
        info.insert_if_some(
            "uploader_url",
            creator
                .and_then(|creator| ninegag_url(json_string(creator, "profileUrl"))),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("thumbnails", serde_json::Value::Array(thumbnails));
        info.insert_if_some("like_count", json_i64(post, "upVoteCount"));
        info.insert_if_some("dislike_count", json_i64(post, "downVoteCount"));
        info.insert_if_some("comment_count", json_i64(post, "commentsCount"));
        if post.get("nsfw").and_then(serde_json::Value::as_i64) == Some(1) {
            info.insert("age_limit", serde_json::json!(18));
        }
        info.insert_if_some("categories", categories);
        info.insert_if_some("tags", tags);
        Ok(ExtractorResult::single(info))
    }
}

fn ninegag_url(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| value.to_owned())
}

fn ninegag_media_fields(
    image: &serde_json::Value,
    media_url: String,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    fields.insert("url".to_owned(), serde_json::json!(media_url));
    if let Some(width) = json_i64(image, "width") {
        fields.insert("width".to_owned(), serde_json::json!(width));
    }
    if let Some(height) = json_i64(image, "height") {
        fields.insert("height".to_owned(), serde_json::json!(height));
    }
    fields
}
