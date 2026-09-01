// Native Locipo extraction is split into API transport, Streaks/media
// normalization, and descriptor-facing video/playlist implementations.
include!("locipo_parts/api.rs");
include!("locipo_parts/media.rs");
include!("locipo_parts/extractor.rs");
