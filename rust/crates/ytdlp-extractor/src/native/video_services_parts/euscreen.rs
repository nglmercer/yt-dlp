/// Native EUScreen two-step player/metadata extractor.
pub struct EuscreenExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl EuscreenExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for EuscreenExtractor {
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
        let video_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(ExtractorErrorKind::InvalidUrl, "EUScreen URL has no item ID")
            })?;
        let endpoint =
            "https://euscreen.eu/lou/LouServlet/domain/euscreenxl/html5application/euscreenxlitem";
        let mut args_request = Request::new(endpoint);
        args_request.update_query(&[
            ("actionlist".to_owned(), "itempage".to_owned()),
            ("id".to_owned(), video_id.clone()),
        ]);
        args_request.set_data(Some(EUSCREEN_CAPABILITY_PAYLOAD.as_bytes().to_vec()));
        let args_response = context.request(&args_request)?;
        let args = String::from_utf8_lossy(args_response.body()).replace("screenid", "screenId");
        let mut player_request = Request::new(endpoint);
        player_request.set_data(Some(args.into_bytes()));
        let player_response = context.request(&player_request)?;
        let player = String::from_utf8_lossy(player_response.body());
        let video_data = json_object_after_marker(&player, "setVideo(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EUScreen item {video_id} has no video data"),
            )
        })?;
        let metadata = json_object_after_marker(&player, "setData(").ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EUScreen item {video_id} has no metadata"),
            )
        })?;
        let sources = video_data
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("EUScreen item {video_id} has no media sources"),
                )
            })?;
        let mut formats = Vec::new();
        for (index, source) in sources.iter().enumerate() {
            let Some(media_url) = json_string(source, "src")
                .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
            else {
                continue;
            };
            let extension = yt_dlp_core::determine_ext(Some(media_url), "mp4");
            let is_hls = extension.eq_ignore_ascii_case("m3u8");
            let format_id = if is_hls {
                "hls".to_owned()
            } else {
                format!("http-{index}")
            };
            formats.push(serde_json::json!({
                "url": media_url,
                "format_id": format_id,
                "protocol": if is_hls { "m3u8_native" } else { "http" },
                "ext": if is_hls { "mp4" } else { extension.as_str() },
            }));
        }
        if formats.is_empty() {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("EUScreen item {video_id} has no playable sources"),
            ));
        }
        let description = [
            json_string(&metadata, "summaryOriginal"),
            json_string(&metadata, "summaryEnglish"),
        ]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
        let first_format = formats.first().cloned().unwrap_or(serde_json::Value::Null);
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(video_id));
        info.insert_if_some("title", json_string(&metadata, "originalTitle"));
        info.insert_if_some("alt_title", json_string(&metadata, "title"));
        info.insert_if_some(
            "duration",
            json_string(&metadata, "duration").and_then(yt_dlp_core::parse_duration),
        );
        info.insert_if_some("description", (!description.is_empty()).then_some(description));
        info.insert_if_some(
            "series",
            json_string(&metadata, "series").or_else(|| json_string(&metadata, "seriesEnglish")),
        );
        info.insert_if_some("episode", json_string(&metadata, "episodeNumber"));
        info.insert_if_some("uploader", json_string(&metadata, "provider"));
        info.insert_if_some(
            "thumbnail",
            json_string(&metadata, "screenshot")
                .or_else(|| json_string(&video_data, "screenshot")),
        );
        info.insert(
            "url",
            first_format
                .get("url")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        info.insert(
            "ext",
            first_format
                .get("ext")
                .cloned()
                .unwrap_or_else(|| serde_json::json!("mp4")),
        );
        info.insert("formats", serde_json::Value::Array(formats));
        info.insert("subtitles", serde_json::json!({}));
        Ok(ExtractorResult::single(info))
    }
}

const EUSCREEN_CAPABILITY_PAYLOAD: &str = r#"<fsxml><screen><properties><screenId>-1</screenId></properties><capabilities id="1"><properties><platform>Win32</platform><appcodename>Mozilla</appcodename><appname>Netscape</appname><appversion>5.0</appversion><useragent>Mozilla/5.0</useragent><cookiesenabled>true</cookiesenabled><screenwidth>784</screenwidth><screenheight>758</screenheight><orientation>undefined</orientation><smt_browserid>Sat, 07 Oct 2021 08:56:50 GMT</smt_browserid><smt_sessionid>1633769810758</smt_sessionid></properties></capabilities></screen></fsxml>"#;
