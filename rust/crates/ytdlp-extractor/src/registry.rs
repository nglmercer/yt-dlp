use super::common::*;
use super::native::*;
use yt_dlp_core::InfoDict;

#[derive(Default)]
pub struct ExtractorRegistry {
    extractors: Vec<Box<dyn InfoExtractor>>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestRecord {
    key: String,
    name: String,
    #[allow(dead_code)]
    module: String,
    #[allow(dead_code)]
    class: String,
    working: bool,
    patterns: Vec<String>,
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the ordered extractor inventory generated from the source
    /// `gen_extractors()` registry. Patterns that use source-only regular
    /// expression features remain visible in the inventory and are reported
    /// through `DescriptorExtractor::matcher_errors` instead of disappearing.
    pub fn generated() -> Result<Self, ExtractorError> {
        let records: Vec<ManifestRecord> =
            serde_json::from_str(include_str!("../data/extractors.json")).map_err(|error| {
                ExtractorError::new(
                    ExtractorErrorKind::Extraction,
                    format!("invalid generated extractor manifest: {error}"),
                )
            })?;
        let mut registry = Self::new();
        for record in records {
            let descriptor = ExtractorDescriptor::with_valid_urls(
                record.key,
                record.name,
                record.patterns.clone(),
                record.working,
            )
            .with_source(record.module, record.class);
            if descriptor.key == "HrefLiRedirectIE" {
                registry.register(HrefLiRedirectExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GenericIE" {
                registry.register(GenericExtractor::new(descriptor))?;
            } else if descriptor.key == "Ku6IE" {
                registry.register(Ku6Extractor::new(descriptor)?)?;
            } else if descriptor.key == "GraspopIE" {
                registry.register(GraspopExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ScreenRecIE" {
                registry.register(ScreenRecExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MatchTVIE" {
                registry.register(MatchTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JWPlatformIE" {
                registry.register(JwPlatformExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BundesligaIE" {
                registry.register(BundesligaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "OutsideTVIE" {
                registry.register(OutsideTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "TeachingChannelIE" {
                registry.register(TeachingChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AtScaleConfEventIE" {
                registry.register(AtScaleConfEventExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NZZIE" {
                registry.register(NzzExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BehindKinkIE" {
                registry.register(BehindKinkExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HistoricFilmsIE" {
                registry.register(HistoricFilmsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "OnePlacePodcastIE" {
                registry.register(OnePlacePodcastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MegaphoneIE" {
                registry.register(MegaphoneExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HypemIE" {
                registry.register(HypemExtractor::new(descriptor)?)?;
            } else if descriptor.key == "QingTingIE" {
                registry.register(QingTingExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SkylineWebcamsIE" {
                registry.register(SkylineWebcamsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "WebcameraplIE" {
                registry.register(WebcameraplExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AltCensoredIE" {
                registry.register(AltCensoredExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AltCensoredChannelIE" {
                registry.register(AltCensoredChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BongaCamsIE" {
                registry.register(BongaCamsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CrowdBunkerIE" {
                registry.register(CrowdBunkerExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CrowdBunkerChannelIE" {
                registry.register(CrowdBunkerChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BTVPlusIE" {
                registry.register(BtvPlusExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BoxCastVideoIE" {
                registry.register(BoxCastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BerufeTVIE" {
                registry.register(BerufeTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CanalAlphaIE" {
                registry.register(CanalAlphaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CanalsurmasIE" {
                registry.register(CanalsurmasExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AGalegaIE" {
                registry.register(AGalegaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CloudyCDNIE" {
                registry.register(CloudyCdnExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CloudflareStreamIE" {
                registry.register(CloudflareStreamExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DBTVIE" {
                registry.register(DbtvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Canal1IE" {
                registry.register(Canal1Extractor::new(descriptor)?)?;
            } else if descriptor.key == "CCMAIE" {
                registry.register(CcmaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DemocracynowIE" {
                registry.register(DemocracynowExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DacastPlaylistIE" {
                registry.register(DacastPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DacastVODIE" {
                registry.register(DacastVodExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DailyMailIE" {
                registry.register(DailyMailExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CrtvgIE" {
                registry.register(CrtvgExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CultureUnpluggedIE" {
                registry.register(CultureUnpluggedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CtsNewsIE" {
                registry.register(CtsNewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DigitekaIE" {
                registry.register(DigitekaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DFBIE" {
                registry.register(DfbExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DVTVIE" {
                registry.register(DvtvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DLFCorpusIE" {
                registry.register(DlfExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DLFIE" {
                registry.register(DlfExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DRBonanzaIE" {
                registry.register(DrBonanzaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DuoplayIE" {
                registry.register(DuoplayExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DeuxMIE" {
                registry.register(DeuxMExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DeuxMNewsIE" {
                registry.register(DeuxMExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DLiveVODIE" {
                registry.register(DliveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DLiveStreamIE" {
                registry.register(DliveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DailyWireIE" {
                registry.register(DailyWireExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DailyWirePodcastIE" {
                registry.register(DailyWireExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DropboxIE" {
                registry.register(DropboxExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DrTuberIE" {
                registry.register(DrTuberExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ERRJupiterIE" {
                registry.register(ErrJupiterExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ERRArhiivIE" {
                registry.register(ErrArhiivExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EUScreenIE" {
                registry.register(EuscreenExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ERTWebtvEmbedIE" {
                registry.register(ErtWebtvEmbedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ERTFlixCodenameIE" {
                registry.register(ErtflixCodenameExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ErocastIE" {
                registry.register(ErocastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EpidemicSoundIE" {
                registry.register(EpidemicSoundExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EpiconIE" {
                registry.register(EpiconExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EpiconSeriesIE" {
                registry.register(EpiconSeriesExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EpornerIE" {
                registry.register(EpornerExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EroProfileIE" {
                registry.register(EroProfileExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EroProfileAlbumIE" {
                registry.register(EroProfileAlbumExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AngelIE" {
                registry.register(AngelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NewsyIE" {
                registry.register(NewsyExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ClubicIE" {
                registry.register(ClubicExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MuenchenTVIE" {
                registry.register(MuenchenTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VODPlatformIE" {
                registry.register(VodPlatformExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AliExpressLiveIE" {
                registry.register(AliExpressLiveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FczenitIE" {
                registry.register(FczenitExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ClipchampIE" {
                registry.register(ClipchampExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BaiduVideoIE" {
                registry.register(BaiduVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FootyRoomIE" {
                registry.register(FootyRoomExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CharlieRoseIE" {
                registry.register(CharlieRoseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ElTreceTVIE" {
                registry.register(ElTreceTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Canalc2IE" {
                registry.register(Canalc2Extractor::new(descriptor)?)?;
            } else if descriptor.key == "EpochIE" {
                registry.register(EpochExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HarpodeonIE" {
                registry.register(HarpodeonExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AlibabaIE" {
                registry.register(AlibabaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MovingImageIE" {
                registry.register(MovingImageExtractor::new(descriptor)?)?;
            } else if descriptor.key == "TweakersIE" {
                registry.register(TweakersExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KrasViewIE" {
                registry.register(KrasViewExtractor::new(descriptor)?)?;
            } else if descriptor.key == "C56IE" {
                registry.register(C56Extractor::new(descriptor)?)?;
            } else if descriptor.key == "TassIE" {
                registry.register(TassExtractor::new(descriptor)?)?;
            } else if descriptor.key == "PhotobucketIE" {
                registry.register(PhotobucketExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NobelPrizeIE" {
                registry.register(NobelPrizeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CaltransIE" {
                registry.register(CaltransExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CozyTVIE" {
                registry.register(CozyTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LivestreamfailsIE" {
                registry.register(LivestreamfailsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MastersIE" {
                registry.register(MastersExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Mir24TvIE" {
                registry.register(Mir24TvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AcademicEarthCourseIE" {
                registry.register(AcademicEarthCourseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BloggerIE" {
                registry.register(BloggerExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MatchiTVIE" {
                registry.register(MatchiTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "PremiershipRugbyIE" {
                registry.register(PremiershipRugbyExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RadioDeIE" {
                registry.register(RadioDeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RadioZetPodcastIE" {
                registry.register(RadioZetPodcastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SztvHuIE" {
                registry.register(SztvHuExtractor::new(descriptor)?)?;
            } else if descriptor.key == "APAIE" {
                registry.register(ApaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ArnesIE" {
                registry.register(ArnesExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CJSWIE" {
                registry.register(CjswExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DaystarClipIE" {
                registry.register(DaystarClipExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DctpTvIE" {
                registry.register(DctpTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "StreamableIE" {
                registry.register(StreamableExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NewgroundsIE" {
                registry.register(NewgroundsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NewgroundsPlaylistIE" {
                registry.register(NewgroundsPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NewgroundsUserIE" {
                registry.register(NewgroundsUserExtractor::new(descriptor)?)?;
            } else if descriptor.key == "WistiaIE" {
                registry.register(WistiaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "WistiaPlaylistIE" {
                registry.register(WistiaPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "WistiaChannelIE" {
                registry.register(WistiaChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VidLiiIE" {
                registry.register(VidLiiExtractor::new(descriptor)?)?;
            } else if descriptor.key == "PeerTubeIE" {
                registry.register(PeerTubeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "PeerTubePlaylistIE" {
                registry.register(PeerTubePlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RumbleChannelIE" {
                registry.register(RumbleChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RumbleIE" {
                registry.register(RumbleExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SlideshareIE" {
                registry.register(SlideshareExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SoundgasmIE" {
                registry.register(SoundgasmExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SoundgasmProfileIE" {
                registry.register(SoundgasmProfileExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ImgurAlbumIE" {
                registry.register(ImgurGalleryExtractor::new(descriptor, false)?)?;
            } else if descriptor.key == "ImgurGalleryIE" {
                registry.register(ImgurGalleryExtractor::new(descriptor, true)?)?;
            } else if descriptor.key == "ImgurIE" {
                registry.register(ImgurExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NineGagIE" {
                registry.register(NineGagExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MyVidsterIE" {
                registry.register(MyVidsterExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlideIE" {
                registry.register(GlideExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EbayIE" {
                registry.register(EbayExtractor::new(descriptor)?)?;
            } else if descriptor.key == "SenIE" {
                registry.register(SenExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RoyaLiveIE" {
                registry.register(RoyaLiveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ReverbNationIE" {
                registry.register(ReverbNationExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EttuTvIE" {
                registry.register(EttuTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ElonetIE" {
                registry.register(ElonetExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GolemIE" {
                registry.register(GolemExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Screen9IE" {
                registry.register(Screen9Extractor::new(descriptor)?)?;
            } else if descriptor.key == "BildIE" {
                registry.register(BildExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FilmArchivIE" {
                registry.register(FilmArchivExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NetzkinoIE" {
                registry.register(NetzkinoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "UnistraIE" {
                registry.register(UnistraExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CinetecaMilanoIE" {
                registry.register(CinetecaMilanoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "NonkTubeIE" {
                registry.register(NonkTubeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LoveHomePornIE" {
                registry.register(LoveHomePornExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FathomIE" {
                registry.register(FathomExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EbaumsWorldIE" {
                registry.register(EbaumsWorldExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FuyinTVIE" {
                registry.register(FuyinTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CAM4IE" {
                registry.register(Cam4Extractor::new(descriptor)?)?;
            } else if descriptor.key == "KommunetvIE" {
                registry.register(KommunetvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "StreamCZIE" {
                registry.register(StreamCzExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VidyardIE" {
                registry.register(VidyardExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ArchiveOrgIE" {
                registry.register(ArchiveOrgExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BandcampIE" {
                registry.register(BandcampTrackExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BannedVideoIE" {
                registry.register(BannedVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "CoubIE" {
                registry.register(CoubExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoogleDriveIE" {
                registry.register(GoogleDriveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VocarooIE" {
                registry.register(VocarooExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreesoundIE" {
                registry.register(FreesoundExtractor::new(descriptor)?)?;
            } else if descriptor.key == "YandexDiskIE" {
                registry.register(YandexDiskExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RumbleEmbedIE" {
                registry.register(RumbleEmbedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudioBoomIE" {
                registry.register(AudioBoomExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BitChuteIE" {
                registry.register(BitChuteExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ClypIE" {
                registry.register(ClypExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BreitBartIE" {
                registry.register(BreitbartExtractor::new(descriptor)?)?;
            } else if matches!(descriptor.key.as_str(), "AudiusIE" | "AudiusTrackIE") {
                registry.register(AudiusExtractor::new(descriptor)?)?;
            } else if descriptor.key == "BlerpIE" {
                registry.register(BlerpExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ACastIE" {
                registry.register(AcastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ACastChannelIE" {
                registry.register(AcastChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "DumpertIE" {
                registry.register(DumpertExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudiodraftGenericIE" {
                registry.register(AudiodraftExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AudiomackIE" {
                registry.register(AudiomackExtractor::new(descriptor)?)?;
            } else if descriptor.key == "AitubeKZVideoIE" {
                registry.register(AitubeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ThisAmericanLifeIE" {
                registry.register(ThisAmericanLifeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "WorldStarHipHopIE" {
                registry.register(WorldStarHipHopExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Art19IE" {
                registry.register(Art19Extractor::new(descriptor)?)?;
            } else {
                registry.register(DescriptorExtractor::from_patterns_lossy(
                    descriptor,
                    &record.patterns,
                ))?;
            }
        }
        Ok(registry)
    }

    pub fn register<E>(&mut self, extractor: E) -> Result<(), ExtractorError>
    where
        E: InfoExtractor + 'static,
    {
        let key = extractor.descriptor().key.as_str();
        if self
            .extractors
            .iter()
            .any(|registered| registered.descriptor().key == key)
        {
            return Err(ExtractorError::new(
                ExtractorErrorKind::Extraction,
                format!("duplicate extractor key: {key}"),
            ));
        }
        self.extractors.push(Box::new(extractor));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.extractors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extractors.is_empty()
    }

    pub fn native_matchable_count(&self) -> usize {
        self.extractors
            .iter()
            .filter(|extractor| {
                extractor.pattern_count() > 0 && extractor.matcher_error_count() == 0
            })
            .count()
    }

    pub fn native_implementation_count(&self) -> usize {
        self.extractors
            .iter()
            .filter(|extractor| extractor.is_native())
            .count()
    }

    pub fn native_pattern_count(&self) -> usize {
        self.extractors
            .iter()
            .map(|extractor| extractor.native_matcher_count())
            .sum()
    }

    pub fn pattern_error_count(&self) -> usize {
        self.extractors
            .iter()
            .map(|extractor| extractor.matcher_error_count())
            .sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn InfoExtractor> {
        self.extractors.iter().map(Box::as_ref)
    }

    pub fn find(&self, url: &str) -> Option<&dyn InfoExtractor> {
        self.extractors
            .iter()
            .find(|extractor| extractor.suitable(url))
            .map(Box::as_ref)
    }

    pub fn extract(&self, url: &str) -> Result<InfoDict, ExtractorError> {
        let extractor = self.find(url).ok_or_else(|| {
            ExtractorError::new(
                ExtractorErrorKind::InvalidUrl,
                format!("no extractor found for URL: {url}"),
            )
        })?;
        extractor.extract(url)
    }
}
