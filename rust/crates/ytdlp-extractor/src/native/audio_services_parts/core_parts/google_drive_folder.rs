/// Native Google Drive folder listing extractor.
pub struct GoogleDriveFolderExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl GoogleDriveFolderExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

impl InfoExtractor for GoogleDriveFolderExtractor {
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
        const MAX_PAGES: usize = 10_000;
        const BOUNDARY: &str = "=====vc17a3rwnndj=====";

        let folder_id = self
            .matcher
            .captures(url)
            .ok()
            .flatten()
            .and_then(|captures| captures.name("id"))
            .map(|value| value.as_str().to_owned())
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::InvalidUrl,
                    "Google Drive folder URL has no folder ID",
                )
            })?;
        let page_response = context.get(url)?;
        let page = String::from_utf8_lossy(page_response.body());
        let api_key = Regex::new(r#""(\w{39})""#)
            .ok()
            .and_then(|matcher| matcher.captures(&page).ok().flatten())
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
            .ok_or_else(|| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("Google Drive folder {folder_id} has no API key"),
                )
            })?;
        let folder_info = google_drive_folder_batch_json(
            context,
            &format!("/drive/v2beta/files/{folder_id}"),
            &api_key,
            BOUNDARY,
        )
        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
        let mut entries = Vec::new();
        let mut page_token = String::new();
        for _ in 0..MAX_PAGES {
            let request_path = google_drive_folder_list_path(&folder_id, &page_token);
            let page = google_drive_folder_batch_json(context, &request_path, &api_key, BOUNDARY)?;
            let items = page
                .get("items")
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for item in items {
                let Some(item_id) = json_string(item, "id") else {
                    continue;
                };
                let mut entry = native_url_result(&format!(
                    "https://drive.google.com/file/d/{item_id}"
                ));
                entry.insert("ie_key", serde_json::json!("GoogleDrive"));
                entries.push(entry);
            }
            let Some(next_page_token) = page
                .get("nextPageToken")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                break;
            };
            page_token = next_page_token.to_owned();
        }
        let mut info = InfoDict::new();
        info.insert("id", serde_json::json!(folder_id));
        info.insert_if_some("title", json_string(&folder_info, "title"));
        Ok(ExtractorResult::Playlist { info, entries })
    }
}

fn google_drive_folder_batch_json(
    context: &ExtractionContext,
    request_path: &str,
    api_key: &str,
    boundary: &str,
) -> Result<serde_json::Value, ExtractorError> {
    let body = format!(
        "--{boundary}\r\ncontent-type: application/http\r\ncontent-transfer-encoding: binary\r\n\r\nGET {request_path} HTTP/1.1\r\n\r\n--{boundary}\r\n"
    );
    let mut request = Request::new("https://clients6.google.com/batch/drive/v2beta");
    request.set_method("POST").map_err(map_request_error)?;
    request.update_query(&[
        (
            "$ct".to_owned(),
            format!("multipart/mixed; boundary=\"{boundary}\""),
        ),
        ("key".to_owned(), api_key.to_owned()),
    ]);
    request
        .headers_mut()
        .set("Content-Type", "text/plain;charset=UTF-8;");
    request
        .headers_mut()
        .set("Origin", "https://drive.google.com");
    request.set_data(Some(body.into_bytes()));
    let response = context.request(&request)?;
    let body = String::from_utf8_lossy(response.body());
    json_object_after_marker(&body, "").ok_or_else(|| {
        ExtractorError::new(
            ExtractorErrorKind::Extraction,
            format!("Google Drive batch API returned no JSON object for {request_path}"),
        )
    })
}

fn google_drive_folder_list_path(folder_id: &str, page_token: &str) -> String {
    let mut path = url::Url::parse("https://clients6.google.com/drive/v2beta/files")
        .expect("static Google Drive files URL");
    path.query_pairs_mut()
        .append_pair("openDrive", "true")
        .append_pair("reason", "102")
        .append_pair("syncType", "0")
        .append_pair("errorRecovery", "false")
        .append_pair("q", &format!("trashed = false and '{folder_id}' in parents"))
        .append_pair("spaces", "drive")
        .append_pair("pageToken", page_token)
        .append_pair("maxResults", "50")
        .append_pair("supportsTeamDrives", "true")
        .append_pair("includeItemsFromAllDrives", "true")
        .append_pair("corpora", "default")
        .append_pair("orderBy", "folder,title_natural asc")
        .append_pair("retryCount", "0");
    let query = path.query().unwrap_or_default();
    format!("{}?{query}", path.path())
}
