/// Native Breitbart extractor. Breitbart exposes a JWPlayer HLS manifest whose
/// URL is derived from the video ID; page metadata is read with the native HTTP
/// stack and the existing Rust HLS downloader handles the media.
pub struct BreitbartExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BreitbartExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audius extractor. Host discovery, URL resolution, and stream URL
/// construction are performed through the Rust request context; the service's
/// JavaScript frontend is not needed.
pub struct AudiusExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiusExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Blerp GraphQL extractor. The query is intentionally limited to the
/// fields needed for a downloadable audio result, which keeps the Rust port
/// deterministic and avoids the web application's JavaScript bundle.
pub struct BlerpExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl BlerpExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast episode extractor. Acast exposes episode metadata through a
/// small JSON endpoint, so the Rust port can preserve the audio result without
/// scraping or executing the embed player.
pub struct AcastExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Acast show/playlist extractor. Playlist entry construction is fully
/// native; selecting and downloading entries is kept as an explicit CLI TODO
/// until the playlist scheduler is ported.
pub struct AcastChannelExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AcastChannelExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Dumpert JSON extractor. Media variants are represented as ordinary
/// Rust format records; HLS variants are handed to the native HLS downloader
/// by URL detection in the CLI.
pub struct DumpertExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl DumpertExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiodraft entry extractor for contest URLs that already expose the
/// numeric entry ID. The custom-domain page-discovery variant remains an
/// explicit TODO because it requires a second HTML player parser.
pub struct AudiodraftExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiodraftExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Audiomack song extractor. The song endpoint provides a final media
/// URL and canonical metadata; wrapper URLs for another service are surfaced
/// as TODO instead of being delegated to a different runtime.
pub struct AudiomackExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AudiomackExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}

/// Native Aitube.kz extractor. The page's Next.js data and the service's HLS
/// endpoint are both consumed directly by Rust.
pub struct AitubeExtractor {
    descriptor: ExtractorDescriptor,
    matcher: Regex,
}

impl AitubeExtractor {
    pub fn new(descriptor: ExtractorDescriptor) -> Result<Self, ExtractorError> {
        Ok(Self {
            matcher: descriptor_matcher(&descriptor)?,
            descriptor,
        })
    }
}
