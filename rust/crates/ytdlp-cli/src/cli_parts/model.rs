#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CliOptions {
    pub urls: Vec<String>,
    pub proxy: Option<String>,
    pub socket_timeout: Option<f64>,
    pub no_check_certificate: bool,
    pub js_runtimes: Vec<String>,
    pub remote_components: Vec<String>,
    pub headers: IndexMap<String, String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub quiet: Option<bool>,
    pub verbose: bool,
    pub no_warnings: bool,
    pub simulate: Option<bool>,
    pub skip_download: bool,
    pub format: Option<String>,
    pub format_sort: Vec<String>,
    pub extractaudio: bool,
    pub audioformat: Option<String>,
    pub audioquality: Option<String>,
    pub merge_output_format: Option<String>,
    pub remuxvideo: Option<String>,
    pub recodevideo: Option<String>,
    pub postprocessor_args: IndexMap<String, Vec<String>>,
    pub keepvideo: bool,
    pub nopostoverwrites: bool,
    pub ffmpeg_location: Option<String>,
    pub sleep_interval_subtitles: f64,
    pub sleep_interval_requests: Option<f64>,
    pub sleep_interval: Option<f64>,
    pub max_sleep_interval: Option<f64>,
    pub outtmpl: IndexMap<String, String>,
    pub overwrites: Option<bool>,
    pub continue_dl: bool,
    pub noplaylist: bool,
    pub dumpjson: bool,
    pub dump_single_json: bool,
    pub geturl: Option<bool>,
    pub gettitle: Option<bool>,
    pub getid: Option<bool>,
    pub getthumbnail: Option<bool>,
    pub getduration: Option<bool>,
    pub writeinfojson: Option<bool>,
    pub listformats: Option<bool>,
    pub batchfile: Option<String>,
    pub playlist_items: Option<String>,
    pub age_limit: Option<i64>,
    pub retries: serde_json::Value,
    pub concurrent_fragments: i64,
    pub ignoreconfig: Option<bool>,
    pub config_locations: Option<Vec<String>>,
    pub download_archive: Option<String>,
    pub cookiefile: Option<String>,
}

/// Option spellings understood by the typed Rust parser. The generated source manifest is
/// compared with this list for migration diagnostics; it
/// prevents an option from disappearing when yt-dlp adds a new alias.
pub fn rust_supported_option_aliases() -> &'static [&'static str] {
    &[
        "-h",
        "--help",
        "--version",
        "--proxy",
        "--socket-timeout",
        "--no-check-certificates",
        "--js-runtimes",
        "--no-js-runtimes",
        "--remote-components",
        "--no-remote-components",
        "--ignore-config",
        "--no-config",
        "--download-archive",
        "--no-download-archive",
        "--cookies",
        "--no-cookies",
        "--no-config-locations",
        "--config-locations",
        "--user-agent",
        "--referer",
        "--add-headers",
        "--quiet",
        "--no-quiet",
        "--verbose",
        "--no-warnings",
        "--simulate",
        "--no-simulate",
        "--skip-download",
        "--no-download",
        "--format",
        "--all-formats",
        "--format-sort",
        "--format-sort-reset",
        "--extract-audio",
        "--audio-format",
        "--audio-quality",
        "--merge-output-format",
        "--remux-video",
        "--recode-video",
        "--postprocessor-args",
        "--ppa",
        "--keep-video",
        "--no-keep-video",
        "--post-overwrites",
        "--no-post-overwrites",
        "--ffmpeg-location",
        "--sleep-subtitles",
        "--sleep-requests",
        "--sleep-interval",
        "--min-sleep-interval",
        "--max-sleep-interval",
        "--output",
        "--no-overwrites",
        "--force-overwrites",
        "--yes-overwrites",
        "--no-force-overwrites",
        "--continue",
        "--no-continue",
        "--no-playlist",
        "--yes-playlist",
        "--list-formats",
        "--batch-file",
        "--playlist-items",
        "--age-limit",
        "--retries",
        "--concurrent-fragments",
        "--alias",
        "--preset-alias",
        "-t",
        "-f",
        "-S",
        "-o",
        "-a",
        "-P",
        "-u",
        "-p",
        "-q",
        "-v",
        "-s",
        "-j",
        "-J",
        "-F",
        "-g",
        "--get-url",
        "-e",
        "--get-title",
        "--get-id",
        "--get-thumbnail",
        "--get-duration",
        "--write-info-json",
        "--no-write-info-json",
        "-x",
        "-k",
        "-i",
        "-n",
        "-c",
        "-w",
    ]
}

impl CliOptions {
    /// Translate the network-facing CLI options into the shared request
    /// contract. Download and extractor layers can reuse this adapter while
    /// the rest of the command remains on the native Rust path.
    pub fn request_for_url(&self, url: &str, cookie_jar: SharedCookieJar) -> Request {
        let mut request = Request::new(url).with_cookie_jar(cookie_jar);
        for (name, value) in &self.headers {
            request.headers_mut().set(name, value);
        }
        if let Some(user_agent) = &self.user_agent {
            request.headers_mut().set("User-Agent", user_agent);
        }
        if let Some(referer) = &self.referer {
            request.headers_mut().set("Referer", referer);
        }
        if let Some(proxy) = &self.proxy {
            request.proxies_mut().insert(
                "all".to_owned(),
                (!proxy.is_empty()).then(|| proxy.to_owned()),
            );
        }
        if let Some(timeout) = self.socket_timeout {
            request
                .extensions_mut()
                .insert("timeout".to_owned(), serde_json::json!(timeout));
        }
        if self.no_check_certificate {
            request
                .extensions_mut()
                .insert("verify".to_owned(), serde_json::json!(false));
        }
        request
    }
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            urls: Vec::new(),
            proxy: None,
            socket_timeout: None,
            no_check_certificate: false,
            js_runtimes: vec!["deno".to_owned()],
            remote_components: Vec::new(),
            headers: IndexMap::new(),
            user_agent: None,
            referer: None,
            quiet: None,
            verbose: false,
            no_warnings: false,
            simulate: None,
            skip_download: false,
            format: None,
            format_sort: Vec::new(),
            extractaudio: false,
            audioformat: Some("best".to_owned()),
            audioquality: Some("5".to_owned()),
            merge_output_format: None,
            remuxvideo: None,
            recodevideo: None,
            postprocessor_args: IndexMap::new(),
            keepvideo: false,
            nopostoverwrites: false,
            ffmpeg_location: None,
            sleep_interval_subtitles: 0.0,
            sleep_interval_requests: None,
            sleep_interval: None,
            max_sleep_interval: None,
            outtmpl: IndexMap::new(),
            overwrites: None,
            continue_dl: true,
            noplaylist: false,
            dumpjson: false,
            dump_single_json: false,
            geturl: Some(false),
            gettitle: Some(false),
            getid: Some(false),
            getthumbnail: Some(false),
            getduration: Some(false),
            writeinfojson: None,
            listformats: None,
            batchfile: None,
            playlist_items: None,
            age_limit: None,
            retries: serde_json::json!(10),
            concurrent_fragments: 1,
            ignoreconfig: None,
            config_locations: None,
            download_archive: None,
            cookiefile: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult {
    Options(CliOptions),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CliError {}
