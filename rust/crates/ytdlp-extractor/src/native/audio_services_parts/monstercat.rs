// Native Monstercat release-page playlist/audio extraction is split into
// page metadata, track rows, and descriptor-facing playlist assembly.
include!("monstercat_parts/page.rs");
include!("monstercat_parts/tracks.rs");
include!("monstercat_parts/extractor.rs");
