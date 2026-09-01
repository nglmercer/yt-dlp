// Native MediaKlikk extraction is split into page/player transport, media
// URL construction, and descriptor-facing metadata assembly.
include!("mediaklikk_parts/api.rs");
include!("mediaklikk_parts/media.rs");
include!("mediaklikk_parts/extractor.rs");
