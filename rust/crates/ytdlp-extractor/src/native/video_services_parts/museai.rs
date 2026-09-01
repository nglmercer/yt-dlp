// Native MuseAI extraction is split into player-data transport, media format
// construction, and descriptor-facing metadata assembly.
include!("museai_parts/api.rs");
include!("museai_parts/media.rs");
include!("museai_parts/extractor.rs");
