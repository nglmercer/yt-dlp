# Native Rust YouTube readiness gate

Status: **NOT READY**

Last audited: 2026-09-03

Current checkpoint: `80f14c720` (+ uncommitted working tree)

This is the stop condition for the Rust migration. New non-YouTube platform
ports are paused until the gate below is satisfied. The Rust executable must
be self-contained in Rust: Python may remain an offline development oracle,
but it must not be a runtime, compatibility backend, FFI dependency, or
download-time fallback.

“All videos” means every supported non-DRM YouTube media path that can be
downloaded with the supplied URL, account/session, and tokens. Access
controls, DRM, unavailable videos, and server-side blocks must fail with a
clear Rust `TODO:` or extraction error rather than silently selecting a
different backend.

## Current evidence

- The Rust registry has a native video-only `YoutubeIE` (`youtube_parts/`,
  committed at `80f14c720`): official watch/youtu.be/shorts/embed/live URLs
  reduce to the 11-character video ID, page `ytcfg` plus the Innertube player
  API yield progressive/adaptive formats, captions, thumbnails, and metadata.
  Playlist, search, feed, clip, and account URLs keep their generated
  descriptors and remain explicit TODOs.
- The working tree adds a native player-JavaScript inventory
  (`youtube_parts/player.rs`, uncommitted): `PLAYER_JS_URL` /
  `WEB_PLAYER_CONTEXT_CONFIGS[*].jsUrl` resolution, default-variant
  normalisation, `{player_id}-{variant}` cache keys, and `sts` extraction from
  page config or player script. Challenge TODOs now name the concrete player
  revision (`player <id>, sts <sts>`).
- The working tree ports the deterministic offline slice of
  `process_format_stream`/`_real_extract` (uncommitted, 573/573 extractor
  tests green via direct `rustc --test` object compilation plus direct `cc`
  linking; `cargo` cannot spawn `rustc`/`cc` in this sandbox): DRM exposure
  with `has_drm`, OTF and live-adaptive skips, itag/language duplicate
  suppression, quality ranks, `source_preference`/`preference`/`container`/
  `filesize_approx`, language preferences, fps cleanup, trailer redirects,
  composed DRM/sign-in/captcha/geo/rate-limit errors, thumbnail synthesis,
  `age_limit`, shorts `media_type`, stretch ratios, clip timestamps, music
  metadata, exact subtitle format list with `xosf` stripping and
  `impersonate`, behind a revision-specific solver TODO.
- The working tree hardens the EJS challenge path: provider preference order
  no longer aborts when Node/QuickJS lack a vendored solver library,
  per-request solver errors leave partial successes intact with explicit
  failure TODOs, runtime stderr is filtered with the provider-specific benign
  banners before stdout is trusted, missing player inventory is reported
  instead of swallowed, and top-level solver failures keep explicit TODOs.
  Multi-client orchestration, PO tokens, `initial_data` fields (chapters,
  heatmap, likes, comments, uploader badges), live DVR, playlists/tabs/search,
  and the native solver remain open.
- `YoutubeTabIE` and the remaining YouTube descriptors still resolve to the
  generic descriptor extractor and end in the explicit “extractor ... is not
  ported to Rust yet” result.
- The Rust CLI now runs a native selector engine (31/31 CLI tests and
  12/12 postprocessor tests green via the same direct `rustc`+`cc`
  pipeline): `/` fallbacks, `+` merges, `,` multi-downloads, `(...)`
  groups, `best`/`worst` atoms (with media/`*`/`.N` modifiers), extension
  atoms, format IDs, `all`/`mergeall`, `[...]` filters, `-S` sort keys, and
  the default `bv*+ba/b` merge spec. Merged selections download each
  `requested_formats` part to `f<id>` files and merge with the native
  `FfmpegMerger`. The DRM slice is ported: `--allow-unplayable-formats`
  (with the exact warning body), truthy-`has_drm` filtering that keeps
  `"maybe"`, the exact `This video is DRM protected` error, and the
  `DRM`/`Maybe DRM` table markers. Still open: `check_formats`
  download probing.
- The downloader has native direct HTTP, basic HLS, and basic DASH support,
  but no YouTube format acquisition, signature deciphering, `n` throttling,
  or PO-token flow.
- The Rust JavaScript crate only invokes an external JavaScript executable;
  it is not a native YouTube challenge implementation. It is not a Python
  dependency, but it does not satisfy the pure-Rust YouTube gate.
- Generic cookie-file loading exists. YouTube login/session extraction,
  browser-cookie import, client selection, and PO-token handling are not
  complete.

Therefore the Rust YouTube path is currently **not usable for downloading a
YouTube video**.

## Feature gate

| Area | Required behavior | Current state |
| --- | --- | --- |
| URL intake | watch, youtu.be, embed, shorts, live, nocookie, music, and canonical ID forms | partial (video IDs plus playlist URLs/bare IDs; tab/clip/search/account remain TODO) |
| Player API | native Innertube requests, client contexts, visitor/session data, retries, and error mapping | partial (WEB client page + player API; configured Player PO token and visitor header sent end-to-end; director-fetched tokens and session flows still TODO) |
| Progressive media | parse and expose playable `formats` with headers, codecs, dimensions, bitrate, and expiry | partial (offline field contract ported: ranks, preferences, container, DRM/OTF/live skips; expiry/PO-token flows TODO) |
| Adaptive media | parse DASH video/audio and HLS/live manifests | partial (format records native; manifest expansion stays downloader-side) |
| URL security | native `signatureCipher`/`s` deciphering and `n`-parameter transformation | partial (player-URL/`sts` inventory native; EJS adapter hardened with provider fallback, partial failures, and stderr checks; from-scratch native solver still TODO, revisions named in TODOs) |
| Tokens | native visitor data, SAPISID/session authorization, PO-token inputs, and token refresh policy | partial (PO-token boundary native: contexts, `CLIENT[.CONTEXT]+TOKEN` config parse + base64url canonicalization, `fetch_pot` policy, WebPO content binding, webpo cache spec/key derivation, memory LRU, visitor/data-sync extraction, `serviceIntegrityDimensions` injection; `--extractor-args` parsing + `ExtractionContext` lookup native, configured Player PO token and `X-Goog-Visitor-Id` flow end-to-end into the player request; token minting, director/provider registry, and SAPISID/session auth still TODO) |
| Availability | public, unlisted, age-restricted, private/authenticated, members-only, region-limited, premiering, and unavailable responses | partial (DRM/sign-in/captcha/geo/rate-limit errors composed; badge-based availability needs initial_data) |
| Live | live/DVR/ongoing streams, `is_live`, retry/refresh behavior, and bounded recording | TODO |
| Metadata | title, description, uploader/channel, counts, dates, duration, categories, tags, thumbnails, chapters, heatmap, and availability | partial (player-response fields + synthesized thumbnails + age_limit + shorts + stretch + music + clip times; chapters/heatmap/likes/comments need initial_data) |
| Captions | manual/automatic captions, language selection, subtitle formats, and authenticated caption requests | partial (exact format list, xosf strip, impersonate flag; translation/tlang expansion and PO-token captions TODO) |
| Playlists | playlist URLs, channel tabs, search/feed URLs, continuations, selection, and no-playlist behavior | partial (playlist URLs + bare IDs natively routed; initial-data tabs, `playlistVideoListRenderer` entries as video URL results, browse-API continuation pagination with loop guard and visitor refresh, core id/title/description/tags metadata; full `videoRenderer` entry fields ported as `youtube_extract_video` (durations incl. Shorts label fallback, counts, badges, live/upcoming states, UCID/handles, thumbnails); tab URL claiming (`youtube_tab_url_parts`/`suitable` over official hosts; invidious stays out per routing policy), tab selection/id (`selected`/`tabIdentifier`/name fallbacks), and first-page Videos dispatch (`itemSection`+`videoRenderer`, `richGrid`+rich items incl. tab `url_result`s, exact continuation cascade; other renderer kinds contribute continuations only) ported; `_real_extract` composition, tab metadata, browse-loop pagination, and header stats stay TODO) |
| Format selection | default best selection, filters, sorting, fallbacks, compound selectors, and `--all-formats` | partial (native selector engine: `/`/`+`/`,`/`()`/atoms/`all`/`mergeall`, `[...]` filters, `-S` sort keys, default `bv*+ba/b`; interactive `-f -` prompt loop native with table pre-print, ENTER-default, and reprompt; DRM/unplayable filter, `--allow-unplayable-formats`, protected error, and table markers ported, `check_formats` probing still TODO) |
| Download | range requests, retries, expiry-aware requests, HLS/DASH segment ordering, and failure reporting | partial |
| Merge | native CLI orchestration of separate video/audio downloads and FFmpeg merge/remux options | partial (`FfmpegMerger` ported: stream copy with per-stream maps, temp output, faststart; merged parts download to `f<id>` files; HLS-AAC fixup native via ffprobe/`ffmpeg -i` audio-codec probe with `aac_adtstoasc` bsf; standalone `FixupM3u8` PP trigger policy still TODO) |
| Output | templates, extension decisions, info JSON, requested fields, archive, overwrite/resume, and multi-input behavior | partial |
| Post-processing | audio extraction, remux, recode, subtitles, thumbnails, metadata, and chapter embedding | partial |
| Security boundary | no Python imports, Python subprocesses, PyO3, or compatibility delegation in the product path | pass for current Rust crates |

## Required implementation order

1. Build a modular native YouTube URL and Innertube client. Add deterministic
   fixtures for each request and response class.
2. Map player responses into a complete internal format model, including
   progressive, DASH, HLS, live, expiry, codec, and subtitle metadata.
3. Implement the YouTube URL transformation algorithms from scratch in Rust.
   If a player revision cannot be handled safely, return an explicit `TODO:`
   error and add the revision to the fixture matrix.
4. ~~Implement format filtering, sorting, compound selection, and video/audio
   merge orchestration before claiming normal downloads work.~~ Done for the
   offline engine (selector, filters, sort keys, `all`/`mergeall`, merger);
   left: format probing, interactive selector, AAC fixup.
5. Add authentication/session and PO-token boundaries. Unsupported account,
   consent, age, geo, or token states must be explicit and testable.
6. Add playlist/channel/search continuation handling and the remaining
   YouTube URL descriptors.
7. Run the full gate, including optional live smoke tests, then commit the
   YouTube checkpoint. Only after that may other platforms resume.

## Verification protocol

Every YouTube checkpoint must pass the deterministic Rust suite:

```text
cargo test --manifest-path rust/Cargo.toml --workspace
make rust-parity
git diff --check
```

The YouTube fixture suite must cover at least:

- a public watch video with progressive and adaptive formats;
- a short, embed, youtu.be, live, and music URL;
- signed and unsigned format URLs;
- `signatureCipher` and `n` transformation fixtures from multiple player
  revisions;
- DASH video plus audio selection and merge;
- HLS live/DVR selection;
- expiry, retry, range, HTTP 403, unavailable, age, geo, and auth errors;
- captions, thumbnails, chapters, and metadata;
- playlist/channel/search continuation and `--no-playlist`;
- cookies, session headers, PO-token-required responses, and explicit TODOs;
- format selectors such as `best`, `bv*+ba`, filters, sort keys, and
  `--all-formats`.

Live smoke testing is an additional release check, not a replacement for
fixtures. It must be opt-in and use a user-provided URL:

```text
YTDLP_YOUTUBE_SMOKE_URL='https://www.youtube.com/watch?v=...' \
  cargo run --manifest-path rust/Cargo.toml --bin yt-dlp-rs -- \
  --native-download --simulate --dump-json "$YTDLP_YOUTUBE_SMOKE_URL"
```

The result is a pass only when the command executes entirely through native
Rust and the selected media URLs are usable by the native downloader. A
metadata-only response, a generic redirect, an external Python/JavaScript
fallback, or an unlabelled partial result is not a pass.

## Decision

The migration is paused at the platform layer. YouTube is the next and only
active platform target until this document changes to **READY** with passing
tests and a committed Rust implementation.
