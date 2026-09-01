struct GoogleDriveFolderSequenceHandler {
    responses: std::sync::Mutex<Vec<Vec<u8>>>,
}

impl yt_dlp_networking::RequestHandler for GoogleDriveFolderSequenceHandler {
    fn name(&self) -> &str {
        "google-drive-folder-test"
    }

    fn supports(
        &self,
        _request: &yt_dlp_networking::Request,
    ) -> Result<(), yt_dlp_networking::RequestError> {
        Ok(())
    }

    fn send(
        &self,
        request: &yt_dlp_networking::Request,
    ) -> Result<yt_dlp_networking::Response, yt_dlp_networking::RequestError> {
        let body = self
            .responses
            .lock()
            .map_err(|_| {
                yt_dlp_networking::RequestError::new(
                    yt_dlp_networking::ErrorKind::Transport,
                    "test response lock poisoned",
                )
            })?
            .first()
            .cloned()
            .ok_or_else(|| {
                yt_dlp_networking::RequestError::new(
                    yt_dlp_networking::ErrorKind::Transport,
                    format!("no test response for {}", request.url()),
                )
            })?;
        self.responses
            .lock()
            .map_err(|_| {
                yt_dlp_networking::RequestError::new(
                    yt_dlp_networking::ErrorKind::Transport,
                    "test response lock poisoned",
                )
            })?
            .remove(0);
        Ok(yt_dlp_networking::Response::new(
            request.url(),
            200,
            "OK",
            body,
        ))
    }
}

#[test]
fn google_drive_folder_native_extractor_lists_paginated_files() {
    let extractor = GoogleDriveFolderExtractor::new(ExtractorDescriptor::new(
        "GoogleDriveFolderIE",
        "GoogleDrive:Folder",
        r#"https?://(?:docs|drive)\.google\.com/drive/folders/(?P<id>[\w-]{28,})"#,
        true,
    ))
    .unwrap();
    let api_key = "abcdefghijklmnopqrstuvwxyz1234567890123";
    let mut director = RequestDirector::new();
    director.add_handler(GoogleDriveFolderSequenceHandler {
        responses: std::sync::Mutex::new(vec![
            format!("<html>\"{api_key}\"</html>").into_bytes(),
            br#"{"title":"Native folder"}"#.to_vec(),
            br#"{"items":[{"id":"native-file-1"}],"nextPageToken":"page-2"}"#.to_vec(),
            br#"{"items":[{"id":"native-file-2"}]}"#.to_vec(),
        ]),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let ExtractorResult::Playlist { info, entries } = extractor
        .extract_with_context(
            "https://drive.google.com/drive/folders/1dQ4sx0-__Nvg65rxTSgQrl7VyW_FZ9QI",
            &context,
        )
        .unwrap()
    else {
        panic!("expected Google Drive folder playlist");
    };

    assert_eq!(info.get_str("id"), Some("1dQ4sx0-__Nvg65rxTSgQrl7VyW_FZ9QI"));
    assert_eq!(info.get_str("title"), Some("Native folder"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get_str("_type"), Some("url"));
    assert_eq!(entries[0].get_str("ie_key"), Some("GoogleDrive"));
    assert_eq!(
        entries[0].get_str("url"),
        Some("https://drive.google.com/file/d/native-file-1")
    );
    assert_eq!(
        entries[1].get_str("url"),
        Some("https://drive.google.com/file/d/native-file-2")
    );
}
