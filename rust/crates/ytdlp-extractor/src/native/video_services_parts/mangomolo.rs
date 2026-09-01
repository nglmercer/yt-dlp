// Native Mangomolo extraction is split into channel-ID/API helpers, player
// media discovery, and the shared VOD/live extractor implementation.
include!("mangomolo_parts/api.rs");
include!("mangomolo_parts/media.rs");
include!("mangomolo_parts/extractor.rs");
