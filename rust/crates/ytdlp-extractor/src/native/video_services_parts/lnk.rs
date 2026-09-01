// Native LNK.lt extraction is split into API normalization and media/result
// mapping so the site contract stays separate from descriptor plumbing.
include!("lnk_parts/api.rs");
include!("lnk_parts/extractor.rs");
