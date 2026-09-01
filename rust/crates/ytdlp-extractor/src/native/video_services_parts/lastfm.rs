// Native Last.fm extraction is split into shared page/entry handling,
// playlist descriptors, and the single-track redirect descriptor.
include!("lastfm_parts/core.rs");
include!("lastfm_parts/playlist.rs");
include!("lastfm_parts/extractor.rs");
