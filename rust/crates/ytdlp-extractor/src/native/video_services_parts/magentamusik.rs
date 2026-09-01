// Native MagentaMusik extraction is split into API traversal, SMIL parsing,
// and descriptor-facing metadata assembly.
include!("magentamusik_parts/api.rs");
include!("magentamusik_parts/smil.rs");
include!("magentamusik_parts/extractor.rs");
