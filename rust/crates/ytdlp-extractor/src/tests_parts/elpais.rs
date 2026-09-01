#[test]
fn elpais_native_extractor_maps_page_media_and_metadata() {
    let extractor = ElPaisExtractor::new(ExtractorDescriptor::new(
        "ElPaisIE",
        "ElPais",
        r"https?://(?:[^.]+\.)?elpais\.com/.*/(?P<id>[^/#?]+)\.html(?:$|[?#])",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<html>
            <script>
                var url_cache = "https://cdn.example/";
                var URLMediaFile = url_cache + 'clip.mp4';
                var URLMediaStill = url_cache + 'poster.jpg';
                var tituloVideo = 'Fallback title';
            </script>
            <meta property="og:description" content="A &amp; useful description">
            <p class="date-header date-int updated" title="2017-02-14T00:00:00Z"></p>
        </html>"#
        .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://elpais.com/elpais/2017/02/14/ciencia/1487062137_417876.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("1487062137_417876"));
    assert_eq!(result.get_str("url"), Some("https://cdn.example/clip.mp4"));
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(result.get_str("title"), Some("Fallback title"));
    assert_eq!(
        result.get_str("description"),
        Some("A & useful description")
    );
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/poster.jpg")
    );
    assert_eq!(result.get_str("upload_date"), Some("20170214"));
}

#[test]
fn elpais_native_extractor_parses_media_jsonp_branch() {
    let extractor = ElPaisExtractor::new(ExtractorDescriptor::new(
        "ElPaisIE",
        "ElPais",
        r"https?://(?:[^.]+\.)?elpais\.com/.*/(?P<id>[^/#?]+)\.html(?:$|[?#])",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(RoutedHandler {
        routes: vec![
            (
                "vdpep".to_owned(),
                br#"window.media && window.media({"mp4":"json-clip.mp4"});"#.to_vec(),
            ),
            (
                "elpais.com/elpais/".to_owned(),
                br#"<script>var url_cache = "https://cdn.example/"; id_multimedia = 'abc123'; URLMediaStill = url_cache + 'json-poster.jpg';</script>
                    <h2 class="entry-header entry-title">JSONP title</h2>
                    <meta name="datePublished" content="2016-03-03T12:00:00Z">"#
                    .to_vec(),
            ),
        ],
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://elpais.com/elpais/2016/03/03/articulo/1456340311_668921.html",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/json-clip.mp4")
    );
    assert_eq!(result.get_str("title"), Some("JSONP title"));
    assert_eq!(
        result.get_str("thumbnail"),
        Some("https://cdn.example/json-poster.jpg")
    );
    assert_eq!(result.get_str("upload_date"), Some("20160303"));
}
