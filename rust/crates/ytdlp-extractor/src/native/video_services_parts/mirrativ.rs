// Native Mirrativ extraction is split into API transport, media mapping, and
// descriptor-facing live/user-history implementations.
include!("mirrativ_parts/api.rs");
include!("mirrativ_parts/media.rs");
include!("mirrativ_parts/extractor.rs");
