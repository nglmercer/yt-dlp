#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn direct_downloader_writes_response_atomically() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = std::io::Read::read(&mut stream, &mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ncontent",
                )
                .unwrap();
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-download-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download(
                &Request::new(format!("http://{address}/media.bin")),
                Some(&output),
                &DownloadOptions::default(),
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.bytes, 7);
        assert_eq!(result.path.as_deref(), Some(output.as_path()));
        assert_eq!(fs::read(&output).unwrap(), b"content");
        assert!(!output.with_extension("bin.part").exists());
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn direct_downloader_resumes_existing_file_with_range_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let count = std::io::Read::read(&mut stream, &mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.contains("Range: bytes=4-\r\n"));
            stream
                .write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Range: bytes 4-7/8\r\nConnection: close\r\n\r\nrest",
                )
                .unwrap();
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-resume-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        fs::write(&output, b"part").unwrap();
        let result = DirectDownloader::native()
            .download(
                &Request::new(format!("http://{address}/media.bin")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: false,
                    resume: true,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert!(result.resumed);
        assert_eq!(result.bytes, 8);
        assert_eq!(fs::read(&output).unwrap(), b"partrest");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn simulated_download_does_not_create_output() {
        let output =
            std::env::temp_dir().join(format!("yt-dlp-rs-simulated-{}.bin", std::process::id()));
        let error = write_atomic(&output, b"body", false).unwrap();
        assert_eq!(error, output);
        fs::remove_file(&output).unwrap();

        let options = DownloadOptions {
            simulate: true,
            overwrite: false,
            resume: true,
            retries: 0,
            concurrent: 1,
        };
        assert!(options.simulate);
    }

    #[test]
    fn parses_hls_media_and_master_playlists() {
        let media = parse_hls_playlist(
            "http://example.test/video/playlist.m3u8",
            b"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:1,\npart.ts\n",
        )
        .unwrap();
        assert_eq!(media.variant, None);
        assert_eq!(media.segments.len(), 2);
        assert_eq!(media.segments[0], "http://example.test/video/init.mp4");
        assert_eq!(media.segments[1], "http://example.test/video/part.ts");
        assert_eq!(media.segment_ranges, [None, None]);

        let byterange = parse_hls_playlist(
            "http://example.test/video/byterange.m3u8",
            b"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"4@0\"\n#EXT-X-BYTERANGE:3@4\n#EXTINF:1,\nmedia.mp4\n#EXT-X-BYTERANGE:2\n#EXTINF:1,\nmedia.mp4\n",
        )
        .unwrap();
        assert_eq!(
            byterange.segment_ranges,
            [
                Some(ByteRange {
                    start: 0,
                    length: 4
                }),
                Some(ByteRange {
                    start: 4,
                    length: 3
                }),
                Some(ByteRange {
                    start: 7,
                    length: 2
                }),
            ]
        );

        let master = parse_hls_playlist(
            "http://example.test/master.m3u8",
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nvideo/low.m3u8\n",
        )
        .unwrap();
        assert_eq!(
            master.variant.as_deref(),
            Some("http://example.test/video/low.m3u8")
        );
        assert!(master.segments.is_empty());

        assert!(matches!(
            parse_hls_playlist(
                "http://example.test/encrypted.m3u8",
                b"#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\npart.ts\n",
            ),
            Err(DownloadError::Unsupported(_))
        ));
    }

    #[test]
    fn fragment_requests_append_native_extra_query_parameters() {
        let mut request = Request::new("http://example.test/video/segment.ts?existing=1");
        request.extensions_mut().insert(
            "extra_param_to_segment_url".to_owned(),
            serde_json::json!("pbs=session-1"),
        );
        native_apply_extra_param_to_segment_url(&mut request).unwrap();
        assert_eq!(
            request.url(),
            "http://example.test/video/segment.ts?existing=1&pbs=session-1"
        );
    }

    #[test]
    fn parses_dash_segment_lists_with_base_url_scope() {
        let manifest = parse_dash_mpd(
            "http://example.test/manifests/main.mpd",
            br#"<MPD><Period><AdaptationSet><Representation>
                <BaseURL>video/</BaseURL>
                <SegmentList>
                    <Initialization sourceURL="init.mp4" />
                    <SegmentURL media="one.m4s" />
                    <SegmentURL media="two.m4s" />
                </SegmentList>
            </Representation></AdaptationSet></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(
            manifest.segments,
            [
                "http://example.test/manifests/video/init.mp4",
                "http://example.test/manifests/video/one.m4s",
                "http://example.test/manifests/video/two.m4s",
            ]
        );
        assert_eq!(manifest.segment_ranges, [None, None, None]);

        let ranges = parse_dash_mpd(
            "http://example.test/manifests/ranges.mpd",
            br#"<MPD><Period><Representation><BaseURL>video/</BaseURL>
                <SegmentList>
                    <Initialization sourceURL="media.mp4" range="0-3" />
                    <SegmentURL media="media.mp4" mediaRange="4-6" />
                    <SegmentURL media="media.mp4" mediaRange="7-8" />
                </SegmentList>
            </Representation></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(
            ranges.segment_ranges,
            [
                Some(ByteRange {
                    start: 0,
                    length: 4
                }),
                Some(ByteRange {
                    start: 4,
                    length: 3
                }),
                Some(ByteRange {
                    start: 7,
                    length: 2
                }),
            ]
        );

        let timeline = parse_dash_mpd(
            "http://example.test/main.mpd",
            br#"<MPD><Period><Representation id="v1">
                <BaseURL>video/</BaseURL>
                <SegmentTemplate timescale="1" media="seg-$Number%02d$.m4s" initialization="init.mp4">
                    <SegmentTimeline><S t="0" d="2" r="1" /></SegmentTimeline>
                </SegmentTemplate>
            </Representation></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(
            timeline.segments,
            [
                "http://example.test/video/init.mp4",
                "http://example.test/video/seg-01.m4s",
                "http://example.test/video/seg-02.m4s",
            ]
        );

        let duration = parse_dash_mpd(
            "http://example.test/main.mpd",
            br#"<MPD mediaPresentationDuration="PT5S"><Period><Representation>
                <SegmentTemplate duration="2" media="seg-$Number$.m4s" />
            </Representation></Period></MPD>"#,
        )
        .unwrap();
        assert_eq!(duration.segments.len(), 3);
    }

    #[test]
    fn hls_downloader_concatenates_initialization_and_media_segments() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let body = match path {
                    "/playlist.m3u8" => {
                        b"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:1,\npart1.m4s\n#EXTINF:1,\npart2.m4s\n".to_vec()
                    }
                    "/init.mp4" => b"INIT".to_vec(),
                    "/part1.m4s" => b"ONE".to_vec(),
                    "/part2.m4s" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-hls-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_hls(
                &Request::new(format!("http://{address}/playlist.m3u8")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.fragments, Some(3));
        assert_eq!(result.bytes, 10);
        assert_eq!(fs::read(&output).unwrap(), b"INITONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn hls_downloader_sends_byte_ranges_for_reused_media_urls() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]).into_owned();
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                requests.push(request);
                let (status, body) = match path.as_str() {
                    "/playlist.m3u8" => (
                        "200 OK",
                        b"#EXTM3U\n#EXT-X-BYTERANGE:3@4\n#EXTINF:1,\nmedia.mp4\n#EXT-X-BYTERANGE:2\n#EXTINF:1,\nmedia.mp4\n".to_vec(),
                    ),
                    "/media.mp4" => {
                        let range = requests.last().and_then(|request| {
                            request
                                .lines()
                                .find(|line| line.starts_with("Range:"))
                                .map(str::to_owned)
                        });
                        match range.as_deref() {
                            Some("Range: bytes=4-6") => ("206 Partial Content", b"abc".to_vec()),
                            Some("Range: bytes=7-8") => ("206 Partial Content", b"de".to_vec()),
                            _ => ("416 Range Not Satisfiable", Vec::new()),
                        }
                    }
                    _ => ("404 Not Found", Vec::new()),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-hls-range-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_hls(
                &Request::new(format!("http://{address}/playlist.m3u8")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        let requests = server.join().unwrap();
        assert!(requests[1].contains("Range: bytes=4-6\r\n"));
        assert!(requests[2].contains("Range: bytes=7-8\r\n"));
        assert_eq!(result.bytes, 5);
        assert_eq!(fs::read(&output).unwrap(), b"abcde");
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn dash_downloader_concatenates_segment_list() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let body = match path {
                    "/main.mpd" => br#"<MPD><Period><Representation>
                        <BaseURL>video/</BaseURL><SegmentList>
                        <Initialization sourceURL="init.mp4" />
                        <SegmentURL media="one.m4s" /><SegmentURL media="two.m4s" />
                        </SegmentList></Representation></Period></MPD>"#
                        .to_vec(),
                    "/video/init.mp4" => b"INIT".to_vec(),
                    "/video/one.m4s" => b"ONE".to_vec(),
                    "/video/two.m4s" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-dash-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_dash(
                &Request::new(format!("http://{address}/main.mpd")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(result.fragments, Some(3));
        assert_eq!(result.bytes, 10);
        assert_eq!(fs::read(&output).unwrap(), b"INITONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn dash_downloader_sends_byte_ranges_for_segment_list() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]).into_owned();
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                let range = request
                    .lines()
                    .find(|line| line.starts_with("Range:"))
                    .map(str::to_owned);
                requests.push(request);
                let (status, body) = match (path.as_str(), range.as_deref()) {
                    ("/main.mpd", None) => (
                        "200 OK",
                        br#"<MPD><Period><Representation><BaseURL>video/</BaseURL>
                            <SegmentList>
                                <Initialization sourceURL="media.mp4" range="0-3" />
                                <SegmentURL media="media.mp4" mediaRange="4-6" />
                                <SegmentURL media="media.mp4" mediaRange="7-8" />
                            </SegmentList>
                        </Representation></Period></MPD>"#
                            .to_vec(),
                    ),
                    ("/video/media.mp4", Some("Range: bytes=0-3")) => {
                        ("206 Partial Content", b"INIT".to_vec())
                    }
                    ("/video/media.mp4", Some("Range: bytes=4-6")) => {
                        ("206 Partial Content", b"abc".to_vec())
                    }
                    ("/video/media.mp4", Some("Range: bytes=7-8")) => {
                        ("206 Partial Content", b"de".to_vec())
                    }
                    _ => ("416 Range Not Satisfiable", Vec::new()),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });

        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-dash-range-{}-{}.mp4",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_dash(
                &Request::new(format!("http://{address}/main.mpd")),
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 1,
                },
            )
            .unwrap();

        let requests = server.join().unwrap();
        assert!(requests[1].contains("Range: bytes=0-3\r\n"));
        assert!(requests[2].contains("Range: bytes=4-6\r\n"));
        assert!(requests[3].contains("Range: bytes=7-8\r\n"));
        assert_eq!(result.fragments, Some(3));
        assert_eq!(result.bytes, 9);
        assert_eq!(fs::read(&output).unwrap(), b"INITabcde");
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn fragment_downloader_limits_workers_and_restores_playlist_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let count = std::io::Read::read(&mut stream, &mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let body = match request.split_whitespace().nth(1).unwrap_or_default() {
                    "/zero" => b"ZERO".to_vec(),
                    "/one" => b"ONE".to_vec(),
                    "/two" => b"TWO".to_vec(),
                    _ => Vec::new(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let fragments = ["zero", "one", "two"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| Fragment {
                index,
                request: Request::new(format!("http://{address}/{name}")),
            })
            .collect::<Vec<_>>();
        let output = std::env::temp_dir().join(format!(
            "yt-dlp-rs-fragments-{}-{}.bin",
            std::process::id(),
            address.port()
        ));
        let result = DirectDownloader::native()
            .download_fragments(
                &fragments,
                Some(&output),
                &DownloadOptions {
                    simulate: false,
                    overwrite: true,
                    resume: false,
                    retries: 0,
                    concurrent: 2,
                },
            )
            .unwrap();

        assert_eq!(result.fragments, Some(3));
        assert_eq!(fs::read(&output).unwrap(), b"ZEROONETWO");
        fs::remove_file(output).unwrap();
        server.join().unwrap();
    }
}
