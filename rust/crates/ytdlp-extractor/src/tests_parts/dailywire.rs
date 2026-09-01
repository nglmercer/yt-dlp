#[test]
fn dailywire_native_extractor_maps_next_episode_and_formats() {
    let extractor = DailyWireExtractor::new(ExtractorDescriptor::new(
        "DailyWireIE",
        "DailyWire",
        r"https?://(?:www\.)dailywire(?:\.com)/(?P<sites_type>episode|videos)/(?P<id>[\w-]+)",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"episodeData":{"episode":{
            "id":"native-episode","title":"Native Daily Wire","description":"Native description",
            "duration":42.5,"isLive":false,"thumbnail":"https://cdn.example/dw.jpg",
            "createdBy":{"firstName":"Native","lastName":"Creator"},
            "show":{"id":"native-show","name":"Native series"},
            "segments":[{"videoUrl":"https://cdn.example/native.m3u8"},{"audio":"https://cdn.example/native.mp3"}]
        }}}}}</script>"#.to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context("https://www.dailywire.com/episode/native-episode", &context)
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-episode"));
    assert_eq!(result.get_str("display_id"), Some("native-episode"));
    assert_eq!(result.get_str("title"), Some("Native Daily Wire"));
    assert_eq!(result.get_str("creator"), Some("Native Creator"));
    assert_eq!(result.get_str("series"), Some("Native series"));
    assert_eq!(result.get_f64("duration"), Some(42.5));
    assert_eq!(
        result.get_str("url"),
        Some("https://cdn.example/native.m3u8")
    );
    assert_eq!(result.get_str("ext"), Some("mp4"));
    assert_eq!(
        result
            .get("formats")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn dailywire_podcast_native_extractor_maps_audio_playback_id() {
    let extractor = DailyWireExtractor::new(ExtractorDescriptor::new(
        "DailyWirePodcastIE",
        "DailyWirePodcast",
        r"https?://(?:www\.)dailywire(?:\.com)/(?P<sites_type>podcasts)/(?P<podcaster>[\w-]+/(?P<id>[\w-]+))",
        true,
    ))
    .unwrap();
    let mut director = RequestDirector::new();
    director.add_handler(FakeHandler {
        body: br#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"episode":{
            "id":"native-podcast","title":"Native podcast","duration":900.117667,
            "thumbnail":"https://cdn.example/podcast.jpg","description":"Podcast description",
            "audioMuxPlaybackId":"native-audio"
        }}}}</script>"#
            .to_vec(),
    });
    let context = ExtractionContext::new(director, CookieJar::new().shared());
    let result = extractor
        .extract_with_context(
            "https://www.dailywire.com/podcasts/native-show/native-podcast",
            &context,
        )
        .unwrap()
        .into_info_dict();

    assert_eq!(result.get_str("id"), Some("native-podcast"));
    assert_eq!(result.get_str("display_id"), Some("native-podcast"));
    assert_eq!(result.get_str("title"), Some("Native podcast"));
    assert_eq!(result.get_f64("duration"), Some(900.117667));
    assert_eq!(
        result.get_str("url"),
        Some("https://stream.media.dailywire.com/native-audio/audio.m4a")
    );
    assert_eq!(result.get_str("ext"), Some("m4a"));
}
