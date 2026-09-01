/// Native European Parliament webstream extractor.
pub struct EuroParlWebstreamExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EuroParlWebstreamExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EuroParlWebstreamExtractor {
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
                    "European Parliament webstream URL has no display ID",
                )
            })?;
        let page_response = context.get(url)?;
        let webpage = String::from_utf8_lossy(page_response.body());
        let next_data = html_script_json(&webpage, "__NEXT_DATA__")?;
        let page_props = next_data
            .get("props")
            .and_then(|props| props.get("pageProps"))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("European Parliament page {display_id} has no page props"),
                )
            })?;

        let mut api_request =
            Request::new("https://acs-api.europarl.connectedviews.eu/api/FullMeeting");
        api_request.update_query(&[
            ("api-version".to_owned(), "1.0".to_owned()),
            (
                "tenantId".to_owned(),
                "bae646ca-1fc8-4363-80ba-2c04f06b4968".to_owned(),
            ),
            ("externalReference".to_owned(), display_id.clone()),
        ]);
        let api_response = context.request(&api_request)?;
        let meeting = serde_json::from_slice::<serde_json::Value>(api_response.body()).map_err(
            |error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid European Parliament meeting JSON for {display_id}: {error}"),
                )
            },
        )?;
        let meeting_id = json_value_string(meeting.get("id")).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("European Parliament meeting {display_id} has no ID"),
            )
        })?;
        let mut hls_urls = Vec::new();
        europarl_add_hls_url(&mut hls_urls, meeting.get("meetingVideo"));
        if let Some(meeting_videos) = meeting
            .get("meetingVideos")
            .and_then(serde_json::Value::as_array)
        {
            for meeting_video in meeting_videos {
                europarl_add_hls_url(&mut hls_urls, Some(meeting_video));
            }
        }
        if hls_urls.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("European Parliament meeting {display_id} has no HLS stream"),
            ));
        }
        let formats = hls_urls
            .iter()
            .enumerate()
            .map(|(index, hls_url)| {
                let format_id = if index == 0 {
                    "hls".to_owned()
                } else {
                    format!("hls-{index}")
                };
                serde_json::json!({
                    "url": hls_url,
                    "format_id": format_id,
                    "protocol": "m3u8_native",
                    "ext": "mp4",
                })
            })
            .collect::<Vec<_>>();
        let title = page_props
            .get("mediaItem")
            .and_then(|media_item| json_string(media_item, "title"))
            .or_else(|| json_string(page_props, "title"))
            .map(str::to_owned);
        let start_date_time = json_string(&meeting, "startDateTime").map(str::to_owned);
        let is_live = page_props
            .get("mediaItem")
            .and_then(|media_item| json_string(media_item, "mediaSubType"))
            == Some("Live");

        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(meeting_id));
        info.insert("display_id", serde_json::json!(display_id));
        info.insert_if_some("title", title);
        info.insert_if_some(
            "release_timestamp",
            start_date_time.clone().and_then(parse_timestamp),
        );
        info.insert_if_some(
            "release_date",
            start_date_time.as_deref().and_then(date_digits),
        );
        info.insert("is_live", serde_json::json!(is_live));
        info.insert("url", serde_json::json!(hls_urls[0]));
        info.insert("ext", serde_json::json!("mp4"));
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

fn europarl_add_hls_url(urls: &mut Vec<String>, value: Option<&serde_json::Value>) {
    let Some(hls_url) = value
        .and_then(|value| json_string(value, "hlsUrl"))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    else {
        return;
    };
    if !urls.iter().any(|url| url == hls_url) {
        urls.push(hls_url.to_owned());
    }
}
