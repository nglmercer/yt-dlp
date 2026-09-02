# Native Rust YouTube readiness gate

Status: **NOT READY**  
Last audited: 2026-09-01  
Current checkpoint: `33e8ae46a`

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

- The generated manifest contains the YouTube descriptors, but the Rust
  registry has no native `YoutubeIE` or `YoutubeTabIE` implementation.
- There is no native Rust YouTube extractor source or YouTube-specific test
  fixture.
- A YouTube URL currently resolves to the generic descriptor extractor and
  ends in the explicit “extractor ... is not ported to Rust yet” result.
- The Rust CLI currently selects one simple format. Compound selectors such
  as `bestvideo+bestaudio`, format filters, and `--all-formats` are explicit
  TODOs.
- The downloader has native direct HTTP, basic HLS, and basic DASH support,
  but no YouTube format acquisition, signature deciphering, `n` throttling,
  PO-token flow, or adaptive video/audio merge.
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
| URL intake | watch, youtu.be, embed, shorts, live, nocookie, music, and canonical ID forms | TODO |
| Player API | native Innertube requests, client contexts, visitor/session data, retries, and error mapping | TODO |
| Progressive media | parse and expose playable `formats` with headers, codecs, dimensions, bitrate, and expiry | TODO |
| Adaptive media | parse DASH video/audio and HLS/live manifests | partial downloader only |
| URL security | native `signatureCipher`/`s` deciphering and `n`-parameter transformation | TODO |
| Tokens | native visitor data, SAPISID/session authorization, PO-token inputs, and token refresh policy | TODO |
| Availability | public, unlisted, age-restricted, private/authenticated, members-only, region-limited, premiering, and unavailable responses | TODO |
| Live | live/DVR/ongoing streams, `is_live`, retry/refresh behavior, and bounded recording | TODO |
| Metadata | title, description, uploader/channel, counts, dates, duration, categories, tags, thumbnails, chapters, heatmap, and availability | TODO |
| Captions | manual/automatic captions, language selection, subtitle formats, and authenticated caption requests | TODO |
| Playlists | playlist URLs, channel tabs, search/feed URLs, continuations, selection, and no-playlist behavior | TODO |
| Format selection | default best selection, filters, sorting, fallbacks, compound selectors, and `--all-formats` | TODO |
| Download | range requests, retries, expiry-aware requests, HLS/DASH segment ordering, and failure reporting | partial |
| Merge | native CLI orchestration of separate video/audio downloads and FFmpeg merge/remux options | TODO |
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
4. Implement format filtering, sorting, compound selection, and video/audio
   merge orchestration before claiming normal downloads work.
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
