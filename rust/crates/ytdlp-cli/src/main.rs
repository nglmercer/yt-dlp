mod cli;

use std::env;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

use cli::{ParseResult, parse_args, parse_configured_args, rust_supported_option_aliases};
use yt_dlp_core::{DownloadArchive, INITIAL_CAPABILITIES, InfoDict, MIGRATION_VERSION};
use yt_dlp_core::{format_bytes, render_output_template};
use yt_dlp_downloader::{
    DirectDownloader, DownloadOptions, DownloadResult, parse_dash_mpd, parse_hls_playlist,
};
use yt_dlp_extractor::{ExtractionContext, ExtractorRegistry, ExtractorResult};
use yt_dlp_javascript::{JavascriptRuntime, RuntimeKind};
use yt_dlp_networking::{CookieJar, Request, RequestDirector, Response};
use yt_dlp_postprocessor::{
    FfmpegExtractAudio, FfmpegMerger, FfmpegRemuxer, FfmpegVideoConvertor, PostProcessOptions,
    PostProcessResult, PostProcessor,
};

include!("main_parts/parity.rs");
include!("main_parts/request.rs");
include!("main_parts/sort.rs");
include!("main_parts/formats.rs");
include!("main_parts/postprocess.rs");
include!("main_parts/output.rs");
include!("main_parts/playlist.rs");
include!("main_parts/download.rs");
include!("main_parts/status.rs");
include!("main_parts/entry.rs");
