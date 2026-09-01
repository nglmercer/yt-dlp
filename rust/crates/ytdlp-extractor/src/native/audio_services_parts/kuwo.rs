// Native Kuwo extraction is split into shared anti-server/page helpers,
// single-song and MV extraction, and the three HTML playlist extractors.
include!("kuwo_parts/shared.rs");
include!("kuwo_parts/song.rs");
include!("kuwo_parts/playlists.rs");
