/// Native KiKA.de brand playlist extractor.
pub struct KikaPlaylistExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl KikaPlaylistExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for KikaPlaylistExtractor {
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
        let playlist_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "KiKA playlist URL has no brand ID",
                )
            })?;
        let brand_data = context.get_json(&format!(
            "https://www.kika.de/_next-api/proxy/v1/brands/{playlist_id}"
        ))?;
        let first_page = brand_data
            .get("videoSubchannel")
            .and_then(|subchannel| json_string(subchannel, "videosPageUrl"))
            .and_then(kika_http_url)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("KiKA brand {playlist_id} has no video page URL"),
                )
            })?;
        let entries = kika_playlist_entries(context, &first_page, &playlist_id)?;
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(playlist_id));
        info.insert_if_some("title", json_string(&brand_data, "title"));
        info.insert_if_some("description", json_string(&brand_data, "description"));
        info.insert("webpage_url", serde_json::json!(url));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn kika_playlist_entries(
    context: &ExtractionContext,
    first_page: &str,
    playlist_id: &str,
) -> Result<Vec<InfoDict>, ExtractorError> {
    let mut entries = Vec::new();
    let mut page_url = Some(first_page.to_owned());
    let mut seen_pages = Vec::new();
    while let Some(current_url) = page_url.take() {
        if seen_pages.contains(&current_url) {
            break;
        }
        seen_pages.push(current_url.clone());
        let data = context.get_json(&current_url).map_err(|error| {
            ExtractorError::new(
                error.kind.clone(),
                format!("KiKA brand {playlist_id} page request failed: {error}"),
            )
        })?;
        for item in data
            .get("content")
            .into_iter()
            .flat_map(json_object_values)
        {
            let Some(api_url) = item
                .get("api")
                .and_then(|api| json_string(api, "url"))
                .and_then(kika_http_url)
            else {
                continue;
            };
            let mut entry = native_url_result(&api_url);
            entry.insert("ie_key", serde_json::json!("Kika"));
            entry.insert_if_some("id", json_string(item, "id"));
            entry.insert_if_some("title", json_string(item, "title"));
            entry.insert_if_some("duration", json_i64(item, "duration"));
            entry.insert_if_some(
                "timestamp",
                json_string(item, "date").and_then(|value| parse_timestamp(value.to_owned())),
            );
            entries.push(entry);
        }
        page_url = data
            .get("links")
            .and_then(|links| json_string(links, "next"))
            .and_then(kika_http_url)
            .filter(|next| !seen_pages.contains(next));
    }
    Ok(entries)
}
