//! Screenshot tests of the chrome at 1×, 1.5× and 2×: the viewer
//! with its toolbar, Settings, the command palette and the info panel, in dark and light, and
//! in Hebrew (right to left). Rendered offscreen on lavapipe and compared with the PNGs in
//! `tests/shots/`; see `specular-shots` for the tolerance and what a failure leaves behind.
//!
//! To accept an intended change: `SPECULAR_BLESS=1 cargo test -p specular-widgets --test shots`,
//! then review and commit the rewritten PNGs.
//!
//! One test runs every case in turn: the UI language is process-wide.

use specular_gfx::{DrawList, Icon, Rect, TexId, wgpu};
use specular_i18n::{I18n, LanguageIdentifier, isolate_ltr, langid, t};
use specular_input::{
    Action, ActionState, Capabilities, Capability, ImageState, Key, KeyCombo, Keymap, Mode, Mods,
    NamedKey, Profile,
};
use specular_metadata::{Field, FieldGroup, MetadataReport, SummaryGroup, SummaryItem};
use specular_settings::{ModelSize, Origin, Settings, SettingsStore};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};
use specular_widgets::{
    AiAddon, AiEngine, AiFeatureRow, AiLicence, AiModelInfo, AiPage, AiPerson, AiQuestion,
    AiRowState, AiSize, AiUnfit, CaliperKind, CheckState, Chrome, DicomTags, HudStats, HwReport,
    Input, InsightItem, InsightStage, Insights, Modal, Overlays, PageBadge, PaletteItem, PanelRow,
    Playback, PlotCell, PlotGrid, PlotSheet, PlotTrace, Presenting, ReportPath, ReportRow,
    ReportSection, ReportTable, ReportTask, SettingPanel, Studies, StudyKind, StudyLine, TagLine,
    Tone, VideoPlayback, ViewerState, WorkChip, WorkPanel, WorkRow, nice_ticks,
};
use specular_widgets::{CrashSheet, DiagEntry, DiagSheet, PlayerMenu};

/// Logical window size: small enough to keep the goldens small, big enough for Settings.
const SIZE: (f32, f32) = (720.0, 450.0);
const SCALES: [(f32, &str); 3] = [(1.0, "1x"), (1.5, "1.5x"), (2.0, "2x")];
/// The synthetic photo's size in pixels (4:3).
const PHOTO: (f32, f32) = (4000.0, 3000.0);

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Viewer,
    Settings,
    Palette,
    /// "Read codes": the codes found, listed in the palette's sheet.
    CodesList,
    /// "Find text": the lines read, listed in the palette's sheet.
    OcrList,
    Info,
    /// Performance insights over the viewer (1× and 1.5× only).
    Insights,
    /// The gallery with the "Why is this slow?" indicator above its bottom bar (1× and 1.5×
    /// only).
    GalleryIndicator,
    /// Performance insights about thumbnails, over the gallery, with hidden insights to show
    /// again (1× and 1.5× only).
    GalleryInsights,
    /// The keyboard on the toolbar: Tab, then two steps along it to Next (1× and 1.5× only).
    FocusToolbar,
    /// The zoom presets opened from the keyboard on the zoom readout, the second row
    /// selected (1× and 1.5× only).
    FocusZoomMenu,
    /// The keyboard on the first button of the insights panel (1× and 1.5× only).
    FocusInsights,
    /// The info panel's All fields: Shift+Tab onto the last row in view, then two rows up
    /// (1× and 1.5× only).
    FocusInfoRows,
    /// The gallery's indicator with the keyboard on it (F6 there; 1× and 1.5× only).
    FocusGalleryIndicator,
    /// Settings → Integration with "More file types" open under the default viewer: group
    /// tick boxes (one mixed), one group expanded (1× and 1.5× only).
    SettingsMoreTypes,
    /// Move to trash? with the keyboard on Cancel (1× and 1.5× only).
    FocusConfirm,
    /// Move to trash? with the keyboard on Move to trash, the filled default (1× and 1.5×
    /// only).
    FocusConfirmDefault,
    /// The zoom presets open over an Undo toast (as FocusZoomMenu; 1× and 1.5× only).
    ZoomMenuToast,
    /// Settings with the keyboard on its second control (1× and 1.5× only).
    FocusSettings,
    /// The palette with "zoom" typed and the keyboard on its Close button (1× and 1.5×
    /// only).
    FocusPalette,
    /// The language list with the keyboard on its Close button (1× and 1.5× only).
    FocusLanguage,
    /// About with the keyboard on its Close button, the push buttons' offset ring (1× and
    /// 1.5× only).
    FocusAbout,
    /// The insights about thumbnails with the keyboard on Show hidden insights again (two
    /// Tabs; 1× and 1.5× only).
    FocusGalleryInsightsRestore,
    /// Settings opened at Show hidden insights again, the keyboard on it (1× and 1.5× only).
    FocusSettingsRestore,
    /// Settings at Show hidden insights again, greyed out with nothing hidden (1× and 1.5×
    /// only).
    SettingsRestoreOff,
    /// Settings → Keys with the keyboard on a key of the third row (1× and 1.5× only).
    FocusKeys,
    /// Settings → Keys asking what to do with a key that runs another action (1× and 1.5×
    /// only).
    KeysConflict,
    /// The DICOM tag browser, a sequence expanded and its first item selected.
    DicomTags,
    /// The Studies browser of a patient's disc: patient data hidden, thumbnails, a series
    /// missing from the disc (1× and 1.5× only).
    Studies,
    /// The Studies browser with "Show patient data" switched on from the keyboard (1× and
    /// 1.5× only).
    StudiesPatient,
    /// The Studies browser while a disc without a DICOMDIR is read: the progress and
    /// Cancel in the footer (1× and 1.5× only).
    StudiesReading,
    /// The medium read: "Send to PACS…" in the footer.
    StudiesSend,
    /// The DICOM tag browser filtered ("pixel"), the keyboard on Show patient data (1× and
    /// 1.5× only).
    DicomTagsFilter,
    /// The DICOM tag browser filtered ("pixel"), the last three letters selected with
    /// Shift+Left (1× and 1.5× only).
    DicomTagsFilterSelect,
    /// The settings.toml editor with problems, the caret after a closing bracket (1× and
    /// 1.5× only).
    ConfigEditor,
    /// The settings.toml editor's find field (Ctrl+F, "pixel"), its last two letters
    /// selected with Shift+Left (1× and 1.5× only).
    ConfigEditorFind,
    /// The go-to field (Ctrl+G): "12" typed, its last digit selected with Shift+Left (1× and
    /// 1.5× only).
    GoToSelect,
    /// Settings → Advanced → Reset all settings… pressed from the keyboard: the confirmation,
    /// the keyboard on Cancel (1× and 1.5× only).
    ConfirmReset,
    /// Removing a Library location asks first: the folder named, the photos said to stay on
    /// disk, the keyboard on Cancel (1× and 1.5× only).
    ConfirmRemove,
    /// Adding the home folder to the Library asks first and offers Pictures: three buttons,
    /// the keyboard on "Add Pictures" (1× and 1.5× only).
    ConfirmHome,
    /// A slideshow on another display: the paused "Presenting on" control (1× and 1.5×
    /// only).
    Presenting,
    /// The viewer on another display, in the gallery (1× and 1.5× only).
    PresentingViewerGallery,
    /// The settings.toml editor after Ctrl+S was refused: the alert, the keyboard on the
    /// first problem (1× and 1.5× only).
    ConfigEditorRefused,
    /// The keymap.toml editor before the file exists: the effective keymap as exported.
    KeymapEditor,
    /// The keymap.toml editor with problems and the list of them, the keyboard on the
    /// first (1× and 1.5× only).
    KeymapEditorProblems,
    /// The hardware report (Settings → Advanced; 1× and 1.5× only).
    HwReport,
    /// The hardware report scrolled down to its path table (1× and 1.5× only).
    HwReportTable,
    /// The hardware report at text scale 2 (1× and 1.5× only).
    HwReportLargeText,
    /// Settings › Privacy with No network on, held by the system policy (1× and 1.5× only).
    PrivacyOff,
    /// Settings › Devices with a gamepad and an unreadable SpaceMouse (1× and 1.5× only).
    Devices,
    /// Renderer information with made-up adapter facts (1× and 1.5× only).
    RendererInfo,
    /// Renderer information scrolled to its Display section (1× and 1.5× only).
    RendererInfoDisplay,
    /// The keyboard cheat sheet opened in the gallery: the gallery's tab.
    CheatGallery,
    /// The cheat sheet opened while comparing: compare's tab.
    CheatCompare,
    /// The cheat sheet over a medical image (the keymap's medical layer on): its tab.
    CheatMedical,
    /// The Medical tab scrolled to the end: what is only in the command palette (1× and 1.5×
    /// only).
    CheatMedicalEnd,
    /// A multi-image file (an ICNS icon): the page stepper above the toolbar.
    Pages,
    /// A movie playing: the transport bar with its seek bar, time, loop and sound buttons.
    Video,
    /// A movie's full-screen play mode with its controls shown: no toolbar or panel, the
    /// transport at the bottom over the picture (the info panel open underneath).
    VideoFullscreen,
    /// A movie with its player menus: chapter and A-B marks on the seek bar, the subtitle
    /// menu open, a Hebrew subtitle over the picture.
    VideoSubtitles,
    /// The CC menu while subtitles are generated from the speech (the track chosen).
    VideoGenerated,
    /// The chapter menu open, an Arabic two-line subtitle over the picture.
    VideoChapters,
    /// The pointer over the seek bar: the time and chapter there.
    VideoHover,
    /// A movie resumed where it was left: the toast with Start over.
    VideoResumed,
    /// The volume slider pressed at 130 %: the knob there, its readout above (1× and 1.5×
    /// only).
    VideoVolume,
    /// A movie at 1.5×: the speed menu open from the speed on the bar (1× and 1.5× only).
    VideoSpeed,
    /// The picture adjustments (More › Picture…) with some set (1× and 1.5× only).
    VideoPicture,
    /// The sound menu: tracks, dialogue boost, mono, the equaliser, the output (1× and 1.5×
    /// only).
    VideoSound,
    /// The output menu (Sound › Output…): devices and speakers (1× and 1.5× only).
    VideoOutput,
    /// "Go to time…": the field over the time with a time typed (1× and 1.5× only).
    VideoGoTo,
    /// "Play on…" open with two devices found and still looking (1× and 1.5× only).
    VideoCast,
    /// Playing on a TV: the "Playing on" card, paused there (1× and 1.5× only).
    VideoCasting,
    /// Full screen: the zoom presets opened from the keyboard, the keyboard on the ticked
    /// Full screen entry (1× and 1.5× only).
    ZoomMenuFullscreen,
    /// The page stepper at text scale 2 (1× and 1.5× only).
    PagesLargeText,
    /// The object picker of a CERN ROOT file: the page stepper naming the object by its path
    /// and class (1× and 1.5× only).
    PagesRoot,
    /// Settings → Local AI in a build without models (1× and 1.5× only).
    AiNone,
    /// Settings → Local AI offering features, one downloading and one failed (1× and 1.5×
    /// only).
    AiOffers,
    /// Settings → Local AI with features installed and indexing (1× and 1.5× only).
    AiInstalled,
    /// Settings → Local AI scrolled to the People switch and the people found (1× and 1.5×
    /// only).
    AiPeople,
    AiFaces,
    /// Settings → Local AI offering features at text scale 2 (1× and 1.5× only).
    AiLargeText,
    /// Settings → Local AI at Search's model size, nothing installed (1× and 1.5× only).
    AiSizeOff,
    /// The model size with Standard installed and recommended, and a larger size suggested
    /// (1× and 1.5× only).
    AiSizeRecommended,
    /// The model size while Standard downloads (1× and 1.5× only).
    AiSizeDownloading,
    /// Switching from Standard to Large: Large indexing, Standard still searched (1× and 1.5×
    /// only).
    AiSizeSwitching,
    /// A switch to Large whose download failed (1× and 1.5× only).
    AiSizeFailed,
    /// Small chosen by name and installed: Standard offered, Large with the memory it lacks,
    /// and the way back to Recommended (1× and 1.5× only).
    AiSizeNamed,
    /// Text installed, its model setting: reading this library's text, the sizes and the
    /// script add-ons in every state (1× and 1.5× only).
    AiTextAddons,
    /// The first-run question, Medical checked (1× and 1.5× only).
    ProfileQuestion,
    /// Settings → Profile (1× and 1.5× only).
    SettingsProfile,
    /// A DICOM image with only the General profile on: the "Medical tools" chip above the
    /// toolbar (1× and 1.5× only).
    ProfileChip,
    /// False colour on: the colour bar (a CT window in HU) above the histogram, and HiLo's
    /// badge (DESIGN.md §6.2).
    ColourBar,
    /// Settings → Inspection → False colour map, its list opened from the keyboard (1× and
    /// 1.5× only).
    MapPicker,
    /// The colour-vision simulation (deuteranopia) over the whole window, its badge on top
    /// (1× and 1.5× only).
    Cvd,
    /// The info panel of an HP-GL plot, its group built by `specular-metadata` (1× and
    /// 1.5× only).
    InfoPlot,
    /// The info panel of a DXF drawing whose SHX fonts were partly found: SHX fonts and
    /// substituted fonts with their styles (1× and 1.5× only).
    InfoDrawingFonts,
    /// The info panel of LandXML survey data: coordinate system, surfaces, alignments (1×
    /// and 1.5× only).
    InfoSurvey,
    /// The info panel of a protein crystal structure, its Structure group built by
    /// `specular-metadata` (1× and 1.5× only).
    InfoStructure,
    /// The info panel of a thermal still, its Thermal group built by `specular-metadata`
    /// (1× and 1.5× only).
    InfoThermal,
    /// The info panel of a CERN ROOT histogram, its ROOT group built by `specular-metadata`
    /// (1× and 1.5× only).
    InfoRoot,
    /// A structured report in the tag browser: its content tree, image jumps (1× and 1.5×
    /// only).
    SrTree,
    /// Dose volume histograms with their table (1× and 1.5× only).
    PlotDvh,
    /// A 12-lead ECG on mm paper at 25 mm/s and 10 mm/mV (1× and 1.5× only).
    PlotEcg,
    /// The window with nothing open: the start screen in a short window (compact entry
    /// points; the tips left out for room; 1× and 1.5× only).
    Empty,
    /// The start screen in a taller window with its "Things to try" cards, General profile
    /// only (1× and 1.5× only).
    Welcome,
    /// The start screen with recent folders (1× and 1.5× only).
    WelcomeRecents,
    /// The start screen with its tips hidden: the line about the keys instead (1× and 1.5×
    /// only).
    WelcomeTipsHidden,
    /// The start screen with the Medical profile on: its card among the tips (1× and 1.5×
    /// only).
    WelcomeMedical,
    /// A one-time hint over the viewer: "Press G to see this folder as a gallery" (1× and
    /// 1.5× only).
    Hint,
    /// The manual at the viewer's page, at its Zooming section (its paragraphs, lists and
    /// links).
    HelpPage,
    /// The manual searched for "crash report".
    HelpSearch,
    /// The manual's contents.
    HelpContents,
    /// About: version, build, Help and Save diagnostics bundle….
    About,
    /// The crash prompt with the report shown.
    Crash,
    /// The diagnostics bundle's preview.
    Diagnostics,
    /// The empty window while a file is dragged over it (1× and 1.5× only).
    DropHover,
    /// A file still opening two seconds on: the spinner and its name (1× and 1.5× only).
    Opening,
    /// A file that cannot be opened, with the reason (1× and 1.5× only).
    OpenError,
    /// A file too big for the free memory: the needs-memory reason (1× and 1.5× only).
    OpenErrorMemory,
    /// A Blackmagic RAW clip without the vendor's decoder: the "needs a free decoder" card
    /// with Get it, I've installed it and Locate… (1× and 1.5× only).
    OpenErrorDecoder,
    /// A file a decoder panicked on (caught at the decode boundary): the internal-error
    /// reason, the same card as a damaged file (1× and 1.5× only).
    OpenErrorInternal,
    /// The background work indicator above the viewer's toolbar (1× and 1.5× only).
    WorkChip,
    /// The indicator in the gallery's top bar, the main job paused (1× and 1.5× only).
    GalleryWork,
    /// The "Background work" panel: a job with its time left, a paused one, a download
    /// still estimating (1× and 1.5× only).
    Work,
    /// The panel with large text (1× and 1.5× only).
    WorkLargeText,
    /// Downloads in full beside the toolbar of a wide window (1× and 1.5× only).
    DownloadsInline,
    /// A narrow window: the small downloads indicator above the toolbar (1× and 1.5× only).
    DownloadsNarrow,
    /// The toolbar hidden (a slideshow): only the small indicator (1× and 1.5× only).
    DownloadsHiddenBar,
    /// The "Downloads" panel (1× and 1.5× only).
    DownloadsPanel,
    /// The panel with a DICOM series retrieved by C-GET, counted by image (1× and 1.5×
    /// only).
    DownloadsRetrieve,
    /// A download on one line in the gallery's bottom bar (1× and 1.5× only).
    GalleryDownloads,
    /// Settings → Library: its text fields, the keyboard on the first with text typed at
    /// its end (1× and 1.5× only).
    SettingsFields,
    /// Settings → Viewing at the subtitles' custom colour: its hex field, the colour beside
    /// it and the swatches under it, white in use (1× and 1.5× only).
    SettingsColour,
    /// Settings → Import: a folder (Choose… and Clear), and a rename pattern refused with
    /// the reason under it (1× and 1.5× only).
    SettingsImport,
    /// Settings → Colour with an OCIO config: its display, view and input colour space
    /// offered as the config's names (1× and 1.5× only).
    SettingsOcio,
    /// The play queue panel over a sound file: the item playing, lengths, a network item,
    /// repeat all and shuffle on (1× and 1.5× only).
    QueuePanel,
    /// A sound file's playback bar: previous and next item, repeat one, shuffle and the
    /// queue (1× and 1.5× only).
    SoundTransport,
    /// The narrowest window (360 px): the buttons that don't fit, Discord first, in the
    /// open More menu (1× and 1.5× only).
    ToolbarNarrow,
}

impl State {
    /// Shot at 1× and 1.5× only.
    fn small(self) -> bool {
        matches!(
            self,
            State::QueuePanel
                | State::SoundTransport
                | State::ToolbarNarrow
                | State::Insights
                | State::VideoResumed
                | State::VideoVolume
                | State::VideoSpeed
                | State::VideoPicture
                | State::VideoSound
                | State::VideoOutput
                | State::VideoGoTo
                | State::VideoCast
                | State::VideoCasting
                | State::GalleryIndicator
                | State::GalleryInsights
                | State::FocusToolbar
                | State::FocusZoomMenu
                | State::ZoomMenuFullscreen
                | State::FocusInsights
                | State::FocusInfoRows
                | State::FocusGalleryIndicator
                | State::SettingsMoreTypes
                | State::CheatMedicalEnd
                | State::FocusConfirm
                | State::FocusConfirmDefault
                | State::ZoomMenuToast
                | State::FocusSettings
                | State::FocusPalette
                | State::FocusLanguage
                | State::FocusAbout
                | State::FocusGalleryInsightsRestore
                | State::FocusSettingsRestore
                | State::SettingsRestoreOff
                | State::FocusKeys
                | State::KeysConflict
                | State::DicomTagsFilter
                | State::DicomTagsFilterSelect
                | State::Studies
                | State::StudiesPatient
                | State::StudiesReading
                | State::StudiesSend
                | State::ConfigEditor
                | State::ConfigEditorFind
                | State::GoToSelect
                | State::ConfigEditorRefused
                | State::KeymapEditorProblems
                | State::ConfirmReset
                | State::ConfirmRemove
                | State::ConfirmHome
                | State::Presenting
                | State::PresentingViewerGallery
                | State::HwReport
                | State::HwReportTable
                | State::HwReportLargeText
                | State::PrivacyOff
                | State::Devices
                | State::RendererInfo
                | State::RendererInfoDisplay
                | State::PagesLargeText
                | State::AiNone
                | State::AiOffers
                | State::AiInstalled
                | State::AiPeople
                | State::AiFaces
                | State::AiLargeText
                | State::AiSizeOff
                | State::AiSizeRecommended
                | State::AiSizeDownloading
                | State::AiSizeSwitching
                | State::AiSizeFailed
                | State::AiSizeNamed
                | State::ProfileQuestion
                | State::SettingsProfile
                | State::ProfileChip
                | State::InfoPlot
                | State::InfoDrawingFonts
                | State::InfoSurvey
                | State::InfoStructure
                | State::InfoThermal
                | State::InfoRoot
                | State::PagesRoot
                | State::MapPicker
                | State::Cvd
                | State::SrTree
                | State::PlotDvh
                | State::PlotEcg
                | State::Empty
                | State::Welcome
                | State::WelcomeRecents
                | State::WelcomeTipsHidden
                | State::WelcomeMedical
                | State::Hint
                | State::DropHover
                | State::Opening
                | State::OpenError
                | State::OpenErrorMemory
                | State::OpenErrorDecoder
                | State::OpenErrorInternal
                | State::WorkChip
                | State::GalleryWork
                | State::Work
                | State::WorkLargeText
                | State::DownloadsInline
                | State::DownloadsNarrow
                | State::DownloadsHiddenBar
                | State::DownloadsPanel
                | State::DownloadsRetrieve
                | State::GalleryDownloads
                | State::SettingsFields
                | State::SettingsColour
                | State::SettingsImport
                | State::SettingsOcio
        )
    }

    /// The start screen.
    fn welcome(self) -> bool {
        matches!(
            self,
            State::Empty
                | State::DropHover
                | State::Welcome
                | State::WelcomeRecents
                | State::WelcomeTipsHidden
                | State::WelcomeMedical
        )
    }

    /// The window's logical size: taller for the start screen with all its parts.
    fn size(self) -> (f32, f32) {
        match self {
            State::Welcome
            | State::WelcomeRecents
            | State::WelcomeTipsHidden
            | State::WelcomeMedical => (720.0, 620.0),
            State::DownloadsInline => (1280.0, 450.0),
            State::DownloadsNarrow => (480.0, 450.0),
            // The window's minimum size.
            State::ToolbarNarrow => (360.0, 260.0),
            State::GalleryDownloads => (960.0, 450.0),
            // Room for the volume slider beside the transport's buttons.
            State::VideoVolume => (800.0, SIZE.1),
            _ => SIZE,
        }
    }

    fn gallery(self) -> bool {
        self.gallery_scene() && !matches!(self, State::CheatGallery | State::CheatCompare)
    }

    /// The gallery is behind the chrome.
    fn gallery_scene(self) -> bool {
        matches!(self, State::PresentingViewerGallery)
            || matches!(
                self,
                State::CheatGallery
                    | State::CheatCompare
                    | State::GalleryIndicator
                    | State::GalleryInsights
                    | State::FocusGalleryIndicator
                    | State::FocusGalleryInsightsRestore
                    | State::GalleryWork
                    | State::GalleryDownloads
            )
    }
}

/// Settings → Performance → "Show hidden insights again".
const RESTORE: &str = "performance.insights_restore";
/// Settings → Advanced → Reset all settings….
const RESET_ALL: &str = "advanced.reset_all";
/// Settings → Keys.
const KEYS: &str = "keys.bindings";
/// Settings → Inspection → False colour map.
const COLOUR_MAP: &str = "inspection.false_colour_map";
/// From the search field to the list of Settings → Keys: the three modes, the four presets,
/// Import, Export, Export this mode, Edit keymap.toml, then the first row.
const TABS_TO_KEY_LIST: usize = 3 + 4 + 4 + 1;

/// What the settings.toml editor shows: one of every kind of token, and problems.
const CONFIG: &str = r#"# Specular settings, edited by hand
version = 2

[general]
theme = "dark"

[viewing]
initial_zoom = "fit"
backround = "black"
pixel_grid_threshold = 9000
toolbar_autohide_delay = 1.5
toolbar_items = ["prev", "next", "zoom_level"]

[browsing]
loop_folder = true
"#;

/// Where keymap.toml lives in the keymap editor's header.
const KEYMAP_PATH: &str = "/home/ana/.config/specular/keymap.toml";

/// A keymap.toml edited by hand: unknown actions (one only misspelt), a key that doesn't
/// parse, a key the system keeps, and a sequence behind a single key.
const KEYMAP: &str = r#"# Specular key bindings, edited by hand
[viewer]
"Ctrl+Shift+i" = "infoo"
"Ctrl+Hyper+x" = "next"
"Alt+F4" = "quit"
"=" = "zoom_in"
"g" = "zoom-fit"
"g g" = "next"
"#;
/// Arrow-key steps (48 px each) from the top of the hardware report to its path table.
const REPORT_TABLE_STEPS: usize = 23;
/// Arrow-down steps of the renderer information to its Display section.
const RENDERER_DISPLAY_STEPS: usize = 10;

/// A file that cannot be opened, as the app words it (`error_text::decode_reason`).
const OPEN_ERROR: &str = "Can’t open IMG_0042.jpg: the file is damaged or isn’t what its name \
                          says (bad Huffman table).";

/// A file too big for the free memory, as the app words it (`error_text::needs_memory`).
const OPEN_ERROR_MEMORY: &str = "Can’t open IMG_0042.jpg: it needs 2.4 GB of free RAM, but only \
                                 1.1 GB is free right now. Close some other apps and try again, \
                                 or open it on a computer with more memory.";

/// A Blackmagic RAW clip without its decoder, as the app words it (`app/optional.rs`).
/// A decoder panic on the file, as the app words it (`error-reason-internal`).
const OPEN_ERROR_INTERNAL: &str = "Can’t open IMG_0042.jpg: something went wrong inside \
                                   Specular. The log has the details.";

const OPEN_ERROR_DECODER: &str = "Can’t open A001_C002.braw: Blackmagic RAW files need the free \
                                  Blackmagic RAW SDK from Blackmagic Design, which is not \
                                  installed.";

/// The gallery's bottom bar (as the app's gallery draws it).
const GALLERY_BAR: f32 = 44.0;

struct Case {
    name: &'static str,
    state: State,
    dark: bool,
    lang: LanguageIdentifier,
}

fn cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    let mut v = vec![
        c("help-page-light", State::HelpPage, false, langid!("en-US")),
        c("help-page-dark", State::HelpPage, true, langid!("en-US")),
        c(
            "help-search-light",
            State::HelpSearch,
            false,
            langid!("en-US"),
        ),
        c(
            "help-search-dark",
            State::HelpSearch,
            true,
            langid!("en-US"),
        ),
        c(
            "help-contents-dark",
            State::HelpContents,
            true,
            langid!("en-US"),
        ),
        c("about-light", State::About, false, langid!("en-US")),
        c("about-dark", State::About, true, langid!("en-US")),
        c("empty-dark", State::Empty, true, langid!("en-US")),
        c("empty-he-light", State::Empty, false, langid!("he")),
        c("welcome-light", State::Welcome, false, langid!("en-US")),
        c("welcome-dark", State::Welcome, true, langid!("en-US")),
        c(
            "welcome-recents-dark",
            State::WelcomeRecents,
            true,
            langid!("en-US"),
        ),
        c(
            "welcome-recents-he-light",
            State::WelcomeRecents,
            false,
            langid!("he"),
        ),
        c(
            "welcome-tips-hidden-light",
            State::WelcomeTipsHidden,
            false,
            langid!("en-US"),
        ),
        c(
            "welcome-medical-dark",
            State::WelcomeMedical,
            true,
            langid!("en-US"),
        ),
        c("hint-dark", State::Hint, true, langid!("en-US")),
        c(
            "drop-hover-light",
            State::DropHover,
            false,
            langid!("en-US"),
        ),
        c("opening-dark", State::Opening, true, langid!("en-US")),
        c(
            "open-error-light",
            State::OpenError,
            false,
            langid!("en-US"),
        ),
        c(
            "open-error-memory-dark",
            State::OpenErrorMemory,
            true,
            langid!("en-US"),
        ),
        c(
            "open-error-decoder-light",
            State::OpenErrorDecoder,
            false,
            langid!("en-US"),
        ),
        c(
            "open-error-internal-dark",
            State::OpenErrorInternal,
            true,
            langid!("en-US"),
        ),
        c(
            "gallery-work-light",
            State::GalleryWork,
            false,
            langid!("en-US"),
        ),
        c(
            "gallery-work-he-dark",
            State::GalleryWork,
            true,
            langid!("he"),
        ),
        c("colour-bar-dark", State::ColourBar, true, langid!("en-US")),
        c(
            "colour-bar-he-light",
            State::ColourBar,
            false,
            langid!("he"),
        ),
        c("map-picker-dark", State::MapPicker, true, langid!("en-US")),
        c("map-picker-he-dark", State::MapPicker, true, langid!("he")),
        c("cvd-dark", State::Cvd, true, langid!("en-US")),
        c("cvd-he-light", State::Cvd, false, langid!("he")),
        c("presenting-dark", State::Presenting, true, langid!("en-US")),
        c(
            "presenting-he-light",
            State::Presenting,
            false,
            langid!("he"),
        ),
        c(
            "presenting-viewer-gallery-light",
            State::PresentingViewerGallery,
            false,
            langid!("en-US"),
        ),
        c(
            "config-editor-dark",
            State::ConfigEditor,
            true,
            langid!("en-US"),
        ),
        c(
            "config-editor-refused-light",
            State::ConfigEditorRefused,
            false,
            langid!("en-US"),
        ),
        c(
            "config-editor-he-dark",
            State::ConfigEditor,
            true,
            langid!("he"),
        ),
        c(
            "keymap-editor-dark",
            State::KeymapEditor,
            true,
            langid!("en-US"),
        ),
        c(
            "keymap-editor-he-dark",
            State::KeymapEditor,
            true,
            langid!("he"),
        ),
        c(
            "keymap-editor-problems-dark",
            State::KeymapEditorProblems,
            true,
            langid!("en-US"),
        ),
        c("pages-dark", State::Pages, true, langid!("en-US")),
        c("video-dark", State::Video, true, langid!("en-US")),
        c("video-light", State::Video, false, langid!("en-US")),
        c("video-he-light", State::Video, false, langid!("he")),
        c("queue-dark", State::QueuePanel, true, langid!("en-US")),
        c("queue-he-light", State::QueuePanel, false, langid!("he")),
        c(
            "sound-transport-dark",
            State::SoundTransport,
            true,
            langid!("en-US"),
        ),
        c(
            "sound-transport-he-light",
            State::SoundTransport,
            false,
            langid!("he"),
        ),
        c(
            "video-subtitles-light",
            State::VideoSubtitles,
            false,
            langid!("en-US"),
        ),
        c(
            "video-subtitles-he-light",
            State::VideoSubtitles,
            false,
            langid!("he"),
        ),
        c(
            "video-generated-dark",
            State::VideoGenerated,
            true,
            langid!("en-US"),
        ),
        c(
            "video-generated-he-light",
            State::VideoGenerated,
            false,
            langid!("he"),
        ),
        c(
            "video-chapters-dark",
            State::VideoChapters,
            true,
            langid!("en-US"),
        ),
        c(
            "video-chapters-he-light",
            State::VideoChapters,
            false,
            langid!("he"),
        ),
        c(
            "video-hover-dark",
            State::VideoHover,
            true,
            langid!("en-US"),
        ),
        c(
            "video-hover-he-light",
            State::VideoHover,
            false,
            langid!("he"),
        ),
        c(
            "video-volume-dark",
            State::VideoVolume,
            true,
            langid!("en-US"),
        ),
        c(
            "video-volume-he-light",
            State::VideoVolume,
            false,
            langid!("he"),
        ),
        c(
            "video-speed-dark",
            State::VideoSpeed,
            true,
            langid!("en-US"),
        ),
        c(
            "video-speed-he-light",
            State::VideoSpeed,
            false,
            langid!("he"),
        ),
        c(
            "video-picture-dark",
            State::VideoPicture,
            true,
            langid!("en-US"),
        ),
        c(
            "video-picture-he-light",
            State::VideoPicture,
            false,
            langid!("he"),
        ),
        c(
            "video-sound-dark",
            State::VideoSound,
            true,
            langid!("en-US"),
        ),
        c(
            "video-sound-he-light",
            State::VideoSound,
            false,
            langid!("he"),
        ),
        c(
            "video-output-dark",
            State::VideoOutput,
            true,
            langid!("en-US"),
        ),
        c(
            "video-output-he-light",
            State::VideoOutput,
            false,
            langid!("he"),
        ),
        c("video-goto-dark", State::VideoGoTo, true, langid!("en-US")),
        c("video-cast-dark", State::VideoCast, true, langid!("en-US")),
        c(
            "video-cast-he-light",
            State::VideoCast,
            false,
            langid!("he"),
        ),
        c(
            "video-casting-dark",
            State::VideoCasting,
            true,
            langid!("en-US"),
        ),
        c(
            "video-casting-he-light",
            State::VideoCasting,
            false,
            langid!("he"),
        ),
        c(
            "video-goto-he-light",
            State::VideoGoTo,
            false,
            langid!("he"),
        ),
        c(
            "video-resumed-dark",
            State::VideoResumed,
            true,
            langid!("en-US"),
        ),
        c(
            "video-resumed-he-light",
            State::VideoResumed,
            false,
            langid!("he"),
        ),
        c(
            "video-fullscreen-dark",
            State::VideoFullscreen,
            true,
            langid!("en-US"),
        ),
        c(
            "video-fullscreen-he-light",
            State::VideoFullscreen,
            false,
            langid!("he"),
        ),
        c(
            "zoom-menu-fullscreen-light",
            State::ZoomMenuFullscreen,
            false,
            langid!("en-US"),
        ),
        c(
            "he-zoom-menu-fullscreen-light",
            State::ZoomMenuFullscreen,
            false,
            langid!("he"),
        ),
        c("pages-light", State::Pages, false, langid!("en-US")),
        c("pages-he-dark", State::Pages, true, langid!("he")),
        c(
            "pages-text-2-light",
            State::PagesLargeText,
            false,
            langid!("en-US"),
        ),
        c("viewer-dark", State::Viewer, true, langid!("en-US")),
        c(
            "profile-question-dark",
            State::ProfileQuestion,
            true,
            langid!("en-US"),
        ),
        c(
            "profile-question-light",
            State::ProfileQuestion,
            false,
            langid!("en-US"),
        ),
        c(
            "he-profile-question-dark",
            State::ProfileQuestion,
            true,
            langid!("he"),
        ),
        // The five profiles, right to left in a light theme.
        c(
            "he-profile-question-light",
            State::ProfileQuestion,
            false,
            langid!("he"),
        ),
        c(
            "settings-profile-dark",
            State::SettingsProfile,
            true,
            langid!("en-US"),
        ),
        c(
            "profile-chip-dark",
            State::ProfileChip,
            true,
            langid!("en-US"),
        ),
        c("viewer-light", State::Viewer, false, langid!("en-US")),
        c(
            "toolbar-narrow-dark",
            State::ToolbarNarrow,
            true,
            langid!("en-US"),
        ),
        c(
            "toolbar-narrow-light",
            State::ToolbarNarrow,
            false,
            langid!("en-US"),
        ),
        c("settings-dark", State::Settings, true, langid!("en-US")),
        c(
            "settings-more-types-dark",
            State::SettingsMoreTypes,
            true,
            langid!("en-US"),
        ),
        c(
            "he-settings-more-types-light",
            State::SettingsMoreTypes,
            false,
            langid!("he"),
        ),
        c("palette-light", State::Palette, false, langid!("en-US")),
        c("codes-list-dark", State::CodesList, true, langid!("en-US")),
        c("ocr-list-dark", State::OcrList, true, langid!("en-US")),
        c("he-ocr-list-light", State::OcrList, false, langid!("he")),
        c(
            "he-codes-list-light",
            State::CodesList,
            false,
            langid!("he"),
        ),
        c(
            "gallery-indicator-light",
            State::GalleryIndicator,
            false,
            langid!("en-US"),
        ),
        c(
            "gallery-insights-dark",
            State::GalleryInsights,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-toolbar-dark",
            State::FocusToolbar,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-zoom-menu-light",
            State::FocusZoomMenu,
            false,
            langid!("en-US"),
        ),
        c(
            "focus-insights-dark",
            State::FocusInsights,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-info-rows-dark",
            State::FocusInfoRows,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-gallery-indicator-light",
            State::FocusGalleryIndicator,
            false,
            langid!("en-US"),
        ),
        c(
            "focus-confirm-dark",
            State::FocusConfirm,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-confirm-default-light",
            State::FocusConfirmDefault,
            false,
            langid!("en-US"),
        ),
        c(
            "zoom-menu-toast-dark",
            State::ZoomMenuToast,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-settings-light",
            State::FocusSettings,
            false,
            langid!("en-US"),
        ),
        c(
            "he-focus-toolbar-light",
            State::FocusToolbar,
            false,
            langid!("he"),
        ),
        c(
            "focus-palette-dark",
            State::FocusPalette,
            true,
            langid!("en-US"),
        ),
        c(
            "focus-language-light",
            State::FocusLanguage,
            false,
            langid!("en-US"),
        ),
        c(
            "he-focus-palette-light",
            State::FocusPalette,
            false,
            langid!("he"),
        ),
        c(
            "he-focus-language-dark",
            State::FocusLanguage,
            true,
            langid!("he"),
        ),
        c(
            "focus-about-dark",
            State::FocusAbout,
            true,
            langid!("en-US"),
        ),
        c(
            "he-focus-about-light",
            State::FocusAbout,
            false,
            langid!("he"),
        ),
        c(
            "pages-root-light",
            State::PagesRoot,
            false,
            langid!("en-US"),
        ),
        c("he-settings-dark", State::Settings, true, langid!("he")),
        c(
            "settings-fields-dark",
            State::SettingsFields,
            true,
            langid!("en-US"),
        ),
        c(
            "he-settings-fields-light",
            State::SettingsFields,
            false,
            langid!("he"),
        ),
        c(
            "settings-colour-dark",
            State::SettingsColour,
            true,
            langid!("en-US"),
        ),
        c(
            "he-settings-colour-light",
            State::SettingsColour,
            false,
            langid!("he"),
        ),
        c(
            "settings-ocio-light",
            State::SettingsOcio,
            false,
            langid!("en-US"),
        ),
        c(
            "he-settings-ocio-dark",
            State::SettingsOcio,
            true,
            langid!("he"),
        ),
        c(
            "settings-import-light",
            State::SettingsImport,
            false,
            langid!("en-US"),
        ),
        c(
            "he-settings-import-dark",
            State::SettingsImport,
            true,
            langid!("he"),
        ),
        c(
            "focus-gallery-insights-restore-light",
            State::FocusGalleryInsightsRestore,
            false,
            langid!("en-US"),
        ),
        c(
            "focus-settings-restore-dark",
            State::FocusSettingsRestore,
            true,
            langid!("en-US"),
        ),
        c(
            "settings-restore-off-light",
            State::SettingsRestoreOff,
            false,
            langid!("en-US"),
        ),
        c("focus-keys-dark", State::FocusKeys, true, langid!("en-US")),
        c(
            "he-focus-keys-light",
            State::FocusKeys,
            false,
            langid!("he"),
        ),
        c(
            "keys-conflict-light",
            State::KeysConflict,
            false,
            langid!("en-US"),
        ),
        c(
            "confirm-reset-light",
            State::ConfirmReset,
            false,
            langid!("en-US"),
        ),
        c(
            "he-confirm-reset-dark",
            State::ConfirmReset,
            true,
            langid!("he"),
        ),
        c(
            "confirm-remove-dark",
            State::ConfirmRemove,
            true,
            langid!("en-US"),
        ),
        c(
            "he-confirm-remove-light",
            State::ConfirmRemove,
            false,
            langid!("he"),
        ),
        c(
            "confirm-home-dark",
            State::ConfirmHome,
            true,
            langid!("en-US"),
        ),
        c(
            "cheatsheet-gallery-dark",
            State::CheatGallery,
            true,
            langid!("en-US"),
        ),
        c(
            "cheatsheet-compare-dark",
            State::CheatCompare,
            true,
            langid!("en-US"),
        ),
        c(
            "cheatsheet-medical-dark",
            State::CheatMedical,
            true,
            langid!("en-US"),
        ),
        c(
            "cheatsheet-medical-end-dark",
            State::CheatMedicalEnd,
            true,
            langid!("en-US"),
        ),
        c(
            "he-cheatsheet-gallery-dark",
            State::CheatGallery,
            true,
            langid!("he"),
        ),
        c(
            "config-editor-find-dark",
            State::ConfigEditorFind,
            true,
            langid!("en-US"),
        ),
        c(
            "config-editor-find-he-light",
            State::ConfigEditorFind,
            false,
            langid!("he"),
        ),
        c(
            "go-to-select-dark",
            State::GoToSelect,
            true,
            langid!("en-US"),
        ),
        c(
            "go-to-select-he-light",
            State::GoToSelect,
            false,
            langid!("he"),
        ),
    ];
    v.extend(medical_shot_cases());
    v.extend(ai_shot_cases());
    v.extend(info_shot_cases());
    v.extend(report_shot_cases());
    v.extend(work_shot_cases());
    v.extend(net_shot_cases());
    v.extend(privacy_shot_cases());
    v.extend(devices_shot_cases());
    v
}

/// The start screen's recent folders: four, the last without a thumbnail yet.
fn welcome_recents(thumbs: &[(wgpu::Texture, (TexId, u32, u32))]) -> specular_widgets::Welcome {
    let names = [
        ("Iceland 2026", "~/Pictures"),
        ("CT abdomen", "~/Studies"),
        ("Scans", "~/Documents"),
        ("Wedding", "/media/ana/Archive"),
    ];
    specular_widgets::Welcome {
        recents: names
            .iter()
            .enumerate()
            .map(|(i, (name, parent))| specular_widgets::RecentFolder {
                path: std::path::PathBuf::from(format!("{parent}/{name}")),
                name: (*name).into(),
                parent: (*parent).into(),
                thumb: (i < 3).then(|| thumbs[i % thumbs.len()].1),
            })
            .collect(),
        pictures_offer: None,
    }
}

/// Where the photo sits: fitted into the window with a margin.
fn photo_rect(size: (f32, f32)) -> Rect {
    let (w, h) = size;
    let s = ((w - 48.0) / PHOTO.0).min((h - 48.0) / PHOTO.1);
    Rect::new(0.0, 0.0, w, h).centered(PHOTO.0 * s, PHOTO.1 * s)
}

/// A synthetic landscape (sky, sun, hills, lake) standing in for the image: flat shapes, so
/// the goldens stay small, with enough colour for the glass to blur.
fn scene(dl: &mut DrawList, pal: &Palette, size: (f32, f32)) {
    let (w, h) = size;
    dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
    let r = photo_rect(size);
    dl.push_clip(r);
    dl.rect(r, 0.0, Color::hex(0x6fa8dc));
    dl.rect(
        Rect::new(r.x, r.y, r.w, r.h * 0.35),
        0.0,
        Color::hex(0x3d78b8),
    );
    dl.circle(
        (r.x + r.w * 0.72, r.y + r.h * 0.28),
        r.h * 0.1,
        Color::hex(0xffd966),
    );
    dl.circle(
        (r.x + r.w * 0.2, r.y + r.h * 1.15),
        r.h * 0.6,
        Color::hex(0x38761d),
    );
    dl.circle(
        (r.x + r.w * 0.75, r.y + r.h * 1.25),
        r.h * 0.65,
        Color::hex(0x6aa84f),
    );
    dl.rect(
        Rect::new(r.x, r.y + r.h * 0.82, r.w, r.h * 0.18),
        0.0,
        Color::hex(0x1c4587),
    );
    dl.pop_clip();
}

/// A gallery stand-in: a grid of flat tiles under a top bar, and the bottom bar.
fn gallery_scene(dl: &mut DrawList, pal: &Palette, size: (f32, f32)) {
    let (w, h) = size;
    dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
    dl.rect(Rect::new(0.0, 0.0, w, 44.0), 0.0, pal.surface);
    let colours = [
        0x6fa8dc, 0x38761d, 0xffd966, 0x1c4587, 0x6aa84f, 0x3d78b8, 0xe69138,
    ];
    let (tw, th, gap) = (128.0, 96.0, 8.0);
    let mut i = 0;
    let mut y = 52.0;
    while y < h - GALLERY_BAR {
        let mut x = 12.0;
        while x + tw <= w {
            dl.rect(
                Rect::new(x, y, tw, th),
                4.0,
                Color::hex(colours[i % colours.len()]),
            );
            i += 1;
            x += tw + gap;
        }
        y += th + gap;
    }
    dl.rect(
        Rect::new(0.0, h - GALLERY_BAR, w, GALLERY_BAR),
        0.0,
        pal.surface,
    );
}

/// Synthetic series thumbnails (no patient images): an axial slice and a wider scout, as
/// textures with their sizes.
fn thumb_textures(h: &mut Harness) -> Vec<(wgpu::Texture, (TexId, u32, u32))> {
    let slice = |w: u32, hh: u32, scout: bool| {
        let mut rgba = Vec::with_capacity((w * hh * 4) as usize);
        for y in 0..hh {
            for x in 0..w {
                let (fx, fy) = (
                    x as f32 / w as f32 * 2.0 - 1.0,
                    y as f32 / hh as f32 * 2.0 - 1.0,
                );
                let v = if scout {
                    let body = (fx * 1.6).powi(2) < 1.0 - fy * fy * 0.2;
                    let spine = fx.abs() < 0.08;
                    if spine {
                        230.0
                    } else if body {
                        110.0 + 30.0 * (fy * 6.0).sin()
                    } else {
                        8.0
                    }
                } else {
                    let r = (fx * fx / 0.8 + fy * fy / 0.55).sqrt();
                    let lung = ((fx.abs() - 0.42).powi(2) + fy * fy * 0.6).sqrt() < 0.3;
                    if r > 1.0 {
                        6.0
                    } else if r > 0.93 {
                        210.0
                    } else if lung {
                        30.0
                    } else {
                        120.0
                    }
                };
                let v = v as u8;
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        (w, hh, rgba)
    };
    [slice(96, 96, false), slice(128, 64, true)]
        .into_iter()
        .map(|(w, hh, rgba)| {
            let size = wgpu::Extent3d {
                width: w,
                height: hh,
                depth_or_array_layers: 1,
            };
            let tex = h.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("studies thumbnail"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            h.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: None,
                },
                size,
            );
            let view = tex.create_view(&Default::default());
            let id = h.ui.register_texture(&h.gpu, &view);
            (tex, (id, w, hh))
        })
        .collect()
}

#[test]
fn chrome_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    // DICOM tag values are shown as stored (a hyphen-minus is their sign).
    for d in ["-312.5", "-600\\40"] {
        h.data_text(d);
    }
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-widgets",
    );
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let mut medical = Keymap::default();
    medical.set_active(&[Mode::Medical]);
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let mut no_tips = Settings::default();
    no_tips.general.start_tips = false;
    let thumbs = thumb_textures(&mut h);
    let t0 = std::time::Instant::now();
    let mut audit = Vec::new();
    for case in cases() {
        // A size case's setting does not leak into the next case.
        store.update(Origin::Program, |s| {
            s.ai.search_size = ModelSize::Recommended;
            s.privacy.no_network = case.state == State::PrivacyOff;
            s.devices.enabled = case.state == State::Devices;
            s.import.destination = (case.state == State::SettingsImport)
                .then(|| std::path::PathBuf::from("/media/photos/Imported from cards"));
            let ocio = case.state == State::SettingsOcio;
            s.colour.ocio_config =
                ocio.then(|| std::path::PathBuf::from("/studio/aces/cg-config-v2.2.0.ocio"));
            s.colour.ocio_view = if ocio {
                "ACES 1.0 - SDR Video".into()
            } else {
                String::new()
            };
        });
        I18n::global().set_language(case.lang.clone());
        let hebrew = case.lang.language.as_str() == "he";
        h.text
            .set_locale(Some(case.lang.language.as_str().to_owned()));
        let pal = if case.dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mut md = if case.state == State::InfoPlot {
            plot_report(&case.lang)
        } else if case.state == State::InfoDrawingFonts {
            drawing_fonts_report(&case.lang)
        } else if case.state == State::InfoSurvey {
            survey_report(&case.lang)
        } else if case.state == State::InfoStructure {
            structure_report(&case.lang)
        } else if case.state == State::InfoThermal {
            thermal_report(&case.lang)
        } else if case.state == State::InfoRoot {
            root_report(&case.lang)
        } else {
            report(hebrew)
        };
        if case.state == State::FocusInfoRows {
            md.all_fields = all_fields();
        }
        let size = case.state.size();
        let photo = photo_rect(size);
        let paged = matches!(
            case.state,
            State::Pages | State::PagesLargeText | State::PagesRoot
        );
        let empty = case.state.welcome();
        let movie = matches!(
            case.state,
            State::Video
                | State::VideoFullscreen
                | State::VideoSubtitles
                | State::VideoGenerated
                | State::VideoChapters
                | State::VideoHover
                | State::VideoResumed
                | State::VideoVolume
                | State::QueuePanel
                | State::SoundTransport
                | State::VideoSpeed
                | State::VideoPicture
                | State::VideoSound
                | State::VideoOutput
                | State::VideoGoTo
                | State::VideoCast
                | State::VideoCasting
        );
        // A sound file: no picture (its cover and waveform), the queue's buttons.
        let sound_file = matches!(case.state, State::QueuePanel | State::SoundTransport);
        let st = ViewerState {
            actions: ActionState {
                comparing: false,
                compare_images: 0,
                volume: false,
                mpr: false,
                image: match case.state {
                    _ if empty => ImageState::None,
                    State::Opening => ImageState::Opening,
                    State::OpenError
                    | State::OpenErrorMemory
                    | State::OpenErrorDecoder
                    | State::OpenErrorInternal => ImageState::Failed,
                    _ => ImageState::Ready,
                },
                count: if empty { 0 } else { 12 },
                gallery_files: 0,
                animated: movie,
                video: movie,
                sound: movie,
                sound_file: false,
                in_archive: false,
                has_alpha: false,
                gallery_views: None,
                medical: None,
                dicom: case.state == State::ProfileChip,
                series: 0,
                presentation_states: 0,
                science: None,
                debayer: false,
                annotations: false,
                stain: None,
                stain_region: false,
                slide_annotations: false,
                thermal_visible: false,
                drawing: false,
                mesh: false,
                points: false,
                molecule: false,
                tracts: false,
                relight: false,
                map: None,
                import_sources: false,
                pages: if paged { 10 } else { 0 },
                rotatable: false,
                wallpaper: false,
                share: false,
                email: false,
                profile: if case.state == State::ProfileChip || empty {
                    Profile::of(if case.state == State::WelcomeMedical {
                        Capabilities::GENERAL.with(Capability::Medical)
                    } else {
                        Capabilities::GENERAL
                    })
                } else {
                    Profile::UNRESTRICTED
                },
            },
            has_image: !empty && case.state != State::Opening,
            index: 3,
            count: if empty { 0 } else { 12 },
            zoom: photo.w / PHOTO.0,
            at_fit: true,
            fullscreen: matches!(
                case.state,
                State::VideoFullscreen | State::ZoomMenuFullscreen
            ),
            file_name: (!empty).then_some("IMG_0042.jpg"),
            dims: (!empty && case.state != State::Opening)
                .then_some((PHOTO.0 as u32, PHOTO.1 as u32)),
            loading: case.state == State::Opening,
            error: match case.state {
                State::OpenError => Some(OPEN_ERROR),
                State::OpenErrorMemory => Some(OPEN_ERROR_MEMORY),
                State::OpenErrorDecoder => Some(OPEN_ERROR_DECODER),
                State::OpenErrorInternal => Some(OPEN_ERROR_INTERNAL),
                _ => None,
            },
            truncated: false,
            metadata: (!empty && case.state != State::Opening).then_some(&md),
            hud: &hud,
            keymap: if matches!(case.state, State::CheatMedical | State::CheatMedicalEnd) {
                &medical
            } else {
                &keymap
            },
            store: &store,
            settings: if case.state == State::WelcomeTipsHidden {
                &no_tips
            } else {
                &settings
            },
            overlays: Overlays::default(),
            playback: movie.then_some(Playback {
                frame: 1874,
                count: Some(5400),
                playing: true,
                speed: if case.state == State::VideoSpeed {
                    1.5
                } else {
                    1.0
                },
                video: Some(VideoPlayback {
                    position: 74.96,
                    duration: 216.0,
                    looping: true,
                    sound: Some((0.8, false)),
                    volume_max: 1.0,
                    picture: !sound_file,
                }),
            }),
            motion: None,
            spread: None,
            pages: paged.then_some(PageBadge {
                index: 2,
                count: 10,
                label: Some(if case.state == State::PagesRoot {
                    "run1/tracker/inner/occ (TH2F)"
                } else {
                    "128×128 @2x"
                }),
            }),
        };
        for (scale, suffix) in SCALES {
            if case.state.small() && scale > 1.5 {
                continue;
            }
            // A fresh chrome per shot: no hover, no fades (reduced motion), toolbar shown.
            let mut c = Chrome::new();
            c.reduce_motion = true;
            c.set_size(size.0, size.1);
            let now = 1.0;
            c.poke(now);
            if case.state.gallery() {
                c.gallery = true;
                c.gallery_bar = GALLERY_BAR;
                // As the app words it, in the case's language.
                c.set_insight_indicator(Some((
                    specular_i18n::t!("open-report-insights-indicator"),
                    specular_i18n::t!("open-report-insights-announce"),
                )));
            }
            match case.state {
                State::Viewer
                | State::ProfileChip
                | State::Empty
                | State::Welcome
                | State::WelcomeTipsHidden
                | State::WelcomeMedical
                | State::OpenError
                | State::OpenErrorMemory
                | State::OpenErrorInternal => {}
                State::OpenErrorDecoder => {
                    c.set_decoder_card(Some(decoder_card()));
                }
                State::WorkChip => c.set_work_chip(Some(work_chip(false))),
                State::GalleryWork => {
                    // As the app's top bar leaves it: before the filter and View buttons.
                    c.gallery_work_end = Some(SIZE.0 - 8.0 - 92.0 - 48.0);
                    c.set_work_chip(Some(work_chip(true)));
                }
                State::Work => {
                    c.set_work_chip(Some(work_chip(false)));
                    c.show_work(work_panel());
                }
                State::WorkLargeText => {
                    c.text_scale = 2.0;
                    c.set_work_chip(Some(work_chip(false)));
                    c.show_work(work_panel());
                }
                State::DownloadsInline | State::DownloadsNarrow => {
                    c.set_downloads(Some(downloads()));
                }
                State::ToolbarNarrow => c.show_overflow_menu(),
                State::DownloadsHiddenBar => {
                    c.set_presenting(true);
                    c.set_downloads(Some(downloads()));
                }
                State::DownloadsPanel => {
                    c.set_downloads(Some(downloads()));
                    c.show_downloads();
                }
                State::DownloadsRetrieve => {
                    c.set_downloads(Some(retrieve_downloads()));
                    c.show_downloads();
                }
                State::SettingsFields => c.open_settings_at("library"),
                State::SettingsColour => c.open_settings_at("viewing.subtitle_custom_color"),
                State::QueuePanel => {
                    c.set_play_order(Some(play_order()));
                    c.set_queue(queue_view());
                    c.show_queue();
                }
                State::SoundTransport => c.set_play_order(Some(play_order_one())),
                State::SettingsOcio => {
                    let names = |v: &[&str]| {
                        Some(v.iter().map(|n| (n.to_string(), n.to_string())).collect())
                    };
                    c.set_setting_choices(
                        "colour.ocio_display",
                        names(&[
                            "sRGB - Display",
                            "Display P3 - Display",
                            "Rec.1886 Rec.709 - Display",
                        ]),
                    );
                    c.set_setting_choices(
                        "colour.ocio_view",
                        names(&["ACES 1.0 - SDR Video", "Un-tone-mapped", "Raw"]),
                    );
                    c.set_setting_choices("colour.ocio_input", names(&["ACEScg", "ACES2065-1"]));
                    c.open_settings_at("colour.ocio_config");
                }
                State::SettingsImport => {
                    specular_settings::set_text_check("import.rename_pattern", |s| {
                        if s.contains("{nme}") {
                            Err("unknown token {nme}".into())
                        } else {
                            Ok(())
                        }
                    });
                    c.open_settings_at("import");
                }
                State::GalleryDownloads => {
                    // As the app's gallery leaves it: after "12 items", before its controls.
                    c.gallery_free = Some((96.0, 960.0 - 8.0 - 360.0));
                    c.set_downloads(Some(one_download()));
                }
                State::WelcomeRecents => c.set_welcome(welcome_recents(&thumbs)),
                State::Hint => {
                    c.hint(
                        "Press g to see this folder as a gallery.".into(),
                        1,
                        now - 1.0,
                    );
                }
                State::DropHover => c.file_hover = true,
                // Opened two seconds ago: past the spinner's delay and the name's.
                State::Opening => {
                    let mut dl = DrawList::new(scale);
                    h.text.begin_frame(scale);
                    c.draw(&mut dl, &mut h.text, &st, &pal, now - 2.0);
                }
                State::Settings => c.open_settings_at("viewing"),
                State::SettingsProfile => c.open_settings_at("profile"),
                State::SettingsMoreTypes => {
                    c.set_setting_panel("integration.default_viewer", Some(more_types()));
                    c.open_settings_at("integration.default_viewer");
                }
                State::ProfileQuestion => c.ask_profile(
                    Capabilities::GENERAL
                        .with(Capability::Medical)
                        .with(Capability::Maps),
                ),
                State::AiNone => {
                    c.set_ai_page(&AiPage::default());
                    c.open_settings_at("ai");
                }
                State::AiOffers => {
                    c.set_ai_page(&ai_offers());
                    c.open_settings_at("ai");
                }
                State::AiLargeText => {
                    c.text_scale = 2.0;
                    c.set_ai_page(&ai_offers());
                    c.open_settings_at("ai");
                }
                State::AiInstalled => {
                    c.set_ai_page(&ai_installed());
                    c.open_settings_at("ai");
                }
                State::AiPeople => {
                    c.set_ai_page(&ai_installed());
                    c.open_settings_at("ai.people");
                }
                State::AiFaces => {
                    c.set_ai_page(&ai_faces());
                    c.open_settings_at("ai.people_size");
                }
                State::AiSizeOff
                | State::AiSizeRecommended
                | State::AiSizeDownloading
                | State::AiSizeSwitching
                | State::AiSizeFailed
                | State::AiSizeNamed => {
                    // The size setting the page's state comes from.
                    let size = match case.state {
                        State::AiSizeNamed => ModelSize::Small,
                        State::AiSizeSwitching | State::AiSizeFailed => ModelSize::Large,
                        _ => ModelSize::Recommended,
                    };
                    store.update(Origin::Program, |s| s.ai.search_size = size);
                    c.set_ai_page(&ai_sized(case.state));
                    c.open_settings_at("ai.search_size");
                }
                State::AiTextAddons => {
                    c.set_ai_page(&ai_text_addons());
                    // Text's model setting scrolled to its add-on rows.
                    c.open_settings_at("ai");
                    c.scroll_settings(1150.0);
                }
                State::OcrList => {
                    use specular_i18n::t;
                    let row = |id: u64, label: String, key: Option<String>| PaletteItem {
                        id,
                        label,
                        key,
                        unavailable: None,
                        icon: Icon::Copy,
                    };
                    // As the app's `confidence_chip`: the percent isolated, the chip in the
                    // UI's direction.
                    let dir = specular_i18n::I18n::global().direction();
                    let sure = |p: &str| {
                        let p = specular_i18n::isolate_label(p, specular_i18n::Direction::Ltr);
                        Some(specular_i18n::isolate_label(
                            &t!("ocr-confidence", percent = p),
                            dir,
                        ))
                    };
                    let items = vec![
                        row(1, t!("ocr-copy-all"), None),
                        row(2, t!("ocr-copy", text = "Open daily 9 to 5"), sure("99%")),
                        row(3, t!("ocr-copy", text = "ברוכים הבאים"), sure("95%")),
                        row(4, t!("ocr-copy", text = "أهلا وسهلا"), sure("97%")),
                        row(5, t!("ocr-copy", text = "Bäckerei Müller"), sure("98%")),
                    ];
                    c.open_list(t!("ocr-placeholder"), items, None);
                }
                State::CodesList => {
                    use specular_i18n::t;
                    let row = |id: u64, label: String, key: Option<String>, icon| PaletteItem {
                        id,
                        label,
                        key,
                        unavailable: None,
                        icon,
                    };
                    let kind = |v: i64| Some(t!("codes-kind", kind = "QR", version = v));
                    let items = vec![
                        row(
                            1,
                            t!("codes-copy", text = "https://example.org/menu"),
                            kind(3),
                            Icon::Copy,
                        ),
                        row(
                            2,
                            t!("codes-open-row", host = "example.org"),
                            None,
                            Icon::Globe,
                        ),
                        row(
                            3,
                            t!("codes-copy", text = "WIFI:S:Café;T:WPA;P:••••;;"),
                            kind(2),
                            Icon::Copy,
                        ),
                        row(4, t!("codes-copy", text = "שלום עולם"), kind(1), Icon::Copy),
                    ];
                    c.open_list(t!("codes-placeholder"), items, None);
                }
                State::Palette | State::FocusPalette => {
                    c.open_modal(Modal::Palette);
                    c.input(&Input::Text("zoom".into()), &st, now);
                }
                State::Info
                | State::InfoPlot
                | State::InfoDrawingFonts
                | State::InfoSurvey
                | State::InfoStructure
                | State::InfoThermal
                | State::InfoRoot => c.set_info(true, false),
                State::Insights => c.show_insights(insights()),
                State::GalleryIndicator => {}
                State::GalleryInsights => c.show_insights(thumb_insights()),
                State::FocusInsights => c.show_insights(insights()),
                State::FocusToolbar | State::FocusZoomMenu | State::ZoomMenuFullscreen => {}
                State::FocusInfoRows => c.set_info(true, true),
                State::FocusGalleryIndicator => {
                    assert!(c.focus_indicator(false));
                }
                State::FocusConfirm | State::FocusConfirmDefault => {
                    c.confirm_trash("IMG_0042.jpg".into())
                }
                State::ZoomMenuToast => {
                    c.toast(
                        "Moved “IMG_0041 — harbour at dusk, long exposure.jpg” to the trash".into(),
                        Some(1),
                        now,
                    );
                }
                State::FocusSettings => c.open_settings_at("viewing"),
                State::FocusLanguage => c.open_modal(Modal::Language),
                State::FocusAbout => {
                    c.set_about_build_line("Build 0123abcd 2026-10-12 · release · x86_64-linux");
                    c.open_modal(Modal::About);
                }
                State::About => {
                    c.set_contact("https://example.com/specular/issues", "support@example.com");
                    c.set_about_build_line("Build 0123abcd 2026-10-12 · release · x86_64-linux");
                    c.open_modal(Modal::About);
                }
                State::HelpPage => c.show_help_topic(specular_help::Topic {
                    page: "viewer",
                    anchor: "zoom",
                }),
                State::HelpSearch => {
                    c.show_help("viewer");
                    c.input(&Input::Text("crash report".into()), &st, now);
                }
                State::HelpContents => c.show_help_contents(),
                State::Crash => c.show_crash(CrashSheet {
                    report: CRASH_REPORT.into(),
                }),
                State::Diagnostics => c.show_diagnostics(diag_sheet()),
                State::FocusGalleryInsightsRestore => c.show_insights(thumb_insights()),
                State::FocusSettingsRestore => c.open_settings_at(RESTORE),
                State::ConfirmReset => c.open_settings_at(RESET_ALL),
                State::ConfirmRemove => c.confirm_destructive(
                    specular_i18n::t!("library-remove-confirm", location = "Pictures"),
                    specular_i18n::t!("library-remove-confirm-detail"),
                    specular_i18n::t!("library-remove-confirm-ok"),
                    1,
                ),
                State::ConfirmHome => c.confirm_choice(
                    specular_i18n::t!("library-home-confirm", folder = "ana"),
                    format!(
                        "{}\n{}",
                        specular_i18n::t!("library-home-confirm-detail"),
                        specular_i18n::t!("library-home-confirm-pictures")
                    ),
                    specular_i18n::t!("library-home-confirm-use-pictures"),
                    1,
                    specular_i18n::t!("library-home-confirm-keep"),
                    2,
                ),
                State::SettingsRestoreOff => {
                    c.open_settings_at(RESTORE);
                    assert!(c.set_setting_disabled(RESTORE, true));
                }
                State::FocusKeys | State::KeysConflict => c.open_settings_at(KEYS),
                State::DicomTags => c.show_dicom_tags(dicom_tags()),
                State::SrTree => c.show_dicom_tags(sr_tree()),
                State::PlotDvh => c.show_plot(dvh_sheet()),
                State::PlotEcg => c.show_plot(ecg_sheet()),
                State::Studies | State::StudiesPatient => c.show_studies(studies(&thumbs)),
                State::StudiesSend => {
                    let mut s = studies(&thumbs);
                    s.sendable = true;
                    c.show_studies(s);
                }
                State::StudiesReading => {
                    let mut s = studies(&thumbs);
                    s.rows.truncate(5);
                    s.note = None;
                    s.status = Some("Reading the disc: 214 of 1830 files".into());
                    s.cancellable = true;
                    c.show_studies(s);
                }
                State::DicomTagsFilter | State::DicomTagsFilterSelect => {
                    c.show_dicom_tags(dicom_tags());
                    c.input(&Input::Text("pixel".into()), &st, now);
                }
                State::ConfigEditor => c.open_config_editor_with(&store, Some(CONFIG)),
                State::ConfigEditorFind => {
                    c.open_config_editor_with(&store, Some(CONFIG));
                    let r = c.input(
                        &Input::Key {
                            combo: KeyCombo::new(Key::Char('f'), Mods::CTRL),
                            repeat: false,
                        },
                        &st,
                        now,
                    );
                    assert!(r.consumed);
                    c.input(&Input::Text("pixel".into()), &st, now);
                }
                State::GoToSelect => {
                    assert!(c.handle_action(Action::GoToImage));
                    c.input(&Input::Text("12".into()), &st, now);
                }
                State::HwReport | State::HwReportTable => c.show_report(hw_report(hebrew)),
                State::PrivacyOff => {
                    c.set_setting_panel("privacy.no_network", Some(privacy_panel()));
                    c.open_settings_at("privacy");
                }
                State::Devices => {
                    c.set_setting_panel("devices.look_again", Some(devices_panel()));
                    c.open_settings_at("devices.look_again");
                }
                State::RendererInfo | State::RendererInfoDisplay => {
                    c.show_report(specular_widgets::renderer_info::report(
                        &specular_widgets::renderer_info::sample(),
                    ))
                }
                State::Pages | State::PagesRoot | State::Video => {}
                State::VideoFullscreen => {
                    c.set_info(true, false);
                    c.set_video_fullscreen(true, now);
                }
                State::VideoSubtitles
                | State::VideoGenerated
                | State::VideoChapters
                | State::VideoHover
                | State::VideoResumed
                | State::VideoVolume
                | State::VideoSpeed
                | State::VideoPicture
                | State::VideoSound
                | State::VideoOutput
                | State::VideoGoTo
                | State::VideoCast
                | State::VideoCasting => {
                    let mut pv = player_view();
                    if case.state == State::VideoGenerated {
                        pv.subtitles.push(t!("stt-track", language = "English"));
                        pv.subtitle = Some(pv.subtitles.len() - 1);
                        pv.generate = Some(t!("player-subtitles-generating", percent = "34 %"));
                        pv.save_subtitles = true;
                        pv.charset = None;
                    }
                    if case.state == State::VideoSpeed {
                        pv.speed = 1.5;
                    }
                    if case.state == State::VideoPicture {
                        pv.picture = [0.15, 1.2, 0.8, 1.1, -30.0];
                    }
                    if case.state == State::VideoCast {
                        pv.cast.devices = vec!["Living Room TV".into(), "סלון – רמקול".into()];
                        pv.cast.searching = true;
                    }
                    if case.state == State::VideoCasting {
                        pv.cast.devices = vec!["Living Room TV".into()];
                        pv.cast.active = Some(0);
                        pv.cast.playing_on = Some("Living Room TV".into());
                        pv.cast.status = Some(specular_i18n::t!("cast-paused"));
                    }
                    c.set_player(Some(pv));
                    // Laid out once, so the menus find their buttons.
                    let mut first = DrawList::new(scale);
                    h.text.begin_frame(scale);
                    c.draw(&mut first, &mut h.text, &st, &pal, now);
                    match case.state {
                        State::VideoSubtitles | State::VideoGenerated => {
                            c.toggle_player_menu(PlayerMenu::Subtitles)
                        }
                        State::VideoChapters => c.toggle_player_menu(PlayerMenu::Chapters),
                        State::VideoSpeed => c.toggle_player_menu(PlayerMenu::Speed),
                        State::VideoSound => c.toggle_player_menu(PlayerMenu::Sound),
                        State::VideoOutput => c.toggle_player_menu(PlayerMenu::Output),
                        State::VideoPicture => {
                            assert!(c.handle_action(Action::VideoPicture));
                        }
                        State::VideoCast => {
                            assert!(c.handle_action(Action::CastMenu));
                        }
                        State::VideoCasting => {}
                        State::VideoGoTo => {
                            assert!(c.handle_action(Action::GoToTime));
                            c.input(&Input::Text("1:23:4".into()), &st, now);
                        }
                        State::VideoVolume => {
                            let r = c.volume_slider_rect().expect("a volume slider");
                            // 65 % along its track (8 px in from either end).
                            let at = (r.x + 8.0 + (r.w - 16.0) * 0.65, r.y + r.h * 0.5);
                            c.input(&Input::PointerMove { x: at.0, y: at.1 }, &st, now);
                            c.input(
                                &Input::PointerButton {
                                    x: at.0,
                                    y: at.1,
                                    down: true,
                                    button: specular_widgets::Button::Primary,
                                },
                                &st,
                                now,
                            );
                        }
                        State::VideoHover => {
                            let r = c.seek_bar_rect().expect("a seek bar");
                            let at = (r.x + r.w * 0.62, r.y + r.h * 0.5);
                            c.input(&Input::PointerMove { x: at.0, y: at.1 }, &st, now);
                        }
                        _ => {
                            c.toast_action(
                                specular_i18n::t!("toast-video-resumed", time = "12:34"),
                                specular_i18n::t!("toast-video-start-over"),
                                1,
                                now - 1.0,
                            );
                        }
                    }
                }
                State::ColourBar => {
                    assert!(c.handle_action(Action::ToggleFalseColour));
                    assert!(c.handle_action(Action::ToggleHiLo));
                    c.inspect.tools.histogram = true;
                    c.inspect.legend = Some(specular_inspect::Legend {
                        map: specular_inspect::ColourMap::Viridis,
                        lo: -160.0,
                        hi: 240.0,
                        unit: "HU".into(),
                    });
                }
                State::MapPicker => c.open_settings_at(COLOUR_MAP),
                State::Cvd => {
                    // As in the app: the settings are synced on the first frame, before keys.
                    c.inspect.sync_settings(&settings);
                    assert!(c.handle_action(Action::SimulateDeuteranopia));
                }
                State::PagesLargeText => c.text_scale = 2.0,
                State::HwReportLargeText => {
                    c.text_scale = 2.0;
                    c.show_report(hw_report(hebrew));
                }
                State::ConfigEditorRefused => {
                    c.open_config_editor_with(&store, Some(CONFIG));
                    let r = c.input(
                        &Input::Key {
                            combo: KeyCombo::new(Key::Char('s'), Mods::CTRL),
                            repeat: false,
                        },
                        &st,
                        now,
                    );
                    assert!(r.consumed);
                }
                State::KeymapEditor => {
                    c.set_keymap_file(Some(std::path::Path::new(KEYMAP_PATH)), None);
                    c.open_keymap_editor_with(&keymap, None);
                }
                State::KeymapEditorProblems => {
                    c.set_keymap_file(Some(std::path::Path::new(KEYMAP_PATH)), Some(KEYMAP));
                    c.open_keymap_editor_with(&keymap, Some(KEYMAP));
                    // Ctrl+S is refused while errors remain; the keyboard goes to the list.
                    let r = c.input(
                        &Input::Key {
                            combo: KeyCombo::new(Key::Char('s'), Mods::CTRL),
                            repeat: false,
                        },
                        &st,
                        now,
                    );
                    assert!(r.consumed && r.commands.is_empty());
                }
                State::Presenting => c.set_presenting_on(Some(Presenting {
                    display: "HDMI-A-1".into(),
                    paused: Some(true),
                })),
                State::PresentingViewerGallery => c.set_presenting_on(Some(Presenting {
                    display: "DP-2".into(),
                    paused: None,
                })),
                State::CheatGallery
                | State::CheatCompare
                | State::CheatMedical
                | State::CheatMedicalEnd => {
                    if case.state.gallery_scene() {
                        c.gallery = true;
                        c.gallery_bar = GALLERY_BAR;
                    }
                    c.comparing = case.state == State::CheatCompare;
                    c.open_modal(Modal::CheatSheet);
                }
            }
            // One frame to settle what the chrome measures on the way (scroll ranges, the
            // toolbar's overflow), then the one that is compared.
            let mut warm = DrawList::new(scale);
            h.text.begin_frame(scale);
            c.draw(&mut warm, &mut h.text, &st, &pal, now);
            // The keyboard states: keys after the first frame (the bar's stops are measured
            // while drawing it), then a second frame to settle.
            let shift = Mods {
                shift: true,
                ..Mods::NONE
            };
            // The field selections: Shift+Left once the field has been drawn (and so knows
            // its direction).
            let select = match case.state {
                State::DicomTagsFilterSelect => 3,
                State::ConfigEditorFind => 2,
                State::GoToSelect => 1,
                _ => 0,
            };
            for _ in 0..select {
                let r = c.input(
                    &Input::Key {
                        combo: KeyCombo::new(Key::Named(NamedKey::ArrowLeft), shift),
                        repeat: false,
                    },
                    &st,
                    now,
                );
                assert!(r.consumed, "{}: Shift+Left", case.name);
            }
            let to_keys: Vec<NamedKey> = match case.state {
                // Down two rows, right onto its first key.
                State::FocusKeys => std::iter::repeat_n(NamedKey::Tab, TABS_TO_KEY_LIST)
                    .chain([
                        NamedKey::ArrowDown,
                        NamedKey::ArrowDown,
                        NamedKey::ArrowRight,
                    ])
                    .collect(),
                // Enter records on the first row.
                State::KeysConflict => std::iter::repeat_n(NamedKey::Tab, TABS_TO_KEY_LIST)
                    .chain([NamedKey::Enter])
                    .collect(),
                _ => Vec::new(),
            };
            let keys: &[NamedKey] = match case.state {
                State::FocusKeys | State::KeysConflict => &to_keys,
                // Position readout, Previous, Next.
                State::FocusToolbar => &[NamedKey::Tab, NamedKey::ArrowRight, NamedKey::ArrowRight],
                // Shift+Tab (below), then up two rows.
                State::FocusInfoRows => &[NamedKey::Tab, NamedKey::ArrowUp, NamedKey::ArrowUp],
                State::FocusConfirm => &[NamedKey::Tab],
                State::CheatMedicalEnd => &[NamedKey::ArrowDown; 40],
                State::FocusConfirmDefault => &[NamedKey::Tab, NamedKey::Tab],
                State::FocusSettings => &[NamedKey::Tab, NamedKey::Tab],
                State::FocusPalette | State::FocusLanguage | State::FocusAbout => &[NamedKey::Tab],
                // Onto View report, and press it.
                State::Crash => &[NamedKey::Tab, NamedKey::Enter],
                // On to the zoom readout; ↓ opens the presets on the first, ↓ again.
                State::FocusZoomMenu | State::ZoomMenuToast => &[
                    NamedKey::Tab,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                ],
                // The same, then onto the last entry.
                State::ZoomMenuFullscreen => &[
                    NamedKey::Tab,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowRight,
                    NamedKey::ArrowDown,
                    NamedKey::End,
                ],
                State::FocusInsights | State::FocusSettingsRestore => &[NamedKey::Tab],
                // Onto the map's list, and open it.
                State::MapPicker => &[NamedKey::Tab, NamedKey::Enter],
                // Onto Reset all settings…, which asks.
                State::ConfirmReset => &[NamedKey::Tab, NamedKey::Enter],
                // Don't show again, then Show hidden insights again.
                State::FocusGalleryInsightsRestore => &[NamedKey::Tab, NamedKey::Tab],
                // Down to the sequence, open it, onto its first item.
                State::DicomTags => &[
                    NamedKey::End,
                    NamedKey::ArrowUp,
                    NamedKey::Enter,
                    NamedKey::ArrowDown,
                ],
                State::DicomTagsFilter => &[NamedKey::Tab],
                // Open the report, down to the finding with an image.
                State::SrTree => &[NamedKey::ArrowRight, NamedKey::End],
                // Down to the second series, then onto Show patient data and switch it on.
                State::StudiesPatient => &[NamedKey::ArrowDown, NamedKey::Tab, NamedKey::Space],
                // Down past System and Graphics to the path table.
                State::HwReportTable => &[NamedKey::ArrowDown; REPORT_TABLE_STEPS],
                // Down past the adapter and its limits to the display.
                State::RendererInfoDisplay => &[NamedKey::ArrowDown; RENDERER_DISPLAY_STEPS],
                // Down to `toolbar_items`, then after its closing bracket.
                State::ConfigEditor => &[
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::ArrowDown,
                    NamedKey::End,
                ],
                _ => &[],
            };
            if !keys.is_empty() {
                for (i, &k) in keys.iter().enumerate() {
                    let mods = if i == 0 && case.state == State::FocusInfoRows {
                        shift
                    } else {
                        Mods::NONE
                    };
                    // The bar runs from the right in Hebrew: its arrows are mirrored.
                    let k = match k {
                        NamedKey::ArrowRight if hebrew => NamedKey::ArrowLeft,
                        k => k,
                    };
                    let r = c.input(
                        &Input::Key {
                            combo: KeyCombo::new(Key::Named(k), mods),
                            repeat: false,
                        },
                        &st,
                        now,
                    );
                    assert!(r.consumed, "{}: {k:?}", case.name);
                }
                warm.clear(scale);
                h.text.begin_frame(scale);
                c.draw(&mut warm, &mut h.text, &st, &pal, now);
            }
            if case.state == State::KeysConflict {
                // The key of Info, then no other key: recording ends after its timeout.
                let taken = keymap.keys_in(Mode::Viewer, Action::ToggleInfo)[0].keys()[0];
                let r = c.input(
                    &Input::Key {
                        combo: taken,
                        repeat: false,
                    },
                    &st,
                    now,
                );
                assert!(r.consumed && r.commands.is_empty());
                let later = now + settings.keys.sequence_timeout + 1.0;
                warm.clear(scale);
                h.text.begin_frame(scale);
                c.draw(&mut warm, &mut h.text, &st, &pal, later);
                assert!(!c.recording_keys(), "the recording ended");
                warm.clear(scale);
                h.text.begin_frame(scale);
                c.draw(&mut warm, &mut h.text, &st, &pal, now);
            }
            // Settings' fields: onto one with Tab, typed into (Enter: kept, or refused).
            let typing: Option<(usize, &str, bool)> = match case.state {
                State::SettingsFields => Some((1, ",Backups", false)),
                // Reset section, On card, the folder, Choose…, Clear, the folder's Reset, the
                // folder pattern, the rename pattern.
                State::SettingsImport => Some((8, "{nme}-{n:03}", true)),
                _ => None,
            };
            if let Some((tabs, text, enter)) = typing {
                let key = |c: &mut Chrome, k: NamedKey| {
                    let combo = KeyCombo::new(Key::Named(k), Mods::NONE);
                    let ev = Input::Key {
                        combo,
                        repeat: false,
                    };
                    c.input(&ev, &st, now).consumed
                };
                for _ in 0..tabs {
                    assert!(key(&mut c, NamedKey::Tab), "{}: Tab", case.name);
                }
                c.input(&Input::Text(text.into()), &st, now);
                if enter {
                    assert!(key(&mut c, NamedKey::Enter), "{}: Enter", case.name);
                }
                for _ in 0..2 {
                    warm.clear(scale);
                    h.text.begin_frame(scale);
                    c.draw(&mut warm, &mut h.text, &st, &pal, now);
                }
            }
            let gallery = case.state.gallery_scene();
            // No picture on screen yet: the empty window, a drop, the first open and its error.
            let bare = empty
                || matches!(
                    case.state,
                    State::Opening
                        | State::OpenError
                        | State::OpenErrorMemory
                        | State::OpenErrorDecoder
                        | State::OpenErrorInternal
                );
            // The simulation is the compositor's final pass, as in the app.
            h.comp.set_cvd(&h.gpu, c.inspect.tools.cvd.matrix());
            let shot = h.render(
                size,
                scale,
                |dl| {
                    if gallery {
                        gallery_scene(dl, &pal, size);
                    } else if bare {
                        let (w, h) = size;
                        dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.canvas);
                    } else {
                        scene(dl, &pal, size);
                    }
                },
                |dl, text| {
                    if let Some(cue) = subtitle_for(case.state) {
                        // The picture above the playback bar (which hides while playing).
                        let p = photo_rect(size);
                        let pic = Rect::new(p.x, p.y, p.w, p.h * 0.84);
                        use specular_widgets::subtitles as subs;
                        let look = subs::Look::default();
                        let mut stack = subs::Stack::new(pic, look.position);
                        subs::draw_text(dl, text, &cue, pic, &look, &mut stack);
                    }
                    c.draw(dl, text, &st, &pal, now + 0.016);
                },
            );
            goldens.check(&format!("{}-{suffix}", case.name), &shot);
            // The screen-reader audit of what was just drawn (once per case).
            if scale == 1.0 {
                let rtl = matches!(case.lang.language.as_str(), "he" | "ar");
                let tree = c.accessibility_tree(&st, "IMG_0042.jpg", f64::from(scale));
                for f in specular_widgets::a11y_audit::audit(&tree, rtl) {
                    audit.push(format!("{}: {f}", case.name));
                }
            }
        }
    }
    let shots: usize = cases()
        .iter()
        .map(|c| if c.state.small() { 2 } else { SCALES.len() })
        .sum();
    eprintln!(
        "{} chrome screenshots on {} in {:.2} s",
        shots,
        h.gpu.info.name,
        t0.elapsed().as_secs_f64()
    );
    I18n::global().set_language(langid!("en-US"));
    assert!(
        audit.is_empty(),
        "screen-reader audit, {} findings:\n{}",
        audit.len(),
        audit.join("\n")
    );
    goldens.finish();
}

#[path = "shot_cases/medical_cases.rs"]
mod medical_cases;
use medical_cases::*;

#[path = "shot_cases/info_cases.rs"]
mod info_cases;
use info_cases::*;

#[path = "shot_cases/ai_cases.rs"]
mod ai_cases;
use ai_cases::*;

#[path = "shot_cases/report_cases.rs"]
mod report_cases;
use report_cases::*;

#[path = "shot_cases/work_cases.rs"]
mod work_cases;
use work_cases::*;

#[path = "shot_cases/net_cases.rs"]
mod net_cases;
use net_cases::*;

#[path = "shot_cases/privacy_cases.rs"]
mod privacy_cases;
use privacy_cases::*;

#[path = "shot_cases/devices_cases.rs"]
mod devices_cases;
use devices_cases::*;

#[path = "shot_cases/queue_cases.rs"]
mod queue_cases;
use queue_cases::*;

/// The player menus' content for the movie shots.
fn player_view() -> specular_widgets::PlayerView {
    specular_widgets::PlayerView {
        subtitles: vec!["English".into(), "עברית".into(), "العربية – SDH".into()],
        subtitle: Some(1),
        subtitle_delay_ms: 350,
        charset: Some("windows-1255".into()),
        sounds: vec!["English (5.1)".into(), "Français".into()],
        sound: 0,
        sound_delay_ms: -120,
        chapters: vec![
            (0.0, 0.0, "Opening".into()),
            (0.25, 54.0, "The harbour".into()),
            (0.5, 108.0, "Night market".into()),
            (0.8, 172.8, "Credits".into()),
        ],
        ab: (Some(0.3), Some(0.42)),
        preview: None,
        has_sound: true,
        dialogue: true,
        mono: false,
        mini: false,
        generate: None,
        save_subtitles: false,
        speed: 1.0,
        picture_menu: true,
        picture: specular_widgets::PICTURE.map(|k| k.neutral),
        zoomed: false,
        outputs: vec![
            specular_i18n::t!("sound-output-default"),
            // Device names are isolated as the application does.
            isolate_ltr("USB DAC"),
            isolate_ltr("HDMI \u{2013} Living room TV"),
        ],
        output: 1,
        speakers: [
            "settings-viewing-sound-channels-auto",
            "settings-viewing-sound-channels-stereo",
            "settings-viewing-sound-channels-surround-5-1",
            "settings-viewing-sound-channels-surround-7-1",
        ]
        .map(|k| specular_i18n::I18n::global().tr(k))
        .to_vec(),
        speaker: 2,
        cast: specular_widgets::CastView::default(),
        qualities: Vec::new(),
        quality: None,
    }
}

/// The subtitle drawn over the picture in a movie shot.
fn subtitle_for(state: State) -> Option<specular_subs::TextCue> {
    match state {
        State::VideoSubtitles => Some(specular_subs::markup::parse(
            "שלום! הסרט מתחיל בשעה 20:30.\n<i>נתראה שם, Dana</i>",
        )),
        State::VideoChapters => Some(specular_subs::markup::parse(
            "مرحبا بكم في السوق الليلي.\nالساعة 9 مساءً",
        )),
        _ => None,
    }
}
