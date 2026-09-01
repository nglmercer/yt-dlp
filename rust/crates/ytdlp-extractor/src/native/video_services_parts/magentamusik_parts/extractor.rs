/// Native MagentaMusik page/API/SMIL extractor.
pub struct MagentaMusikExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MagentaMusikExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MagentaMusikExtractor {
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
        let display_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "MagentaMusik URL has no display ID",
                )
            })?;
        let response = context.get(url)?;
        let webpage = String::from_utf8_lossy(response.body());
        let player_config = magentamusik_page_config(&webpage).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagentaMusik page {display_id} has no video player"),
            )
        })?;
        let asset_id = magentamusik_value_string(player_config.get("assetId")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagentaMusik page {display_id} has no asset ID"),
            )
        })?;
        let asset_details_url = format!(
            "https://wcps.t-online.de/cvss/magentamusic/vodclient/v2/assetdetails/58938/{asset_id}"
        );
        let asset_details = magentamusik_api_json(context, &asset_details_url, "asset details")?;
        let video_id = magentamusik_find_reference(&asset_details).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagentaMusik asset {asset_id} has no video reference"),
            )
        })?;
        let vod_url = format!(
            "https://wcps.t-online.de/cvss/magentamusic/vodclient/v2/player/58935/{video_id}/Main%20Movie"
        );
        let vod_data = magentamusik_api_json(context, &vod_url, "VOD")?;
        let smil_url = magentamusik_find_media_href(&vod_data).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MagentaMusik video {video_id} has no SMIL media URL"),
            )
        })?;
        let smil_response = context.get(&smil_url)?;
        let formats =
            magentamusik_parse_smil(smil_response.body(), smil_response.url(), &video_id)?;
        let metadata = magentamusik_feature_metadata(&vod_data);
        let first = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert_if_some("title", json_string(metadata, "title"));
        info.insert_if_some("alt_title", json_string(metadata, "originalTitle"));
        info.insert_if_some("description", json_string(metadata, "longDescription"));
        info.insert_if_some("duration", json_i64(metadata, "runtimeInSeconds"));
        info.insert_if_some(
            "location",
            magentamusik_string_list(metadata.get("countriesOfProduction")),
        );
        info.insert_if_some("release_year", json_i64(metadata, "yearOfProduction"));
        info.insert_if_some(
            "categories",
            magentamusik_categories(metadata.get("mainGenre")),
        );
        info.insert_if_some("url", first.get("url").and_then(serde_json::Value::as_str));
        info.insert_if_some("ext", first.get("ext").and_then(serde_json::Value::as_str));
        Ok(ExtractorResult::single(info))
    }
}
