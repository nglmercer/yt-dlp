// Native Mocha extraction is split into API transport, media mapping, and
// metadata materialization so the service-specific code stays reviewable.
include!("mocha_parts/api.rs");
include!("mocha_parts/media.rs");
include!("mocha_parts/extractor.rs");
