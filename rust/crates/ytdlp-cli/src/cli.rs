use indexmap::IndexMap;
use std::path::{Path, PathBuf};
use yt_dlp_networking::{Request, SharedCookieJar};

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

fn missing_value(option: &str) -> CliError {
    CliError::new(format!("{option} requires an argument"))
}

fn next_value(args: &[String], index: &mut usize, option: &str) -> Result<String, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| missing_value(option))
}

fn option_value(
    args: &[String],
    index: &mut usize,
    option: &str,
    inline_value: Option<String>,
) -> Result<String, CliError> {
    inline_value.map_or_else(|| next_value(args, index, option), Ok)
}

fn parse_f64(value: String, option: &str) -> Result<f64, CliError> {
    value
        .parse()
        .map_err(|error| CliError::new(format!("invalid value for {option}: {error}")))
}

fn parse_i64(value: String, option: &str) -> Result<i64, CliError> {
    value
        .parse()
        .map_err(|error| CliError::new(format!("invalid value for {option}: {error}")))
}

fn split_long_option(argument: &str) -> (&str, Option<String>) {
    argument
        .split_once('=')
        .map_or((argument, None), |(name, value)| {
            (name, Some(value.to_owned()))
        })
}

fn add_csv(values: &mut Vec<String>, value: &str, prepend: bool) {
    let parsed = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if prepend {
        values.splice(0..0, parsed);
    } else {
        values.extend(parsed);
    }
}

fn add_prefixed_value(
    values: &mut IndexMap<String, String>,
    value: &str,
    allowed_prefixes: &[&str],
    default_prefix: &str,
) {
    if let Some((prefix, value)) = value.split_once(':') {
        if allowed_prefixes.contains(&prefix) {
            values.insert(prefix.to_owned(), value.to_owned());
            return;
        }
    }
    values.insert(default_prefix.to_owned(), value.to_owned());
}

fn add_postprocessor_args(
    values: &mut IndexMap<String, Vec<String>>,
    value: &str,
) -> Result<(), CliError> {
    let (key, arguments) = value
        .split_once(':')
        .ok_or_else(|| CliError::new("--postprocessor-args must be NAME:ARGS"))?;
    if key.is_empty() {
        return Err(CliError::new(
            "--postprocessor-args requires a non-empty processor name",
        ));
    }
    let key = key.to_ascii_lowercase();
    values
        .entry(key)
        .or_default()
        .extend(split_shell_words(arguments)?);
    Ok(())
}

fn split_shell_words(value: &str) -> Result<Vec<String>, CliError> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut in_word = false;

    for character in value.chars() {
        if escaped {
            word.push(character);
            escaped = false;
            in_word = true;
            continue;
        }
        match quote {
            Some('\'') if character != '\'' => word.push(character),
            Some('\'') => quote = None,
            Some('"') if character == '"' => quote = None,
            Some('"') if character == '\\' => escaped = true,
            Some('"') => word.push(character),
            Some(_) => word.push(character),
            None if character == '\\' => escaped = true,
            None if character == '\'' || character == '"' => {
                quote = Some(character);
                in_word = true;
            }
            None if character.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(character);
                in_word = true;
            }
        }
    }
    if escaped || quote.is_some() {
        return Err(CliError::new("unterminated quote or escape in alias"));
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

fn strip_config_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') if character == '\\' => escaped = true,
            Some('"') if character == '"' => quote = None,
            Some('\'') if character == '\'' => quote = None,
            Some(_) => {}
            None if character == '\'' || character == '"' => quote = Some(character),
            None if character == '#' => return &line[..index],
            None => {}
        }
    }
    line
}

/// Read a yt-dlp configuration file into the same argument tokens used by
/// the command line. Missing-file policy is left to the caller so explicit
/// config locations can fail loudly while default locations can be skipped.
pub fn read_config_file(path: &Path) -> Result<Vec<String>, CliError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| CliError::new(format!("could not read config {path:?}: {error}")))?;
    contents
        .lines()
        .try_fold(Vec::new(), |mut arguments, line| {
            arguments.extend(split_shell_words(strip_config_comment(line))?);
            Ok(arguments)
        })
}

/// Parse lower-priority config files followed by the command line, matching
/// yt-dlp's last-source-wins option precedence.
pub fn parse_args_with_config_files(
    args: &[String],
    config_files: &[PathBuf],
) -> Result<ParseResult, CliError> {
    let mut combined = Vec::new();
    for path in config_files {
        combined.extend(read_config_file(path)?);
    }
    combined.extend(args.iter().cloned());
    parse_args(&combined)
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn push_existing(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() && !paths.contains(&path) {
        paths.push(path);
    }
}

/// Return default config files in yt-dlp's precedence order.
pub fn default_config_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            push_existing(&mut paths, directory.join("yt-dlp.conf"));
        }
    }
    if let Ok(directory) = std::env::current_dir() {
        push_existing(&mut paths, directory.join("yt-dlp.conf"));
    }

    let home = config_home();
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|path| path.join(".config")));
    if let Some(xdg) = xdg {
        push_existing(&mut paths, xdg.join("yt-dlp.conf"));
        push_existing(&mut paths, xdg.join("yt-dlp/config"));
        push_existing(&mut paths, xdg.join("yt-dlp/config.txt"));
    }
    if let Some(appdata) = std::env::var_os("appdata").or_else(|| std::env::var_os("APPDATA")) {
        let appdata = PathBuf::from(appdata);
        push_existing(&mut paths, appdata.join("yt-dlp.conf"));
        push_existing(&mut paths, appdata.join("yt-dlp/config"));
        push_existing(&mut paths, appdata.join("yt-dlp/config.txt"));
    }
    if let Some(home) = home {
        push_existing(&mut paths, home.join("yt-dlp.conf"));
        push_existing(&mut paths, home.join("yt-dlp.conf.txt"));
        push_existing(&mut paths, home.join(".yt-dlp/config"));
        push_existing(&mut paths, home.join(".yt-dlp/config.txt"));
    }
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp.conf"));
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp/config"));
    push_existing(&mut paths, PathBuf::from("/etc/yt-dlp/config.txt"));
    paths
}

fn resolve_config_location(path: &str) -> PathBuf {
    let home = config_home();
    let path = path
        .strip_prefix("~/")
        .and_then(|suffix| home.as_ref().map(|home| home.join(suffix)))
        .unwrap_or_else(|| PathBuf::from(path));
    if path.is_dir() {
        path.join("yt-dlp.conf")
    } else {
        path
    }
}

/// Parse command-line arguments together with the default and explicit config
/// locations. Explicit locations are required to exist; default candidates
/// are already filtered to existing files.
pub fn parse_configured_args(args: &[String]) -> Result<ParseResult, CliError> {
    let ignore_config = args.iter().any(|argument| {
        argument == "--ignore-config"
            || argument == "--no-config"
            || argument == "--ignore-config=true"
    });
    let mut files = if ignore_config {
        Vec::new()
    } else {
        default_config_files()
    };
    let mut custom_locations = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let (option, inline_value) = split_long_option(&args[index]);
        match option {
            "--config-locations" => {
                let value = inline_value
                    .clone()
                    .map_or_else(|| next_value(args, &mut index, option), Ok)?;
                custom_locations.push(resolve_config_location(&value));
            }
            "--no-config-locations" => custom_locations.clear(),
            _ => {}
        }
        index += 1;
    }
    for path in custom_locations {
        if !path.is_file() {
            return Err(CliError::new(format!(
                "config location {path:?} does not exist"
            )));
        }
        if !files.contains(&path) {
            files.push(path);
        }
    }
    parse_args_with_config_files(args, &files)
}

fn alias_arity(template: &str) -> usize {
    let mut arity = 0;
    let bytes = template.as_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index] == b'{' && bytes[index + 1].is_ascii_digit() && bytes[index + 2] == b'}' {
            arity = arity.max((bytes[index + 1] - b'0' + 1) as usize);
        }
    }
    arity
}

fn expand_alias_token(template: &str, values: &[String]) -> String {
    let mut expanded = template.to_owned();
    for (index, value) in values.iter().enumerate() {
        expanded = expanded.replace(&format!("{{{index}}}"), value);
    }
    expanded
}

fn expand_aliases(args: &[String]) -> Result<Vec<String>, CliError> {
    let mut aliases = IndexMap::new();
    let mut definitions_removed = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--alias" => {
                let names = args
                    .get(index + 1)
                    .ok_or_else(|| missing_value("--alias"))?;
                let template = args
                    .get(index + 2)
                    .ok_or_else(|| missing_value("--alias"))?;
                for name in names
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    let name = if name.starts_with('-') {
                        name.to_owned()
                    } else {
                        format!("--{name}")
                    };
                    aliases.insert(name, template.clone());
                }
                index += 3;
            }
            "--preset-alias" | "-t" => {
                let preset = args
                    .get(index + 1)
                    .ok_or_else(|| missing_value(args[index].as_str()))?;
                let template = match preset.as_str() {
                    "mp3" => "-f ba[acodec^=mp3]/ba/b -x --audio-format mp3",
                    "aac" => "-f ba[acodec^=aac]/ba[acodec^=mp4a.40.]/ba/b -x --audio-format aac",
                    "mp4" => {
                        "--merge-output-format mp4 --remux-video mp4 -S vcodec:h264,lang,quality,res,fps,hdr:12,acodec:aac"
                    }
                    "mkv" => "--merge-output-format mkv --remux-video mkv",
                    "sleep" => {
                        "--sleep-subtitles 5 --sleep-requests 0.75 --sleep-interval 10 --max-sleep-interval 20"
                    }
                    _ => return Err(CliError::new(format!("unknown preset alias: {preset}"))),
                };
                definitions_removed.extend(split_shell_words(template)?);
                index += 2;
            }
            _ => {
                definitions_removed.push(args[index].clone());
                index += 1;
            }
        }
    }

    let mut expanded = definitions_removed;
    for _ in 0..100 {
        let mut changed = false;
        let mut next = Vec::with_capacity(expanded.len());
        let mut index = 0;
        while index < expanded.len() {
            let token = &expanded[index];
            let Some(template) = aliases.get(token) else {
                next.push(token.clone());
                index += 1;
                continue;
            };
            let arity = alias_arity(template);
            if expanded.len() < index + 1 + arity {
                return Err(CliError::new(format!(
                    "{token} requires {arity} argument{}",
                    if arity == 1 { "" } else { "s" }
                )));
            }
            let values = &expanded[index + 1..index + 1 + arity];
            let replacement = split_shell_words(&expand_alias_token(template, values))?;
            next.extend(replacement);
            index += 1 + arity;
            changed = true;
        }
        expanded = next;
        if !changed {
            return Ok(expanded);
        }
    }
    Err(CliError::new("alias exceeded invocation limit"))
}

/// Parse the migration's typed CLI subset.
///
/// The parser deliberately reports unknown options instead of silently
/// accepting them. The supported set is expanded in lockstep with the source
/// option schema and differential fixtures.
pub fn parse_args(args: &[String]) -> Result<ParseResult, CliError> {
    let args = expand_aliases(args)?;
    parse_args_inner(&args)
}

fn parse_args_inner(args: &[String]) -> Result<ParseResult, CliError> {
    let mut options = CliOptions::default();
    let mut index = 0;
    let mut parse_options = true;

    while index < args.len() {
        let argument = &args[index];
        if parse_options && argument == "--" {
            parse_options = false;
            index += 1;
            continue;
        }
        if !parse_options || !argument.starts_with('-') || argument == "-" {
            options.urls.push(argument.clone());
            index += 1;
            continue;
        }

        if argument == "-h" || argument == "--help" {
            return Ok(ParseResult::Help);
        }
        if argument == "--version" {
            return Ok(ParseResult::Version);
        }

        if argument.starts_with("--") {
            let (option, inline_value) = split_long_option(argument);
            match option {
                "--proxy" => {
                    options.proxy = Some(option_value(args, &mut index, option, inline_value)?)
                }
                "--socket-timeout" => {
                    options.socket_timeout = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--no-check-certificates" => options.no_check_certificate = true,
                "--js-runtimes" => {
                    options
                        .js_runtimes
                        .push(option_value(args, &mut index, option, inline_value)?)
                }
                "--no-js-runtimes" => options.js_runtimes.clear(),
                "--remote-components" => options.remote_components.push(option_value(
                    args,
                    &mut index,
                    option,
                    inline_value,
                )?),
                "--no-remote-components" => options.remote_components.clear(),
                "--ignore-config" | "--no-config" => options.ignoreconfig = Some(true),
                "--no-config-locations" => options.config_locations = None,
                "--config-locations" => {
                    options
                        .config_locations
                        .get_or_insert_with(Vec::new)
                        .push(option_value(args, &mut index, option, inline_value)?);
                }
                "--user-agent" => {
                    options.user_agent =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--referer" => {
                    options.referer = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--add-headers" => {
                    let value = option_value(args, &mut index, option, inline_value)?;
                    let (name, value) = value
                        .split_once(':')
                        .ok_or_else(|| CliError::new("--add-headers must be FIELD:VALUE"))?;
                    options
                        .headers
                        .insert(name.to_ascii_lowercase(), value.to_owned());
                }
                "--quiet" => options.quiet = Some(true),
                "--no-quiet" => options.quiet = Some(false),
                "--verbose" => options.verbose = true,
                "--no-warnings" => options.no_warnings = true,
                "--simulate" => options.simulate = Some(true),
                "--no-simulate" => options.simulate = Some(false),
                "--skip-download" | "--no-download" => options.skip_download = true,
                "--get-url" => options.geturl = Some(true),
                "--get-title" => options.gettitle = Some(true),
                "--get-id" => options.getid = Some(true),
                "--get-thumbnail" => options.getthumbnail = Some(true),
                "--get-duration" => options.getduration = Some(true),
                "--write-info-json" => options.writeinfojson = Some(true),
                "--no-write-info-json" => options.writeinfojson = Some(false),
                "--format" => {
                    options.format = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--all-formats" => options.format = Some("all".to_owned()),
                "--format-sort" => add_csv(
                    &mut options.format_sort,
                    &option_value(args, &mut index, option, inline_value)?,
                    true,
                ),
                "--format-sort-reset" => options.format_sort.clear(),
                "--extract-audio" => options.extractaudio = true,
                "--audio-format" => {
                    options.audioformat =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--audio-quality" => {
                    options.audioquality =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--merge-output-format" => {
                    options.merge_output_format =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--remux-video" => {
                    options.remuxvideo =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--recode-video" => {
                    options.recodevideo =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--postprocessor-args" | "--ppa" => {
                    let value = option_value(args, &mut index, option, inline_value)?;
                    add_postprocessor_args(&mut options.postprocessor_args, &value)?;
                }
                "--keep-video" => options.keepvideo = true,
                "--no-keep-video" => options.keepvideo = false,
                "--post-overwrites" => options.nopostoverwrites = false,
                "--no-post-overwrites" => options.nopostoverwrites = true,
                "--ffmpeg-location" => {
                    options.ffmpeg_location =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--sleep-subtitles" => {
                    options.sleep_interval_subtitles = parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?;
                }
                "--sleep-requests" => {
                    options.sleep_interval_requests = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--sleep-interval" | "--min-sleep-interval" => {
                    options.sleep_interval = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--max-sleep-interval" => {
                    options.max_sleep_interval = Some(parse_f64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--output" => add_prefixed_value(
                    &mut options.outtmpl,
                    &option_value(args, &mut index, option, inline_value)?,
                    &[
                        "default",
                        "chapter",
                        "thumbnail",
                        "pl_thumbnail",
                        "pl_video",
                        "pl_audio",
                        "pl_infojson",
                        "subtitle",
                    ],
                    "default",
                ),
                "--no-overwrites" => options.overwrites = Some(false),
                "--force-overwrites" | "--yes-overwrites" => options.overwrites = Some(true),
                "--no-force-overwrites" => options.overwrites = None,
                "--continue" => options.continue_dl = true,
                "--no-continue" => options.continue_dl = false,
                "--no-playlist" => options.noplaylist = true,
                "--yes-playlist" => options.noplaylist = false,
                "--list-formats" => options.listformats = Some(true),
                "--batch-file" => {
                    options.batchfile = Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--playlist-items" => {
                    options.playlist_items =
                        Some(option_value(args, &mut index, option, inline_value)?);
                }
                "--age-limit" => {
                    options.age_limit = Some(parse_i64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?);
                }
                "--retries" => {
                    options.retries = serde_json::Value::String(option_value(
                        args,
                        &mut index,
                        option,
                        inline_value,
                    )?);
                }
                "--concurrent-fragments" => {
                    options.concurrent_fragments = parse_i64(
                        option_value(args, &mut index, option, inline_value)?,
                        option,
                    )?;
                }
                _ => return Err(CliError::new(format!("unknown option {option}"))),
            }
            index += 1;
            continue;
        }

        let short = argument.as_str();
        if short.len() > 2 && !matches!(&short[..2], "-f" | "-S" | "-o" | "-a" | "-P" | "-u" | "-p")
        {
            for flag in short[1..].chars() {
                match flag {
                    'q' => options.quiet = Some(true),
                    'v' => options.verbose = true,
                    's' => options.simulate = Some(true),
                    'j' => options.dumpjson = true,
                    'J' => options.dump_single_json = true,
                    'F' => options.listformats = Some(true),
                    'g' => options.geturl = Some(true),
                    'e' => options.gettitle = Some(true),
                    'x' => options.extractaudio = true,
                    'k' => options.keepvideo = true,
                    'i' | 'n' => {}
                    'c' => options.continue_dl = true,
                    'w' => options.overwrites = Some(false),
                    _ => return Err(CliError::new(format!("unknown option -{flag}"))),
                }
            }
            index += 1;
            continue;
        }
        let (flag, suffix) = if short.len() > 2 {
            short.split_at(2)
        } else {
            (short, "")
        };
        let value_option = matches!(flag, "-f" | "-S" | "-o" | "-a" | "-P" | "-u" | "-p");
        if value_option {
            let value = if suffix.is_empty() {
                next_value(args, &mut index, flag)?
            } else {
                suffix.to_owned()
            };
            match flag {
                "-f" => options.format = Some(value),
                "-S" => add_csv(&mut options.format_sort, &value, true),
                "-o" => add_prefixed_value(
                    &mut options.outtmpl,
                    &value,
                    &[
                        "default",
                        "chapter",
                        "thumbnail",
                        "pl_thumbnail",
                        "pl_video",
                        "pl_audio",
                        "pl_infojson",
                        "subtitle",
                    ],
                    "default",
                ),
                "-a" => options.batchfile = Some(value),
                "-P" => add_prefixed_value(&mut options.outtmpl, &value, &["home", "temp"], "home"),
                "-u" | "-p" => {
                    return Err(CliError::new(format!("{flag} is not active in Rust yet")));
                }
                _ => unreachable!(),
            }
            index += 1;
            continue;
        }

        match short {
            "-q" => options.quiet = Some(true),
            "-v" => options.verbose = true,
            "-s" => options.simulate = Some(true),
            "-j" => options.dumpjson = true,
            "-J" => options.dump_single_json = true,
            "-F" => options.listformats = Some(true),
            "-g" => options.geturl = Some(true),
            "-e" => options.gettitle = Some(true),
            "-x" => options.extractaudio = true,
            "-k" => options.keepvideo = true,
            "-i" => {}
            "-n" => {}
            "-c" => options.continue_dl = true,
            "-w" => options.overwrites = Some(false),
            _ => return Err(CliError::new(format!("unknown option {short}"))),
        }
        index += 1;
    }

    Ok(ParseResult::Options(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_common_options_and_preserves_last_value() {
        let parsed = parse_args(&args(&[
            "--proxy=http://proxy.example",
            "--socket-timeout",
            "4.5",
            "--no-check-certificates",
            "--add-headers",
            "X-Test: one",
            "--add-headers",
            "Cookie:a=b",
            "-q",
            "-f",
            "bv*",
            "-o",
            "%(id)s.%(ext)s",
            "https://example.com",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };

        assert_eq!(options.proxy.as_deref(), Some("http://proxy.example"));
        assert_eq!(options.socket_timeout, Some(4.5));
        assert!(options.no_check_certificate);
        assert_eq!(options.headers["x-test"], " one");
        assert_eq!(options.headers["cookie"], "a=b");
        assert_eq!(options.user_agent, None);
        assert_eq!(options.referer, None);
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.format.as_deref(), Some("bv*"));
        assert_eq!(options.outtmpl["default"], "%(id)s.%(ext)s");
        assert_eq!(options.urls, ["https://example.com"]);
    }

    #[test]
    fn parses_aliases_and_option_terminator() {
        let parsed = parse_args(&args(&[
            "--no-playlist",
            "--yes-playlist",
            "--skip-download",
            "--no-simulate",
            "-v",
            "--",
            "-not-an-option",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert!(!options.noplaylist);
        assert!(options.skip_download);
        assert_eq!(options.simulate, Some(false));
        assert!(options.verbose);
        assert_eq!(options.urls, ["-not-an-option"]);
    }

    #[test]
    fn rejects_unknown_options_and_bad_values() {
        assert!(parse_args(&args(&["--not-real"])).is_err());
        assert!(parse_args(&args(&["--socket-timeout", "slow"])).is_err());
        assert!(parse_args(&args(&["--add-headers", "missing-value"])).is_err());
    }

    #[test]
    fn expands_dynamic_and_preset_aliases() {
        let parsed = parse_args(&args(&[
            "--alias",
            "quick,-Q",
            "--format {0} --quiet",
            "--quick",
            "bestvideo",
            "video",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert_eq!(options.format.as_deref(), Some("bestvideo"));
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.urls, ["video"]);

        let ParseResult::Options(options) = parse_args(&args(&["-t", "mp3", "video"])).unwrap()
        else {
            panic!("expected options")
        };
        assert!(options.extractaudio);
        assert_eq!(options.audioformat.as_deref(), Some("mp3"));
    }

    #[test]
    fn config_files_are_tokenized_and_overridden_by_command_line() {
        let path = std::env::temp_dir().join(format!(
            "yt-dlp-rs-config-{}-{}.conf",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(
            &path,
            "# comment\n--quiet -o 'config.%(ext)s'\n--alias cfg --format {0}\n",
        )
        .unwrap();

        let parsed = parse_args_with_config_files(
            &args(&["--cfg", "best", "-o", "command.%(ext)s", "video"]),
            std::slice::from_ref(&path),
        )
        .unwrap();
        let _ = std::fs::remove_file(&path);
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };
        assert_eq!(options.quiet, Some(true));
        assert_eq!(options.format.as_deref(), Some("best"));
        assert_eq!(options.outtmpl["default"], "command.%(ext)s");
    }

    #[test]
    fn request_adapter_carries_network_options_into_native_request() {
        let parsed = parse_args(&args(&[
            "--proxy",
            "http://proxy.example:8080",
            "--socket-timeout",
            "3.5",
            "--no-check-certificates",
            "--add-headers",
            "X-Trace: enabled",
            "--user-agent",
            "Rust test agent",
            "--referer",
            "https://referrer.example/",
            "https://example.com/video",
        ]))
        .unwrap();
        let ParseResult::Options(options) = parsed else {
            panic!("expected options")
        };

        let request = options.request_for_url(
            &options.urls[0],
            yt_dlp_networking::CookieJar::new().shared(),
        );
        assert_eq!(request.headers().get("X-Trace"), Some("enabled"));
        assert_eq!(request.headers().get("User-Agent"), Some("Rust test agent"));
        assert_eq!(
            request.headers().get("Referer"),
            Some("https://referrer.example/")
        );
        assert_eq!(
            request.proxies().get("all").and_then(Option::as_deref),
            Some("http://proxy.example:8080")
        );
        assert_eq!(
            request
                .extensions()
                .get("timeout")
                .and_then(serde_json::Value::as_f64),
            Some(3.5)
        );
        assert_eq!(
            request.extensions().get("verify"),
            Some(&serde_json::json!(false))
        );
    }
}
