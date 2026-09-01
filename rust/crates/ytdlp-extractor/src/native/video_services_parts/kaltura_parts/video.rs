/// Native Kaltura player and flavor-asset extractor.
pub struct KalturaExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KalturaExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KalturaExtractor {
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
        let target = kaltura_target(url)?;
        let (info, flavor_assets, captions) = kaltura_fetch_video(context, &target)?;
        let data_url = json_string(&info, "dataUrl")
            .filter(|value| !value.trim().is_empty())
            .map(kaltura_normalize_data_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Kaltura entry {} has no data URL", target.entry_id),
                )
            })?;
        let mut formats = kaltura_flavor_formats(&flavor_assets, &data_url, target.ks.as_deref());
        if data_url.contains("/playManifest/") {
            let manifest_url = kaltura_signed_url(
                &data_url.replace("format/url", "format/applehttp"),
                target.ks.as_deref(),
            );
            formats.push(serde_json::json!({
                "url": manifest_url,
                "format_id": "hls",
                "ext": "mp4",
                "protocol": "m3u8_native",
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Kaltura entry {} has no playable formats", target.entry_id),
            ));
        }
        let subtitles = kaltura_subtitles(&captions);
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let title = json_string(&info, "name")
            .map(str::to_owned)
            .unwrap_or_else(|| target.entry_id.clone());
        let mut output = InfoDict::new();
        output.insert("id", serde_json::json!(target.entry_id));
        output.insert("title", serde_json::json!(title));
        output.insert_if_some(
            "description",
            json_string(&info, "description")
                .map(html_text_fragment)
                .filter(|value| !value.is_empty()),
        );
        output.insert_if_some("thumbnail", json_string(&info, "thumbnailUrl"));
        output.insert_if_some("duration", json_f64(&info, "duration"));
        output.insert_if_some("timestamp", json_i64(&info, "createdAt"));
        output.insert_if_some(
            "uploader_id",
            json_value_string(info.get("userId"))
                .filter(|value| !matches!(value.as_str(), "" | "None")),
        );
        output.insert_if_some("view_count", json_i64(&info, "plays"));
        output.insert(
            "url",
            first
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        output.insert(
            "ext",
            first
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        output.insert("formats", serde_json::Value::Array(formats));
        output.insert("subtitles", subtitles);
        output.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::single(output))
    }
}

fn kaltura_normalize_data_url(value: &str) -> String {
    value.find("/flvclipper/").map_or_else(
        || value.to_owned(),
        |index| format!("{}{}", &value[..index], "/serveFlavor"),
    )
}

fn kaltura_flavor_formats(
    flavor_assets: &serde_json::Value,
    data_url: &str,
    ks: Option<&str>,
) -> Vec<serde_json::Value> {
    let mut formats = Vec::new();
    for asset in flavor_assets
        .get("objects")
        .into_iter()
        .flat_map(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if json_i64(asset, "status") != Some(2) {
            continue;
        }
        let source_ext = json_string(asset, "fileExt").map(str::to_owned);
        if matches!(source_ext.as_deref(), Some("chun" | "wvm")) {
            continue;
        }
        let extension = source_ext.unwrap_or_else(|| {
            if json_string(asset, "containerFormat") == Some("qt") {
                "mov".to_owned()
            } else {
                "mp4".to_owned()
            }
        });
        let Some(asset_id) = json_value_string(asset.get("id")) else {
            continue;
        };
        let bitrate =
            json_value_string(asset.get("bitrate")).unwrap_or_else(|| "unknown".to_owned());
        let media_url =
            kaltura_signed_url(&format!("{data_url}/flavorId/{asset_id}"), ks);
        let mut format = serde_json::json!({
            "format_id": format!("{extension}-{bitrate}"),
            "ext": extension,
            "url": media_url,
        });
        if let Some(object) = format.as_object_mut() {
            if let Some(value) = json_i64(asset, "bitrate") {
                object.insert("tbr".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(asset, "frameRate") {
                object.insert("fps".to_owned(), serde_json::json!(value));
            }
            if let Some(value) =
                json_i64(asset, "size").and_then(|value| value.checked_mul(1024))
            {
                object.insert("filesize_approx".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_string(asset, "containerFormat") {
                object.insert("container".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(asset, "height") {
                object.insert("height".to_owned(), serde_json::json!(value));
            }
            if let Some(value) = json_i64(asset, "width") {
                object.insert("width".to_owned(), serde_json::json!(value));
            }
            let video_codec = json_value_string(asset.get("videoCodecId"));
            if video_codec.is_none() && json_i64(asset, "frameRate") == Some(0) {
                object.insert("vcodec".to_owned(), serde_json::json!("none"));
            } else if let Some(value) = video_codec {
                object.insert("vcodec".to_owned(), serde_json::json!(value));
            }
        }
        formats.push(format);
    }
    formats
}

fn kaltura_subtitles(captions: &serde_json::Value) -> serde_json::Value {
    let mut subtitles = serde_json::Map::new();
    for caption in captions
        .get("objects")
        .into_iter()
        .flat_map(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if json_i64(caption, "status") != Some(2) {
            continue;
        }
        let Some(caption_id) = json_value_string(caption.get("id")) else {
            continue;
        };
        let language = json_string(caption, "languageCode")
            .or_else(|| json_string(caption, "language"))
            .unwrap_or("und")
            .to_owned();
        let extension = json_string(caption, "fileExt")
            .map(str::to_owned)
            .or_else(|| {
                json_i64(caption, "format").and_then(|value| {
                    Some(
                        match value {
                            1 => "srt",
                            2 => "ttml",
                            3 => "vtt",
                            _ => return None,
                        }
                        .to_owned(),
                    )
                })
            })
            .unwrap_or_else(|| "ttml".to_owned());
        let track = serde_json::json!({
            "url": format!(
                "{KALTURA_SERVICE_URL}/api_v3/service/caption_captionasset/action/serve/captionAssetId/{caption_id}"
            ),
            "ext": extension,
        });
        subtitles
            .entry(language)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("Kaltura subtitle language is an array")
            .push(track);
    }
    serde_json::Value::Object(subtitles)
}
