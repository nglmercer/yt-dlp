/// Native Icareus playback and metadata extractor.
pub struct IcareusExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl IcareusExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for IcareusExtractor {
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
                "Icareus URL did not match its native pattern",
            )
        })?;
        let base_url = captures
            .name("base_url")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Icareus URL has no base URL")
            })?;
        let requested_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Icareus URL has no asset ID")
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let video_id = icareus_assignment(&webpage, "itemId").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Icareus asset {requested_id} has no item ID"),
            )
        })?;
        let organization_id = icareus_assignment(&webpage, "organizationId").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Icareus asset {video_id} has no organization ID"),
            )
        })?;
        let service_url = icareus_assignment(&webpage, "publishingServiceURL").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Icareus asset {video_id} has no playback API URL"),
            )
        })?;
        let token = icareus_assignment(&webpage, "token").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Icareus asset {video_id} has no playback token"),
            )
        })?;
        let assets = icareus_post_json(
            context,
            &service_url,
            [
                ("version", "03"),
                ("action", "getAssetPlaybackUrls"),
                ("organizationId", organization_id.as_str()),
                ("assetId", video_id.as_str()),
                ("token", token.as_str()),
            ],
            &video_id,
        )?;
        let mut formats = icareus_audio_formats(&assets);
        formats.extend(icareus_video_formats(&assets));
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Icareus asset {video_id} has no playable formats"),
            ));
        }
        let subtitles = icareus_subtitles(&assets);
        let json_ld = html_json_ld(&webpage).unwrap_or(serde_json::Value::Null);
        let mut metadata = json_ld.clone();
        let mut live_title = None;
        if json_ld.is_null() {
            if let Some(asset_token) = icareus_asset_token(&webpage) {
                let metadata_url = format!("{base_url}/icareus-suite-api-portlet/publishing");
                metadata = icareus_post_json(
                    context,
                    &metadata_url,
                    [
                        ("version", "03"),
                        ("action", "getAsset"),
                        ("organizationId", organization_id.as_str()),
                        ("assetId", video_id.as_str()),
                        ("languageId", "en_US"),
                        ("userId", "0"),
                        ("token", asset_token.as_str()),
                    ],
                    &video_id,
                )
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            } else {
                live_title = html_element_by_class(
                    &webpage,
                    "unpublished-info-item future-event-title",
                )
                .map(|value| html_text_fragment(&value));
            }
        }
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let first_url = first
            .get("url")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let first_ext = first
            .get("ext")
            .cloned()
            .unwrap_or_else(|| serde_json::json!("mp4"));
        let title = icareus_json_ld_string(&metadata, &["title", "name"])
            .or(live_title)
            .or_else(|| html_title_value(&webpage))
            .unwrap_or_else(|| video_id.clone());
        let description = icareus_json_ld_string(&metadata, &["description"]).or_else(|| {
            html_element_by_class(
                &webpage,
                "unpublished-info-item future-event-description",
            )
            .map(|value| html_text_fragment(&value))
        });
        let timestamp = json_f64(&metadata, "date")
            .map(|value| (value / 1000.0) as i64)
            .or_else(|| {
                icareus_json_ld_string(&metadata, &["datePublished"])
                    .and_then(parse_timestamp)
            })
            .or_else(|| {
                icareus_assignment(&webpage, "startEvent")
                    .and_then(|value| value.parse::<i64>().ok())
                    .map(|value| value / 1000)
            });
        let duration = json_f64(&metadata, "duration");
        let thumbnail = icareus_json_ld_string(&metadata, &["thumbnail", "image"])
            .or_else(|| json_string(&assets, "thumbnail").map(str::to_owned))
            .or_else(|| json_string(&metadata, "thumbnailMedium").map(str::to_owned));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", first_url);
        info.insert("ext", first_ext);
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", subtitles);
        info.insert_if_some("description", description);
        info.insert_if_some("timestamp", timestamp);
        info.insert_if_some("duration", duration);
        info.insert_if_some("thumbnail", thumbnail.clone());
        if let Some(thumbnail) = thumbnail {
            info.insert("thumbnails", serde_json::json!([{"url": thumbnail}]));
        }
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(info))
    }
}

fn icareus_assignment(webpage: &str, name: &str) -> Option<String> {
    let escaped_name = regex::escape(name);
    [
        format!(r#"(?is)\b{}\s*=\s*["']([^"']+)["']"#, escaped_name),
        format!(
            r#"(?is)\b_icareus\s*\[\s*["']{}\s*["']\s*\]\s*=\s*["']([^"']+)["']"#,
            escaped_name
        ),
    ]
    .iter()
    .find_map(|pattern| {
        Regex::new(pattern)
            .ok()
            .and_then(|matcher| matcher.captures(webpage).ok().flatten())
            .and_then(|captures| captures.get(1))
            .map(|value| unescape_html_attribute(value.as_str()))
            .filter(|value| !value.trim().is_empty())
    })
}

fn icareus_post_json<const N: usize>(
    context: &ExtractionContext,
    url: &str,
    fields: [(&str, &str); N],
    video_id: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in fields {
        form.append_pair(key, value);
    }
    let mut request = Request::new(url);
    request.set_method("POST").map_err(map_request_error)?;
    request.set_data(Some(form.finish().into_bytes()));
    let response = context.request(&request)?;
    serde_json::from_slice(response.body()).map_err(|error| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("invalid Icareus JSON for {video_id}: {error}"),
        )
    })
}

fn icareus_audio_formats(assets: &serde_json::Value) -> Vec<serde_json::Value> {
    assets
        .get("audio_urls")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let media_url = json_string(item, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let name = json_string(item, "name");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": "audio",
                "ext": yt_dlp_core::determine_ext(Some(media_url), "mp3"),
                "vcodec": "none",
            });
            if let Some(name) = name {
                format["format"] = serde_json::json!(name);
                if let Some(tbr) = icareus_bitrate(name) {
                    format["tbr"] = serde_json::json!(tbr);
                }
            }
            Some(format)
        })
        .collect()
}

fn icareus_video_formats(assets: &serde_json::Value) -> Vec<serde_json::Value> {
    assets
        .get("urls")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let media_url = json_string(item, "url")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))?;
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let name = json_string(item, "name");
            let mut format = serde_json::json!({
                "url": media_url,
                "format_id": json_string(item, "id").unwrap_or("source"),
                "ext": if extension == "m3u8" { "mp4" } else { extension.as_str() },
                "protocol": if extension == "m3u8" { "m3u8_native" } else { "http" },
            });
            if let Some(name) = name {
                format["format"] = serde_json::json!(name);
                if let Some(tbr) = icareus_bitrate(name) {
                    format["tbr"] = serde_json::json!(tbr);
                }
                if let Some((width, height)) = icareus_resolution(name) {
                    format["width"] = serde_json::json!(width);
                    format["height"] = serde_json::json!(height);
                } else if let Some(height) = icareus_height(name) {
                    format["height"] = serde_json::json!(height);
                }
            }
            Some(format)
        })
        .collect()
}

fn icareus_subtitles(assets: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    for item in assets
        .get("subtitles")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(values) = item.as_array() else {
            continue;
        };
        let Some(description) = values.get(1).and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(media_url) = values
            .get(2)
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        else {
            continue;
        };
        let language = description
            .split_whitespace()
            .next()
            .unwrap_or("und")
            .trim_end_matches(':');
        subtitles
            .entry(language.to_owned())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("Icareus subtitle entry is an array")
            .push(serde_json::json!({"url": media_url}));
    }
    serde_json::Value::Object(subtitles)
}

fn icareus_bitrate(value: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?i)\b(\d+)\s*kbps\b"#).ok()?;
    matcher
        .captures(value)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse().ok())
}

fn icareus_resolution(value: &str) -> Option<(i64, i64)> {
    let matcher = Regex::new(r#"(?i)(\d+)\s*[x×]\s*(\d+)"#).ok()?;
    let captures = matcher.captures(value).ok().flatten()?;
    Some((
        captures.get(1)?.as_str().parse().ok()?,
        captures.get(2)?.as_str().parse().ok()?,
    ))
}

fn icareus_height(value: &str) -> Option<i64> {
    let matcher = Regex::new(r#"(?i)\b(\d{2,5})p\b"#).ok()?;
    matcher
        .captures(value)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse().ok())
}

fn icareus_asset_token(webpage: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)\bdata\s*:\s*\{\s*action\s*:\s*["']getAsset["'][^}]*?\btoken\s*:\s*['"]([a-f0-9]+)"#,
    )
    .ok()?;
    matcher
        .captures(webpage)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
}

fn icareus_json_ld_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(values) => {
            for key in keys {
                if let Some(value) = values.get(*key).and_then(serde_json::Value::as_str) {
                    return Some(value.to_owned());
                }
            }
            values
                .values()
                .find_map(|value| icareus_json_ld_string(value, keys))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| icareus_json_ld_string(value, keys)),
        _ => None,
    }
}
