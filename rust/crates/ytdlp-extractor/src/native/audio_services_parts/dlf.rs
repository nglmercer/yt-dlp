/// Native Deutschlandfunk button-attribute audio and corpus playlist extractors.
pub struct DlfExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DlfExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for DlfExtractor {
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
                "Deutschlandfunk URL did not match its native pattern",
            )
        })?;
        let page_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Deutschlandfunk URL has no page ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let buttons = dlf_audio_buttons(&webpage);
        if self.descriptor.key == "DLFCorpusIE" {
            if buttons.is_empty() {
                return Err(ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Deutschlandfunk corpus {page_id} has no audio buttons"),
                ));
            }
            let entries = buttons
                .iter()
                .map(|button| dlf_parse_button(button, None))
                .collect::<Result<Vec<_>, _>>()?;
            let mut info = InfoDict::new();
            info.insert("id", serde_json::json!(page_id));
            info.insert_if_some(
                "title",
                dlf_page_meta(&webpage, "og:title").or_else(|| dlf_page_meta(&webpage, "twitter:title")),
            );
            info.insert_if_some(
                "description",
                dlf_page_meta(&webpage, "description")
                    .or_else(|| dlf_page_meta(&webpage, "og:description")),
            );
            return Ok(ExtractorResult::Playlist { info, entries });
        }

        let button = buttons.first().ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Deutschlandfunk page {page_id} has no audio button"),
            )
        })?;
        Ok(ExtractorResult::single(dlf_parse_button(
            button,
            Some(page_id.as_str()),
        )?))
    }
}

fn dlf_audio_buttons(html: &str) -> Vec<String> {
    let Ok(matcher) = Regex::new(r"(?is)<button\b[^>]*>") else {
        return Vec::new();
    };
    matcher
        .captures_iter(html)
        .flatten()
        .filter_map(|captures| captures.get(0).map(|value| value.as_str().to_owned()))
        .filter(|button| {
            dlf_attribute(button, "alt").as_deref() == Some("Anhören")
                && dlf_attribute(button, "data-audio-diraid").is_some()
        })
        .collect()
}

fn dlf_parse_button(
    button: &str,
    explicit_id: Option<&str>,
) -> Result<InfoDict, ExtractorError> {
    let audio_id = explicit_id
        .map(str::to_owned)
        .or_else(|| dlf_attribute(button, "data-audio-diraid"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                "Deutschlandfunk audio button has no audio ID",
            )
        })?;
    let media_url = [
        "data-audio-download-src",
        "data-audio",
        "data-audioreference",
        "data-audio-src",
    ]
    .into_iter()
    .find_map(|name| dlf_attribute(button, name))
    .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    .ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Deutschlandfunk audio {audio_id} has no media URL"),
        )
    })?;
    let detected_ext = yt_dlp_core::determine_ext(Some(&media_url), "unknown");
    let is_hls = detected_ext.eq_ignore_ascii_case("m3u8");
    let output_ext = if is_hls {
        "m4a".to_owned()
    } else {
        detected_ext.clone()
    };
    let format = if is_hls {
        serde_json::json!({
            "url": media_url,
            "format_id": "hls",
            "protocol": "m3u8_native",
            "ext": output_ext,
            "vcodec": "none",
        })
    } else {
        serde_json::json!({
            "url": media_url,
            "format_id": "http",
            "ext": output_ext,
            "protocol": "http",
            "vcodec": "none",
        })
    };
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(audio_id));
    info.insert("extractor_key", serde_json::json!("DLF"));
    info.insert("extractor", serde_json::json!("dlf"));
    info.insert_if_some(
        "title",
        [
            "data-audiotitle",
            "data-audio-title",
            "data-audio-download-tracking-title",
        ]
        .into_iter()
        .find_map(|name| dlf_attribute(button, name)),
    );
    info.insert_if_some(
        "duration",
        ["data-audioduration", "data-audio-duration"]
            .into_iter()
            .find_map(|name| dlf_attribute(button, name))
            .and_then(|value| value.parse::<i64>().ok()),
    );
    for (field, attribute) in [
        ("thumbnail", "data-audioimage"),
        ("uploader", "data-audio-producer"),
        ("series", "data-audio-series"),
        ("channel", "data-audio-origin-site-name"),
        ("webpage_url", "data-audio-download-tracking-path"),
    ] {
        info.insert_if_some(field, dlf_attribute(button, attribute));
    }
    info.insert("url", serde_json::json!(media_url));
    info.insert("ext", serde_json::json!(output_ext));
    info.insert("formats", serde_json::json!([format]));
    Ok(info)
}

fn dlf_attribute(button: &str, attribute: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*[\"']([^\"']*)"#,
        regex::escape(attribute)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(button)
        .ok()
        .flatten()
        .and_then(|captures| captures.get(1))
        .map(|value| unescape_html_attribute(value.as_str()))
}

fn dlf_page_meta(html: &str, key: &str) -> Option<String> {
    html_meta_value(html, key).map(|value| unescape_html_attribute(&value))
}
