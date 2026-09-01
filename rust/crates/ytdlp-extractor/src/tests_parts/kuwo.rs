struct KuwoHandler;

impl RequestHandler for KuwoHandler {
    fn name(&self) -> &str {
        "kuwo-test"
    }

    fn supports(&self, _request: &Request) -> Result<(), RequestError> {
        Ok(())
    }

    fn send(&self, request: &Request) -> Result<Response, RequestError> {
        let url = request.url();
        if url.contains("/yinyue/12345") {
            let body = r#"
                <p id="lrcName">Native Kuwo song</p>
                <a href="http://www.kuwo.cn/artist/content?name=歌手Native%20Singer">artist</a>
                <div id="lrcContent"><p>Native lyric line</p></div>
                <a href="http://www.kuwo.cn/album/42/">album</a>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("/album/42/") {
            let body = r#"
                <div class="comm">
                    <h1 title="Native Kuwo album">Native Kuwo album</h1>
                </div>
                <div id="intro">Native Kuwo album简介：Native album description</div>
                <span>发行时间：2008-01-22</span>
                <p class="listen"><a href="http://www.kuwo.cn/yinyue/111/">one</a></p>
                <p class="listen"><a href="http://www.kuwo.cn/yinyue/222/">two</a></p>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("billboard_native.htm") {
            let body = r#"
                <a href="http://www.kuwo.cn/yinyue/301">one</a>
                <a href="http://www.kuwo.cn/yinyue/302">two</a>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("/mingxing/native-singer") {
            let body = r#"
                <h1>Native Singer</h1>
                <div data-artistid="77" data-page="2"></div>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("/artist/contentMusicsAjax") {
            let body = if url.contains("pn=1") {
                r#"<div class="name"><a href="/yinyue/402">second</a></div>"#
            } else {
                r#"<div class="name"><a href="/yinyue/401">first</a></div>"#
            };
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("cinfo_99.htm") {
            let body = r#"
                <h1 title="Native Kuwo category">Native Kuwo category</h1>
                <div id="intro">Native Kuwo category简介：Native category description</div>
                <script>var jsonm = {"musiclist":[{"musicrid":"501"},{"musicrid":502}]};</script>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("/mv/77/") {
            let body = r#"
                <h1 title="Native Kuwo MV">Native Kuwo MV<span title="Native MV singer"></span></h1>
            "#;
            return Ok(Response::new(url, 200, "OK", body.as_bytes().to_vec()));
        }
        if url.contains("/yy/st/mvurl?rid=MUSIC_77") {
            return Ok(Response::new(
                url,
                200,
                "OK",
                b"https://cdn.example/kuwo/native-mv.mp4".to_vec(),
            ));
        }
        if url.starts_with("http://antiserver.kuwo.cn/anti.s") {
            let media_url = if url.contains("format=ape") {
                "https://cdn.example/kuwo/native.ape"
            } else if url.contains("format=mp3") && url.contains("320kmp3") {
                "https://cdn.example/kuwo/native-320.mp3"
            } else if url.contains("format=mp3") && url.contains("192kmp3") {
                "https://cdn.example/kuwo/native-192.mp3"
            } else if url.contains("format=mp3") {
                "https://cdn.example/kuwo/native-128.mp3"
            } else if url.contains("format=wma") {
                "https://cdn.example/kuwo/native.wma"
            } else if url.contains("format=aac") {
                "https://cdn.example/kuwo/native.aac"
            } else if url.contains("format=mkv") {
                "https://cdn.example/kuwo/native.mkv"
            } else if url.contains("format=mp4") {
                "https://cdn.example/kuwo/native.mp4"
            } else {
                return Err(RequestError::new(
                    ErrorKind::Transport,
                    format!("unknown Kuwo format request {url}"),
                ));
            };
            return Ok(Response::new(
                url,
                200,
                "OK",
                media_url.as_bytes().to_vec(),
            ));
        }
        Err(RequestError::new(
            ErrorKind::Transport,
            format!("no Kuwo route for {url}"),
        ))
    }
}

fn kuwo_context() -> ExtractionContext {
    let mut director = RequestDirector::new();
    director.add_handler(KuwoHandler);
    ExtractionContext::new(director, CookieJar::new().shared())
}

#[test]
fn kuwo_native_song_maps_page_lyrics_formats_and_album_date() {
    let extractor = KuwoExtractor::new(ExtractorDescriptor::new(
        "KuwoIE",
        "kuwo:song",
        r#"https?://(?:www\.)?kuwo\.cn/yinyue/(?P<id>\d+)"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("http://www.kuwo.cn/yinyue/12345/", &kuwo_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("12345"));
    assert_eq!(result.get_str("title"), Some("Native Kuwo song"));
    assert_eq!(result.get_str("creator"), Some("Native Singer"));
    assert_eq!(result.get_str("description"), Some("Native lyric line"));
    assert_eq!(result.get_str("upload_date"), Some("20080122"));
    let formats = result
        .get("formats")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(formats.len(), 6);
    assert_eq!(formats[0].get("format_id"), Some(&serde_json::json!("ape")));
    assert_eq!(formats[1].get("abr"), Some(&serde_json::json!(320)));
}

#[test]
fn kuwo_native_album_and_chart_build_song_entries() {
    let album = KuwoAlbumExtractor::new(ExtractorDescriptor::new(
        "KuwoAlbumIE",
        "kuwo:album",
        r#"https?://(?:www\.)?kuwo\.cn/album/(?P<id>\d+?)/"#,
        false,
    ))
    .unwrap();
    let album_result = album
        .extract_with_context("http://www.kuwo.cn/album/42/", &kuwo_context())
        .unwrap()
        .into_info_dict();
    assert_eq!(album_result.get_str("title"), Some("Native Kuwo album"));
    assert_eq!(
        album_result.get_str("description"),
        Some("Native album description")
    );
    assert_eq!(
        album_result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let chart = KuwoChartExtractor::new(ExtractorDescriptor::new(
        "KuwoChartIE",
        "kuwo:chart",
        r#"https?://yinyue\.kuwo\.cn/billboard_(?P<id>[^.]+).htm"#,
        false,
    ))
    .unwrap();
    let chart_result = chart
        .extract_with_context(
            "http://yinyue.kuwo.cn/billboard_native.htm",
            &kuwo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(chart_result.get_str("id"), Some("native"));
    assert_eq!(
        chart_result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn kuwo_native_singer_and_category_build_playlists() {
    let singer = KuwoSingerExtractor::new(ExtractorDescriptor::new(
        "KuwoSingerIE",
        "kuwo:singer",
        r#"https?://(?:www\.)?kuwo\.cn/mingxing/(?P<id>[^/]+)"#,
        false,
    ))
    .unwrap();
    let singer_result = singer
        .extract_with_context(
            "http://www.kuwo.cn/mingxing/native-singer/",
            &kuwo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(singer_result.get_str("title"), Some("Native Singer"));
    assert_eq!(
        singer_result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let category = KuwoCategoryExtractor::new(ExtractorDescriptor::new(
        "KuwoCategoryIE",
        "kuwo:category",
        r#"https?://yinyue\.kuwo\.cn/yy/cinfo_(?P<id>\d+?).htm"#,
        false,
    ))
    .unwrap();
    let category_result = category
        .extract_with_context(
            "http://yinyue.kuwo.cn/yy/cinfo_99.htm",
            &kuwo_context(),
        )
        .unwrap()
        .into_info_dict();
    assert_eq!(
        category_result.get_str("title"),
        Some("Native Kuwo category")
    );
    assert_eq!(
        category_result.get_str("description"),
        Some("Native category description")
    );
    assert_eq!(
        category_result
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn kuwo_native_mv_maps_audio_and_video_formats() {
    let extractor = KuwoMvExtractor::new(ExtractorDescriptor::new(
        "KuwoMvIE",
        "kuwo:mv",
        r#"https?://(?:www\.)?kuwo\.cn/mv/(?P<id>\d+?)/"#,
        false,
    ))
    .unwrap();
    let result = extractor
        .extract_with_context("http://www.kuwo.cn/mv/77/", &kuwo_context())
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("title"), Some("Native Kuwo MV"));
    assert_eq!(result.get_str("creator"), Some("Native MV singer"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .and_then(|formats| formats.last())
            .and_then(|format| format.get("format_id")),
        Some(&serde_json::json!("mv"))
    );
}
