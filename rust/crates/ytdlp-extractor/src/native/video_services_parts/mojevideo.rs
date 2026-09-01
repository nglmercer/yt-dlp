// Native Mojevideo extraction is split into page/API-style variable parsing,
// signed media construction, and JSON-LD metadata mapping.
include!("mojevideo_parts/page.rs");
include!("mojevideo_parts/media.rs");
include!("mojevideo_parts/extractor.rs");
