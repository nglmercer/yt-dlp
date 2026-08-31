#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn headers_are_case_insensitive_but_retain_original_spelling() {
        let mut headers = Headers::new();
        headers.set("x-dlp", " data ");
        headers.set("Ytdl-Test", "one");
        headers.set("ytdl-test", "two");

        assert_eq!(headers.get("X-DLP"), Some("data"));
        assert_eq!(headers.get("YTDL-TEST"), Some("two"));
        assert_eq!(
            headers.sensitive().get("ytdl-test"),
            Some(&"two".to_owned())
        );
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn response_headers_preserve_multiplicity_and_cookie_semantics() {
        let mut headers = ResponseHeaders::new();
        headers.add("Test", "test1");
        headers.add("test", "test2");
        headers.add("Set-Cookie", "cookie1");
        headers.add("Set-cookie", "cookie2");

        assert_eq!(headers.get_all("TEST"), vec!["test1", "test2"]);
        assert_eq!(headers.get_header("test"), Some("test1, test2".to_owned()));
        assert_eq!(headers.get_header("set-cookie"), Some("cookie1".to_owned()));
        assert_eq!(headers.get_header("missing"), None);
    }

    #[test]
    fn cookie_jar_applies_domain_path_and_secure_rules() {
        let mut jar = CookieJar::new();
        let mut headers = ResponseHeaders::new();
        headers.add("Set-Cookie", "sid=abc; Path=/account");
        headers.add("Set-Cookie", "secure=yes; Secure; Path=/");
        jar.store_response("https://example.com/login", &headers)
            .unwrap();

        assert_eq!(
            jar.cookie_header("https://example.com/account/page")
                .unwrap(),
            Some("sid=abc; secure=yes".to_owned())
        );
        assert_eq!(
            jar.cookie_header("http://example.com/account/page")
                .unwrap(),
            Some("sid=abc".to_owned())
        );
        assert_eq!(
            jar.cookie_header("https://example.com/other").unwrap(),
            Some("secure=yes".to_owned())
        );
    }

    #[test]
    fn cookie_jar_loads_and_saves_netscape_files() {
        let input =
            std::env::temp_dir().join(format!("yt-dlp-rs-cookies-input-{}", std::process::id()));
        let output =
            std::env::temp_dir().join(format!("yt-dlp-rs-cookies-output-{}", std::process::id()));
        std::fs::write(
            &input,
            "# Netscape HTTP Cookie File\n.example.test\tTRUE\t/\tFALSE\t4102444800\tsid\tabc\nexample.test\tFALSE\t/\tTRUE\t1\told\tignored\n",
        )
        .unwrap();

        let mut jar = CookieJar::new();
        assert_eq!(jar.load_netscape_file(&input).unwrap(), 1);
        assert_eq!(
            jar.cookie_header("https://cdn.example.test/video").unwrap(),
            Some("sid=abc".to_owned())
        );
        assert_eq!(jar.save_netscape_file(&output).unwrap(), 1);
        let saved = std::fs::read_to_string(&output).unwrap();
        assert!(saved.contains("sid\tabc"));

        std::fs::remove_file(input).unwrap();
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn proxy_selection_honors_scheme_all_and_no_proxy() {
        let proxies = IndexMap::from([
            ("all".to_owned(), Some("http://proxy.example".to_owned())),
            (
                "https".to_owned(),
                Some("http://secure-proxy.example".to_owned()),
            ),
            (
                "no".to_owned(),
                Some("localhost,example.com:8080".to_owned()),
            ),
        ]);
        assert_eq!(
            select_proxy("https://video.example", &proxies).unwrap(),
            Some("http://secure-proxy.example".to_owned())
        );
        assert_eq!(
            select_proxy("http://video.example", &proxies).unwrap(),
            Some("http://proxy.example".to_owned())
        );
        assert_eq!(
            select_proxy("http://localhost:8080", &proxies).unwrap(),
            None
        );
        assert_eq!(
            select_proxy("http://example.com:8080/path", &proxies).unwrap(),
            None
        );
    }

    #[test]
    fn request_defaults_method_and_content_headers_from_data() {
        let mut request = Request::new("https://example.com");
        assert_eq!(request.method(), "GET");
        assert!(request.data().is_none());
        assert!(!request.headers().contains("Content-Type"));

        request.set_data(Some(b"a=1".to_vec()));
        assert_eq!(request.method(), "POST");
        assert_eq!(
            request.headers().get("content-type"),
            Some("application/x-www-form-urlencoded")
        );

        request.headers_mut().set("Content-Length", "3");
        request.set_data(Some(b"a=2".to_vec()));
        assert!(!request.headers().contains("Content-Length"));

        request.set_method("put").unwrap();
        assert_eq!(request.method(), "PUT");
        request.set_data(None);
        assert_eq!(request.method(), "PUT");
        assert!(!request.headers().contains("Content-Type"));
    }

    #[test]
    fn request_rejects_control_characters_in_method() {
        let mut request = Request::new("https://example.com");
        assert!(request.set_method("GET\n").is_err());
    }

    #[test]
    fn normalize_url_matches_reference_examples() {
        assert_eq!(normalize_url("//example.com"), "http://example.com");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(
            normalize_url("https://фtest.example.com/ some spaceв?ä=c"),
            "https://xn--test-z6d.example.com/%20some%20space%D0%B2?%C3%A4=c"
        );
        assert_eq!(
            normalize_url("https://example.com/a/../b"),
            "https://example.com/b"
        );
    }

    #[test]
    fn update_url_query_replaces_existing_keys_and_appends_new_keys() {
        assert_eq!(
            update_url_query(
                "http://example.com?q=something",
                &[("v".to_owned(), "xyz".to_owned())]
            ),
            "http://example.com?q=something&v=xyz"
        );
        assert_eq!(
            update_url_query(
                "http://example.com?q=something&v=old",
                &[("v".to_owned(), "123".to_owned())]
            ),
            "http://example.com?q=something&v=123"
        );
        assert_eq!(
            update_url_query(
                "http://example.com?a=1&a=2&blank=",
                &[
                    ("a".to_owned(), "3".to_owned()),
                    ("v".to_owned(), "hello world".to_owned())
                ]
            ),
            "http://example.com?a=3&v=hello+world"
        );
    }

    fn serve_once(response: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            stream.write_all(&response).unwrap();
        });
        (format!("http://{address}"), thread)
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
        }

        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            return request;
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let content_length = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        let expected_length = header_end + 4 + content_length.unwrap_or(0);
        while request.len() < expected_length {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
        }
        request
    }

    fn serve_sequence(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                stream.write_all(&response).unwrap();
            }
            requests
        });
        (format!("http://{address}"), thread)
    }

    #[test]
    fn http_handler_sends_request_and_reads_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello"
                .to_vec(),
        );
        let mut request = Request::new(format!("{url}/hello?x=1"));
        request.headers_mut().set("X-Test", "one");

        let mut director = RequestDirector::new();
        director.add_handler(HttpHandler);
        let response = director.send(&request).unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.reason(), "OK");
        assert_eq!(response.body(), b"hello");
        assert_eq!(response.headers().get("content-type"), Some("text/plain"));
        server.join().unwrap();
    }

    #[test]
    fn http_handler_decodes_chunked_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n".to_vec(),
        );
        let request = Request::new(url);
        let response = HttpHandler.send(&request).unwrap();

        assert_eq!(response.reason(), "OK");
        assert_eq!(response.body(), b"hello world");
        server.join().unwrap();
    }

    #[test]
    fn http_handler_does_not_claim_https() {
        let request = Request::new("https://example.com");
        let error = HttpHandler.supports(&request).unwrap_err();

        assert_eq!(error.kind, ErrorKind::Unsupported);
    }

    #[test]
    fn http_handler_follows_redirects_and_rewrites_post_to_get() {
        let (url, server) = serve_sequence(vec![
            b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
        ]);
        let mut request = Request::new(format!("{url}/start"));
        request.set_data(Some(b"a=1".to_vec()));

        let response = HttpHandler.send(&request).unwrap();
        let requests = server.join().unwrap();
        let first = String::from_utf8_lossy(&requests[0]);
        let second = String::from_utf8_lossy(&requests[1]);

        assert_eq!(response.url(), format!("{url}/final"));
        assert_eq!(response.body(), b"ok");
        assert!(first.starts_with("POST /start HTTP/1.1\r\n"));
        assert!(first.ends_with("a=1"));
        assert!(second.starts_with("GET /final HTTP/1.1\r\n"));
        assert!(!second.contains("Content-Length:"));
        assert!(!second.contains("Content-Type:"));
    }

    #[test]
    fn http_handler_reports_redirect_loops() {
        let (url, server) = serve_once(
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n"
                .to_vec(),
        );
        let error = HttpHandler
            .send(&Request::new(format!("{url}/loop")))
            .unwrap_err();
        server.join().unwrap();

        assert_eq!(
            error.kind,
            ErrorKind::Http {
                status: 0,
                reason: "redirect loop detected".to_owned(),
            }
        );
    }

    #[test]
    fn http_handler_persists_set_cookie_across_redirects() {
        let (url, server) = serve_sequence(vec![
            b"HTTP/1.1 302 Found\r\nSet-Cookie: sid=abc; Path=/\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n"
                .to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
        ]);
        let jar = CookieJar::new().shared();
        let request = Request::new(format!("{url}/start")).with_cookie_jar(jar);

        let response = HttpHandler.send(&request).unwrap();
        let requests = server.join().unwrap();
        let second = String::from_utf8_lossy(&requests[1]);

        assert_eq!(response.body(), b"ok");
        assert!(second.contains("Cookie: sid=abc\r\n"));
    }

    #[test]
    fn reqwest_handler_supports_https_and_decodes_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello"
                .to_vec(),
        );
        let request = Request::new(format!("{url}/hello"));
        let response = ReqwestHandler.send(&request).unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.body(), b"hello");
        server.join().unwrap();
        assert!(
            ReqwestHandler
                .supports(&Request::new("https://example.com"))
                .is_ok()
        );
    }

    #[test]
    fn redirect_method_matches_python_policy() {
        for (method, status, expected) in [
            ("GET", 303, "GET"),
            ("HEAD", 303, "HEAD"),
            ("PUT", 303, "GET"),
            ("POST", 301, "GET"),
            ("HEAD", 301, "HEAD"),
            ("POST", 302, "GET"),
            ("PUT", 302, "PUT"),
            ("POST", 307, "POST"),
            ("POST", 308, "POST"),
        ] {
            assert_eq!(redirect_method(method, status), expected);
        }
    }

    struct FakeHandler {
        name: &'static str,
        preference: i32,
        supported: bool,
        body: &'static [u8],
    }

    impl RequestHandler for FakeHandler {
        fn name(&self) -> &str {
            self.name
        }

        fn preference(&self, _request: &Request) -> i32 {
            self.preference
        }

        fn supports(&self, _request: &Request) -> Result<(), RequestError> {
            if self.supported {
                Ok(())
            } else {
                Err(RequestError::new(ErrorKind::Unsupported, "test scheme"))
            }
        }

        fn send(&self, request: &Request) -> Result<Response, RequestError> {
            Ok(Response::new(request.url(), 200, "OK", self.body.to_vec()))
        }
    }

    #[test]
    fn director_prefers_supported_handler() {
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            name: "low",
            preference: 1,
            supported: true,
            body: b"low",
        });
        director.add_handler(FakeHandler {
            name: "high-but-unsupported",
            preference: 2,
            supported: false,
            body: b"unused",
        });
        director.add_handler(FakeHandler {
            name: "high",
            preference: 2,
            supported: true,
            body: b"high",
        });

        let response = director.send(&Request::new("test://example")).unwrap();
        assert_eq!(response.body(), b"high");
    }

    #[test]
    fn director_reports_unsupported_handlers() {
        let mut director = RequestDirector::new();
        director.add_handler(FakeHandler {
            name: "fake",
            preference: 0,
            supported: false,
            body: b"unused",
        });

        let error = director.send(&Request::new("test://example")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::NoSupportingHandlers);
        assert_eq!(error.message, "Unable to handle request: fake: test scheme");
    }

    #[test]
    fn native_director_selects_direct_http_handler() {
        let (url, server) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec());
        let response = RequestDirector::native().send(&Request::new(url)).unwrap();
        server.join().unwrap();
        assert_eq!(response.body(), b"ok");
    }
}
