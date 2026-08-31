/// Native Angel Studios JSON-LD/HLS episode extractor.
pub struct AngelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AngelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for AngelExtractor {
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
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Angel URL has no episode ID")
            })?;
        let response = context.get(url)?;
        let html = String::from_utf8_lossy(response.body());
        let json_ld = html_json_ld(&html).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Angel episode {video_id} has no JSON-LD metadata"),
            )
        })?;
        let json_ld = angel_json_ld_object(&json_ld).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Angel episode {video_id} has no JSON-LD video object"),
            )
        })?;
        let media_url = json_string(json_ld, "contentUrl")
            .or_else(|| json_string(json_ld, "url"))
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Angel episode {video_id} has no HLS URL"),
                )
            })?;
        let stream_ext = yt_dlp_core::determine_ext(Some(&media_url), "unknown");
        if stream_ext != "m3u8" {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Unsupported,
                format!(
                    "TODO: Angel native extractor only implements JSON-LD HLS streams, got {stream_ext}"
                ),
            ));
        }

        let title = html_meta_value(&html, "og:title")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| angel_json_string(json_ld, "name"))
            .unwrap_or_else(|| video_id.clone());
        let description = html_meta_value(&html, "og:description")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| angel_json_string(json_ld, "description"));
        let thumbnail = html_meta_value(&html, "og:image")
            .map(|value| unescape_html_attribute(&value))
            .filter(|value| !value.is_empty())
            .or_else(|| angel_json_ld_thumbnail(json_ld))
            .map(angel_base_thumbnail_url);

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        angel_insert_json_ld_metadata(&mut info, json_ld);
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
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn angel_json_ld_object(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("contentUrl") || object.contains_key("url") {
                Some(value)
            } else {
                object
                    .get("@graph")
                    .and_then(angel_json_ld_object)
                    .or_else(|| object.contains_key("name").then_some(value))
            }
        }
        serde_json::Value::Array(values) => values.iter().find_map(angel_json_ld_object),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn angel_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    json_string(value, key)
        .map(|value| unescape_html_attribute(value))
        .filter(|value| !value.is_empty())
}

fn angel_json_ld_thumbnail(value: &serde_json::Value) -> Option<String> {
    ["thumbnails", "thumbnailUrl", "thumbnailURL", "thumbnail_url"]
        .iter()
        .find_map(|key| value.get(*key).and_then(angel_thumbnail_value))
        .map(|value| proto_relative_url(&value, "https:"))
}

fn angel_thumbnail_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(unescape_html_attribute(value)),
        serde_json::Value::Array(values) => values.iter().find_map(angel_thumbnail_value),
        serde_json::Value::Object(_) => json_string(value, "url")
            .or_else(|| json_string(value, "contentUrl"))
            .map(|value| unescape_html_attribute(value)),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_) => None,
    }
}

fn angel_base_thumbnail_url(value: String) -> String {
    let stripped = Regex::new(r#"(/upload)/.+(/angel-app/.+)$"#)
        .ok()
        .and_then(|matcher| matcher.captures(&value).ok().flatten())
        .and_then(|captures| {
            let whole = captures.get(0)?;
            let replacement = format!(
                "{}{}",
                captures.get(1)?.as_str(),
                captures.get(2)?.as_str()
            );
            Some(format!(
                "{}{}{}",
                &value[..whole.start()],
                replacement,
                &value[whole.end()..]
            ))
        });
    stripped.unwrap_or(value)
}

fn angel_insert_json_ld_metadata(info: &mut InfoDict, json_ld: &serde_json::Value) {
    info.insert_if_some(
        "duration",
        json_f64(json_ld, "duration")
            .or_else(|| json_string(json_ld, "duration").and_then(yt_dlp_core::parse_duration)),
    );
    info.insert_if_some(
        "timestamp",
        json_string(json_ld, "uploadDate")
            .map(str::to_owned)
            .and_then(parse_timestamp),
    );
    info.insert_if_some("uploader", angel_author(json_ld.get("author")));
    info.insert_if_some("artist", angel_author(json_ld.get("byArtist")));
    info.insert_if_some(
        "filesize",
        json_f64(json_ld, "contentSize").map(|value| value as i64),
    );
    info.insert_if_some("tbr", json_f64(json_ld, "bitrate").map(|value| value as i64));
    info.insert_if_some("width", json_f64(json_ld, "width").map(|value| value as i64));
    info.insert_if_some("height", json_f64(json_ld, "height").map(|value| value as i64));
    info.insert_if_some(
        "view_count",
        json_f64(json_ld, "interactionCount").map(|value| value as i64),
    );
    info.insert_if_some(
        "tags",
        json_string(json_ld, "keywords")
            .map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>()),
    );
    if json_ld
        .get("@type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value == "AudioObject")
    {
        info.insert("vcodec", serde_json::Value::String("none".to_owned()));
        info.insert_if_some("abr", json_f64(json_ld, "bitrate"));
    }
}

fn angel_author(value: Option<&serde_json::Value>) -> Option<String> {
    match value {
        Some(serde_json::Value::String(value)) => Some(unescape_html_attribute(value)),
        Some(serde_json::Value::Object(value)) => value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(unescape_html_attribute),
        Some(
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Array(_),
        )
        | None => None,
    }
}
