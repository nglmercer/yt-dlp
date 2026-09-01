// Native Mixlr extraction is split into API transport, media probing, and
// descriptor-facing event/recording implementations.
include!("mixlr_parts/api.rs");
include!("mixlr_parts/media.rs");
include!("mixlr_parts/extractor.rs");
