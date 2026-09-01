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
            } else if descriptor.key == "IlPostIE" {
                registry.register(IlPostExtractor::new(descriptor)?)?;
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
            } else if descriptor.key == "IvideonIE" {
                registry.register(IvideonExtractor::new(descriptor)?)?;
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
            } else if descriptor.key == "ERTFlixIE" {
                registry.register(ErtflixExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ExpressenIE" {
                registry.register(ExpressenExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ESPNCricInfoIE" {
                registry.register(EspnCricinfoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ESPNArticleIE" {
                registry.register(EspnArticleExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ESPNIE" {
                registry.register(EspnExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ElPaisIE" {
                registry.register(ElPaisExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EuroParlWebstreamIE" {
                registry.register(EuroParlWebstreamExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EggheadCourseIE" {
                registry.register(EggheadCourseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EggheadLessonIE" {
                registry.register(EggheadLessonExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EggsIE" {
                registry.register(EggsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EggsArtistIE" {
                registry.register(EggsArtistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EmbedlyIE" {
                registry.register(EmbedlyExtractor::new(descriptor)?)?;
            } else if descriptor.key == "EuropeanTourIE" {
                registry.register(EuropeanTourExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FiveThirtyEightIE" {
                registry.register(FiveThirtyEightExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreespeechIE" {
                registry.register(FreespeechExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FoxNewsIE" {
                registry.register(FoxNewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FoxNewsVideoIE" {
                registry.register(FoxNewsVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FoxNewsArticleIE" {
                registry.register(FoxNewsArticleExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FazIE" {
                registry.register(FazExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Funker530IE" {
                registry.register(Funker530Extractor::new(descriptor)?)?;
            } else if descriptor.key == "FifaIE" {
                registry.register(FifaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FoxSportsIE" {
                registry.register(FoxSportsExtractor::new(descriptor)?)?;
            } else if matches!(
                descriptor.key.as_str(),
                "FourTubeIE" | "FuxIE" | "PornTubeIE" | "PornerBrosIE"
            ) {
                registry.register(FourTubeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FilmOnIE" {
                registry.register(FilmOnExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FilmOnChannelIE" {
                registry.register(FilmOnChannelExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FunkIE" {
                registry.register(FunkExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Formula1IE" {
                registry.register(Formula1Extractor::new(descriptor)?)?;
            } else if descriptor.key == "FptplayIE" {
                registry.register(FptplayExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FranceTVIE" {
                registry.register(FranceTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FranceTVSiteIE" {
                registry.register(FranceTvSiteExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FranceTVInfoIE" {
                registry.register(FranceTvInfoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FrontendMastersIE" {
                registry.register(FrontendMastersExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FrontendMastersLessonIE" {
                registry.register(FrontendMastersLessonExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FrontendMastersCourseIE" {
                registry.register(FrontendMastersCourseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoToStageIE" {
                registry.register(GoToStageExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GBNewsIE" {
                registry.register(GbNewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GaskrankIE" {
                registry.register(GaskrankExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GameSpotIE" {
                registry.register(GameSpotExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoProIE" {
                registry.register(GoProExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GronkhIE" {
                registry.register(GronkhExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GronkhFeedIE" {
                registry.register(GronkhFeedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GronkhVodsIE" {
                registry.register(GronkhVodsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoodGameIE" {
                registry.register(GoodGameExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlobalPlayerLiveIE" {
                registry.register(GlobalPlayerLiveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlobalPlayerLivePlaylistIE" {
                registry.register(GlobalPlayerLivePlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlobalPlayerAudioIE" {
                registry.register(GlobalPlayerAudioExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlobalPlayerAudioEpisodeIE" {
                registry.register(GlobalPlayerAudioEpisodeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlobalPlayerVideoIE" {
                registry.register(GlobalPlayerVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GameStarIE" {
                registry.register(GameStarExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GodResourceIE" {
                registry.register(GodResourceExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GodTubeIE" {
                registry.register(GodTubeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlomexEmbedIE" {
                registry.register(GlomexEmbedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GlomexIE" {
                registry.register(GlomexExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GiantBombIE" {
                registry.register(GiantBombExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GediDigitalIE" {
                registry.register(GediDigitalExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GrouponIE" {
                registry.register(GrouponExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GMANetworkVideoIE" {
                registry.register(GmaNetworkVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GermanupaIE" {
                registry.register(GermanupaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HitRecordIE" {
                registry.register(HitRecordExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HuajiaoIE" {
                registry.register(HuajiaoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HytaleIE" {
                registry.register(HytaleExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HearThisAtIE" {
                registry.register(HearThisAtExtractor::new(descriptor)?)?;
            } else if descriptor.key == "MonsterSirenHypergryphMusicIE" {
                registry.register(MonsterSirenHypergryphMusicExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HGTVComShowIE" {
                registry.register(HgtvComShowExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HuffPostIE" {
                registry.register(HuffPostExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HellPornoIE" {
                registry.register(HellPornoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HollywoodReporterIE" {
                registry.register(HollywoodReporterExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HollywoodReporterPlaylistIE" {
                registry.register(HollywoodReporterPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HRFernsehenIE" {
                registry.register(HrFernsehenExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HSEProductIE" {
                registry.register(HseProductExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HSEShowIE" {
                registry.register(HseShowExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HeiseIE" {
                registry.register(HeiseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HungamaIE" {
                registry.register(HungamaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HungamaSongIE" {
                registry.register(HungamaSongExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HungamaAlbumPlaylistIE" {
                registry.register(HungamaAlbumPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HBOIE" {
                registry.register(HboExtractor::new(descriptor)?)?;
            } else if descriptor.key == "HuyaVideoIE" {
                registry.register(HuyaVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "InaIE" {
                registry.register(InaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "InternazionaleIE" {
                registry.register(InternazionaleExtractor::new(descriptor)?)?;
            } else if descriptor.key == "IcareusIE" {
                registry.register(IcareusExtractor::new(descriptor)?)?;
            } else if descriptor.key == "IxiguaIE" {
                registry.register(IxiguaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ImdbIE" {
                registry.register(ImdbExtractor::new(descriptor)?)?;
            } else if descriptor.key == "InfoQIE" {
                registry.register(InfoqExtractor::new(descriptor)?)?;
            } else if descriptor.key == "IltalehtiIE" {
                registry.register(IltalehtiExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JStreamIE" {
                registry.register(JstreamExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JoveIE" {
                registry.register(JoveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JojIE" {
                registry.register(JojExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JTBCIE" {
                registry.register(JtbcExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JTBCProgramIE" {
                registry.register(JtbcProgramExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnAlbumIE" {
                registry.register(JioSaavnAlbumExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnArtistIE" {
                registry.register(JioSaavnArtistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnPlaylistIE" {
                registry.register(JioSaavnPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnShowIE" {
                registry.register(JioSaavnShowExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnShowPlaylistIE" {
                registry.register(JioSaavnShowPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JioSaavnSongIE" {
                registry.register(JioSaavnSongExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JamendoAlbumIE" {
                registry.register(JamendoAlbumExtractor::new(descriptor)?)?;
            } else if descriptor.key == "JamendoIE" {
                registry.register(JamendoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KakaoIE" {
                registry.register(KakaoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Kenh14PlaylistIE" {
                registry.register(Kenh14PlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Kenh14VideoIE" {
                registry.register(Kenh14VideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KankaNewsIE" {
                registry.register(KankaNewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KatsomoIE" {
                registry.register(KatsomoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KaraoketvIE" {
                registry.register(KaraoketvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KelbyOneIE" {
                registry.register(KelbyOneExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KhanAcademyIE" {
                registry.register(KhanAcademyExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KhanAcademyUnitIE" {
                registry.register(KhanAcademyUnitExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoAlbumIE" {
                registry.register(KuwoAlbumExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoCategoryIE" {
                registry.register(KuwoCategoryExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoChartIE" {
                registry.register(KuwoChartExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoIE" {
                registry.register(KuwoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoMvIE" {
                registry.register(KuwoMvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KuwoSingerIE" {
                registry.register(KuwoSingerExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LnkIE" {
                registry.register(LnkExtractor::new(descriptor)?)?;
            } else if descriptor.key == "Lecture2GoIE" {
                registry.register(Lecture2GoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LA7PodcastEpisodeIE" {
                registry.register(La7PodcastEpisodeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LA7PodcastIE" {
                registry.register(La7PodcastExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LecturioIE" {
                registry.register(LecturioExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LecturioCourseIE" {
                registry.register(LecturioCourseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LecturioDeCourseIE" {
                registry.register(LecturioDeCourseExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LiTVIE" {
                registry.register(LitvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LifeEmbedIE" {
                registry.register(LifeEmbedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LifeNewsIE" {
                registry.register(LifeNewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ManyVidsIE" {
                registry.register(ManyVidsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LRTStreamIE" {
                registry.register(LrtStreamExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LRTVODIE" {
                registry.register(LrtVodExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LRTRadioIE" {
                registry.register(LrtRadioExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LeFigaroVideoEmbedIE" {
                registry.register(LeFigaroVideoEmbedExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LeFigaroVideoSectionIE" {
                registry.register(LeFigaroVideoSectionExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LearningOnScreenIE" {
                registry.register(LearningOnScreenExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LoomIE" {
                registry.register(LoomExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LocipoIE" {
                registry.register(LocipoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LocipoPlaylistIE" {
                registry.register(LocipoPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LocoIE" {
                registry.register(LocoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LeIE" {
                registry.register(LeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LePlaylistIE" {
                registry.register(LePlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LemondeIE" {
                registry.register(LemondeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LentaIE" {
                registry.register(LentaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LibraryOfCongressIE" {
                registry.register(LibraryOfCongressExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LikeeIE" {
                registry.register(LikeeExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LikeeUserIE" {
                registry.register(LikeeUserExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LastFMIE" {
                registry.register(LastFmExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LastFMPlaylistIE" {
                registry.register(LastFmPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LastFMUserIE" {
                registry.register(LastFmUserExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LumniIE" {
                registry.register(LumniExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LEGOIE" {
                registry.register(LegoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "LibsynIE" {
                registry.register(LibsynExtractor::new(descriptor)?)?;
            } else if descriptor.key == "ListenNotesIE" {
                registry.register(ListenNotesExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KikaIE" {
                registry.register(KikaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KikaPlaylistIE" {
                registry.register(KikaPlaylistExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KickerIE" {
                registry.register(KickerExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KompasVideoIE" {
                registry.register(KompasVideoExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KickIE" {
                registry.register(KickExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KickVODIE" {
                registry.register(KickVodExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KickClipIE" {
                registry.register(KickClipExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KickStarterIE" {
                registry.register(KickstarterExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KukuluLiveIE" {
                registry.register(KukuluLiveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KalturaIE" {
                registry.register(KalturaExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KTHIE" {
                registry.register(KthExtractor::new(descriptor)?)?;
            } else if descriptor.key == "KinoPoiskIE" {
                registry.register(KinopoiskExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GabIE" {
                registry.register(GabExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GettrIE" {
                registry.register(GettrExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GettrStreamingIE" {
                registry.register(GettrStreamingExtractor::new(descriptor)?)?;
            } else if descriptor.key == "UplynkIE" {
                registry.register(UplynkExtractor::new(descriptor)?)?;
            } else if descriptor.key == "UplynkPreplayIE" {
                registry.register(UplynkPreplayExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FOX9IE" {
                registry.register(Fox9Extractor::new(descriptor)?)?;
            } else if descriptor.key == "FOX9NewsIE" {
                registry.register(Fox9NewsExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FaulioIE" {
                registry.register(FaulioExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FaulioLiveIE" {
                registry.register(FaulioLiveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FilmwebIE" {
                registry.register(FilmwebExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FiveTVIE" {
                registry.register(FiveTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FlickrIE" {
                registry.register(FlickrExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreeTvMoviesIE" {
                registry.register(FreeTvMoviesExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreeTvIE" {
                registry.register(FreeTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FlexTVIE" {
                registry.register(FlexTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FirstTVIE" {
                registry.register(FirstTvExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FirstTVLiveIE" {
                registry.register(FirstTvLiveExtractor::new(descriptor)?)?;
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
            } else if descriptor.key == "GoogleDriveFolderIE" {
                registry.register(GoogleDriveFolderExtractor::new(descriptor)?)?;
            } else if descriptor.key == "GoogleDriveIE" {
                registry.register(GoogleDriveExtractor::new(descriptor)?)?;
            } else if descriptor.key == "VocarooIE" {
                registry.register(VocarooExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FreesoundIE" {
                registry.register(FreesoundExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FrancaisFacileIE" {
                registry.register(FrancaisFacileExtractor::new(descriptor)?)?;
            } else if descriptor.key == "FranceCultureIE" {
                registry.register(FranceCultureExtractor::new(descriptor)?)?;
            } else if descriptor.key == "RadioFranceIE" {
                registry.register(RadioFranceExtractor::new(descriptor)?)?;
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
