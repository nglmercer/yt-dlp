// Native Toggle/MeWatch extraction is split into API payload/response helpers,
// format and thumbnail mapping, and the two descriptor implementations.
include!("toggle_parts/api.rs");
include!("toggle_parts/media.rs");
include!("toggle_parts/extractor.rs");
include!("toggle_parts/mewatch.rs");
