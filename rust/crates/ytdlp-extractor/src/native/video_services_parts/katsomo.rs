// Native Katsomo extraction is split into API transport, playback format
// mapping, and the descriptor-facing extractor.
include!("katsomo_parts/api.rs");
include!("katsomo_parts/media.rs");
include!("katsomo_parts/extractor.rs");
