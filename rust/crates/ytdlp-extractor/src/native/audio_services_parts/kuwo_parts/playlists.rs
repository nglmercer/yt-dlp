// Kuwo playlist surfaces stay in separate units because each page has a
// different source contract and pagination model.
include!("playlists/album.rs");
include!("playlists/chart.rs");
include!("playlists/singer.rs");
include!("playlists/category.rs");
