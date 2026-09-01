/// Native Radio France podcast episode JSON-LD/audio extractor.
pub struct FranceCultureExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl FranceCultureExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for FranceCultureExtractor {
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
                "Radio France podcast URL did not match its native pattern",
            )
        })?;
        let video_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Radio France podcast URL has no episode ID",
                )
            })?;
        let display_id = captures
            .name("display_id")
            .map(|value| value.as_str().to_owned())
            .unwrap_or_else(|| video_id.clone());
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let json_ld = html_json_ld(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Radio France episode {video_id} has no JSON-LD metadata"),
            )
        })?;
        let audio = france_culture_audio_object(&json_ld).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Radio France episode {video_id} has no audio data"),
            )
        })?;
        let media_url = json_string(audio, "contentUrl")
            .map(|value| proto_relative_url(value, "https:"))
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Radio France episode {video_id} has no audio URL"),
                )
            })?;
        let extension = mimetype_extension(json_string(audio, "encodingFormat"))
            .unwrap_or_else(|| yt_dlp_core::determine_ext(Some(&media_url), "mp3"));
        let title = france_culture_h1_title(&webpage)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .unwrap_or_else(|| display_id.clone());
        let description = html_meta_value(&webpage, "description");
        let thumbnail = html_meta_value(&webpage, "og:image");
        let published = france_culture_json_string(&json_ld, "datePublished");
        let duration = json_f64(audio, "duration").or_else(|| {
            json_string(audio, "duration").and_then(yt_dlp_core::parse_duration)
        });
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("title", serde_json::json!(title));
        info.insert("url", serde_json::json!(media_url.clone()));
        info.insert("ext", serde_json::json!(extension.clone()));
        info.insert_if_some("description", description);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert_if_some("uploader", france_culture_uploader(&webpage));
        info.insert_if_some("duration", duration);
        info.insert_if_some(
            "timestamp",
            published.clone().and_then(parse_timestamp),
        );
        info.insert_if_some(
            "upload_date",
            published.as_deref().and_then(date_digits),
        );
        if json_string(audio, "encodingFormat") == Some("mp3") {
            info.insert("vcodec", serde_json::json!("none"));
        }
        info.insert(
            "formats",
            serde_json::json!([{
                "url": media_url,
                "format_id": "audio",
                "ext": extension,
                "vcodec": "none",
            }]),
        );
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn france_culture_audio_object(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if json_string(value, "@type") == Some("AudioObject")
                && object.contains_key("contentUrl")
            {
                Some(value)
            } else {
                object.values().find_map(france_culture_audio_object)
            }
        }
        serde_json::Value::Array(values) => values.iter().find_map(france_culture_audio_object),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn france_culture_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => json_string(value, key)
            .map(str::to_owned)
            .or_else(|| object.values().find_map(|value| france_culture_json_string(value, key))),
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| france_culture_json_string(value, key)),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

fn france_culture_h1_title(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<h1\b[^>]*\bitemprop\s*=\s*["'][^"']*\bname\b[^"']*["'][^>]*>(.*?)</h1\s*>"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn france_culture_uploader(html: &str) -> Option<String> {
    let matcher = Regex::new(
        r#"(?is)<span\b[^>]*\bclass\s*=\s*["'][^"']*\bauthor\b[^"']*["'][^>]*>(.*?)</span\s*>"#,
    )
    .ok()?;
    matcher
        .captures(html)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| html_text_fragment(value.as_str()))
        .filter(|value| !value.is_empty())
}
