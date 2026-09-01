// Native Melon VOD extraction is split into the two JSON API calls, HLS
// format construction, and descriptor-facing metadata assembly.
include!("melonvod_parts/api.rs");
include!("melonvod_parts/media.rs");
include!("melonvod_parts/extractor.rs");
