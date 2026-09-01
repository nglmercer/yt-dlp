// Native Mave extraction is split into API transport, episode field mapping,
// and descriptor-facing episode/channel implementations.
include!("mave_parts/api.rs");
include!("mave_parts/media.rs");
include!("mave_parts/extractor.rs");
