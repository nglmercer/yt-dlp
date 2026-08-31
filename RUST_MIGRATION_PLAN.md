# Python to Rust migration plan

Status: Phases 0–6 in progress. The Python tree remains available only as an offline
behavioral reference while the executable is migrated. The Rust workspace is
the only product runtime; unsupported surfaces fail explicitly as TODO.

## Scope

“1:1” means parity for the supported CLI, embedding API, extractor behavior,
networking, downloads, postprocessing, JavaScript support, cookies, plugins,
cache, updates, packaging, and observable output. It does not mean a
line-for-line translation of Python internals.

The current repository is approximately 225k lines of Python, including 971
extractor modules, 2,038 extractor classes, 1,595 extraction methods, 295 CLI
option definitions, and 56 test modules.

## Invariants

1. Python may be used as an offline behavioral oracle during development, but
   the Rust executable never imports, embeds, or invokes Python.
2. No extractor or protocol is silently dropped; every capability is either
   native Rust or explicitly marked TODO.
3. Behavioral parity is measured with fixtures and differential tests, not
   only compilation or unit-test success.
4. Rust-only execution is the default from the first usable binary; feature
   gaps are reported instead of falling back to another runtime.

## Target workspace

```text
rust/
  crates/
    ytdlp-core/       data model, errors, shared semantics
    ytdlp-cli/        experimental command-line entry point
    ytdlp-extractor/     registry, base extractor, ported extractors
    ytdlp-networking/   requests, handlers, cookies, proxies
    ytdlp-downloader/   protocols, fragments, resume, live streams
    ytdlp-postprocessor/ FFmpeg and metadata integrations
    ytdlp-javascript/   runtime and EJS interfaces
    ytdlp-plugins/      native plugin ABI
    ytdlp-validation/   offline differential runner and fixtures
```

## Stages

### 0. Behavioral parity contract

Build a reference runner that captures exit code, stdout, stderr, warnings,
debug output, sanitized info JSON, generated files, and media metadata. Add
deterministic local HTTP/media fixtures before relying on live extractor tests.

### 1. Core model and utilities

Port dynamic info dictionaries, result types, errors, output templates, format
selection, traversal, URL/date/JSON/XML helpers, retries, cache, config, and
download archives. Preserve arbitrary info fields and Python insertion order.

### 2. CLI and embedding API

Reproduce option parsing, aliases, configuration precedence, environment
variables, help/completion output, errors, exit codes, and signal handling.
Expose a native Rust API. Callbacks, custom format selectors, loggers, and
postprocessors are Rust traits; unsupported extension points are TODO until
their native interfaces are implemented.

### 3. Networking and cookies

Port the request director/handler model, headers, proxies, TLS, redirects,
compression, WebSockets, SOCKS, browser-cookie databases, keyrings, and
impersonation. Unsupported browser fingerprinting behavior is a visible TODO,
not a compatibility backend.

### 4. Downloaders and postprocessors

Port direct HTTP, fragments, HLS, DASH, live protocols, HDS/F4M, MSS, RTMP,
and service-specific downloaders. Preserve resume, retries, concurrency,
partial-file names, stdout streaming, and FFmpeg decisions. Keep FFmpeg and
other external tools as subprocess integrations first.

### 5. JavaScript

Implement runtime adapters for Node, Deno, Bun, and QuickJS, then port the EJS
protocol and internal JavaScript interpreter. YouTube challenge-solving parity
is a release gate.

### 6. Extractors

Generate a registry that preserves names, keys, matching order, URL patterns,
inheritance, lazy loading, playlist semantics, and extractor filtering. Port
simple/direct extractors first, then API/manifest/auth/live extractors, with
YouTube and other JavaScript-heavy extractors last. Every port must retain its
existing URL-matching and `info_dict` tests.

### 7. Plugins and rollout

Define a versioned native plugin ABI for the Python-free distribution. Python
plugin source is not loaded by the Rust binary. Release in stages by increasing
native coverage; no runtime fallback is permitted.

### 8. Packaging and updates

Reproduce the Linux, macOS, and Windows artifact matrix, standalone/archive
formats, completions, manpage, license inventory, signed updates, and
self-update behavior.

## Parity gates for promotion

- all offline/core tests pass in both implementations
- extractor matching has no missing or duplicate owners
- every supported extractor/protocol is native or explicitly marked TODO
- CLI/API outputs, errors, warnings, and exit codes match
- networking, cookies, JavaScript, and FFmpeg fixture tests pass
- native plugins have a documented ABI and unsupported Python plugins fail TODO
- release artifacts and self-update verification pass on every target

## Initial implementation slice

- [x] Rust workspace and experimental `yt-dlp-rs` binary
- [x] insertion-order-preserving `InfoDict` foundation
- [x] explicit migration status/capability model
- [x] Python source retained only as an offline migration reference
- [x] newline-delimited Python/Rust differential runner
- [x] first utility port: `format_bytes`
- [x] second utility port: `parse_bytes`
- [x] request/header/error model without socket I/O
- [x] request director/handler dispatch contract without socket I/O
- [x] URL normalization and query-update semantics
- [x] native HTTP/1.1 handler with loopback response tests
- [x] repeated response headers and `Set-Cookie` lookup semantics
- [x] redirect method policy, relative targets, and loop detection
- [x] HTTPS/proxy-capable handler with compression decoding
- [x] RFC 6265 cookie-jar state across requests and redirects
- [x] native Netscape cookie-file load/save and CLI cookie-file options
- [x] response framing for content-length, chunked, and close-delimited bodies
- [x] native request director with direct and secondary native handlers
- [x] typed Rust CLI option model and deterministic parser slice
- [x] Python/Rust differential fixtures for the initial CLI option slice
- [x] dynamic aliases, preset aliases, and shell-like config tokenization
- [x] explicit config-file precedence overlaid by command-line options
- [x] generated ordered inventory for all 1,752 Python extractor registrations
- [x] source-compatible extractor regex compilation with coverage diagnostics
- [x] modular extractor crate split into shared contracts, native implementations, registry, and tests
- [x] domain-focused native extractor modules behind a shared Rust namespace
- [x] modular networking, downloader, CLI, core, postprocessor, JavaScript, and extractor test sources into bounded Rust units
- [x] CLI network-option adapter into the native request director
- [x] opt-in extractor selection diagnostics (`--extractor-info`)
- [x] native direct-resource downloader with atomic output commits
- [x] native GenericIE URL metadata and opt-in `--native-download` path
- [x] native extractor context with shared Rust request/cookie state
- [x] native Archive.org metadata/file and playlist-result extractor
- [x] native AltCensored Archive.org transparent video and channel playlist extractors
- [x] native BongaCams room API/live HLS extractor
- [x] native AudioBoom embedded clip-store/audio extractor
- [x] native Bandcamp track JSON/audio extractor
- [x] native BannedVideo GraphQL metadata/media/comment extractor
- [x] native BitChute media and metadata API extractor
- [x] native Coub API media-version extractor
- [x] native Freesound HTML/Open Graph audio extractor
- [x] native Yandex Disk store/public-media extractor
- [x] native Rumble embed API/live/caption extractor
- [x] native Rumble canonical-page wrapper and page-level metadata merge
- [x] native Vocaroo HEAD-checked direct-audio extractor
- [x] native Google Drive playback-transcode extractor
- [x] native Clyp JSON API extractor with audio format records
- [x] native Breitbart page metadata and JWPlayer HLS extractor
- [x] native Audius host discovery, resolution, and stream extractor
- [x] native Blerp GraphQL audio extractor with POST request support
- [x] native Acast episode JSON/API extractor
- [x] native Acast channel playlist result contract
- [x] native Dumpert API/variant extractor
- [x] native Audiodraft contest-entry API extractor
- [x] native Audiomack song API extractor
- [x] native Aitube Next-data/HLS extractor
- [x] native HLS media-playlist parsing, segment concatenation, and retries
- [x] native HLS byte-range parsing and Range requests
- [x] native DASH SegmentList parsing and concatenation
- [x] native DASH SegmentList byte-range parsing and Range requests
- [x] explicit Rust TODO guards for unsupported HDS/F4M, MSS, RTMP, and legacy transports
- [x] native DASH SegmentTemplate/SegmentTimeline expansion
- [x] bounded concurrent fragment assembly with ordered output and retries
- [x] direct-resource resume using HTTP Range and partial-file reconciliation
- [x] initial `InfoDict` accessors and Python-style output-template rendering
- [x] core URL/protocol and scalar conversion utility slice with differential fixtures
- [x] native ISO-8601 timestamp parsing with differential fixtures
- [x] generated inventory of all 323 Python CLI option definitions and 394 aliases
- [x] postprocessor lifecycle contract and shell-free FFmpeg subprocess bridge
- [x] opt-in native postprocessing for remuxing and audio extraction
- [x] JavaScript runtime discovery/version gating and stdin/temp-file adapters
- [x] Rust-only default executable with explicit TODO errors for unported surfaces
- [x] Python invocation path rejected by the product binary
- [x] native format-ID/extension selection for extracted format records
- [x] native metadata and format-list output for `--dump-json`/`--list-formats`
- [x] native `--get-url`, `--get-title`, `--get-id`, `--get-thumbnail`, and `--get-duration`
- [x] native `--write-info-json` output beside the selected media path
- [x] native multi-URL execution and `--batch-file` input for the download loop
- [x] native download archive persistence, duplicate skipping, and CLI option parity
- [x] native playlist selection for single-entry output, -j, -J, and ranges
- [x] native Streamable AJAX extractor with multi-format metadata
- [x] native Newgrounds media, collection, search, and user listing extractors
- [x] native Wistia media, playlist, and API-backed channel extractors
- [x] native VidLii page/source extractor with HEAD validation
- [x] native href.li redirect result and Rust redirect-chain execution
- [x] Rust/Python differential fixtures for HLS and DASH manifest expansion
- [x] native PeerTube v1 video API extractor across generated instances
- [x] native PeerTube account/channel/playlist pagination and entry expansion
- [x] native Rumble channel/user pagination and entry expansion
- [x] native Slideshare embedded-JSON video extractor
- [x] native Soundgasm audio and profile playlist extractors
- [x] native Imgur animated media and gallery/album extractors
- [x] native NineGag API/animated-media extractor
- [x] native MyVidster videolink redirect extractor
- [x] native Glide HTML5 video-message extractor
- [x] native eBay embedded HLS/DASH extractor
- [x] native Sen API/HLS extractor
- [x] native Roya TV live-channel API/HLS extractor
- [x] native ReverbNation song API/audio extractor
- [x] native ETTU TV player-settings/HLS extractor
- [x] native Elonet embedded HLS/DASH extractor
- [x] native Fathom share-page/API-state HLS extractor
- [x] native Golem XML player/configuration extractor
- [x] native Screen9 embed-configuration extractor
- [x] native Bild.de JSON clip/source extractor
- [x] native FilmArchiv.at deterministic CDN/HLS extractor
- [x] native Netzkino Next.js/CMS movie extractor
- [x] native UTV Strasbourg progressive-video extractor
- [x] native Cineteca Milano catalog/API HLS extractor
- [x] native NonkTube HTML5-video extractor
- [x] native LoveHomePorn Nuevo XML extractor
- [x] native Angel Studios JSON-LD/HLS extractor
- [x] native Newsy page-data/HLS extractor
- [x] native Clubic M6Web player-configuration extractor
- [x] native münchen.tv live playlist extractor with SMIL TODO guard
- [x] native VOD Platform hidden-input HLS/DASH extractor with Wowza TODO guard
- [x] native AliExpress Live run-parameters/HLS extractor
- [x] native FC Zenit player-config/API progressive extractor
- [x] native Clipchamp Next.js/Cloudflare Stream extractor
- [x] native Baidu Video API playlist extractor
- [x] native FootyRoom playlist with Streamable URL entries
- [x] native Charlie Rose HTML5 player extractor with WebVTT subtitles
- [x] native El Trece TV Fusion/HLS extractor
- [x] native CanalC2 archive-page media extractor with RTMP TODO metadata
- [x] native Epoch Times/YouMaker deterministic HLS extractor
- [x] native Harpodeon deterministic MP4 extractor
- [x] native EbaumsWorld XML player extractor
- [x] native Fuyin TV JSON API extractor
- [x] native CAM4 live HLS extractor
- [x] native Kommunetv stream API/HLS extractor
- [x] native Stream.cz GraphQL/playlist extractor with subtitles
- [x] native Vidyard player JSON extractor with HLS, captions, and chapters
- [x] native Ku6 page/API extractor
- [x] native Graspop festival API/HLS extractor
- [x] native ScreenRec page/HLS extractor
- [x] native MatchTV live-channel extractor
- [x] native JWPlatform JSON/media/playlist extractor with captions
- [x] native Bundesliga, OutsideTV, and TeachingChannel JWPlatform wrappers
- [x] native AtScale conference event playlist extractor
- [x] native NZZ embedded-JWPlayer playlist extractor
- [x] native BehindKink HTML5 video extractor
- [x] native HistoricFilms direct archive-media extractor
- [x] native OnePlace podcast episode extractor
- [x] native Megaphone embedded podcast extractor
- [x] native Hypem track/source extractor
- [x] native QingTing program extractor
- [x] native Skyline Webcams live HLS extractor
- [x] native Webcamera.pl ROT13/HLS extractor
- [x] native Alibaba embedded product-video extractor
- [x] native Moving Image archive HLS extractor
- [x] native Tweakers progressive-video API extractor
- [x] native KrasView player-JSON extractor
- [x] native 56.com video API extractor
- [x] native TASS embedded-source extractor
- [x] native Photobucket metadata/file extractor
- [x] native Nobel Prize JSON-LD media extractor
- [x] native Caltrans traffic-camera live HLS extractor
- [x] native CozyTV replay extractor
- [x] native Livestreamfails API/direct-media extractor
- [x] native Masters tournament HLS extractor
- [x] native Mir24 article/player HLS extractor
- [x] native Blogger video-config extractor
- [x] native radio.de station extractor
- [x] native RadioZET podcast API extractor
- [x] native WorldStarHipHop HTML5 media extractor
- [x] native This American Life archive/audio extractor
- [x] native Academic Earth course playlist extractor
- [x] native Premiership Rugby article/HLS extractor
- [x] native MatchiTV Next.js/HLS extractor
- [x] native SZTV.hu VOD extractor
- [x] native APA direct-player and JWPlatform redirect extractor
- [x] native Arnes Video public-media API extractor
- [x] native CJSW episode audio extractor
- [x] native Daystar Lightcast configuration/HLS extractor
- [x] native DCTP versioned REST/API extractor
- [x] native Art19 episode/RSS podcast extractor

Next: expand the base extractor/result contracts, port the remaining FFmpeg operations and option
groups, wire EJS challenge batches through the Rust runtime adapter, and
continue protocol coverage. Python remains only a test oracle and is never
called by the product binary.
