/// Native Mojevideo page/signed-MP4 extractor.
pub struct MojevideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MojevideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MojevideoExtractor {
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
                "Mojevideo URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "Mojevideo URL has no ID")
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Mojevideo URL has no display ID",
                )
            })?;
        let webpage = mojevideo_page(context, url)?;
        let video_id_dec = mojevideo_capture(&webpage, r#"\bvId\s*=\s*(\d+)"#, "video ID")
            .or_else(|_| {
                u128::from_str_radix(&video_id, 16)
                    .map(|value| value.to_string())
                    .map_err(|_| {
                        ExtractorError::new(
                            ExtractorErrorKind::Extraction,
                            format!("Mojevideo ID {video_id} is not hexadecimal"),
                        )
                    })
            })?;
        let video_expiry = mojevideo_capture(
            &webpage,
            r#"\bvEx\s*=\s*["'](\d+)"#,
            "video expiry",
        )?;
        let hashes = mojevideo_hashes(&webpage)?;
        let formats = mojevideo_formats(&video_id, &video_id_dec, &video_expiry, &hashes)?;
        let json_ld = html_json_ld(&webpage).unwrap_or(serde_json::Value::Null);
        let json_ld = mojevideo_json_ld_object(&json_ld);
        let title = json_ld
            .and_then(|value| json_string(value, "name").map(str::to_owned))
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| {
                html_title_value(&webpage).map(|title| {
                    title
                        .strip_suffix(" - Mojevideo")
                        .unwrap_or(&title)
                        .to_owned()
                })
            });
        let description = json_ld
            .and_then(|value| json_string(value, "description").map(str::to_owned))
            .or_else(|| html_meta_value(&webpage, "og:description"));
        let thumbnail = json_ld
            .and_then(|value| mojevideo_thumbnail(value.get("thumbnailUrl")))
            .or_else(|| html_meta_value(&webpage, "og:image"));
        let first_url = formats
            .first()
            .and_then(|format| format.get("url"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", title);
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        if let Some(json_ld) = json_ld {
            info.insert_if_some(
                "duration",
                mojevideo_json_ld_duration(json_ld.get("duration")),
            );
            let upload_date = json_string(json_ld, "uploadDate")
                .map(str::to_owned)
                .and_then(|value| date_digits(&value));
            info.insert_if_some("upload_date", upload_date);
            info.insert_if_some(
                "timestamp",
                json_string(json_ld, "uploadDate")
                    .map(str::to_owned)
                    .and_then(parse_timestamp),
            );
            mojevideo_insert_interaction_counts(&mut info, json_ld);
        }
        info.insert("url", first_url);
        info.insert("ext", serde_json::json!("mp4"));
        Ok(ExtractorResult::single(info))
    }
}

fn mojevideo_json_ld_object(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("name")
                || object.contains_key("contentUrl")
                || object.contains_key("interactionStatistic")
            {
                Some(value)
            } else {
                object.get("@graph").and_then(mojevideo_json_ld_object)
            }
        }
        serde_json::Value::Array(values) => values.iter().find_map(mojevideo_json_ld_object),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn mojevideo_json_ld_duration(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|value| value as f64))
            .or_else(|| value.as_u64().map(|value| value as f64))
            .or_else(|| value.as_str().and_then(yt_dlp_core::parse_duration))
    })
}

fn mojevideo_thumbnail(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.to_owned()),
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| mojevideo_thumbnail(Some(value))),
        serde_json::Value::Object(object) => object
            .get("url")
            .and_then(|value| value.as_str().map(str::to_owned)),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => None,
    }
}

fn mojevideo_interaction_count(value: &serde_json::Value) -> Option<i64> {
    json_i64(value, "userInteractionCount")
        .or_else(|| json_i64(value, "interactionCount"))
}

fn mojevideo_insert_interaction_counts(info: &mut InfoDict, json_ld: &serde_json::Value) {
    let Some(statistics) = json_ld.get("interactionStatistic") else {
        return;
    };
    let values = match statistics {
        serde_json::Value::Array(values) => values.iter().collect::<Vec<_>>(),
        _ => vec![statistics],
    };
    for statistic in values {
        let kind = json_string(statistic, "interactionType")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let Some(count) = mojevideo_interaction_count(statistic) else {
            continue;
        };
        let key = if kind.contains("dislike") {
            "dislike_count"
        } else if kind.contains("like") {
            "like_count"
        } else if kind.contains("comment") {
            "comment_count"
        } else if kind.contains("watch") || kind.contains("view") || kind.contains("play") {
            "view_count"
        } else {
            continue;
        };
        info.insert(key, serde_json::json!(count));
    }
}
