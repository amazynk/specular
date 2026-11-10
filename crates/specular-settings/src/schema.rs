//! The settings schema: every Stage 1 setting, declared once (DESIGN.md §11).
//!
//! Defaults follow principle #8 ("always the highest-end option"): full colour management,
//! dithering, every fast path on, the high-performance GPU, and automatic cache budgets
//! sized from the machine.

use std::path::PathBuf;

use specular_i18n::LanguagePreference;

use crate::value::{Action, Color, Kind, MonitorChoice, RegionFormats, SettingEnum, Unit};

/// A Local AI feature's model size: Recommended (the feature's default model where it fits
/// this computer's memory and GPU, else a smaller one that fits), or a named size.
///
/// A named size is absolute: that size on any machine. `Smaller` and `Larger` are the old
/// relative choices (one size either side of the recommendation). Settings files written
/// before named sizes still load with them, but nothing offers them: the app turns each
/// into the size it resolves to on this machine (or Recommended) the first time it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModelSize {
    #[default]
    Recommended,
    Small,
    Standard,
    Large,
    /// Old relative choice, migrated by the app.
    Smaller,
    /// Old relative choice, migrated by the app.
    Larger,
}

impl ModelSize {
    /// The old relative choices: accepted when read, never offered.
    pub const LEGACY: &'static [&'static str] = &["smaller", "larger"];

    /// One of the old relative choices, which the app still has to migrate.
    pub fn is_legacy(self) -> bool {
        matches!(self, Self::Smaller | Self::Larger)
    }
}

impl SettingEnum for ModelSize {
    const OPTIONS: &'static [&'static str] = &["recommended", "small", "standard", "large"];
    const ALL: &'static [Self] = &[Self::Recommended, Self::Small, Self::Standard, Self::Large];
    fn as_str(self) -> &'static str {
        match self {
            Self::Recommended => "recommended",
            Self::Small => "small",
            Self::Standard => "standard",
            Self::Large => "large",
            Self::Smaller => "smaller",
            Self::Larger => "larger",
        }
    }
    fn from_name(s: &str) -> Option<Self> {
        match s {
            "smaller" => Some(Self::Smaller),
            "larger" => Some(Self::Larger),
            _ => Self::OPTIONS
                .iter()
                .position(|o| *o == s)
                .map(|i| Self::ALL[i]),
        }
    }
}

impl crate::SettingValue for ModelSize {
    fn default_kind() -> Kind {
        Kind::Enum {
            options: Self::OPTIONS,
        }
    }
    fn to_value(&self) -> crate::Value {
        crate::Value::Text(self.as_str().to_owned())
    }
    fn from_value(v: &crate::Value) -> Option<Self> {
        match v {
            crate::Value::Text(s) => Self::from_name(s),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModelSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ModelSize {
    type Err = crate::SettingError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(&s.trim().to_ascii_lowercase().replace('-', "_")).ok_or(
            crate::SettingError::UnknownOption {
                options: Self::OPTIONS,
            },
        )
    }
}

/// Values an enum setting still reads but no longer offers: they validate, load without a
/// warning and survive a round trip until the app migrates them.
pub(crate) fn legacy_option(options: &[&str], s: &str) -> bool {
    options == ModelSize::OPTIONS && ModelSize::LEGACY.contains(&s)
}
setting_enum! {
    /// What Specular is used for: each turns on a set of tools. General is always on.
    pub enum ProfileCapability {
        General = "general",
        Engineering = "engineering",
        Medical = "medical",
        Science = "science",
        Maps = "maps",
    }
}
setting_enum! {
    /// The map view's base map: the built-in offline map only, OpenStreetMap's tiles, a tile
    /// server of the user's choice, the user's own MBTiles or PMTiles file, or a layer of an
    /// online map service chosen in the map's services panel.
    pub enum Basemap {
        Off = "off",
        OpenStreetMap = "openstreetmap",
        Custom = "custom",
        File = "file",
        Service = "service",
    }
}
setting_enum! {
    /// How the map view writes positions (the pointer readout, Copy coordinate).
    pub enum CoordinateFormat { Decimal = "dd", Dms = "dms", Utm = "utm", Mgrs = "mgrs" }
}

setting_enum! {
    /// Window and control appearance.
    pub enum Theme { System = "system", Light = "light", Dark = "dark" }
}
setting_enum! {
    /// What a start without a file shows (a file to open always shows in the viewer).
    pub enum OnStart { StartScreen = "start_screen", Gallery = "gallery", Library = "library" }
}
setting_enum! {
    /// Zoom for a newly opened image.
    pub enum InitialZoom {
        Fit = "fit",
        FitIfLarger = "fit_if_larger",
        ActualSize = "actual_size",
        Fill = "fill",
    }
}
setting_enum! {
    /// What the mouse wheel does without modifiers.
    pub enum WheelBehaviour { Zoom = "zoom", Navigate = "navigate" }
}
setting_enum! {
    /// Zoom animation speed.
    pub enum AnimationSpeed { Relaxed = "relaxed", Normal = "normal", Fast = "fast" }
}
setting_enum! {
    /// Window background behind the image.
    pub enum Background {
        Theme = "theme",
        Black = "black",
        DarkGrey = "dark_grey",
        LightGrey = "light_grey",
        White = "white",
        Custom = "custom",
    }
}
setting_enum! {
    /// How transparent areas are drawn.
    pub enum Transparency {
        Checkerboard = "checkerboard",
        Background = "background",
        Black = "black",
        White = "white",
    }
}
setting_enum! {
    /// Filtering above 100%.
    pub enum HighZoomSmoothing { Auto = "auto", Smooth = "smooth", Pixelated = "pixelated" }
}
setting_enum! {
    /// How medical greys (P-values) reach the display: as sRGB greys, or in equal steps of
    /// the DICOM Grayscale Standard Display Function between the display's black and white.
    pub enum MedicalDisplay { Srgb = "srgb", Gsdf = "gsdf" }
}
setting_enum! {
    /// Double-click on the image.
    pub enum DoubleClick { ToggleZoom = "toggle_zoom", Fullscreen = "fullscreen", Nothing = "nothing" }
}
setting_enum! {
    /// Full-screen style (macOS; other platforms always use borderless).
    pub enum FullscreenMode { Native = "native", Borderless = "borderless" }
}
setting_enum! {
    /// A toolbar button, or a spacer (a flexible gap: spacers share the room left when the
    /// bar spans the window) or separator (a thin divider between groups of buttons).
    pub enum ToolbarItem {
        Open = "open",
        Prev = "prev",
        Next = "next",
        ZoomOut = "zoom_out",
        ZoomLevel = "zoom_level",
        ZoomIn = "zoom_in",
        Fit = "fit",
        RotateLeft = "rotate_left",
        RotateRight = "rotate_right",
        Info = "info",
        Copy = "copy",
        Delete = "delete",
        Palette = "palette",
        Settings = "settings",
        Fullscreen = "fullscreen",
        Slideshow = "slideshow",
        ExportSlideshow = "export_slideshow",
        Mpr = "mpr",
        Discord = "discord",
        Spacer = "spacer",
        Separator = "separator",
    }
}

impl ToolbarItem {
    /// The items a toolbar may hold more than once (the rest are buttons, at most once each).
    pub const REPEATABLE: &'static [&'static str] = &["spacer", "separator"];

    /// A spacer or separator rather than a button.
    pub fn is_layout(self) -> bool {
        matches!(self, Self::Spacer | Self::Separator)
    }
}
setting_enum! {
    /// Text subtitles' fill colour: a preset, or the colour of `subtitle_custom_color`.
    pub enum SubtitleColor { White = "white", Yellow = "yellow", Cyan = "cyan", Green = "green", Custom = "custom" }
}
setting_enum! {
    /// Text subtitles' outline colour, or none.
    pub enum SubtitleOutline { Black = "black", Grey = "grey", White = "white", None = "none" }
}
setting_enum! {
    /// Which subtitles show by themselves when a movie starts: none, forced ones only (signs
    /// and foreign dialogue), or full ones in the subtitle language. Forced subtitles in the
    /// sound's language show in every mode.
    pub enum SubtitleMode { Off = "off", Forced = "forced", Always = "always" }
}
setting_enum! {
    /// Behind text subtitles: nothing, or a translucent dark box.
    pub enum SubtitleBackground { None = "none", Box = "box" }
}
setting_enum! {
    /// How often an animation plays.
    pub enum AnimationLoop { File = "file", Forever = "forever", Once = "once" }
}
setting_enum! {
    /// What happens when sound files, videos or the play queue reach the end.
    pub enum PlayRepeat { Off = "off", All = "all", One = "one" }
}
setting_enum! {
    /// Which volume levelling a sound gets: none, each file's, its album's, or the album's
    /// when an album plays in order and each file's otherwise.
    pub enum SoundLevelling { Off = "off", Track = "track", Album = "album", Auto = "auto" }
}
setting_enum! {
    /// The speakers' channels: what the device says, or stereo, 5.1 or 7.1 as set.
    pub enum SoundChannels { Auto = "auto", Stereo = "stereo", Surround51 = "surround_5_1", Surround71 = "surround_7_1" }
}
setting_enum! {
    /// Frame delays of 10 ms or less: slowed to 100 ms like web browsers do, or played as stored.
    pub enum ShortDelays { Browser = "browser", AsStored = "as_stored" }
}
setting_enum! {
    /// Page order of archives (comic books): from ComicInfo.xml, or forced.
    pub enum ReadingOrder { Auto = "auto", LeftToRight = "left_to_right", RightToLeft = "right_to_left" }
}
setting_enum! {
    /// One page at a time or two side by side (the cover alone): from ComicInfo.xml, or forced.
    pub enum PageLayout { Auto = "auto", Single = "single", Spreads = "spreads" }
}
setting_enum! {
    /// Folder order.
    pub enum SortOrder {
        Natural = "natural",
        DateTaken = "date_taken",
        Modified = "modified",
        Size = "size",
        Type = "type",
    }
}
setting_enum! {
    /// Hold-to-flip acceleration.
    pub enum HoldAcceleration { Off = "off", Gentle = "gentle", Normal = "normal", Fast = "fast" }
}
setting_enum! {
    /// ICC rendering intent.
    pub enum RenderingIntent {
        Perceptual = "perceptual",
        RelativeColorimetric = "relative_colorimetric",
        Saturation = "saturation",
        AbsoluteColorimetric = "absolute_colorimetric",
    }
}
setting_enum! {
    /// The rendering intent of Develop's soft proof into the printer profile.
    pub enum ProofIntent { Perceptual = "perceptual", Relative = "relative" }
}
setting_enum! {
    /// Images without a profile.
    pub enum Untagged { Srgb = "srgb", Display = "display" }
}
setting_enum! {
    /// HDR output: an extended-range surface where the display and graphics API offer one
    /// (`auto`: when the display reports HDR headroom), otherwise tone-mapped to SDR.
    pub enum HdrOutput { Auto = "auto", On = "on", Off = "off" }
}
setting_enum! {
    /// Info panel default view.
    pub enum InfoDetail { Summary = "summary", All = "all" }
}
setting_enum! {
    /// Measurement units.
    pub enum Units { Auto = "auto", Metric = "metric", Imperial = "imperial" }
}
setting_enum! {
    /// GPU adapter preference.
    pub enum GpuAdapter { HighPerformance = "high_performance", LowPower = "low_power" }
}
setting_enum! {
    /// Where work with both a CPU and a GPU (or hardware) implementation runs (DESIGN.md
    /// §12.3.1).
    pub enum PathMode { Automatic = "automatic", PreferGpu = "prefer_gpu", CpuOnly = "cpu_only" }
}
setting_enum! {
    /// One path's switch over the automatic choice.
    pub enum PathOverride { Auto = "auto", On = "on", Off = "off" }
}
setting_enum! {
    /// How Develop draws the picture while a slider is dragged.
    pub enum DragPreview { Automatic = "automatic", Sharp = "sharp", Fast = "fast" }
}
setting_enum! {
    /// Tri-state accessibility switch.
    pub enum SystemOnOff { System = "system", On = "on", Off = "off" }
}
setting_enum! {
    /// Which releases the update check offers.
    pub enum UpdateChannel { Stable = "stable", Beta = "beta" }
}
setting_enum! {
    /// Screen reader verbosity.
    pub enum Verbosity { Concise = "concise", Normal = "normal", Verbose = "verbose" }
}
setting_enum! {
    /// Log level.
    pub enum LogLevel { Error = "error", Warn = "warn", Info = "info", Debug = "debug", Trace = "trace" }
}

setting_enum! {
    /// Which curves the histogram shows.
    pub enum HistogramChannels { Colour = "colour", Luminance = "luminance", Both = "both" }
}
setting_enum! {
    /// The colour map of false colour and its colour bar (DESIGN.md §6.2). Perceptually
    /// uniform and colour-vision-safe except jet, which is kept for users who need it.
    pub enum FalseColourMap {
        Viridis = "viridis",
        Cividis = "cividis",
        Batlow = "batlow",
        Roma = "roma",
        Vik = "vik",
        Oslo = "oslo",
        Lajolla = "lajolla",
        Thermal = "thermal",
        Haline = "haline",
        Solar = "solar",
        Ice = "ice",
        Deep = "deep",
        Dense = "dense",
        Algae = "algae",
        Rain = "rain",
        Speed = "speed",
        Balance = "balance",
        Curl = "curl",
        Phase = "phase",
        Jet = "jet",
    }
}
setting_enum! {
    /// How the colour channels of a multichannel composite are coloured.
    pub enum ChannelColourPreset {
        RedGreen = "red_green",
        MagentaGreen = "magenta_green",
        MagentaYellowCyan = "magenta_yellow_cyan",
    }
}
setting_enum! {
    /// Colour-vision-deficiency simulation of the whole display (never of exports).
    pub enum ColourVision {
        Off = "off",
        Protanopia = "protanopia",
        Deuteranopia = "deuteranopia",
        Tritanopia = "tritanopia",
        Achromatopsia = "achromatopsia",
    }
}

const fn int(min: i64, max: i64, step: i64, unit: Unit) -> Kind {
    Kind::Int {
        min,
        max,
        step,
        unit,
        auto: false,
    }
}

/// An integer where 0 means "automatic".
const fn int_auto(min: i64, max: i64, step: i64, unit: Unit) -> Kind {
    Kind::Int {
        min,
        max,
        step,
        unit,
        auto: true,
    }
}

const fn float(min: f64, max: f64, step: f64) -> Kind {
    float_unit(min, max, step, Unit::None)
}

const fn float_unit(min: f64, max: f64, step: f64, unit: Unit) -> Kind {
    Kind::Float {
        min,
        max,
        step,
        unit,
    }
}

/// Default toolbar, in order (DESIGN.md §11 "On-screen controls").
pub const DEFAULT_TOOLBAR: [ToolbarItem; 13] = [
    ToolbarItem::Prev,
    ToolbarItem::Next,
    ToolbarItem::ZoomOut,
    ToolbarItem::ZoomLevel,
    ToolbarItem::ZoomIn,
    ToolbarItem::Fit,
    ToolbarItem::RotateLeft,
    ToolbarItem::RotateRight,
    ToolbarItem::Info,
    ToolbarItem::Mpr,
    ToolbarItem::Fullscreen,
    ToolbarItem::Settings,
    ToolbarItem::Discord,
];

/// Systems with the freedesktop.org integration (Settings → Integration).
const UNIX_DESKTOPS: &[&str] = &["linux", "freebsd", "openbsd", "netbsd", "dragonfly"];
/// Systems with the recent-files option: the freedesktop ones and Windows.
const RECENT_FILES_OS: &[&str] = &[
    "linux",
    "freebsd",
    "openbsd",
    "netbsd",
    "dragonfly",
    "windows",
];

setting_enum! {
    /// How the gallery lays tiles out (DESIGN §10).
    pub enum GalleryLayout {
        Squares = "squares",
        Justified = "justified",
        Uncropped = "uncropped",
        Details = "details",
    }
}
setting_enum! {
    /// Gallery sort key.
    pub enum GallerySort {
        Name = "name",
        DateTaken = "date_taken",
        Modified = "modified",
        Size = "size",
        Type = "type",
        Dimensions = "dimensions",
    }
}
setting_enum! {
    /// The space compare's error metrics are taken in: linear light as displayed, or encoded
    /// sRGB (what other tools measure on 8-bit files).
    pub enum CompareMetricSpace { Linear = "linear", Srgb = "srgb" }
}
setting_enum! {
    /// Gallery grouping (sticky headers). `target` and `filter` group astronomy subframes by
    /// their `OBJECT` and `FILTER`.
    pub enum GalleryGroup {
        None = "none",
        Day = "day",
        Month = "month",
        Year = "year",
        Event = "event",
        Folder = "folder",
        Camera = "camera",
        Format = "format",
        Rating = "rating",
        Target = "target",
        Filter = "filter",
    }
}

setting_enum! {
    /// How one slide gives way to the next.
    pub enum SlideshowTransition { None = "none", CrossFade = "cross_fade", KenBurns = "ken_burns" }
}
setting_enum! {
    /// What the slideshow's caption shows.
    pub enum SlideshowCaption { Off = "off", Name = "name", NameExif = "name_exif" }
}
setting_enum! {
    /// Which file of a RAW+JPEG pair cull mode shows.
    pub enum CullShow { Jpeg = "jpeg", Raw = "raw" }
}
setting_enum! {
    /// What cull mode does when a name is taken in a slot's folder.
    pub enum CullCollision { Suffix = "suffix", Skip = "skip" }
}
setting_enum! {
    /// The format Convert… writes.
    pub enum ConvertFormat { Jpeg = "jpeg", Png = "png", Webp = "webp", Jxl = "jxl", Avif = "avif", Tiff = "tiff" }
}
setting_enum! {
    /// TIFF compression for Convert….
    pub enum ConvertTiffCompression { Deflate = "deflate", None = "none" }
}
setting_enum! {
    /// Bits per sample for Convert… (automatic: 16 when the source has more than 8 and the
    /// format holds them).
    pub enum ConvertDepth { Auto = "auto", Eight = "8", Sixteen = "16" }
}
setting_enum! {
    /// How Convert… resizes.
    pub enum ConvertResize { None = "none", Fit = "fit", LongEdge = "long_edge", Percent = "percent" }
}
setting_enum! {
    /// Colour for Convert…: keep the profile, or convert to sRGB.
    pub enum ConvertColour { Keep = "keep", Srgb = "srgb" }
}
setting_enum! {
    /// HDR for Convert…: kept where the format holds it, or tone-mapped.
    pub enum ConvertHdr { Keep = "keep", ToneMap = "tone_map" }
}
setting_enum! {
    /// Metadata Convert… writes.
    pub enum ConvertMetadata { All = "all", NoLocation = "no_location", Strip = "strip" }
}
setting_enum! {
    /// What Convert… does when a name is taken.
    pub enum ConvertConflict { Suffix = "suffix", Skip = "skip" }
}
setting_enum! {
    /// What happens when a memory card, camera or phone with photos is connected.
    pub enum ImportOnCard { Ask = "ask", Open = "open", Nothing = "nothing" }
}

/// Folders the Library skips by name, by default.
pub const LIBRARY_SKIP_FOLDERS: &str =
    "node_modules, target, build, __pycache__, site-packages, corpus, corpora";
/// Folders holding one of these are skipped by the Library, by default.
pub const LIBRARY_SKIP_MARKED: &str = ".git, pyvenv.cfg, CACHEDIR.TAG";

crate::settings! {
    general: General {
        language: LanguagePreference = LanguagePreference::System;
        region_formats: RegionFormats = RegionFormats::FollowLanguage;
        theme: Theme = Theme::System;
        on_start: OnStart = OnStart::StartScreen;
        // The "Things to try" cards of the start screen ("Hide these tips" turns them off,
        // "Show welcome" in the palette back on).
        start_tips: bool = true;
        // One-time hints (a quiet toast the first time something useful is near).
        tips: bool = true;
        // The one-time hints already given, not shown in the window: a comma-separated list
        // of their ids (`gallery,compare`).
        tips_shown: String = String::new();
        // The name on stamps and markup (empty: none).
        your_name: String = String::new();
        single_instance: bool = true, restart = true;
        file_associations: Action = Action;
    }
    profile: Profiles {
        capabilities: Vec<ProfileCapability> = vec![ProfileCapability::General],
            kind = Kind::List { options: ProfileCapability::OPTIONS, repeat: &[] };
        // What was offered once and is not offered again, not shown in the window:
        // `question` (the first-run sheet) and the capabilities whose toolbar chip offered
        // "Always" (`question,medical`).
        shown: String = String::new();
    }
    viewing: Viewing {
        initial_zoom: InitialZoom = InitialZoom::FitIfLarger;
        // Zoomed in, the next and previous image show at the same zoom and the same part of
        // the frame (a burst compared at 100 %); at fit they open as `initial_zoom` says.
        keep_zoom: bool = false;
        wheel_behaviour: WheelBehaviour = WheelBehaviour::Zoom;
        zoom_animation: bool = true;
        zoom_animation_speed: AnimationSpeed = AnimationSpeed::Normal;
        background: Background = Background::Theme;
        background_color: Color = Color::rgb(0x20, 0x20, 0x20);
        fullscreen_background: Color = Color::rgb(0, 0, 0);
        transparency: Transparency = Transparency::Checkerboard;
        pixel_grid_threshold: i64 = 800, kind = int(200, 6400, 100, Unit::Percent);
        smoothing_high_zoom: HighZoomSmoothing = HighZoomSmoothing::Auto;
        double_click: DoubleClick = DoubleClick::ToggleZoom;
        fullscreen_on_open: bool = false;
        fullscreen_mode: FullscreenMode = FullscreenMode::Native, os = &["macos"];
        toolbar_autohide: bool = true;
        toolbar_autohide_delay: f64 = 1.5, kind = float_unit(0.5, 10.0, 0.25, Unit::Seconds);
        toolbar_items: Vec<ToolbarItem> = DEFAULT_TOOLBAR.to_vec(),
            kind = Kind::List { options: ToolbarItem::OPTIONS, repeat: ToolbarItem::REPEATABLE };
        toolbar_drag: bool = true;
        animation_autoplay: bool = true;
        animation_loop: AnimationLoop = AnimationLoop::File;
        animation_short_delays: ShortDelays = ShortDelays::Browser;
        // A movie's sound: the volume and mute keys and buttons change these.
        video_volume: i64 = 100, kind = int(0, 100, 5, Unit::Percent);
        video_muted: bool = false;
        // Videos longer than three minutes start where they were left.
        video_resume: bool = true;
        forget_playback: Action = Action;
        // Subtitles in this language (a code such as `he`) show by themselves; empty: none.
        // Choosing a subtitle track sets it.
        subtitle_language: String = String::new();
        // Which subtitles start by themselves; forced ones in the sound's language always do.
        subtitle_mode: SubtitleMode = SubtitleMode::Always;
        // Subtitles for the deaf and hard of hearing (SDH, closed captions) come first.
        subtitle_prefer_sdh: bool = false;
        // Text subtitles' size, against the default (a share of the picture's height).
        subtitle_size: i64 = 100, kind = int(50, 250, 10, Unit::Percent);
        // Subtitles' distance from the bottom of the picture (a share of its height).
        // Cues placed by the file (a position, or at the top) keep their place.
        subtitle_position: i64 = 5, kind = int(0, 90, 1, Unit::Percent);
        // Text subtitles' colour; `custom` takes `subtitle_custom_color`.
        subtitle_color: SubtitleColor = SubtitleColor::White;
        subtitle_custom_color: Color = Color::rgb(0xFF, 0xFF, 0xFF);
        subtitle_outline: SubtitleOutline = SubtitleOutline::Black;
        subtitle_background: SubtitleBackground = SubtitleBackground::None;
        // A font family by its installed name; empty: the interface's font. Scripts it
        // lacks fall back to other fonts.
        subtitle_font: String = String::new(), kind = Kind::Pick;
        // The colour, outline, size and font above apply to every cue, over the file's own.
        subtitle_override_styles: bool = false;
        // The sound track in this language plays first; empty: the movie's own choice.
        sound_language: String = String::new();
        // A sound track with audio description (the picture described) comes first.
        sound_prefer_description: bool = false;
        // When a video ends, the next video of the folder plays.
        video_play_next: bool = false;
        // When a sound file ends, the next sound file of the folder plays (the queue, when
        // one plays, always plays on).
        sound_play_next: bool = true;
        // At the end: stop (off), start over (all) or play the same item again (one).
        play_repeat: PlayRepeat = PlayRepeat::Off;
        // Sound files, videos and the queue play in a random order, each once a round.
        play_shuffle: bool = false;
        // What a sound file's page shows: `waveform`, `spectrogram`, `spectrum`, `meters`,
        // `loudness`, comma separated, and `curve` for the spectrum drawn as a curve. The
        // page's choices and the `w` key change it.
        sound_visuals: String = String::from("waveform,spectrogram");
        // Sound (specular-video's chain, every movie and sound file): volume levelling from
        // ReplayGain, R128 and Sound Check tags, optionally measured for untagged files.
        sound_levelling: SoundLevelling = SoundLevelling::Auto;
        sound_levelling_preamp: f64 = 0.0, kind = float_unit(-12.0, 12.0, 0.5, Unit::Decibels);
        sound_prevent_clipping: bool = true;
        sound_measure_untagged: bool = false;
        // Dialogue boost (night mode): compression, a centre lift and speech emphasis; the
        // player's sound menu turns it on and off.
        sound_dialogue_boost: bool = false;
        sound_dialogue_strength: i64 = 50, kind = int(0, 100, 5, Unit::Percent);
        // The graphic equaliser; its panel (not shown in the window) sets the preset, the
        // ten bands (dB, spaces between), the preamp and the user's presets (one per line,
        // `name = ten gains`).
        sound_equaliser: bool = false;
        sound_eq_preset: String = String::from("flat");
        sound_eq_bands: String = String::new();
        sound_eq_preamp: f64 = 0.0, kind = float_unit(-12.0, 12.0, 0.5, Unit::Decibels);
        sound_eq_user_presets: String = String::new();
        // Balance (−100 left only, 100 right only) and both channels as one.
        sound_balance: i64 = 0, kind = int(-100, 100, 5, Unit::Percent);
        sound_mono: bool = false;
        // A crossfade into the next sound file (never between files of one album).
        sound_crossfade: f64 = 0.0, kind = float_unit(0.0, 12.0, 0.5, Unit::Seconds);
        // A sound file's lyrics (a `.lrc` beside it, or its tags') show in a column on its
        // page; the `y` key changes it.
        sound_lyrics: bool = true;
        // A sound file's page takes its colours from the cover (text kept readable; the
        // visuals' own colours stay).
        sound_cover_colours: bool = true;
        // A sound file's page shows its cover, blurred, behind it.
        sound_backdrop: bool = true;
        // The sound's device by its id (empty: the system's default, followed when it
        // changes); the player's sound menu chooses it too.
        sound_output_device: String = String::new(), kind = Kind::Pick;
        // The speakers' channels: 5.1 and 7.1 sound plays as is on that many, else folds
        // to stereo.
        sound_channels: SoundChannels = SoundChannels::Auto;
    }
    browsing: Browsing {
        sort_order: SortOrder = SortOrder::Natural;
        sort_descending: bool = false;
        loop_folder: bool = false;
        include_subfolders: bool = false;
        hold_acceleration: HoldAcceleration = HoldAcceleration::Normal;
        confirm_trash: bool = false;
        arrows_follow_reading_direction: bool = false;
        reading_order: ReadingOrder = ReadingOrder::Auto;
        page_layout: PageLayout = PageLayout::Auto;
        // Rotating a file (Alt+[ / Alt+]): a JPEG whose size is not a multiple of its block
        // size turns losslessly only by trimming the last partial blocks; without consent
        // here its orientation tag is changed instead. And whether the file keeps its
        // modification date.
        rotate_trim_edges: bool = false;
        rotate_keep_modified: bool = false;
        // Copy / move to folder: the folders used last, newest first, one per line. Not
        // shown in the window.
        recent_folders: String = String::new();
    }
    gallery: Gallery {
        layout: GalleryLayout = GalleryLayout::Squares;
        thumbnail_size: i64 = 160, kind = int(64, 512, 8, Unit::None);
        captions: bool = true;
        badges: bool = true;
        // Develop: the "Edited" badge on photos with a Specular edit.
        edited_badges: bool = true;
        sort_order: GallerySort = GallerySort::Name;
        sort_descending: bool = false;
        group_by: GalleryGroup = GalleryGroup::None;
        per_folder_view: bool = true;
        open_folders_in_gallery: bool = true;
        escape_opens_gallery: bool = false;
        wrap_around: bool = false;
        show_hidden: bool = false;
        show_other_files: bool = false;
        show_archives: bool = false;
        subfolder_depth: i64 = 0, kind = int(0, 16, 1, Unit::None);
        stacks: bool = false;
        info_overlay: bool = false;
        prefer_embedded: bool = true;
        // Develop: edited raws' thumbnails developed at 1024 px, then reduced (sharper,
        // about 40 ms more each); off, at 512 px (`specular_develop::thumb::raw_long`).
        sharper_raw_thumbnails: bool = true;
        rating_sidecars: bool = true;
        // The metadata editor: also write IPTC and XMP into JPEG, TIFF and DNG files (else
        // only XMP sidecars).
        metadata_into_files: bool = false;
        cache_size: i64 = 1024, kind = int(64, 1 << 20, 64, Unit::Mebibytes);
        cache_location: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        clear_cache: Action = Action;
        // Compare (DESIGN §6.3): the space MAE, MSE, PSNR and SSIM are taken in, and the
        // fastest auto-flicker in image changes per second (photosensitivity: 3 unless the
        // user raises it).
        compare_metric_space: CompareMetricSpace = CompareMetricSpace::Linear;
        compare_flicker_limit: f64 = 3.0, kind = float(1.0, 10.0, 0.5);
    }
    library: Library {
        // Folders the Library does not look in: by name, and those holding one of the
        // marks (comma-separated). Hidden folders are never looked in. The defaults are
        // `specular_library::skip`'s (the app's tests check they agree).
        skip_folders: String = LIBRARY_SKIP_FOLDERS.into();
        skip_marked: String = LIBRARY_SKIP_MARKED.into();
        // The date timeline above the Library's grid.
        show_timeline: bool = true;
        // Folders (`;` between them) where other apps' sidecars (darktable, Lightroom…) have
        // the last word on marks and keywords; elsewhere they fill in what the user has not set.
        prefer_sidecars_in: String = String::new();
        // Local copies for offline editing: the most their store in the data folder may
        // take (`specular_library::local`).
        local_copies_limit: i64 = 20480, kind = int(1024, 1 << 22, 1024, Unit::Mebibytes);
    }
    slideshow: Slideshow {
        interval: i64 = 5, kind = int(1, 60, 1, Unit::Seconds);
        shuffle: bool = false;
        repeat: bool = false;
        include_subfolders: bool = false;
        transition: SlideshowTransition = SlideshowTransition::CrossFade;
        caption: SlideshowCaption = SlideshowCaption::Off;
        monitor: MonitorChoice = MonitorChoice::Auto;
    }
    cull: Cull {
        auto_enter: bool = true;
        auto_advance: bool = true;
        show: CullShow = CullShow::Jpeg;
        bursts: bool = true;
        burst_gap: f64 = 1.0, kind = float_unit(0.1, 10.0, 0.1, Unit::Seconds);
        watch: bool = true;
        follow_newest: bool = false;
        slot_1: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        slot_2: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        slot_3: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        collisions: CullCollision = CullCollision::Suffix;
        // Tethered capture: where shots are downloaded (unset: Pictures › Tethered) and
        // their names (the Import rename tokens; empty keeps the camera's names).
        tether_folder: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        tether_rename: String = String::new();
    }
    medical: Medical {
        keep_window: bool = false;
        orientation_labels: bool = true;
        wheel_steps_frames: bool = true;
        display_function: MedicalDisplay = MedicalDisplay::Srgb;
        display_black: f64 = 0.5, kind = float_unit(0.01, 50.0, 0.05, Unit::Luminance);
        display_white: f64 = 350.0, kind = float_unit(50.0, 4000.0, 10.0, Unit::Luminance);
    }
    maps: Maps {
        coordinate_format: CoordinateFormat = CoordinateFormat::Decimal;
        // Lengths and areas on maps; automatic follows `metadata.units` (then the region).
        units: Units = Units::Auto;
        // Online map services (WMS, WMTS, WFS, OGC API, STAC): off by default; each server
        // only after the explanation of what it sees.
        services: bool = false;
        // The servers the user agreed to, not shown in the window: comma-separated hosts.
        services_allowed: String = String::new();
    }
    convert: Convert {
        format: ConvertFormat = ConvertFormat::Jpeg;
        jpeg_quality: i64 = 90, kind = int(1, 100, 1, Unit::None);
        jpeg_full_chroma: bool = false;
        png_effort: i64 = 6, kind = int(1, 9, 1, Unit::None);
        webp_lossless: bool = true;
        webp_quality: i64 = 85, kind = int(1, 100, 1, Unit::None);
        webp_effort: i64 = 5, kind = int(1, 9, 1, Unit::None);
        jxl_lossless: bool = true;
        jxl_quality: i64 = 90, kind = int(1, 100, 1, Unit::None);
        jxl_effort: i64 = 1, kind = int(1, 9, 1, Unit::None);
        avif_quality: i64 = 80, kind = int(1, 100, 1, Unit::None);
        avif_effort: i64 = 1, kind = int(1, 9, 1, Unit::None);
        tiff_compression: ConvertTiffCompression = ConvertTiffCompression::Deflate;
        depth: ConvertDepth = ConvertDepth::Auto;
        resize: ConvertResize = ConvertResize::None;
        width: i64 = 1920, kind = int(16, 65535, 16, Unit::None);
        height: i64 = 1080, kind = int(16, 65535, 16, Unit::None);
        long_edge: i64 = 2048, kind = int(16, 65535, 16, Unit::None);
        percent: i64 = 50, kind = int(1, 400, 5, Unit::Percent);
        upscale: bool = false;
        colour: ConvertColour = ConvertColour::Keep;
        hdr: ConvertHdr = ConvertHdr::Keep;
        metadata: ConvertMetadata = ConvertMetadata::All;
        folder: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        subfolder: String = "converted".into();
        pattern: String = "{name}".into();
        conflicts: ConvertConflict = ConvertConflict::Suffix;
    }
    import: Import {
        on_card: ImportOnCard = ImportOnCard::Ask;
        // Unset: the Pictures folder.
        destination: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        folder_pattern: String = "{year}/{year}-{month}-{day}".into();
        rename_pattern: String = String::new();
        keep_pairs: bool = true;
        low_res_videos: bool = false;
        eject: bool = false;
    }
    colour: Colour {
        management: bool = true;
        display_profile: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        rendering_intent: RenderingIntent = RenderingIntent::RelativeColorimetric;
        black_point_compensation: bool = true;
        untagged_images: Untagged = Untagged::Srgb;
        dithering: bool = true;
        hdr_output: HdrOutput = HdrOutput::Auto;
        // OpenColorIO for scene-linear images (EXR, Radiance HDR, PFM): the config file, else
        // the one $OCIO names; never bundled. Display, view and input colour space are the
        // config's names (empty: its first display, its first view, its scene_linear role),
        // offered by the application once the config is read.
        ocio_config: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        ocio_display: String = String::new(), kind = Kind::Pick;
        ocio_view: String = String::new(), kind = Kind::Pick;
        ocio_input: String = String::new(), kind = Kind::Pick;
        // Develop's Soft proof card remembers its choices here, per user and never
        // in an edit or sidecar: the printer profile, the intent, black point compensation,
        // paper white and ink black. Set by the card; not shown in the window.
        proof_profile: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        proof_intent: ProofIntent = ProofIntent::Perceptual;
        proof_bpc: bool = false;
        proof_paper: bool = false;
        proof_ink: bool = false;
    }
    inspection: Inspection {
        clip_highlight: i64 = 254, kind = int(128, 255, 1, Unit::None);
        clip_shadow: i64 = 1, kind = int(0, 127, 1, Unit::None);
        highlight_color: Color = Color::rgb(255, 0, 0);
        shadow_color: Color = Color::rgb(0, 0x7c, 255);
        peaking_sensitivity: i64 = 50, kind = int(1, 100, 1, Unit::Percent);
        peaking_color: Color = Color::rgb(255, 0, 255);
        alpha_matte_color: Color = Color::rgb(0, 200, 0);
        histogram_channels: HistogramChannels = HistogramChannels::Both;
        histogram_log: bool = false;
        false_colour_map: FalseColourMap = FalseColourMap::Viridis;
        colour_bar: bool = true;
        channel_colours: ChannelColourPreset = ChannelColourPreset::RedGreen;
        colour_vision: ColourVision = ColourVision::Off;
        auto_range: bool = true;
    }
    metadata: Metadata {
        show_info_on_open: bool = false;
        info_detail: InfoDetail = InfoDetail::Summary;
        reverse_geocoding: bool = true;
        hide_gps: bool = false;
        units: Units = Units::Auto;
        // The map view's base map over the built-in offline one: off by default; online
        // sources only after the one-time explanation of what the tile server sees.
        basemap: Basemap = Basemap::Off;
        basemap_url: String = String::new();
        basemap_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        basemap_cache: i64 = 256, kind = int(32, 8192, 32, Unit::Mebibytes);
        // The tile servers the user agreed to after the explanation, not shown in the
        // window: a comma-separated list of host names.
        basemap_allowed: String = String::new();
        // The map service layer the base map shows (Basemap::Service), as the services
        // panel wrote it; not shown in the window.
        basemap_service: String = String::new();
        // Develop edits are also written next to the photo as `<file>.<ext>.specular.xmp`;
        // off, they stay in the catalogue only.
        develop_sidecar: bool = true;
        // Lightroom settings (`crs:`) found with a photo that has no Develop edit are imported
        // (approximately) when it opens in Develop; off, Develop offers a button.
        develop_import_lightroom: bool = false;
        // Develop's flat and dark frames: a folder searched for raws whose path
        // names "flat" or "dark", for automatic calibration; nothing is fetched.
        develop_calibration: Option<PathBuf> = None, kind = Kind::Path { directory: true };
    }
    ai: Ai {
        // The Local AI pack (DESIGN.md "Local AI pack"): its page, then per feature a switch
        // and its model size: Recommended (the default model where it fits this computer, else
        // a smaller one that fits; see specular_ai::choose) or a named size. The page
        // shows a size only for a feature whose offer has more than one (only Search does
        // today). Switching a feature on installs it (at the chosen size); an install from
        // the page switches it on, so every feature, People included, is on once installed.
        // Off stops using it; the models stay until removed. Nothing runs before an install.
        pack: Action = Action;
        search: bool = false;
        search_size: ModelSize = ModelSize::Recommended;
        text: bool = false;
        text_size: ModelSize = ModelSize::Recommended;
        culling: bool = false;
        culling_size: ModelSize = ModelSize::Recommended;
        people: bool = false;
        people_size: ModelSize = ModelSize::Recommended;
        // Speech to text (generated subtitles, Whisper): nothing runs until "Generate
        // subtitles" is chosen in the player.
        speech: bool = false;
        speech_size: ModelSize = ModelSize::Recommended;
        // Develop's AI masks (subject, sky, object at a click): nothing runs until a
        // mask row asks for one.
        masks: bool = false;
        masks_size: ModelSize = ModelSize::Recommended;
        // Develop's AI denoise: nothing runs until Denoise is set to AI and the
        // photo is viewed at 100 % or exported.
        denoise: bool = false;
        denoise_size: ModelSize = ModelSize::Recommended;
        // Develop's "Remove (AI)" spots and depth tools: nothing runs until a spot,
        // a depth row or a depth module asks.
        remove: bool = false;
        remove_size: ModelSize = ModelSize::Recommended;
        depth: bool = false;
        depth_size: ModelSize = ModelSize::Recommended;
        // Sideloading (offline and air-gapped machines): pick model files or a folder; each
        // file is checked against the SHA-256 in the shipped manifest, then copied in.
        install_from_file: Action = Action;
        install_from_folder: Action = Action;
    }
    performance: Performance {
        ram_cache: i64 = 0, kind = int_auto(256, 1 << 20, 256, Unit::Mebibytes);
        vram_cache: i64 = 0, kind = int_auto(128, 1 << 18, 128, Unit::Mebibytes);
        prefetch_depth: i64 = 2, kind = int(0, 8, 1, Unit::Images);
        decoder_threads: i64 = 0, kind = int_auto(1, 256, 1, Unit::Threads);
        gpu_adapter: GpuAdapter = GpuAdapter::HighPerformance, restart = true;
        processing: PathMode = PathMode::Automatic;
        // Develop's drafts while a slider is dragged (specular_develop::drag): Automatic draws
        // smaller pictures only when frames take longer than the display's refresh; Sharp
        // always the view's size; Fast always half of it.
        drag_preview: DragPreview = DragPreview::Automatic;
        display_cache: bool = true;
        display_cache_size: i64 = 4096, kind = int(256, 1 << 20, 256, Unit::Mebibytes);
        display_cache_location: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        fast_simd: bool = true;
        fast_dct_scaling: bool = true;
        fast_parallel_decode: bool = true;
        fast_restart_index: bool = true;
        fast_yuv_upload: PathOverride = PathOverride::Auto;
        fast_async_io: bool = true;
        fast_mmap: bool = true;
        hardware_decode: PathOverride = PathOverride::Auto;
        frame_time_hud: bool = false;
        stall_warning: f64 = 2.0, kind = float_unit(0.5, 60.0, 0.5, Unit::Seconds);
        insights: bool = true;
        insights_storage: bool = true;
        insights_decoder: bool = true;
        insights_restore: Action = Action;
    }
    accessibility: Accessibility {
        reduce_motion: SystemOnOff = SystemOnOff::On;
        increase_contrast: SystemOnOff = SystemOnOff::System;
        text_scale: f64 = 1.0, kind = float(0.8, 2.0, 0.05);
        ui_scale: f64 = 1.0, kind = float(0.5, 3.0, 0.05);
        focus_ring_always: bool = false;
        screen_reader_verbosity: Verbosity = Verbosity::Normal;
        announce_cursor: bool = false;
    }
    keys: Keys {
        sequence_timeout: f64 = 1.0, kind = float_unit(0.3, 3.0, 0.1, Unit::Seconds);
        bindings: Action = Action;
    }
    // Gamepads and 3Dconnexion SpaceMouse (specular-devices): off by default; nothing is
    // opened until turned on. The page shows what is connected in a panel under "Look again"
    // (the app builds it). Buttons bind through the keymap (`Pad-A`, `Puck-1`).
    devices: Devices {
        enabled: bool = false;
        // Deflection ignored around the centre, in percent of full.
        dead_zone: i64 = 15, kind = int(0, 50, 1, Unit::Percent);
        speed: i64 = 100, kind = int(25, 400, 25, Unit::Percent);
        look_again: Action = Action;
    }
    integration: Integration {
        default_viewer: Action = Action, os = UNIX_DESKTOPS;
        thumbnailer: Action = Action, os = UNIX_DESKTOPS;
        single_instance: Action = Action, os = UNIX_DESKTOPS;
        file_manager_actions: Action = Action, os = UNIX_DESKTOPS;
        search_provider: Action = Action, os = UNIX_DESKTOPS;
        url_scheme: Action = Action, os = UNIX_DESKTOPS;
        recent_files: Action = Action, os = RECENT_FILES_OS;
        // The types added under "More file types" (comma-separated MIME types), shown as
        // tick boxes under the default viewer, not as a row of its own.
        extra_default_types: String = String::new();
        // Remote control and events: the private per-user socket other programs drive the
        // viewer through and hear what happens in it (`specular ipc`, docs/ipc.md). Never a
        // network port. Off: no socket, and no single-instance hand-over through it. The
        // hooks of settings.toml (`[[hooks]]`) are listed in a panel under it (the app
        // builds it).
        ipc: bool = true, restart = true;
        // SAMP (astronomy applications such as DS9, TOPCAT and Aladin exchanging images,
        // tables and sky positions): off by default; only a hub on this computer is used.
        samp: bool = false;
        // Plate solvers the user installed (never bundled or downloaded): a program picked
        // by hand, else `astap_cli`/`astap` and `solve-field` on PATH or where their
        // installers put them.
        astap_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        solve_field_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
    }
    // Optional decoders: other vendors' free software that opens a few formats (Blackmagic
    // RAW through the Blackmagic RAW SDK, DWG through the ODA File Converter), never bundled
    // or downloaded. Each is looked for where its installer puts it and used when found,
    // unless switched off here; a path picked by hand (Locate…) replaces the search. The page
    // shows them as one panel under "Look again" (the app builds it); the switches and paths
    // are not rows of their own.
    decoders: Decoders {
        look_again: Action = Action;
        // Formats chosen for extensions several formats share ("Which kind of file is …?"):
        // kept in `file-kinds.tsv` in the data folder, listed in a panel under this button
        // (the app builds it), which forgets them all after asking.
        file_kinds: Action = Action;
        braw: bool = true;
        braw_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        dwg: bool = true;
        dwg_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
        // STEP and IGES through OpenCASCADE in the sandboxed decode host; the folder holds
        // its libraries when they are not where the system looks.
        occt: bool = true;
        occt_path: Option<PathBuf> = None, kind = Kind::Path { directory: true };
        // Zeiss CZI through libCZI in the sandboxed decode host; the folder holds its library
        // when it is not where the system looks.
        czi: bool = true;
        czi_path: Option<PathBuf> = None, kind = Kind::Path { directory: true };
    }
    // Drawings (DXF, DWG): folders holding SHX (AutoCAD shape) fonts, separated by `;`,
    // searched after the drawing's own folder. Specular never bundles or downloads them;
    // styles whose font is not found are drawn with a substitute and listed in Info.
    drawings: Drawings {
        shx_folders: String = String::new();
    }
    // The no-network switch (specular-core `net::off`): on, nothing in Specular opens a
    // connection. `--no-network`, SPECULAR_NO_NETWORK=1 and the system policy file turn it
    // on too; the page then says so and the switch cannot turn the network back on.
    privacy: Privacy {
        no_network: bool = false;
        // yt-dlp (never bundled or downloaded by Specular): off by default; on, a web page's
        // address opened as a network stream is handed to it. Empty path: the one on PATH.
        ytdlp: bool = false;
        ytdlp_path: Option<PathBuf> = None, kind = Kind::Path { directory: false };
    }
    // Off by default: Specular never connects for updates unless asked (once a day, a plain
    // GET of the release feed, no identifiers; specular-diag's `update`).
    updates: Updates {
        check: bool = false;
        channel: UpdateChannel = UpdateChannel::Stable;
        check_now: Action = Action;
    }
    advanced: Advanced {
        portable_mode: bool = false, restart = true, stored = false;
        log_level: LogLevel = LogLevel::Warn;
        developer_mode: bool = false;
        // The local audit log (DESIGN.md §7): opened DICOM images, exports, anonymisation and
        // prints as JSON lines in `audit.jsonl` in the data folder. Off by default; nothing is
        // sent anywhere.
        audit_log: bool = false;
        hw_report: Action = Action;
        renderer_info: Action = Action;
        edit_settings_file: Action = Action;
        import: Action = Action;
        export: Action = Action;
        reset_all: Action = Action;
    }
}
