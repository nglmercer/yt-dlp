pub struct LeFigaroVideoEmbedExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

pub struct LeFigaroVideoSectionExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl LeFigaroVideoEmbedExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl LeFigaroVideoSectionExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

fn lefigaro_id(
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
                format!("Le Figaro {label} URL has no ID"),
            )
        })
}

fn lefigaro_transparent_result(
    url: &str,
    video_id: &str,
    title: Option<String>,
    description: Option<String>,
    thumbnail: Option<String>,
) -> InfoDict {
    let mut info = InfoDict::new();
    info.insert("_type", serde_json::json!("url_transparent"));
    info.insert("url", serde_json::json!(format!("jwplatform:{video_id}")));
    info.insert("ie_key", serde_json::json!("JWPlatform"));
    info.insert("id", serde_json::json!(video_id));
    info.insert_if_some("title", title);
    info.insert_if_some("description", description);
    info.insert_if_some("thumbnail", thumbnail);
    info.insert("webpage_url", serde_json::json!(url));
    info
}

fn lefigaro_section_entries(
    response: &serde_json::Value,
    display_id: &str,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let playlist = lefigaro_playlist(response, display_id)?;
    let elements = playlist
        .get("jsonLd")
        .and_then(serde_json::Value::as_array)
        .and_then(|json_ld| json_ld.first())
        .and_then(|json_ld| json_ld.get("itemListElement"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le Figaro section {display_id} has no video list"),
            )
        })?;
    let mut entries = Vec::new();
    for video in elements {
        let embed_url = json_string(video, "embedUrl")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le Figaro section {display_id} has a video without embed URL"),
                )
            })?;
        let entry_id = json_value_string(video.get("videoId"))
            .or_else(|| json_value_string(video.get("id")))
            .unwrap_or_else(|| embed_url.to_owned());
        entries.push(lefigaro_transparent_result(
            embed_url,
            &entry_id,
            json_string(video, "name").map(html_text_fragment),
            json_string(video, "description").map(html_text_fragment),
            json_string(video, "thumbnailUrl").map(|value| resolve_url(embed_url, value)),
        ));
    }
    Ok(entries)
}

impl InfoExtractor for LeFigaroVideoEmbedExtractor {
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
        let display_id = lefigaro_id(&self.matcher, url, "embed")?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let player_data = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .and_then(|page_props| page_props.get("initialProps"))
            .and_then(|initial_props| initial_props.get("pageData"))
            .and_then(|page_data| page_data.get("playerData"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Le Figaro embed {display_id} has no player data"),
                )
            })?;
        let video_id = json_value_string(player_data.get("videoId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("Le Figaro embed {display_id} has no JWPlatform video ID"),
            )
        })?;
        Ok(ExtractorResult::single(lefigaro_transparent_result(
            url,
            &video_id,
            json_string(player_data, "title").map(html_text_fragment),
            json_string(player_data, "description").map(html_text_fragment),
            json_string(player_data, "poster").map(|value| resolve_url(url, value)),
        )))
    }
}

impl InfoExtractor for LeFigaroVideoSectionExtractor {
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
        let display_id = lefigaro_id(&self.matcher, url, "section")?;
        let initial_response = lefigaro_api_response(context, &display_id, 1)?;
        let page_count = lefigaro_page_count(&initial_response, &display_id)?;
        let mut entries = lefigaro_section_entries(&initial_response, &display_id)?;
        for page in 2..=page_count {
            let response = lefigaro_api_response(context, &display_id, page)?;
            entries.extend(lefigaro_section_entries(&response, &display_id)?);
        }
        let playlist = lefigaro_playlist(&initial_response, &display_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(display_id));
        info.insert_if_some("title", json_string(playlist, "title").map(html_text_fragment));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}
