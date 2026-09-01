/// Native KukuluLive live and time-shift extractor.
pub struct KukuluLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KukuluLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KukuluLiveExtractor {
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
                    "KukuluLive URL has no stream ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        if webpage.contains(">タイムシフトが見つかりませんでした。<") {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KukuluLive stream {video_id} has expired"),
            ));
        }
        let title = html_element_by_id(&webpage, "livetitle")
            .map(|value| html_text_fragment(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| video_id.clone());
        let description = html_meta_value(&webpage, "Description")
            .map(|value| unescape_html_attribute(&value).trim().to_owned())
            .filter(|value| !value.is_empty());
        let thumbnail = html_meta_value(&webpage, "og:image")
            .or_else(|| html_meta_value(&webpage, "twitter:image"))
            .and_then(|value| kukulu_valid_url(&value).or_else(|| Some(resolve_url(url, &value))));

        if kukulu_is_live(&webpage) {
            let mut formats = Vec::new();
            let high_meta = kukulu_quality_meta(context, &video_id, "Z", None)?;
            let high_vcodec = kukulu_query_value(&high_meta, "vcodec");
            kukulu_add_quality_formats(&mut formats, &high_meta);
            if high_vcodec
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("HEVC"))
            {
                let h264_meta = kukulu_quality_meta(context, &video_id, "Z", Some("1"))?;
                kukulu_add_quality_formats(&mut formats, &h264_meta);
            }
            let low_meta = kukulu_quality_meta(context, &video_id, "ForceLow", None)?;
            kukulu_add_quality_formats(&mut formats, &low_meta);
            if formats.is_empty() {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KukuluLive stream {video_id} has no live formats"),
                ));
            }
            let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(video_id));
            info.insert("title", serde_json::json!(title));
            info.insert_if_some("description", description);
            info.insert_if_some("thumbnail", thumbnail);
            info.insert("is_live", serde_json::json!(true));
            info.insert(
                "url",
                first
                    .get("url")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
            info.insert(
                "ext",
                first
                    .get("ext")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("mp4")),
            );
            info.insert("formats", serde_json::Value::Array(formats));
            info.insert("subtitles", serde_json::json!({}));
            info.insert("webpage_url", serde_json::json!(url));
            return Ok(ExtractorResult::single(info));
        }

        let player_url = kukulu_query_url(
            "https://live.erinn.biz/live.timeshift.fplayer.php",
            &[("hash", video_id.as_str())],
        )?;
        let player_response = context.get(&player_url)?;
        let player_html = String::from_utf8_lossy(player_response.body());
        let sources = json_array_after_marker(&player_html, "var fplayer_source").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KukuluLive VOD {video_id} has no segment sources"),
            )
        })?;
        let segments = sources.as_array().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KukuluLive VOD {video_id} has invalid segment sources"),
            )
        })?;
        if segments.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KukuluLive VOD {video_id} has no segments"),
            ));
        }
        let mut entries = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            let Some(entry) = kukulu_vod_entry(
                &video_id,
                index + 1,
                segment,
                &title,
                description.as_deref(),
                thumbnail.as_deref(),
                url,
            ) else {
                continue;
            };
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("KukuluLive VOD {video_id} has no usable segments"),
            ));
        }
        if entries.len() == 1 {
            return Ok(ExtractorResult::single(
                entries.into_iter().next().expect("one KukuluLive entry"),
            ));
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn kukulu_is_live(html: &str) -> bool {
    Regex::new(r#"(?is)\bvar\s+timeshift\s*=\s*false"#)
        .ok()
        .is_some_and(|matcher| matcher.is_match(html).unwrap_or(false))
}

fn kukulu_quality_meta(
    context: &ExtractionContext,
    video_id: &str,
    code: &str,
    force_h264: Option<&str>,
) -> Result<String, ExtractorError> {
    let mut fields = vec![
        ("hash", video_id.to_owned()),
        ("action", format!("get{code}liveByAjax")),
    ];
    if let Some(force_h264) = force_h264 {
        fields.push(("force_h264", force_h264.to_owned()));
    }
    let query = fields
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    let endpoint = kukulu_query_url("https://live.erinn.biz/live.player.fplayer.php", &query)?;
    let response = context.get(&endpoint)?;
    Ok(String::from_utf8_lossy(response.body()).into_owned())
}

fn kukulu_query_url(base: &str, fields: &[(&str, &str)]) -> Result<String, ExtractorError> {
    let mut parsed = url::Url::parse(base).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid KukuluLive endpoint {base}: {error}"),
        )
    })?;
    {
        let mut query = parsed.query_pairs_mut();
        for (key, value) in fields {
            query.append_pair(key, value);
        }
    }
    Ok(parsed.to_string())
}

fn kukulu_query_value(body: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(body.as_bytes())
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn kukulu_add_quality_formats(formats: &mut Vec<serde_json::Value>, body: &str) {
    let quality = kukulu_query_value(body, "now_quality");
    let format_id = quality.clone().unwrap_or_else(|| "unknown".to_owned());
    let quality_priority = quality.as_deref().map_or(-1, kukulu_quality_priority);
    let vcodec = kukulu_query_value(body, "vcodec");
    if let Some(media_url) = kukulu_query_value(body, "hlsaddr").and_then(|value| kukulu_valid_url(&value)) {
        let mut format = serde_json::json!({
            "format_id": format_id,
            "url": media_url,
            "ext": "mp4",
            "quality": quality_priority,
        });
        if let Some(vcodec) = vcodec.as_deref() {
            format["vcodec"] = serde_json::json!(vcodec);
        }
        formats.push(format);
    }
    if let Some(media_url) =
        kukulu_query_value(body, "hlsaddr_audioonly").and_then(|value| kukulu_valid_url(&value))
    {
        formats.push(serde_json::json!({
            "format_id": format!("{format_id}-audioonly"),
            "url": media_url,
            "ext": "m4a",
            "vcodec": "none",
            "quality": quality_priority,
        }));
    }
}

fn kukulu_quality_priority(value: &str) -> i64 {
    match value {
        "low" => 0,
        "h264" => 1,
        "high" => 2,
        _ => -1,
    }
}

fn kukulu_vod_entry(
    video_id: &str,
    index: usize,
    segment: &serde_json::Value,
    title: &str,
    description: Option<&str>,
    thumbnail: Option<&str>,
    webpage_url: &str,
) -> Option<InfoDict> {
    let media_path = json_string(segment, "file")?;
    let media_url = resolve_url("https://live.erinn.biz", media_path);
    let entry_id = format!("{video_id}_{index}");
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(entry_id));
    info.insert("title", serde_json::json!(format!("{title} (Part {index})")));
    info.insert_if_some("description", description);
    info.insert_if_some("timestamp", json_i64(segment, "time_start"));
    info.insert_if_some("thumbnail", thumbnail);
    info.insert("url", serde_json::json!(media_url.clone()));
    info.insert("ext", serde_json::json!("mp4"));
    info.insert(
        "formats",
        serde_json::json!([{
            "url": media_url,
            "ext": "mp4",
            "protocol": "m3u8_native",
        }]),
    );
    info.insert("subtitles", serde_json::json!({}));
    info.insert("webpage_url", serde_json::json!(webpage_url));
    Some(info)
}

fn kukulu_valid_url(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("http://") || value.starts_with("https://"))
        .then(|| value.to_owned())
}
