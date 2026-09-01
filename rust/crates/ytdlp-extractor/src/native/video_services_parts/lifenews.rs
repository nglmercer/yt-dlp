// Native Life.ru article and embed extraction is split into page parsing,
// embed-media mapping, and descriptor-facing implementations.
include!("lifenews_parts/page.rs");
include!("lifenews_parts/embed.rs");
include!("lifenews_parts/extractor.rs");
