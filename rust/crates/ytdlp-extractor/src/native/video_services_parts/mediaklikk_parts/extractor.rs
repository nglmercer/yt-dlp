pub struct MediaKlikkExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl MediaKlikkExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for MediaKlikkExtractor {
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
            ExtractorError::new(ExtractorErrorKind::InvalidUrl, "MediaKlikk URL has no match")
        })?;
        let display_id = captures
            .name("id")
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "MediaKlikk URL has no display ID",
                )
            })?;
        let webpage = mediaklikk_page(context, url)?;
        let player_data = mediaklikk_player_data(&webpage, &display_id)?;
        let video_id = mediaklikk_video_id(&player_data).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("MediaKlikk page {display_id} has no content ID"),
            )
        })?;
        let player_json = mediaklikk_player_json(context, url, &video_id, &player_data)?;
        let media_url = mediaklikk_hls_url(&player_json, &video_id)?;
        let format = mediaklikk_format(media_url.clone());
        let title = json_string(&player_data, "title")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:title"))
            .or_else(|| {
                Regex::new(r#"(?is)<h\d+\b[^>]*\bclass\s*=\s*["'][^"']*article_title[^"']*["'][^>]*>(.*?)<"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .map(|value| html_text_fragment(value.as_str()))
                    .filter(|value| !value.is_empty())
            });
        let upload_date = captures
            .name("year")
            .zip(captures.name("month"))
            .zip(captures.name("day"))
            .map(|((year, month), day)| {
                format!(
                    "{}{:0>2}{:0>2}",
                    year.as_str(),
                    month.as_str(),
                    day.as_str()
                )
            })
            .or_else(|| {
                Regex::new(r#"(?is)<p\b[^>]*\bclass\s*=\s*["'][^"']*article_date[^"']*["'][^>]*>([^<]+)<"#)
                    .ok()
                    .and_then(|matcher| matcher.captures(&webpage).ok().flatten())
                    .and_then(|captures| captures.get(1))
                    .and_then(|value| date_digits(value.as_str()))
            });
        let thumbnail = json_string(&player_data, "bgImage")
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| html_meta_value(&webpage, "og:image"));
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", title);
        info.insert_if_some("upload_date", upload_date);
        info.insert_if_some("thumbnail", thumbnail);
        info.insert("url", serde_json::json!(media_url));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::json!([format]));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}
