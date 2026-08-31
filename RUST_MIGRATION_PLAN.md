# Python to Rust migration plan

Status: Phase 0 started. The Python implementation remains the reference
implementation and the default executable. The Rust workspace is experimental
until the parity gates below are met.

## Scope

“1:1” means parity for the supported CLI, embedding API, extractor behavior,
networking, downloads, postprocessing, JavaScript support, cookies, plugins,
cache, updates, packaging, and observable output. It does not mean a
line-for-line translation of Python internals.

The current repository is approximately 225k lines of Python, including 971
extractor modules, 2,038 extractor classes, 1,595 extraction methods, 295 CLI
option definitions, and 56 test modules.

## Invariants

1. Python is the behavioral oracle until Rust is promoted.
2. No extractor or protocol is silently dropped; every capability is marked
   native, compatibility-bridged, or incomplete.
3. Compatibility is measured with fixtures and differential tests, not only
   compilation or unit-test success.
4. Rust is initially shipped as an opt-in experimental engine.

## Target workspace

```text
rust/
  crates/
    ytdlp-core/       data model, errors, shared semantics
    ytdlp-cli/        experimental command-line entry point
    extractor/        registry, base extractor, ported extractors
    networking/       requests, handlers, cookies, proxies
    downloader/       protocols, fragments, resume, live streams
    postprocessor/    FFmpeg and metadata integrations
    javascript/       runtime and EJS interfaces
    plugins/          compatibility and native plugin ABI
    compatibility/    Python bridge and differential runner
```

## Stages

### 0. Compatibility contract

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
Expose a native Rust API and a Python compatibility wrapper for callbacks,
custom format selectors, loggers, and postprocessors.

### 3. Networking and cookies

Port the request director/handler model, headers, proxies, TLS, redirects,
compression, WebSockets, SOCKS, browser-cookie databases, keyrings, and
impersonation. Keep a compatibility backend for browser fingerprinting until
Rust behavior is equivalent.

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

Keep a Python plugin bridge during migration. Define a versioned native plugin
ABI for the Python-free distribution; existing Python plugin source cannot be
loaded by a pure-Rust binary without embedding Python. Release in stages:
experimental binary, opt-in Rust engine with fallback, Rust default, then
remove the fallback after a deprecation cycle.

### 8. Packaging and updates

Reproduce the Linux, macOS, and Windows artifact matrix, standalone/archive
formats, completions, manpage, license inventory, signed updates, and
self-update behavior.

## Parity gates for promotion

- all offline/core tests pass in both implementations
- extractor matching has no missing or duplicate owners
- every supported extractor/protocol is native or explicitly bridged
- CLI/API outputs, errors, warnings, and exit codes match
- networking, cookies, JavaScript, and FFmpeg fixture tests pass
- plugins have a documented compatibility path
- release artifacts and self-update verification pass on every target

## Initial implementation slice

- [x] Rust workspace and experimental `yt-dlp-rs` binary
- [x] insertion-order-preserving `InfoDict` foundation
- [x] explicit migration status/capability model
- [x] Python reference retained as the default
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
- [x] response framing for content-length, chunked, and close-delimited bodies
- [x] native request director with direct and fallback handlers
- [x] typed Rust CLI option model and deterministic parser slice
- [x] Python/Rust differential fixtures for the initial CLI option slice
- [x] dynamic aliases, preset aliases, and shell-like config tokenization
- [x] explicit config-file precedence overlaid by command-line options
- [x] generated ordered inventory for all 1,752 Python extractor registrations
- [x] Python-compatible extractor regex compilation with coverage diagnostics
- [x] CLI network-option adapter into the native request director
- [x] opt-in extractor selection diagnostics (`--extractor-info`)
- [x] native direct-resource downloader with atomic output commits
- [x] native GenericIE URL metadata and opt-in `--native-download` path
- [x] native HLS media-playlist parsing, segment concatenation, and retries
- [x] native DASH SegmentList parsing and concatenation
- [x] native DASH SegmentTemplate/SegmentTimeline expansion
- [x] bounded concurrent fragment assembly with ordered output and retries
- [x] direct-resource resume using HTTP Range and partial-file reconciliation
- [x] initial `InfoDict` accessors and Python-style output-template rendering
- [x] core URL/protocol and scalar conversion utility slice with differential fixtures
- [x] generated inventory of all 323 Python CLI option definitions and 394 aliases
- [x] postprocessor lifecycle contract and shell-free FFmpeg subprocess bridge
- [x] opt-in native postprocessing for remuxing and audio extraction
- [x] JavaScript runtime discovery/version gating and stdin/temp-file adapters
- [x] explicit process-based Python compatibility crate for unsupported surfaces
- [x] explicit Python compatibility bridge for ordinary Rust-binary invocations

Next: add downloader differential fixtures, expand the base extractor/result
contracts, port the remaining FFmpeg operations and option groups, wire EJS
challenge batches through the runtime adapter, and continue protocol coverage
while keeping the Python compatibility backend available.
