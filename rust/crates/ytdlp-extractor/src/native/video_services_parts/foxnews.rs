/// Native Fox News/Fox Business AMP feed extractor.
pub struct FoxNewsExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FoxNewsExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FoxNewsExtractor {
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
        let requested_id = foxnews_match_id(&self.matcher, url, "Fox News")?;
        let feed_url =
            format!("https://api.foxnews.com/v3/video-player/{requested_id}?callback=uid_{requested_id}");
        let response = context.get(&feed_url)?;
        let body = String::from_utf8_lossy(response.body());
        let feed = foxnews_parse_jsonp(&body).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("invalid Fox News AMP feed for {requested_id}"),
            )
        })?;
        let item = feed
            .get("channel")
            .and_then(|channel| channel.get("item"))
            .ok_or_else(|| {
                let error = json_value_string(feed.get("error"))
                    .unwrap_or_else(|| "feed has no item".to_owned());
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Fox News feed {requested_id} said: {error}"),
                )
            })?;
        let feed_id = json_value_string(item.get("guid")).unwrap_or_else(|| requested_id.clone());
        let media_content = foxnews_media_node(item, "content");
        let content_nodes = match media_content {
            Some(serde_json::Value::Array(values)) => values.iter().collect::<Vec<_>>(),
            Some(value) => vec![value],
            None => Vec::new(),
        };
        let mut formats = Vec::new();
        for media in &content_nodes {
            let Some(media_url) = foxnews_attr_string(media, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let media_type = foxnews_attr_string(media, "type");
            let extension = media_type
                .as_deref()
                .and_then(foxnews_mimetype_extension)
                .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&media_url), "mp4"));
            if extension == "f4m" {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Unsupported,
                    format!(
                        "TODO: Fox News video {requested_id} requires unsupported HDS/F4M manifest parsing"
                    ),
                ));
            }
            let is_hls = extension.eq_ignore_ascii_case("m3u8");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": if is_hls {
                    "hls".to_owned()
                } else {
                    media.get("media-category")
                        .and_then(|category| foxnews_attr_string(category, "label"))
                        .unwrap_or_else(|| "http".to_owned())
                },
                "protocol": if is_hls { "m3u8_native" } else { "http" },
                "ext": if is_hls { "mp4" } else { extension.as_str() },
            });
            if let Some(tbr) = foxnews_attr_string(media, "bitrate")
                .and_then(|value| value.parse::<i64>().ok())
            {
                format["tbr"] = serde_json::json!(tbr);
            }
            if let Some(filesize) = foxnews_attr_string(media, "fileSize")
                .and_then(|value| value.parse::<i64>().ok())
            {
                format["filesize"] = serde_json::json!(filesize);
            }
            formats.push(format);
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Fox News video {requested_id} has no media content"),
            ));
        }

        let mut thumbnails = Vec::new();
        if let Some(media_thumbnail) = foxnews_media_node(item, "thumbnail") {
            let thumbnail_nodes = match media_thumbnail {
                serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
                value => vec![value],
            };
            for thumbnail in thumbnail_nodes {
                let Some(thumbnail_url) = foxnews_attr_string(thumbnail, "url")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                else {
                    continue;
                };
                let mut entry = serde_json::json!({"url": thumbnail_url});
                if let Some(width) = foxnews_attr_string(thumbnail, "width")
                    .and_then(|value| value.parse::<i64>().ok())
                {
                    entry["width"] = serde_json::json!(width);
                }
                if let Some(height) = foxnews_attr_string(thumbnail, "height")
                    .and_then(|value| value.parse::<i64>().ok())
                {
                    entry["height"] = serde_json::json!(height);
                }
                thumbnails.push(entry);
            }
        }

        let mut subtitles = serde_json::Map::new();
        if let Some(media_subtitle) = foxnews_media_node(item, "subTitle") {
            let subtitle_nodes = match media_subtitle {
                serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
                value => vec![value],
            };
            for subtitle in subtitle_nodes {
                let Some(subtitle_url) = foxnews_attr_string(subtitle, "href")
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
                else {
                    continue;
                };
                let language = foxnews_attr_string(subtitle, "lang").unwrap_or_else(|| "en".to_owned());
                let subtitle_ext = foxnews_attr_string(subtitle, "type")
                    .and_then(|value| foxnews_mimetype_extension(&value))
                    .unwrap_or_else(|| {
                        yt_dlp_core::determine_ext(Some(&subtitle_url), "vtt")
                    });
                subtitles
                    .entry(language)
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .expect("Fox News subtitle value is always initialized as an array")
                    .push(serde_json::json!({
                        "url": subtitle_url,
                        "ext": subtitle_ext,
                    }));
            }
        }

        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = foxnews_media_node_string(item, "title");
        let description = foxnews_media_node_string(item, "description");
        let timestamp = json_value_string(item.get("pubDate"))
            .and_then(parse_timestamp)
            .or_else(|| json_value_string(item.get("dc-date")).and_then(parse_timestamp));
        let duration = content_nodes
            .first()
            .and_then(|media| foxnews_attr_string(media, "duration"))
            .and_then(|value| {
                value
                    .parse::<i64>()
                    .map(|value| serde_json::json!(value))
                    .ok()
                    .or_else(|| yt_dlp_core::parse_duration(&value).map(|value| serde_json::json!(value)))
            });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(requested_id));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("duration", duration);
        info.insert_if_some("thumbnails", (!thumbnails.is_empty()).then_some(thumbnails.clone()));
        info.insert_if_some(
            "thumbnail",
            thumbnails
                .first()
                .and_then(|thumbnail| thumbnail.get("url"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        );
        info.insert("url", first_format.get("url").cloned().unwrap_or(serde_json::Value::Null));
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::Value::Object(subtitles));
        info.insert("display_id", serde_json::json!(feed_id));
        Ok(ExtractorResult::single(info))
    }
}

fn foxnews_media_node<'a>(
    item: &'a serde_json::Value,
    name: &str,
) -> Option<&'a serde_json::Value> {
    let media_name = format!("media-{name}");
    item.get("media-group")
        .and_then(|group| group.get(&media_name))
        .or_else(|| item.get(&media_name))
        .or_else(|| item.get(name))
}

fn foxnews_media_node_string(item: &serde_json::Value, name: &str) -> Option<String> {
    let value = foxnews_media_node(item, name)?;
    match value {
        serde_json::Value::String(value) => Some(value.to_owned()),
        serde_json::Value::Array(values) => values
            .first()
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

fn foxnews_attr_string(node: &serde_json::Value, key: &str) -> Option<String> {
    node.get("@attributes")
        .and_then(|attributes| attributes.get(key))
        .and_then(|value| json_value_string(Some(value)))
}

fn foxnews_mimetype_extension(mimetype: &str) -> Option<String> {
    Some(
        match mimetype.to_ascii_lowercase().as_str() {
            "video/mp4" => "mp4",
            "video/x-flv" | "video/flv" => "flv",
            "video/webm" => "webm",
            "application/x-mpegurl" | "application/vnd.apple.mpegurl" => "m3u8",
            "text/vtt" => "vtt",
            "application/ttml+xml" => "ttml",
            _ => return None,
        }
        .to_owned(),
    )
}

fn foxnews_parse_jsonp(value: &str) -> Option<serde_json::Value> {
    let value = value.trim();
    if let Some(parsed) = parse_common_javascript_value(value) {
        return Some(parsed);
    }
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    (close > open).then(|| parse_common_javascript_value(value[open + 1..close].trim()))?
}

fn foxnews_match_id(
    matcher: &Regex,
    url: &str,
    label: &str,
) -> Result<String, ExtractorError> {
    matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("{label} URL has no video ID"),
            )
        })
}
