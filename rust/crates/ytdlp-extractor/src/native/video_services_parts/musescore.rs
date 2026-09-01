// Native MuseScore extraction is split into page/API transport, authenticated
// MP3 format construction, and descriptor-facing metadata assembly.
include!("musescore_parts/api.rs");
include!("musescore_parts/media.rs");
include!("musescore_parts/extractor.rs");
