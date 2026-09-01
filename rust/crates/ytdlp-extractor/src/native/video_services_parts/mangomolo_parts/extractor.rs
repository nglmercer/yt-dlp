fn mangomolo_extract_ids(
    matcher: &Regex,
    url: &str,
    is_live: bool,
) -> Result<(String, String), ExtractorError> {
    let page_id = matcher
        .captures(url)
        .ok()
        .flatten()
        .and_then(|captures| captures.name("id"))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                "Mangomolo URL did not match its native pattern",
            )
        })?;
    let real_id = if is_live {
        mangomolo_live_id(&page_id).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Mangomolo live channel ID {page_id} is not valid base64"),
            )
        })?
    } else {
        page_id
    };
    Ok((real_id, url.to_owned()))
}

fn mangomolo_extract(
    descriptor: &ExtractorDescriptor,
    matcher: &Regex,
    url: &str,
    context: &ExtractionContext,
    player_type: &str,
    is_live: bool,
) -> Result<ExtractorResult, ExtractorError> {
    let (video_id, _) = mangomolo_extract_ids(matcher, url, is_live)?;
    let player_url = mangomolo_player_url(url, player_type).ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::InvalidUrl,
            format!("Mangomolo {player_type} URL has no query string"),
        )
    })?;
    let response = context.get(&player_url)?;
    let webpage = String::from_utf8_lossy(response.body());
    let stream_url = mangomolo_stream_url(&webpage, &video_id)?;
    let format = mangomolo_hls_format(&stream_url, is_live);
    let mut info = InfoDict::new();
    info.insert("id", serde_json::json!(video_id));
    info.insert("title", serde_json::json!(video_id));
    info.insert("url", serde_json::json!(stream_url));
    info.insert("ext", serde_json::json!("mp4"));
    info.insert("formats", serde_json::json!([format]));
    info.insert("is_live", serde_json::json!(is_live));
    info.insert(
        "live_status",
        serde_json::json!(if is_live { "is_live" } else { "not_live" }),
    );
    info.insert_if_some(
        "uploader_id",
        mangomolo_hidden_value(&webpage, "userid"),
    );
    info.insert_if_some(
        "duration",
        mangomolo_hidden_value(&webpage, "duration").and_then(|value| value.parse::<i64>().ok()),
    );
    let _ = descriptor;
    Ok(ExtractorResult::single(info))
}

/// Native Mangomolo VOD player extractor.
pub struct MangomoloVideoExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MangomoloVideoExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MangomoloVideoExtractor {
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
        mangomolo_extract(
            &self.descriptor,
            &self.matcher,
            url,
            context,
            "video",
            false,
        )
    }
}

/// Native Mangomolo live player extractor.
pub struct MangomoloLiveExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MangomoloLiveExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MangomoloLiveExtractor {
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
        mangomolo_extract(
            &self.descriptor,
            &self.matcher,
            url,
            context,
            "live",
            true,
        )
    }
}
