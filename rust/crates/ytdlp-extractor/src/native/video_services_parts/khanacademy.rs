// Native Khan Academy extraction is split into its shared GraphQL request,
// video metadata, and unit playlist modules.
include!("khanacademy_parts/api.rs");
include!("khanacademy_parts/video.rs");
include!("khanacademy_parts/unit.rs");
