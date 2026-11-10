//! Specular's chrome, drawn with `specular-gfx`: the auto-hiding toolbar with tooltips and
//! readouts, edge arrows, the info panel, the command palette, the cheat sheet, toasts, the
//! performance HUD, the settings window and the language picker.
//!
//! The chrome is a small retained state machine with immediate-mode drawing: [`Chrome::draw`]
//! lays everything out for the current [`ViewerState`] and records hit rectangles, which
//! [`Chrome::input`] uses for the next event. Every visible string comes from the Fluent
//! catalogues; the layout mirrors for right-to-left languages (the image never does).

mod a11y;
pub mod a11y_audit;
mod about;
mod ai_page;
mod bar_drag;
mod cast;
mod cheatsheet;
pub mod choice;
pub mod config_editor;
mod context_menu;
mod ctx;
pub mod cursor_readout;
pub mod curve;
mod decoder_card;
pub mod develop_panel;
mod diag_sheet;
mod dicom_tags;
mod downloads;
pub mod eq_panel;
mod focus;
pub mod goto_time;
mod help;
mod history;
mod hud;
mod info;
mod insights;
mod inspect;
mod keys_edit;
pub mod line_edit;
pub mod mask_handles;
mod motion;
mod overlay;
mod pages;
mod palette;
mod player;
mod plot;
mod presenting;
mod profile_sheet;
mod queue_panel;
pub mod renderer_info;
mod report;
pub mod scopes;
mod scrollbar;
mod setting_field;
mod settings;
mod settings_panel;
pub mod slider;
mod studies;
pub mod subtitles;
pub mod text_buffer;
pub mod toml_lex;
mod toolbar;
mod toolbar_edit;
mod transport;
mod volume;
mod welcome;
pub mod wheels;
mod work;

use line_edit::{Filter, LineEdit, PendingClipboard};
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::{I18n, t};
use specular_input::{Action, ActionState, ImageState, Key, KeyCombo, Keymap, NamedKey};
use specular_inspect::{ChannelKey, ChannelView, Cvd};
use specular_metadata::MetadataReport;
use specular_settings::{Settings, SettingsStore};
use specular_theme::{Color, Palette, durations, metrics};

pub use a11y::{strip_isolates, text_run, text_run_sel};
pub use accesskit;
pub use ai_page::{
    AiAddon, AiComing, AiEngine, AiFeatureRow, AiLicence, AiModelInfo, AiPage, AiPerson,
    AiQuestion, AiRowAction, AiRowState, AiSize, AiUnfit,
};
/// Setting keys of the Local AI page.
pub mod ai_keys {
    pub use crate::ai_page::{FEATURES, PACK, PEOPLE, SIZES, size_key};
}
pub use cheatsheet::{SheetMode, digit_run};
use ctx::Target;
pub use ctx::{Ctx, tooltip as draw_tooltip};
pub use decoder_card::DecoderCard;
pub use diag_sheet::{CrashSheet, DiagEntry, DiagSheet};
pub use dicom_tags::{DicomTags, TagLine};
pub use downloads::{DownloadRow, DownloadState, Downloads, DownloadsForm};
pub use insights::{InsightItem, InsightStage, Insights};
pub use inspect::{
    Inspect, InspectTools, LOUPE as PIXEL_LOUPE, PixelInfo, SourceState, channel_colours,
    channel_colours_label, channel_label, colour_map, colour_map_name, colour_vision, cvd_label,
    pixel_columns, pixel_text,
};
pub use overlay::Overlays;
pub use player::{
    CastView, DELAY_STEP_MS, Knob, PICTURE, PlayerCmd, PlayerMenu, PlayerView, SPEEDS, delay_text,
    picture_text,
};
pub use plot::{CaliperKind, PlotCell, PlotGrid, PlotSheet, PlotTrace, nice_ticks};
pub use queue_panel::{
    PlayOrder, QueueCmd, QueueRow, QueueTarget, QueueView, repeat_label, shuffle_label,
};
pub use report::{HwReport, ReportPath, ReportRow, ReportSection, ReportTable, ReportTask, Tone};
pub use settings::{SettingStatus, warm_up_texts};
pub use studies::{Studies, StudyKind, StudyLine};
pub use welcome::{
    RecentFolder, Tip, TipKind, Welcome, WelcomeCommand, compare_key, tip_later, tips,
};
pub use work::{WorkChip, WorkPanel, WorkRow};

/// The name an action goes by in Settings → Keys (and the key sequence hint).
pub fn action_label(a: Action) -> String {
    keys_edit::action_name(a)
}
pub use settings_panel::{CheckState, PanelAction, PanelEvent, PanelRow, SettingPanel};

/// Pointer buttons the chrome cares about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
    Middle,
}

/// Input in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    PointerMove {
        x: f32,
        y: f32,
    },
    PointerLeave,
    PointerButton {
        x: f32,
        y: f32,
        down: bool,
        button: Button,
    },
    /// Scroll in lines (positive `dy` scrolls content up, like a wheel turned away).
    Wheel {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    Key {
        combo: KeyCombo,
        repeat: bool,
    },
    /// A key let go (ends Enter or Space held on a focused Previous or Next).
    KeyUp {
        combo: KeyCombo,
    },
    Text(String),
    /// IME composition in progress (empty: none). `Text` carries the committed result.
    Preedit(String),
    /// The modifier keys now held (Shift+click in a field extends its selection).
    Modifiers(specular_input::Mods),
}

/// What the chrome asks the application to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Action(Action),
    /// The Undo button of a toast was pressed.
    Undo(u64),
    /// An action entry of the settings window ("Export settings…").
    SettingAction(&'static str),
    /// Choose… of a path setting: the platform's file dialog (`directory`: a folder picker);
    /// the application stores what is picked under `key`.
    ChooseSettingPath {
        key: &'static str,
        directory: bool,
    },
    /// A button of the panel under this setting ([`Chrome::set_setting_panel`]).
    SettingPanel(&'static str, PanelEvent),
    /// The move-to-trash confirmation was accepted.
    ConfirmTrash,
    /// Show the image at this position (0-based) of the folder.
    GoTo(usize),
    /// The Copy button of the hardware report was pressed.
    CopyReport,
    /// The motion badge was pressed: play the still's clip, or go back to the still.
    Motion,
    /// "Show on map" in the info panel: the gallery's map view, centred on this image.
    ShowOnMap,
    /// "Edit metadata…" in the info panel: the metadata editor for this image.
    EditMetadata,
    /// The viewer's back button: back to the gallery ([`Chrome::back_to_gallery`]).
    BackToGallery,
    /// Pause or resume the slideshow on another display ([`Chrome::set_presenting_on`]).
    PresentingPause,
    /// Stop the slideshow on another display, or close the viewer there.
    PresentingStop,
    /// Put this text on the clipboard ("Copy value" of a metadata field: the whole value, even
    /// when the panel shows it shortened).
    CopyText(String),
    /// An application entry of the palette was chosen ([`PaletteItem::id`]).
    Item(u64),
    /// Previous or Next on the toolbar, held down this long (seconds), repeats: go one step
    /// (or more, the longer it is held) back or forward, as a held arrow key does.
    NavHold {
        next: bool,
        held: f64,
    },
    /// The held Previous or Next was let go.
    NavHoldEnd,
    /// Full screen from the zoom presets: a movie's full-screen play mode, else the window
    /// full screen with the image fitted. Never sent while already full screen.
    EnterFullscreen,
    /// A button of the performance insights (the indicator, the panel), with the key the
    /// application gave it ([`Insights`]); "open" and "hide" are the indicator's.
    Insight(String),
    /// A button of the background-work panel (Pause, Resume, a job's settings), with the key
    /// the application gave it ([`WorkRow`]).
    Work(String),
    /// Stop on a row of the "Downloads" panel, with the key the application gave it
    /// ([`DownloadRow`]).
    Downloads(String),
    /// A choice in the play queue panel ([`QueueView`]).
    Queue(QueueCmd),
    /// A change to the key bindings (Settings → Keys): apply it and save `keymap.toml`.
    Keymap(specular_input::KeymapEdit),
    /// Import key bindings from a TOML file the user picks.
    KeymapImport,
    /// Export the key bindings (`Some`: of that mode only) to a TOML file the user picks.
    KeymapExport(Option<specular_input::Mode>),
    /// Put a text field's selection on the clipboard (Ctrl+C, Ctrl+X in the settings.toml
    /// editor and the dialogs' search fields; no toast).
    EditorCopy(String),
    /// Ctrl+V in the settings.toml editor or a dialog's search field: give the clipboard's
    /// text to [`Chrome::paste_text`].
    EditorPaste,
    /// Copy-on-select: make a text field's new selection the primary selection (X11,
    /// Wayland; never on other platforms; no toast).
    PrimaryCopy(String),
    /// A middle click put the caret in a text field: give the primary selection's text to
    /// [`Chrome::paste_text`].
    PrimaryPaste,
    /// Save from the keymap.toml editor: write this text to `keymap.toml` (atomically, as
    /// for [`Command::Keymap`]), apply it, then answer with [`Chrome::keymap_saved`].
    KeymapSave(String),
    /// The jump of a row of the tag browser ([`TagLine::jump`]), by index in
    /// [`DicomTags::rows`]: show what the report item points at.
    TagJump(usize),
    /// The plot sheet asks for another gain (`gain`: +1 up, −1 down) or speed (`speed`) of
    /// its ECG: lay it out again and answer with [`Chrome::set_plot`].
    PlotAdjust {
        gain: i32,
        speed: i32,
    },
    /// "Include file names" of the diagnostics dialog changed: build the preview again and
    /// answer with [`Chrome::set_diagnostics`].
    DiagNames(bool),
    /// Save the diagnostics bundle (ask where, then write it as previewed).
    DiagSave {
        include_names: bool,
    },
    /// The movie's seek bar was pressed or dragged to `fraction` of its length (0..=1);
    /// `done` when the button is let go (scrubbing shows frames on the way).
    Seek {
        fraction: f64,
        done: bool,
    },
    /// The volume slider was pressed, dragged, wheeled or stepped to `volume` (1 is normal,
    /// up to [`VideoPlayback::volume_max`]); `done` when it is let go, or a single step.
    /// Raising it unmutes.
    Volume {
        volume: f32,
        done: bool,
    },
    /// A choice in a movie's player menus (subtitles, sound tracks, chapters).
    Player(PlayerCmd),
    /// A series of the studies browser was chosen ([`StudyLine::id`]): open it.
    OpenStudySeries(u64),
    /// Stop reading the medium of the studies browser (its Cancel, or Esc while it reads).
    /// What was read so far stays listed, marked as incomplete.
    StudiesCancel,
    /// "Send to PACS…" in the studies browser, with the selected series ([`StudyLine::id`]),
    /// if any: the app asks what to send (that series, its study or everything) and where.
    StudiesSend(Option<u64>),
    /// Turn a capability's tools on: for this session (the toolbar chip, a hidden command
    /// chosen in the palette), or `always` (added to `profile.capabilities`). Then run the
    /// action, if any.
    Capability {
        capability: specular_input::Capability,
        always: bool,
        then: Option<Action>,
    },
    /// A control of the start screen that the application carries out.
    Welcome(WelcomeCommand),
    /// Button `button` (its position) of the "needs a free decoder" card with this id
    /// ([`Chrome::set_decoder_card`]).
    DecoderCard {
        id: String,
        button: usize,
    },
}

/// An entry the application puts in the palette: a user command among the actions
/// ([`Chrome::set_palette_items`]), or one row of a list ([`Chrome::open_list`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    /// Sent back in [`Command::Item`].
    pub id: u64,
    pub label: String,
    /// Shortcut or short note shown at the end of the row.
    pub key: Option<String>,
    /// Listed greyed out with this reason; choosing it does nothing.
    pub unavailable: Option<String>,
    pub icon: specular_gfx::Icon,
}

#[derive(Debug, Default)]
pub struct Response {
    /// The chrome used the event; the viewer must ignore it.
    pub consumed: bool,
    pub commands: Vec<Command>,
}

impl Response {
    fn used() -> Response {
        Response {
            consumed: true,
            commands: Vec::new(),
        }
    }
    fn cmd(c: Command) -> Response {
        Response {
            consumed: true,
            commands: vec![c],
        }
    }
}

/// Performance numbers for the HUD.
#[derive(Clone, Debug, Default)]
pub struct HudStats {
    /// Displayed pixels per image pixel.
    pub scale: f32,
    pub threads: usize,
    pub queued: usize,
    pub decode_ms: f64,
    pub upload_ms: f64,
    pub frame_ms: f64,
    /// Time to first pixels of the current image, from the open request.
    pub open_ms: f64,
    /// Overview level on screen (JPEG scale denominator, 0 for the EXIF thumbnail).
    pub level: u32,
    pub tiles: usize,
    pub tile_capacity: usize,
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    pub backend: String,
}

/// Everything the chrome shows about the viewer, rebuilt every frame by the application.
/// Why the current image took as long as it did to open, localised (the information panel's
/// "Performance" section; DESIGN.md §12.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenReport {
    pub headline: String,
    /// An obvious fix, if there is one.
    pub hint: Option<String>,
    /// Stage timings and the decoding path, as label/value rows.
    pub rows: Vec<(String, String)>,
}

pub struct ViewerState<'a> {
    /// What [`Action::disabled`] decides on; every control reads it.
    pub actions: ActionState,
    pub has_image: bool,
    /// Zero-based index in the folder.
    pub index: usize,
    pub count: usize,
    /// Current zoom (1.0 = 100%).
    pub zoom: f32,
    pub at_fit: bool,
    pub fullscreen: bool,
    pub file_name: Option<&'a str>,
    pub dims: Option<(u32, u32)>,
    /// Opened but nothing on screen yet.
    pub loading: bool,
    pub error: Option<&'a str>,
    pub truncated: bool,
    pub metadata: Option<&'a MetadataReport>,
    pub hud: &'a HudStats,
    pub keymap: &'a Keymap,
    pub store: &'a SettingsStore,
    pub settings: &'a Settings,
    /// Navigator, loupe and zoom-rectangle geometry.
    pub overlays: Overlays,
    /// Transport of an animated image (draws the transport bar).
    pub playback: Option<Playback>,
    /// The image is a Motion Photo or Live Photo (draws the motion badge, its play control).
    pub motion: Option<MotionBadge>,
    /// Zero-based index of the second page when a two-page spread is on screen.
    pub spread: Option<usize>,
    /// The current file holds several images (draws the page stepper).
    pub pages: Option<PageBadge<'a>>,
}

/// Where the current image is among the pages of its file, for the page stepper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageBadge<'a> {
    /// Zero-based page on screen.
    pub index: u32,
    /// Pages of the file (at least 2).
    pub count: u32,
    /// What the page is ("128×128 @2x", "Face +X", "Layer 2"), already localised.
    pub label: Option<&'a str>,
}

/// A slideshow or the viewer on another display, for the "presenting on" control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presenting {
    /// The display's name, as the system reports it.
    pub display: String,
    /// A slideshow, and whether it is paused (Pause / Resume and Stop); `None`: the viewer
    /// following this window's selection (Close only).
    pub paused: Option<bool>,
}

/// A still with a short video clip (Motion Photo, Live Photo), for the motion badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionBadge {
    /// An Apple Live Photo (the badge says "Live"); otherwise a Motion Photo.
    pub live: bool,
    /// The clip plays (or is starting) in place of the still: the badge brings the still
    /// back.
    pub playing: bool,
}

/// Playback state of an animated image, for the transport bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    /// Zero-based frame on screen.
    pub frame: usize,
    /// Frame count, once known.
    pub count: Option<u32>,
    pub playing: bool,
    /// Speed factor (1.0 = as stored).
    pub speed: f64,
    /// A movie: its times, looping and sound (the bar gets a seek bar, a time readout, a
    /// loop toggle and a mute button).
    pub video: Option<VideoPlayback>,
}

/// What a movie's transport bar shows beyond an animation's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoPlayback {
    /// Seconds from the start: where playback is, and the whole length.
    pub position: f64,
    pub duration: f64,
    /// Loops forever (the loop toggle is on).
    pub looping: bool,
    /// The sound's volume (1 is normal) and whether it is muted; `None` without sound.
    pub sound: Option<(f32, bool)>,
    /// The volume slider's top: 2 for the sound played here (up to 200 %), 1 for a cast
    /// device's own volume.
    pub volume_max: f32,
    /// A picture plays (not a sound file): the bar has the full-screen play button.
    pub picture: bool,
}

/// Modal overlays (one at a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modal {
    Palette,
    CheatSheet,
    Settings,
    Language,
    /// "Move this file to the trash?" (`browsing.confirm_trash`).
    ConfirmTrash,
    /// Version, licence and data attributions.
    About,
    /// What the app detected about the machine ([`Chrome::show_report`]).
    HwReport,
    /// Performance insights ([`Chrome::show_insights`]).
    Insights,
    /// What runs in the background ([`Chrome::show_work`]).
    Work,
    /// What is downloading ([`Chrome::show_downloads`]).
    Downloads,
    /// The DICOM tag browser ([`Chrome::show_dicom_tags`]).
    DicomTags,
    /// What is on a patient's disc ([`Chrome::show_studies`]).
    Studies,
    /// "What will you use Specular for?", asked once ([`Chrome::ask_profile`]).
    Profile,
    /// The config file editor: settings.toml ([`Chrome::open_config_editor`]) or keymap.toml
    /// ([`Chrome::open_keymap_editor`]).
    ConfigEditor,
    /// A plot: dose volume histograms, an ECG ([`Chrome::show_plot`]).
    Plot,
    /// The manual ([`Chrome::show_help`]).
    Help,
    /// "Specular closed unexpectedly", at the start after a crash ([`Chrome::show_crash`]).
    Crash,
    /// The diagnostics bundle's preview ([`Chrome::show_diagnostics`]).
    Diagnostics,
    /// The play queue ([`Chrome::show_queue`]).
    Queue,
}

pub(crate) struct Toast {
    id: u64,
    text: String,
    undo: Option<u64>,
    /// The button's label when it is not Undo ([`Chrome::toast_action`]).
    action: Option<String>,
    /// It has a close (×) button ([`Chrome::hint`]).
    closable: bool,
    born: f64,
    ttl: f64,
}

/// The whole chrome.
pub struct Chrome {
    size: (f32, f32),
    /// Width (logical px) another panel (Develop's) takes from the end side of the window.
    panel_inset: f32,
    pointer: Option<(f32, f32)>,
    hits: Vec<(Rect, Target)>,
    hover: Option<Target>,
    hover_since: f64,
    pressed: Option<Target>,
    last_activity: f64,
    preedit: String,
    text_field: Option<Rect>,
    /// A field drawn unfocused that keeps the input method (`ctx::Fields::ime_only`).
    ime_field: Option<Rect>,
    ime_area: Option<Rect>,
    /// The open dialog's field with an editing state as last drawn (where a click goes).
    field_geom: Option<ctx::FieldGeom>,
    /// The modifier keys held ([`Input::Modifiers`]): Shift+click extends a selection.
    mods: specular_input::Mods,
    /// Selecting in a field offers the primary selection and a middle click pastes it
    /// ([`line_edit::PRIMARY_SELECTION`]: on where the platform has one).
    pub(crate) primary_selection: bool,
    last_now: f64,
    /// A slideshow runs: the toolbar and the edge arrows stay hidden.
    presenting: bool,
    /// A movie's full-screen play mode ([`Chrome::set_video_fullscreen`]).
    video_full: bool,
    /// The play controls' opacity in full-screen play mode.
    play_alpha: f32,
    /// Their buttons and the volume slider as last drawn: Tab reaches them once they have
    /// faded out.
    pub(crate) play_stops: Vec<Target>,
    /// A slideshow or the viewer runs on another display ([`Chrome::set_presenting_on`]).
    pub(crate) presenting_on: Option<Presenting>,
    toolbar_alpha: f32,
    /// How far a playing movie's transport has slid down into the toolbar's place (0: above
    /// the toolbar, 1: in its place with the toolbar hidden). See [`Chrome::dock_wanted`].
    video_dock: f32,
    /// Where the toolbar was last drawn in the viewer (the pointer over it keeps it up).
    toolbar_rect: Option<Rect>,
    /// Where the picture is on screen ([`Chrome::set_picture`]).
    picture: Option<Rect>,
    edge_alpha: [f32; 2],
    modal: Option<Modal>,
    /// The dialogs the open one was opened from, nearest last, each with its keyboard focus:
    /// closing the open one returns to the last (history.rs).
    pub(crate) modal_parents: Vec<(Modal, Option<Target>)>,
    modal_alpha: f32,
    pub(crate) info_open: bool,
    pub(crate) info_all: bool,
    info_alpha: f32,
    pub(crate) info_scroll: f32,
    /// The info panel's "Performance" section is expanded.
    pub(crate) info_perf: bool,
    pub(crate) open_report: Option<OpenReport>,
    hud_open: bool,
    toasts: Vec<Toast>,
    next_toast: u64,
    zoom_menu: bool,
    /// The zoom presets row the keyboard is on (while `zoom_menu`).
    pub(crate) zoom_sel: Option<usize>,
    /// The control of the viewer with the keyboard focus (focus.rs; `None`: the image), since
    /// when, and Enter or Space held on it (Previous or Next).
    pub(crate) kbd: Option<Target>,
    kbd_since: f64,
    pub(crate) kbd_hold: Option<focus::KeyHold>,
    /// The toolbar control the keyboard was last on (a Tab back to the bar returns there),
    /// and the bar's controls as last drawn, in order.
    pub(crate) bar_last: Option<Target>,
    pub(crate) bar_stops: Vec<Target>,
    /// The button of the open dialog with the keyboard focus (the insights panel), and
    /// whether to scroll it into view.
    pub(crate) modal_focus: Option<Target>,
    pub(crate) reveal_focus: bool,
    /// The All-fields rows of the info panel as last drawn, in order (one Tab stop, ↑/↓
    /// move along it), and for how many more frames to scroll the focused one into view
    /// (rows far below the view are measured once they come near).
    pub(crate) info_rows: Vec<Target>,
    pub(crate) info_reveal: u8,
    /// The layout ran right to left when last drawn (the arrow keys follow it).
    pub(crate) last_rtl: bool,
    /// The toolbar items that didn't fit on the bar when it was last drawn (they are in its
    /// More menu), and that menu while it is open, with the row the keyboard is on.
    pub(crate) toolbar_overflow: Vec<specular_settings::ToolbarItem>,
    pub(crate) overflow_menu: Option<Option<usize>>,
    /// The toolbar's right-click menu, open at this point (NaN: over the middle of the bar).
    pub(crate) toolbar_menu: Option<(f32, f32)>,
    /// The application's right-click menu ([`Chrome::open_menu`]).
    pub(crate) menu: Option<context_menu::ContextMenu>,
    /// Where the menu was whose row the event being handled chose (`menu_origin`).
    menu_chosen_at: Option<(f32, f32)>,
    /// The bar as last drawn, a press on one of its items that may become a
    /// drag (bar_drag.rs), and Previous or Next held down.
    pub(crate) bar_geom: Option<bar_drag::BarGeom>,
    /// The toolbar and what sits on it (transport, page stepper) as last drawn, logical px.
    pub(crate) bottom_bar: Option<Rect>,
    pub(crate) bar_press: Option<bar_drag::BarDrag>,
    pub(crate) nav_hold: Option<bar_drag::NavHold>,
    /// Commands raised outside input events ([`Chrome::take_commands`]).
    pending: Vec<Command>,
    /// An item of the bar to show picked up (screenshots): from, over.
    show_drag: Option<(usize, usize)>,
    /// The last change to announce to screen readers: id, text, when.
    pub(crate) announce: Option<(u64, String, f64)>,
    /// The file being opened and when it started to have nothing on screen: the spinner
    /// shows only after [`SPINNER_DELAY`], the file's name after [`OPENING_NAME_DELAY`].
    loading_since: Option<(Option<String>, f64)>,
    /// The file and zoom readout last drawn, to announce zoom changes (not new images).
    zoom_seen: Option<(Option<String>, f32, bool)>,
    /// The file and page last seen, for announcing page changes.
    page_seen: Option<(Option<String>, u32)>,
    /// The image number being typed into the position readout ("Go to image"): ASCII
    /// digits, no leading zero typed, at most nine ([`GOTO_FILTER`]).
    pub(crate) goto: Option<LineEdit>,
    /// The go-to field is a movie's "Go to time…" (over the transport's time) rather than
    /// the image number ([`goto_time`]).
    pub(crate) goto_time: bool,
    fullscreen_hint_until: f64,
    /// Lays the chrome out right to left whatever the language (tests).
    #[cfg(test)]
    pub(crate) force_rtl: bool,
    pub(crate) palette: palette::PaletteState,
    pub(crate) settings: settings::SettingsState,
    pub(crate) editor: Box<config_editor::Editor>,
    /// keymap.toml as the application last read or wrote it ([`Chrome::set_keymap_file`]).
    pub(crate) keymap_file: config_editor::KeymapFile,
    pub(crate) language: settings::LanguageState,
    pub(crate) cheat_scroll: f32,
    /// The cheat sheet tab picked since it opened (`None`: the current mode's).
    pub(crate) cheat_tab: Option<cheatsheet::SheetMode>,
    /// The cheat sheet's tabs as last drawn (the profile hides medical images' tab).
    pub(crate) cheat_tabs: Vec<cheatsheet::SheetMode>,
    /// The cheat sheet's lines cut short as last drawn (tab, group, line).
    pub(crate) cheat_cut: Vec<(cheatsheet::SheetMode, usize, usize)>,
    /// Tests: every group title (`None`) and line (group, line) drawn, with its slot.
    #[cfg(test)]
    pub(crate) cheat_slots: Vec<cheatsheet::Slot>,
    /// The profiles checked in the first-run question.
    pub(crate) profile_pick: specular_input::Capabilities,
    pub(crate) about_scroll: f32,
    /// The hardware report's sections.
    pub(crate) report: HwReport,
    pub(crate) report_scroll: f32,
    /// The performance insights panel's content, and the indicator when it is up.
    pub(crate) insights: Insights,
    pub(crate) insights_scroll: f32,
    pub(crate) insight_indicator: Option<insights::Indicator>,
    /// The background-work panel's content, and the indicator while something runs.
    pub(crate) work: WorkPanel,
    pub(crate) work_scroll: f32,
    pub(crate) work_chip: Option<WorkChip>,
    /// The indicator's width in the gallery's top bar last frame ([`Chrome::work_chip_width`]).
    pub(crate) work_chip_w: f32,
    /// In the gallery: where the background-work indicator ends in its top bar (left-to-right
    /// logical x, before the gallery's own buttons); `None`: above its bottom bar.
    pub gallery_work_end: Option<f32>,
    /// What is downloading, while something is ([`Chrome::set_downloads`]).
    pub(crate) downloads: Downloads,
    pub(crate) downloads_on: bool,
    pub(crate) downloads_form: Option<DownloadsForm>,
    pub(crate) downloads_scroll: f32,
    /// A bar of the open panel moves (no total known).
    pub(crate) downloads_moving: bool,
    /// The play queue panel and the transport's repeat and shuffle.
    pub(crate) queue: queue_panel::QueueState,
    /// In the gallery: the free stretch of its bottom bar after the item count (left-to-right
    /// logical x from, to), where the downloads go in place.
    pub gallery_free: Option<(f32, f32)>,
    /// The DICOM tag browser's rows, filter and switches.
    pub(crate) dicom_tags: dicom_tags::TagsState,
    pub(crate) help: help::HelpState,
    /// The help context of what was last on screen without a dialog over it (F1, the
    /// palette's Help).
    pub(crate) help_ctx: &'static str,
    pub(crate) crash: diag_sheet::CrashState,
    pub(crate) diag: diag_sheet::DiagState,
    pub(crate) plot: plot::PlotState,
    /// The movie's seek bar is being dragged (its hit rectangle).
    pub(crate) seeking: Option<Rect>,
    /// Where the seek bar was dragged to (0..=1) and when it was let go (infinite while
    /// held). The bar shows it rather than the frame on screen, which lags while the decoder
    /// catches up, so the knob follows the pointer; after release it waits for the movie
    /// to reach it ([`SCRUB_HOLD`] at most).
    scrub: Option<(f64, f64)>,
    /// The volume slider is being dragged (its hit rectangle).
    pub(crate) volume_drag: Option<Rect>,
    /// The volume last asked for and when (infinite while dragged): the slider shows it
    /// until the sound reports it, so the knob follows the pointer and steps add up.
    pub(crate) volume_set: Option<(f32, f64)>,
    /// Wheel turns over the volume slider not yet a whole percent.
    pub(crate) volume_wheel: f32,
    /// What the movie's player offers ([`Chrome::set_player`]).
    pub(crate) player: Option<PlayerView>,
    /// The player menu open, and the keyboard's row in it.
    player_menu: Option<(PlayerMenu, Option<usize>)>,
    /// The player menu buttons as last drawn.
    player_anchors: Vec<(PlayerMenu, Rect)>,
    pub(crate) studies: studies::StudiesState,
    /// The GeoNames attribution in the UI language (filled when the About box opens).
    pub(crate) about_geonames: String,
    /// The About box's storage line (portable mode, where settings and the cache live).
    pub(crate) about_storage: String,
    /// Where to report problems and the support address ([`Chrome::set_contact`]).
    pub(crate) contact: Option<(String, String)>,
    /// Screenshots: the build line About shows, in place of this build's.
    pub(crate) about_build: Option<String>,
    /// Live state of action settings that switch something on or off (Settings →
    /// Integration), by setting key.
    pub(crate) setting_status: std::collections::HashMap<&'static str, SettingStatus>,
    /// Action settings whose button is greyed out (`set_setting_disabled`).
    pub(crate) disabled_settings: Vec<&'static str>,
    /// Settings left out of the window (`set_setting_hidden`): Local AI features not
    /// installed.
    pub(crate) hidden_settings: Vec<&'static str>,
    /// The Local AI page's state lines ([`Chrome::set_ai_page`]), apart from
    /// `setting_status` which the desktop integration replaces wholesale.
    pub(crate) ai_status: std::collections::HashMap<&'static str, SettingStatus>,
    /// Read-only panels under settings (Settings → Performance), by setting key.
    pub(crate) setting_panels: std::collections::HashMap<&'static str, SettingPanel>,
    pub(crate) bars: scrollbar::Scrollbars,
    pub(crate) confirm_file: String,
    /// A confirmation other than the trash one ([`Chrome::confirm`]): title, accept button,
    /// and the [`Command::Item`] accepting sends.
    pub(crate) confirm_custom: Option<(String, String, u64)>,
    /// The second choice of a confirmation that offers two ([`Chrome::confirm_choice`]): its
    /// button and the [`Command::Item`] it sends.
    pub(crate) confirm_alt: Option<(String, u64)>,
    /// A third choice ([`Chrome::confirm_choice3`]), between Cancel and the other.
    pub(crate) confirm_alt2: Option<(String, u64)>,
    /// The lines under a confirmation's question that is not destructive.
    pub(crate) confirm_detail_text: Option<String>,
    /// The open confirmation asks before resetting every setting
    /// ([`Chrome::confirm_reset_all`]).
    pub(crate) confirm_reset: bool,
    /// The open custom confirmation is a destructive one ([`Chrome::confirm_destructive`]):
    /// the line under its question, and its accept button in the danger colour.
    pub(crate) confirm_destructive: Option<String>,
    /// Accessibility text scale (multiplies font sizes).
    pub text_scale: f32,
    pub reduce_motion: bool,
    /// Files from another application are being dragged over the window: the empty
    /// window says "Release to open".
    pub file_hover: bool,
    /// What the start screen shows ([`Chrome::set_welcome`]).
    pub(crate) welcome: Welcome,
    /// The "needs a free decoder" card shown in place of the current file's error.
    pub(crate) decoder_card: Option<DecoderCard>,
    /// The centre message is a notice ("No preview inside this file"), not an error.
    notice: bool,
    /// The start screen was drawn at the last frame.
    pub(crate) welcome_drawn: bool,
    /// The start choice picked on the first-run sheet (written by Continue).
    pub(crate) profile_start: specular_settings::OnStart,
    /// The gallery covers the image: no image states, edge arrows, info panel, inspection
    /// tools or toolbar (the gallery draws its own bars).
    pub gallery: bool,
    /// The viewer shows its back button (the image has a gallery to go back to).
    pub back_to_gallery: bool,
    /// The height of the gallery's own bottom bar: the insights indicator stays above it.
    pub gallery_bar: f32,
    /// The gallery is comparing images (the cheat sheet opens on compare's keys).
    pub comparing: bool,
    i18n_gen: u64,
    glass_visible: bool,
    /// The control whose popover is (or was just) open: its tooltip stays hidden until the
    /// pointer leaves it.
    tip_latch: Option<Target>,
    /// What the `ui` log last reported: modal, popover owner, tooltip.
    logged: (Option<Modal>, Option<Target>, Option<String>),
    /// An assistive technology is active ([`Chrome::set_accessibility`]).
    accessibility: bool,
    /// Inspection tools: which are on, and the histogram / pixel data they show.
    pub inspect: Inspect,
}

impl Default for Chrome {
    fn default() -> Self {
        Chrome::new()
    }
}

impl Chrome {
    /// Where the toolbar and what sits on it (transport, page stepper) were last drawn,
    /// logical px; `None` while the bar is off. The application keeps its own bottom-corner
    /// panels clear of it.
    pub fn bottom_bar(&self) -> Option<Rect> {
        self.bottom_bar
    }

    pub fn new() -> Chrome {
        Chrome {
            size: (800.0, 600.0),
            panel_inset: 0.0,
            pointer: None,
            hits: Vec::new(),
            hover: None,
            hover_since: 0.0,
            pressed: None,
            last_activity: 0.0,
            preedit: String::new(),
            text_field: None,
            ime_field: None,
            ime_area: None,
            field_geom: None,
            mods: specular_input::Mods::NONE,
            primary_selection: line_edit::PRIMARY_SELECTION,
            last_now: 0.0,
            presenting: false,
            video_full: false,
            play_alpha: 0.0,
            play_stops: Vec::new(),
            presenting_on: None,
            toolbar_alpha: 1.0,
            video_dock: 0.0,
            toolbar_rect: None,
            picture: None,
            edge_alpha: [0.0; 2],
            modal: None,
            modal_parents: Vec::new(),
            modal_alpha: 0.0,
            info_open: false,
            info_all: false,
            info_alpha: 0.0,
            info_scroll: 0.0,
            info_perf: false,
            open_report: None,
            hud_open: false,
            toasts: Vec::new(),
            next_toast: 1,
            zoom_menu: false,
            zoom_sel: None,
            kbd: None,
            kbd_since: 0.0,
            kbd_hold: None,
            bar_last: None,
            bar_stops: Vec::new(),
            modal_focus: None,
            reveal_focus: false,
            info_rows: Vec::new(),
            info_reveal: 0,
            last_rtl: false,
            toolbar_overflow: Vec::new(),
            overflow_menu: None,
            toolbar_menu: None,
            menu: None,
            menu_chosen_at: None,
            bar_geom: None,
            bottom_bar: None,
            bar_press: None,
            nav_hold: None,
            pending: Vec::new(),
            show_drag: None,
            announce: None,
            loading_since: None,
            file_hover: false,
            welcome: Welcome::default(),
            decoder_card: None,
            notice: false,
            welcome_drawn: false,
            profile_start: specular_settings::OnStart::StartScreen,
            zoom_seen: None,
            page_seen: None,
            goto: None,
            goto_time: false,
            fullscreen_hint_until: 0.0,
            #[cfg(test)]
            force_rtl: false,
            palette: Default::default(),
            settings: Default::default(),
            editor: Default::default(),
            keymap_file: Default::default(),
            language: Default::default(),
            cheat_scroll: 0.0,
            cheat_tab: None,
            cheat_tabs: cheatsheet::SheetMode::ALL.to_vec(),
            cheat_cut: Vec::new(),
            #[cfg(test)]
            cheat_slots: Vec::new(),
            profile_pick: specular_input::Capabilities::GENERAL,
            about_scroll: 0.0,
            report: HwReport::default(),
            report_scroll: 0.0,
            insights: Insights::default(),
            insights_scroll: 0.0,
            insight_indicator: None,
            work: WorkPanel::default(),
            work_scroll: 0.0,
            work_chip: None,
            work_chip_w: 0.0,
            gallery_work_end: None,
            downloads: Downloads::default(),
            downloads_on: false,
            downloads_form: None,
            downloads_scroll: 0.0,
            downloads_moving: false,
            queue: Default::default(),
            gallery_free: None,
            dicom_tags: Default::default(),
            help: Default::default(),
            help_ctx: "viewer",
            crash: Default::default(),
            diag: Default::default(),
            plot: Default::default(),
            seeking: None,
            scrub: None,
            volume_drag: None,
            volume_set: None,
            volume_wheel: 0.0,
            player: None,
            player_menu: None,
            player_anchors: Vec::new(),
            studies: Default::default(),
            gallery_bar: 0.0,
            comparing: false,
            about_geonames: String::new(),
            about_storage: String::new(),
            contact: None,
            about_build: None,
            setting_status: Default::default(),
            disabled_settings: Vec::new(),
            // Where the first-run question and the "Always" offers are recorded, and the
            // folders copy and move used last: not choices.
            hidden_settings: vec![
                "profile.shown",
                "general.tips_shown",
                "metadata.basemap_allowed",
                "metadata.basemap_service",
                "maps.services_allowed",
                "integration.extra_default_types",
                "browsing.recent_folders",
                // Chosen on a sound file's page.
                "viewing.sound_visuals",
                // The equaliser's panel sets these.
                "viewing.sound_eq_preset",
                "viewing.sound_eq_bands",
                "viewing.sound_eq_preamp",
                "viewing.sound_eq_user_presets",
                // Settings › Optional decoders shows these in its panel.
                "decoders.braw",
                "decoders.braw_path",
                "decoders.dwg",
                "decoders.dwg_path",
                "decoders.occt",
                "decoders.occt_path",
                "decoders.czi",
                "decoders.czi_path",
                // Develop's Soft proof card sets these.
                "colour.proof_profile",
                "colour.proof_intent",
                "colour.proof_bpc",
                "colour.proof_paper",
                "colour.proof_ink",
            ],
            ai_status: Default::default(),
            setting_panels: Default::default(),
            bars: Default::default(),
            confirm_file: String::new(),
            confirm_custom: None,
            confirm_alt: None,
            confirm_alt2: None,
            confirm_detail_text: None,
            confirm_reset: false,
            confirm_destructive: None,
            text_scale: 1.0,
            reduce_motion: false,
            gallery: false,
            back_to_gallery: false,
            i18n_gen: u64::MAX,
            glass_visible: false,
            tip_latch: None,
            logged: (None, None, None),
            accessibility: false,
            inspect: Inspect::default(),
        }
    }

    /// Window size in logical pixels.
    pub fn set_size(&mut self, w: f32, h: f32) {
        self.size = (w, h);
    }

    /// Width (logical px) the application's own panel (Develop's) takes from the end side
    /// of the window; the edge arrow there moves in beside it.
    pub fn set_panel_inset(&mut self, w: f32) {
        self.panel_inset = w.max(0.0);
    }

    pub fn modal(&self) -> Option<Modal> {
        self.modal
    }

    pub fn info_open(&self) -> bool {
        self.info_open
    }

    pub fn hud_open(&self) -> bool {
        self.hud_open
    }

    /// Where the pixel inspector's loupe goes (logical px), when it shows one: the
    /// application renders the magnified image there, under the chrome.
    pub fn pixel_loupe(&self) -> Option<Rect> {
        self.inspect.loupe
    }

    pub fn set_hud(&mut self, on: bool) {
        self.hud_open = on;
    }

    /// Width (logical px) the info panel takes from the side of the window, for the
    /// application to keep the image clear of it.
    pub fn info_inset(&self) -> f32 {
        if self.info_open && !self.video_full {
            info::panel_width(self.text_scale) + metrics::TOOLBAR_MARGIN
        } else {
            0.0
        }
    }

    /// Whether glass (blurred backdrop) is on screen; the application then keeps the blur
    /// of the scene up to date.
    pub fn uses_glass(&self) -> bool {
        self.glass_visible
    }

    /// Whether the chrome has faded out (toolbar hidden, no dialog): the application then
    /// hides the pointer in fullscreen.
    /// The focused text field (logical pixels), if one is on screen: the application
    /// enables the input method while there is one.
    pub fn text_field(&self) -> Option<Rect> {
        self.text_field
    }

    /// A screen reader hears the new zoom level when the user zooms (the readout says it
    /// visually), but not when a new image opens at its own fit, nor when the window resizes
    /// in fit mode.
    fn announce_zoom(&mut self, st: &ViewerState) {
        let seen = (st.file_name.map(str::to_owned), st.zoom, st.at_fit);
        if st.loading || !st.has_image {
            return;
        }
        if let Some((file, zoom, fit)) = &self.zoom_seen
            && *file == seen.0
            && *zoom != st.zoom
            && !(*fit && st.at_fit)
            && self.accessibility
        {
            self.announce(crate::toolbar::zoom_label(st.zoom, st.at_fit));
        }
        self.zoom_seen = Some(seen);
    }

    /// A screen reader hears "Page 3 of 12, Face +X" when the page changes within a file (the
    /// stepper says it visually), once the page has opened and its label is known; not when
    /// another file opens.
    fn announce_page(&mut self, st: &ViewerState) {
        if st.loading {
            return;
        }
        let Some(p) = &st.pages else {
            self.page_seen = None;
            return;
        };
        let seen = (st.file_name.map(str::to_owned), p.index);
        if let Some((file, index)) = &self.page_seen
            && *file == seen.0
            && *index != p.index
            && self.accessibility
        {
            self.announce(pages::spoken(p));
        }
        self.page_seen = Some(seen);
    }

    /// Where the input method's candidate window goes (logical pixels): the composition in
    /// the focused field, or its caret while nothing is being composed. `Some` when
    /// [`Chrome::text_field`] is, and also while a control of a dialog has the keyboard but
    /// typing still goes to the dialog's field (Settings, the palette, the language list):
    /// the input method stays on, and a composition takes the keyboard back to the field.
    pub fn ime_cursor_area(&self) -> Option<Rect> {
        self.text_field
            .or(self.ime_field)
            .map(|f| self.ime_area.unwrap_or(f))
    }

    /// The composition the input method is showing in the focused field (empty: none).
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub fn chrome_hidden(&self) -> bool {
        self.toolbar_alpha <= 0.0
            && (!self.video_full || self.play_alpha <= 0.0)
            && self.modal.is_none()
            && self.goto.is_none()
    }

    /// Whether typed text goes to the chrome (a dialog or the go-to field is open).
    pub fn wants_text(&self) -> bool {
        self.modal.is_some() || self.goto.is_some()
    }

    /// Whether a control of the open dialog has the keyboard (not its field, if it has one).
    fn control_focused(&self) -> bool {
        self.modal.is_some()
            && (self.modal_focus.is_some()
                || self.modal == Some(Modal::Settings) && self.settings.tools.focus.is_some())
    }

    /// Typing or composing while a control of the dialog has the keyboard: the keyboard goes
    /// back to the dialog's field (Settings' search, the palette's and the language list's).
    /// False when there is no field to type into (another dialog, or a dropdown of Settings
    /// is open).
    fn typing_to_field(&mut self) -> bool {
        match self.modal {
            Some(Modal::Settings) if self.settings.dropdown.is_none() => {
                self.settings.tools.focus = None;
            }
            Some(Modal::Palette | Modal::Language | Modal::DicomTags | Modal::Help) => {}
            _ => return false,
        }
        self.modal_focus = None;
        true
    }

    /// Whether a movie's toolbar hides and its transport slides down into its place: while
    /// the movie plays in the viewer, unless the toolbar is in use (pointer over either bar,
    /// keyboard in the toolbar, one of its menus open) or set to stay up.
    fn dock_wanted(&self, st: &ViewerState, tool: bool) -> bool {
        let playing = st
            .playback
            .as_ref()
            .is_some_and(|p| p.playing && p.video.is_some_and(|v| v.picture));
        if !playing || self.gallery || self.video_full || self.presenting {
            return false;
        }
        // A picture clear of both bars (a small movie, or zoomed out) is not covered by them:
        // nothing needs to move.
        let bars_top = self.size.1
            - metrics::TOOLBAR_MARGIN
            - metrics::TOOLBAR_HEIGHT
            - 8.0
            - transport::height(true);
        if self.picture.is_some_and(|r| r.y + r.h <= bars_top) {
            return false;
        }
        let in_use = self.accessibility
            || !st.settings.viewing.toolbar_autohide
            || self.bar_busy()
            || self.modal.is_some()
            || self.zoom_menu
            || self.overflow_menu.is_some()
            || self.goto.is_some()
            || self.toolbar_menu.is_some()
            || self
                .kbd
                .as_ref()
                .and_then(focus::region_of)
                .is_some_and(|r| r == focus::Region::Toolbar)
            || !tool
                && self.pointer.is_some_and(|(x, y)| {
                    self.toolbar_rect
                        .is_some_and(|r| r.inset(-6.0).contains((x, y)))
                });
        !in_use
    }

    /// `pb` as the transport shows it: at the seek bar's drag position while it is held, and
    /// after release until the movie gets there (or [`SCRUB_HOLD`] passes).
    fn scrubbed(&mut self, pb: &Playback, now: f64, want: &mut impl FnMut(f64)) -> Playback {
        let mut pb = *pb;
        let Some(v) = pb.video.as_mut() else {
            self.scrub = None;
            return pb;
        };
        if let Some((f, released)) = self.scrub {
            let target = f * v.duration;
            // Within a frame or two of the target: the movie has caught up.
            let arrived = (v.position - target).abs() < 0.25;
            if self.seeking.is_none() && (arrived || now - released > SCRUB_HOLD) {
                self.scrub = None;
            } else {
                v.position = target;
                if self.seeking.is_none() {
                    want((released + SCRUB_HOLD - now).max(0.0) + 0.01);
                }
            }
        }
        self.volume_shown(v, now, want);
        pb
    }

    /// Where the picture is on screen (logical pixels; `None`: nothing shown). Set before
    /// each draw.
    pub fn set_picture(&mut self, r: Option<Rect>) {
        self.picture = r;
    }

    /// Keeps the toolbar visible (navigation, key presses).
    pub fn poke(&mut self, now: f64) {
        self.last_activity = now;
    }

    /// A movie's full-screen play mode started or ended. While on, the bars, panels, edge
    /// arrows and inspection tools are hidden (what was open comes back with the windowed
    /// layout); the transport sits at the bottom over the picture, shown on a pointer move
    /// or a key and hidden again [`PLAY_CONTROLS_IDLE`] later while playing (never while
    /// paused or under the pointer).
    pub fn set_video_fullscreen(&mut self, on: bool, now: f64) {
        if on == self.video_full {
            return;
        }
        self.video_full = on;
        if on {
            self.zoom_menu = false;
            self.overflow_menu = None;
            self.toolbar_menu = None;
            self.fullscreen_hint_until = now + 2.2;
            self.last_activity = now;
            // The keyboard leaves the hidden bar.
            if self
                .kbd
                .as_ref()
                .and_then(focus::region_of)
                .is_some_and(|r| r != focus::Region::Transport)
            {
                self.kbd = None;
            }
        } else {
            self.fullscreen_hint_until = 0.0;
        }
    }

    /// Whether a movie's full-screen play mode is on.
    pub fn video_fullscreen(&self) -> bool {
        self.video_full
    }

    /// What the movie on screen offers in its player menus (`None`: no movie).
    pub fn set_player(&mut self, v: Option<PlayerView>) {
        if v.is_none() {
            self.player_menu = None;
        }
        self.player = v;
    }

    /// Opens (or closes, when open) a player menu, as its button does.
    pub fn toggle_player_menu(&mut self, m: PlayerMenu) {
        self.player_menu = match self.player_menu {
            Some((open, _)) if open == m => None,
            _ if self.player.is_some() => Some((m, None)),
            _ => None,
        };
        self.zoom_menu = false;
        self.overflow_menu = None;
    }

    /// "Go to time…": the field over a movie's time (or closes it when open).
    fn open_goto_time(&mut self) {
        if self.goto.is_some() && self.goto_time {
            self.goto = None;
            return;
        }
        if self.player.is_none() {
            return;
        }
        self.goto = Some(LineEdit::new("").with_filter(goto_time::FILTER));
        self.goto_time = true;
        self.player_menu = None;
        self.zoom_menu = false;
        self.overflow_menu = None;
    }

    /// What a player menu's choice does: some stay in the chrome (Go to time…, Picture…),
    /// a viewer action goes as that action, the rest to the application.
    fn player_choice(&mut self, cmd: PlayerCmd) -> Response {
        match cmd {
            PlayerCmd::GoToTime => {
                self.open_goto_time();
                Response::used()
            }
            PlayerCmd::OpenPicture => {
                self.player_menu = Some((PlayerMenu::Picture, None));
                Response::used()
            }
            PlayerCmd::OpenOutput => {
                self.player_menu = Some((PlayerMenu::Output, None));
                Response::cmd(Command::Player(PlayerCmd::OpenOutput))
            }
            PlayerCmd::OpenCast => {
                self.player_menu = Some((PlayerMenu::Cast, None));
                Response::used()
            }
            PlayerCmd::Action(a) => Response::cmd(Command::Action(a)),
            c => Response::cmd(Command::Player(c)),
        }
    }

    /// The seek bar's hit rectangle as last drawn.
    pub fn seek_bar_rect(&self) -> Option<Rect> {
        self.hits
            .iter()
            .rev()
            .find(|(_, t)| *t == Target::SeekBar)
            .map(|(r, _)| *r)
    }

    /// The volume slider's hit rectangle as last drawn (none without sound, or in a window
    /// too narrow for it).
    pub fn volume_slider_rect(&self) -> Option<Rect> {
        self.hits
            .iter()
            .rev()
            .find(|(_, t)| *t == Target::Volume)
            .map(|(r, _)| *r)
    }

    /// The player's menus are there (a movie or sound file is on screen).
    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    /// The player menu open.
    pub fn player_menu(&self) -> Option<PlayerMenu> {
        self.player_menu.map(|m| m.0)
    }

    /// Where along the movie the pointer is over the seek bar (0..=1), for its preview frame.
    pub fn seek_hover(&self) -> Option<f64> {
        if self.hover != Some(Target::SeekBar) || self.seeking.is_some() {
            return None;
        }
        let (x, _) = self.pointer?;
        let r = self
            .hits
            .iter()
            .rev()
            .find(|(_, t)| *t == Target::SeekBar)
            .map(|(r, _)| *r)?;
        Some(transport::seek_fraction(r, x))
    }

    fn player_key(&mut self, named: Option<NamedKey>) -> Option<Response> {
        let (m, sel) = self.player_menu?;
        let v = self.player.as_ref()?;
        let n = player::stops(m, v);
        if n == 0 {
            return None;
        }
        let to = match (named?, sel.map(|s| s.min(n - 1))) {
            (NamedKey::ArrowDown, None) | (NamedKey::Home, _) => 0,
            (NamedKey::ArrowUp, None) | (NamedKey::End, _) => n - 1,
            (NamedKey::ArrowDown, Some(s)) => (s + 1) % n,
            (NamedKey::ArrowUp, Some(s)) => (s + n - 1) % n,
            (NamedKey::Enter | NamedKey::Space, Some(s)) => {
                let cmd = player::stop_cmd(m, v, s)?;
                if !player::keeps_open(&cmd) {
                    self.player_menu = None;
                }
                return Some(self.player_choice(cmd));
            }
            (NamedKey::Enter | NamedKey::Space, None) => return Some(Response::used()),
            _ => return None,
        };
        self.player_menu = Some((m, Some(to)));
        Some(Response::used())
    }

    /// A slideshow started or stopped: while it runs the toolbar and the edge arrows stay
    /// hidden (dialogs and toasts still show).
    pub fn set_presenting(&mut self, on: bool) {
        self.presenting = on;
    }

    /// A slideshow or the viewer runs on another display (`None`: nothing does): the main
    /// window shows the "presenting on" control with Pause / Resume and Stop.
    pub fn set_presenting_on(&mut self, p: Option<Presenting>) {
        if p.is_none()
            && matches!(
                self.kbd,
                Some(Target::PresentingPause(_) | Target::PresentingStop(_))
            )
        {
            self.kbd = None;
        }
        self.presenting_on = p;
    }

    /// The displays connected now, by name: what Settings offers for a display setting
    /// (`slideshow.monitor`).
    pub fn set_monitors(&mut self, names: Vec<String>) {
        settings::set_monitors(names);
    }

    /// The values enum setting `key` offers, each with its label, when they depend on the
    /// machine (`None`: the schema's). Returns whether they changed (redraw then).
    pub fn set_setting_choices(
        &mut self,
        key: &'static str,
        choices: Option<Vec<(String, String)>>,
    ) -> bool {
        settings::set_choices(key, choices)
    }

    /// Reads `text` out through the polite status node (screen readers), without a toast.
    pub fn announce_status(&mut self, text: String) {
        self.announce(text);
    }

    pub fn entered_fullscreen(&mut self, now: f64) {
        self.fullscreen_hint_until = now + 2.2;
    }

    /// Shows a toast that stays long enough to read a sentence or two; returns its id.
    pub fn warning(&mut self, text: String, now: f64) -> u64 {
        let id = self.toast(text, None, now);
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.ttl = 10.0;
        }
        id
    }

    /// Shows a toast; returns its id.
    pub fn toast(&mut self, text: String, undo: Option<u64>, now: f64) -> u64 {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.retain(|t| t.text != text);
        self.toasts.push(Toast {
            id,
            text,
            undo,
            action: None,
            closable: false,
            born: now,
            ttl: if undo.is_some() {
                8.0
            } else {
                durations::TOAST as f64
            },
        });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
        id
    }

    /// Changes the text of toast `id` in place and keeps it up for another while (progress
    /// that updates); shows a new toast when it is gone. Returns the toast's id.
    pub fn update_toast(&mut self, id: u64, text: String, now: f64) -> u64 {
        match self.toasts.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.text = text;
                t.ttl = now - t.born + durations::TOAST as f64;
                id
            }
            None => self.toast(text, None, now),
        }
    }

    /// A toast with a button labelled `label` other than Undo ("Show in folder"): pressing it
    /// sends [`Command::Undo`] with `key`, which the application tells from its undo keys.
    pub fn toast_action(&mut self, text: String, label: String, key: u64, now: f64) -> u64 {
        let id = self.toast(text, Some(key), now);
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.action = Some(label);
        }
        id
    }

    /// A one-time hint: a quiet toast (announced politely, never taking the keyboard) with
    /// "Turn off tips", which sends [`Command::Undo`] with `key`, and a close button. It stays
    /// up longer than a toast, to be read.
    pub fn hint(&mut self, text: String, key: u64, now: f64) -> u64 {
        let id = self.toast_action(text, t!("hint-turn-off"), key, now);
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.closable = true;
            t.ttl = 12.0;
        }
        id
    }

    /// The label of toast `id`'s button.
    pub(crate) fn toast_button(&self, id: u64) -> String {
        self.toasts
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.action.clone())
            .unwrap_or_else(|| t!("toast-undo"))
    }

    pub fn dismiss_undo(&mut self, undo: u64) {
        self.toasts.retain(|t| t.undo != Some(undo));
    }

    /// Takes toast `id` away (the keys a key sequence can continue with, once it ended).
    pub fn dismiss_toast(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    /// Opens Settings at the section with TOML key `section` (`"viewing"`).
    /// Opens Settings at a section (`"performance"`) or a setting
    /// (`"performance.processing"`, scrolled to the top of the sheet).
    /// Already open (or waiting under another dialog), it moves there as a section click
    /// does, and the move is recorded in its history.
    pub fn open_settings_at(&mut self, section: &str) {
        let open = self.modal == Some(Modal::Settings)
            || self.modal_parents.iter().any(|p| p.0 == Modal::Settings);
        self.open_modal(Modal::Settings);
        let go = |c: &mut Chrome| {
            if open {
                c.settings.search.clear();
                c.settings.scroll = 0.0;
            }
            if let Some(i) = c.section_index(section) {
                c.settings.section = i;
            } else if let Some(m) = specular_settings::find(section)
                && let Some(i) = c.section_index(m.section.key())
            {
                c.settings.section = i;
                c.settings.scroll_to = Some(m.key);
            }
        };
        if open {
            self.settings_go(go);
        } else {
            go(self);
        }
    }

    /// Opened from another dialog, that one waits underneath and Esc returns to it.
    pub fn open_modal(&mut self, m: Modal) {
        if self.modal == Some(m) || self.push_parent(m) {
            return;
        }
        self.modal = Some(m);
        self.modal_focus = None;
        self.kbd_hold = None;
        self.zoom_menu = false;
        self.overflow_menu = None;
        self.toolbar_menu = None;
        match m {
            Modal::Palette => self.palette.reset(),
            Modal::Settings => self.settings.reset(),
            Modal::Language => self.language.reset(),
            Modal::CheatSheet => {
                self.cheat_scroll = 0.0;
                self.cheat_tab = None;
            }
            Modal::About => {
                self.about_scroll = 0.0;
                self.about_geonames.clear();
            }
            Modal::HwReport => self.report_scroll = 0.0,
            Modal::Insights => self.insights_scroll = 0.0,
            Modal::Work => self.work_scroll = 0.0,
            Modal::Downloads => self.downloads_scroll = 0.0,
            Modal::Queue => self.queue_reset(),
            // Filled by `show_dicom_tags` and `show_studies`.
            Modal::DicomTags | Modal::Studies | Modal::Plot => {}
            // Filled by `show_help`, `show_crash` and `show_diagnostics`.
            Modal::Help | Modal::Crash | Modal::Diagnostics => {}
            Modal::ConfirmTrash | Modal::ConfigEditor => {}
            Modal::Profile => self.modal_focus = Some(Target::ProfileDone),
        }
    }

    /// Application entries listed with the actions (user commands). Takes effect the next
    /// time the list is built.
    pub fn set_palette_items(&mut self, items: Vec<PaletteItem>) {
        if self.palette.extra != items {
            self.palette.extra = items;
            self.palette.dirty = true;
        }
    }

    /// Shows `items` in the palette's sheet instead of the actions ("Open with…"), with
    /// `placeholder` in the search field. `loading`: the list is still being filled, and an
    /// empty list shows this text.
    pub fn open_list(
        &mut self,
        placeholder: String,
        items: Vec<PaletteItem>,
        loading: Option<String>,
    ) {
        self.modal = None;
        self.open_modal(Modal::Palette);
        self.palette.list = Some(palette::ListMode {
            placeholder,
            items,
            loading,
        });
    }

    /// Opens the right-click menu at `at` (the pointer, in logical px): `groups` of rows with
    /// a divider between them, under `title` (what the menu is about). Choosing a row sends
    /// [`Command::Item`] with its id, as the lists of [`open_list`](Self::open_list) do; Esc,
    /// or a click elsewhere, closes it.
    pub fn open_menu(
        &mut self,
        at: (f32, f32),
        title: Option<String>,
        groups: Vec<Vec<PaletteItem>>,
    ) {
        let m = context_menu::ContextMenu::new(at, title, groups);
        if m.rows.is_empty() {
            return;
        }
        self.zoom_menu = false;
        self.overflow_menu = None;
        self.toolbar_menu = None;
        self.goto = None;
        self.menu = Some(m);
    }

    /// The right-click menu is open: it takes the keys. (Not once a row was chosen: it is
    /// only on its way out then.)
    pub fn menu_open(&self) -> bool {
        self.menu.as_ref().is_some_and(|m| !m.closing())
    }

    /// The right-click menu is on screen, open or on its way out: what is under it is covered.
    pub fn menu_shown(&self) -> bool {
        self.menu.is_some()
    }

    /// Any of the chrome's menus is on screen: the right-click menu, the toolbar's, the zoom
    /// menu, the overflow menu or the player's.
    pub fn any_menu_shown(&self) -> bool {
        self.menu.is_some()
            || self.zoom_menu
            || self.overflow_menu.is_some()
            || self.toolbar_menu.is_some()
            || self.player_menu.is_some()
    }

    /// While a [`Command::Item`] chosen in the right-click menu is handled: where that menu
    /// was, so a row that leads to more choices can open them in a menu there too.
    pub fn menu_origin(&self) -> Option<(f32, f32)> {
        self.menu_chosen_at
    }

    /// The choices of the right-click menu's row that leads to more of them (it sent
    /// [`Command::Item`] with its id when pointed at, or chosen with the keyboard): they open
    /// beside it, with `loading` above them while more are coming. Returns that row's id, for
    /// [`update_submenu`](Self::update_submenu); `Err` gives `items` back when no row asked
    /// for them, for the caller to show its own way.
    pub fn open_submenu(
        &mut self,
        items: Vec<PaletteItem>,
        loading: Option<String>,
    ) -> Result<u64, Vec<PaletteItem>> {
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return Err(items);
        };
        let Some((i, keyboard)) = m.sub_wanted.take() else {
            return Err(items);
        };
        let id = m.rows[i].id;
        m.sel = Some(i);
        m.sub = Some(context_menu::Sub::new(i, items, loading, keyboard));
        Ok(id)
    }

    /// Replaces the rows of the submenu that row `parent` (an id [`open_submenu`] returned)
    /// opened; does nothing once it was closed.
    ///
    /// [`open_submenu`]: Self::open_submenu
    pub fn update_submenu(
        &mut self,
        parent: u64,
        items: Vec<PaletteItem>,
        loading: Option<String>,
    ) {
        let Some(m) = self.menu.as_mut() else { return };
        let Some(s) = m.sub.as_mut() else { return };
        if m.rows.get(s.parent).is_some_and(|r| r.id == parent) {
            s.update(items, loading);
        }
    }

    /// Asks for the choices row `i` leads to: its id goes to the application, which answers
    /// with [`open_submenu`](Self::open_submenu). Already open, the keyboard goes into them.
    fn menu_want_sub(&mut self, i: usize, keyboard: bool) -> Response {
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return Response::used();
        };
        m.sel = Some(i);
        m.hover_wait = None;
        m.aim = None;
        if let Some(s) = m.sub.as_mut().filter(|s| s.parent == i) {
            if keyboard && s.sel.is_none() {
                s.sel = context_menu::step(&s.rows, None, true);
            }
            return Response::used();
        }
        if m.sub_wanted.is_some_and(|(w, _)| w == i) {
            return Response::used();
        }
        m.sub = None;
        m.sub_wanted = Some((i, keyboard));
        let (id, at) = (m.rows[i].id, m.at);
        self.menu_chosen_at = Some(at);
        Response::cmd(Command::Item(id))
    }

    /// The right-click menu's row `i` was chosen: it sends the row's id at once, and goes
    /// with that row lit for a moment, so it is clear what was taken. A row that leads to more
    /// choices opens them instead.
    fn menu_choose(&mut self, i: usize, keyboard: bool) -> Response {
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return Response::used();
        };
        if !m.enabled(i) {
            return Response::used();
        }
        if m.leads_on(i) {
            return self.menu_want_sub(i, keyboard);
        }
        let (id, at) = (m.rows[i].id, m.at);
        m.chosen = Some((context_menu::Row::Main(i), f64::NAN));
        self.menu_chosen_at = Some(at);
        Response::cmd(Command::Item(id))
    }

    /// The submenu's row `j` was chosen: as a row of the menu is.
    fn menu_choose_sub(&mut self, j: usize) -> Response {
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return Response::used();
        };
        let Some(s) = m.sub.as_ref().filter(|s| s.enabled(j)) else {
            return Response::used();
        };
        let (id, at) = (s.rows[j].id, m.at);
        m.chosen = Some((context_menu::Row::Sub(j), f64::NAN));
        self.menu_chosen_at = Some(at);
        Response::cmd(Command::Item(id))
    }

    /// The pointer moved, from `from`, while the right-click menu is open: the row under it
    /// is the one lit (the keyboard's goes, so only one ever is), and a row that leads to more
    /// choices opens them once it rests there (`menu_tick`). Crossing other rows on the way to
    /// an open submenu keeps it open.
    fn menu_pointer(&mut self, from: Option<(f32, f32)>, to: (f32, f32), now: f64) {
        let hover = self.hover.clone();
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return;
        };
        match hover {
            Some(Target::MenuSubItem(j)) => {
                m.aim = None;
                m.hover_wait = None;
                if let Some(s) = m.sub.as_mut() {
                    s.sel = s.enabled(j).then_some(j);
                    m.sel = Some(s.parent);
                }
            }
            Some(Target::MenuItem(i)) => {
                if let Some(s) = &m.sub
                    && s.parent != i
                    && let (Some(from), Some(r)) = (from, s.rect)
                    && context_menu::heading_to(from, to, r)
                {
                    // On its way to the submenu: it stays. Resting here, this row takes over.
                    if m.aim.is_none_or(|(a, _)| a != i) {
                        m.aim = Some((i, now));
                    }
                    return;
                }
                m.point_at(i, now);
            }
            _ => {
                m.aim = None;
                m.hover_wait = None;
                // Off the rows the highlight goes, but for the row whose submenu is open.
                if m.sub.is_none() {
                    m.sel = None;
                }
            }
        }
    }

    /// The menu's timers, each frame: the choices of the row the pointer rests on are asked
    /// for (the command goes out with [`take_commands`](Self::take_commands)), and a row
    /// rested on while heading for a submenu takes over. Returns when to look again.
    fn menu_tick(&mut self, now: f64) -> Option<f64> {
        let m = self.menu.as_mut().filter(|m| !m.closing())?;
        let mut wake: Option<f64> = None;
        if let Some((i, since)) = m.aim {
            let left = since + context_menu::AIM_SECS - now;
            if left > 0.0 {
                wake = Some(left);
            } else {
                m.point_at(i, now);
            }
        }
        let (i, since) = m.hover_wait?;
        let left = since + context_menu::SUB_DELAY_SECS - now;
        if left > 0.0 {
            return Some(wake.map_or(left, |w| w.min(left)));
        }
        m.hover_wait = None;
        let r = self.menu_want_sub(i, false);
        self.pending.extend(r.commands);
        wake
    }

    /// Keys while the right-click menu is open: ↑/↓ (and Home/End) move between its rows,
    /// Enter or Space runs the one the keyboard is on, Esc closes it. → (← right to left), or
    /// Enter, opens the choices a row leads to, and goes into them; ← or Esc comes back out.
    /// Any other key closes the menu, and does nothing else.
    fn menu_key(&mut self, named: Option<NamedKey>) -> Response {
        let (into, out) = if self.last_rtl {
            (NamedKey::ArrowLeft, NamedKey::ArrowRight)
        } else {
            (NamedKey::ArrowRight, NamedKey::ArrowLeft)
        };
        let Some(m) = self.menu.as_mut().filter(|m| !m.closing()) else {
            return Response::default();
        };
        // In the submenu.
        if let Some(s) = m.sub.as_mut().filter(|s| s.sel.is_some()) {
            match named {
                Some(NamedKey::ArrowDown) => s.sel = context_menu::step(&s.rows, s.sel, true),
                Some(NamedKey::ArrowUp) => s.sel = context_menu::step(&s.rows, s.sel, false),
                Some(NamedKey::Home) => s.sel = context_menu::step(&s.rows, None, true),
                Some(NamedKey::End) => s.sel = context_menu::step(&s.rows, None, false),
                Some(NamedKey::Enter | NamedKey::Space) => {
                    if let Some(j) = s.sel {
                        return self.menu_choose_sub(j);
                    }
                }
                Some(k) if k == out || k == NamedKey::Escape => m.sub = None,
                _ => self.menu = None,
            }
            return Response::used();
        }
        match named {
            Some(NamedKey::ArrowDown | NamedKey::ArrowUp | NamedKey::Home | NamedKey::End) => {
                let (from, forward) = match named {
                    Some(NamedKey::ArrowDown) => (m.sel, true),
                    Some(NamedKey::ArrowUp) => (m.sel, false),
                    Some(NamedKey::Home) => (None, true),
                    _ => (None, false),
                };
                m.sel = m.step(from, forward);
                m.sub = None;
                m.sub_wanted = None;
                m.hover_wait = None;
                m.aim = None;
            }
            Some(k) if k == into => {
                if let Some(i) = m.sel.filter(|&i| m.leads_on(i)) {
                    return self.menu_want_sub(i, true);
                }
            }
            Some(NamedKey::Enter | NamedKey::Space) => {
                if let Some(i) = m.sel {
                    return self.menu_choose(i, true);
                }
            }
            Some(NamedKey::Escape) if m.sub.is_some() => m.sub = None,
            _ => self.menu = None,
        }
        Response::used()
    }

    /// Replaces the rows of the list opened with [`open_list`](Self::open_list); does nothing
    /// once it was closed.
    pub fn update_list(&mut self, items: Vec<PaletteItem>, loading: Option<String>) {
        if self.modal != Some(Modal::Palette) {
            return;
        }
        if let Some(l) = self.palette.list.as_mut() {
            l.items = items;
            l.loading = loading;
            self.palette.dirty = true;
        }
    }

    /// The About box's storage line (empty: none).
    /// Where to report problems and the support address, shown in About and the diagnostics
    /// dialog (never contacted by the app).
    pub fn set_contact(&mut self, issues: &str, email: &str) {
        self.contact = Some((issues.to_owned(), email.to_owned()));
    }

    /// Screenshots: a fixed build line in About (the real one changes with every commit).
    #[doc(hidden)]
    pub fn set_about_build_line(&mut self, line: &str) {
        self.about_build = Some(line.to_owned());
    }

    pub fn set_about_storage(&mut self, text: String) {
        self.about_storage = text;
    }

    /// Sets the state line and button of action settings that switch something on or off
    /// (by setting key). Returns whether anything changed (redraw then).
    pub fn set_setting_status(
        &mut self,
        status: std::collections::HashMap<&'static str, SettingStatus>,
    ) -> bool {
        let changed = self.setting_status != status;
        self.setting_status = status;
        changed
    }

    /// Greys out (or enables again) the button of action setting `key`, when pressing it
    /// would do nothing ("Show hidden insights again" with none hidden): drawn muted, no
    /// Tab stop, announced as unavailable. Returns whether anything changed (redraw then).
    pub fn set_setting_disabled(&mut self, key: &'static str, disabled: bool) -> bool {
        if self.setting_disabled(key) == disabled {
            return false;
        }
        if disabled {
            self.disabled_settings.push(key);
            // The keyboard on it (it was just pressed) goes on to the control after it, as
            // after a reset, or back to the search field after the last.
            let me = Target::Settings(settings::SettingsTarget::Act(key));
            if self.modal_focus.as_ref() == Some(&me) {
                let stops = self.settings_stops();
                let next = stops
                    .iter()
                    .position(|s| *s == me)
                    .and_then(|i| stops.get(i + 1));
                self.modal_focus = next.filter(|n| **n != Target::ModalClose).cloned();
            }
        } else {
            self.disabled_settings.retain(|k| *k != key);
        }
        true
    }

    /// Whether the button of action setting `key` is greyed out.
    pub fn setting_disabled(&self, key: &str) -> bool {
        self.disabled_settings.contains(&key)
    }

    /// Leaves setting `key` out of the window (or shows it again): its row, its panel and
    /// search results. Returns whether anything changed (redraw then).
    pub fn set_setting_hidden(&mut self, key: &'static str, hidden: bool) -> bool {
        if self.setting_hidden(key) == hidden {
            return false;
        }
        if hidden {
            self.hidden_settings.push(key);
        } else {
            self.hidden_settings.retain(|k| *k != key);
        }
        true
    }

    pub fn setting_hidden(&self, key: &str) -> bool {
        self.hidden_settings.contains(&key)
    }

    /// The state line and button of action setting `key`, if the application set one.
    pub(crate) fn status_of(&self, key: &str) -> Option<&SettingStatus> {
        self.setting_status
            .get(key)
            .or_else(|| self.ai_status.get(key))
    }

    /// Whether a wheel turn at `p` (logical) goes to the chrome, as `wheel` decides: a modal is
    /// open, or a control is under the pointer.
    pub fn wheel_over_chrome(&self, p: (f32, f32)) -> bool {
        self.modal.is_some() || self.hit_at(p).is_some_and(|t| !t.passive())
    }

    /// Whether a list from [`open_list`](Self::open_list) is on screen.
    pub fn list_open(&self) -> bool {
        self.modal == Some(Modal::Palette) && self.palette.list.is_some()
    }

    /// The current image's open report (the information panel's "Performance" section), or
    /// none while it is still opening.
    pub fn set_open_report(&mut self, r: Option<OpenReport>) {
        self.open_report = r;
    }

    /// Expands or collapses the "Performance" section (collapsed by default).
    pub fn set_info_performance(&mut self, open: bool) {
        self.info_perf = open;
    }

    /// Opens the hardware report.
    pub fn show_report(&mut self, report: HwReport) {
        self.report = report;
        self.open_modal(Modal::HwReport);
    }

    /// The hardware report shown last, as plain text (its Copy report button).
    pub fn report_text(&self) -> String {
        self.report.text()
    }

    /// Asks before moving `file` to the trash; accepting sends [`Command::ConfirmTrash`].
    pub fn confirm_trash(&mut self, file: String) {
        self.confirm_file = file;
        self.confirm_custom = None;
        self.confirm_alt = None;
        self.confirm_alt2 = None;
        self.confirm_detail_text = None;
        self.confirm_reset = false;
        self.confirm_destructive = None;
        self.modal = None;
        self.open_modal(Modal::ConfirmTrash);
    }

    /// [`Self::confirm_trash`] with a line under the question (the sidecars that go too).
    pub fn confirm_trash_with(&mut self, file: String, detail: Option<String>) {
        self.confirm_trash(file);
        self.confirm_detail_text = detail;
    }

    /// Asks a yes/no question in the same sheet as the trash confirmation: `title` is the
    /// question, `ok` the accept button; accepting sends [`Command::Item`]`(item)`.
    pub fn confirm(&mut self, title: String, ok: String, item: u64) {
        self.confirm_custom = Some((title, ok, item));
        self.confirm_alt = None;
        self.confirm_alt2 = None;
        self.confirm_detail_text = None;
        self.confirm_reset = false;
        self.confirm_destructive = None;
        self.modal = None;
        self.open_modal(Modal::ConfirmTrash);
    }

    /// [`Chrome::confirm`] with `detail` on the line under the question (what follows from
    /// it). The keyboard starts on the accept button.
    pub fn confirm_with_detail(&mut self, title: String, detail: String, ok: String, item: u64) {
        self.confirm(title, ok, item);
        self.confirm_detail_text = Some(detail);
        self.modal_focus = Some(Target::ConfirmOk);
    }

    /// Asks before a destructive step in the same sheet, as an alert: `title` is the
    /// question, `detail` what follows from it (the line under it, and the alert's
    /// description), `ok` the accept button, in the danger colour. The keyboard starts on
    /// Cancel, so Enter, like Esc, keeps things as they are; accepting sends
    /// [`Command::Item`]`(item)`.
    pub fn confirm_destructive(&mut self, title: String, detail: String, ok: String, item: u64) {
        self.confirm(title, ok, item);
        self.confirm_destructive = Some(detail);
        self.modal_focus = Some(Target::ConfirmCancel);
    }

    /// Offers two ways on, or Cancel, in the same sheet: `title` is the question, `detail` the
    /// lines under it, `ok` the suggested choice (the filled button, and Enter) sending
    /// [`Command::Item`]`(item)`, and `alt` the other one sending `alt_item`. The keyboard
    /// starts on the suggested choice.
    pub fn confirm_choice(
        &mut self,
        title: String,
        detail: String,
        ok: String,
        item: u64,
        alt: String,
        alt_item: u64,
    ) {
        self.confirm(title, ok, item);
        self.confirm_alt = Some((alt, alt_item));
        self.confirm_detail_text = Some(detail);
        self.modal_focus = Some(Target::ConfirmOk);
    }

    /// [`Chrome::confirm_choice`] with a third way on, `alt2` sending `alt2_item`.
    #[allow(clippy::too_many_arguments)]
    pub fn confirm_choice3(
        &mut self,
        title: String,
        detail: String,
        ok: String,
        item: u64,
        alt: String,
        alt_item: u64,
        alt2: String,
        alt2_item: u64,
    ) {
        self.confirm_choice(title, detail, ok, item, alt, alt_item);
        self.confirm_alt2 = Some((alt2, alt2_item));
    }

    /// Asks before resetting every setting (Settings → Advanced → Reset all settings…), in
    /// the same sheet, over Settings: the keyboard starts on Cancel, and Cancel or Esc go
    /// back to Settings with the keyboard on the button. Accepting resets here.
    pub(crate) fn confirm_reset_all(&mut self) {
        if self.modal == Some(Modal::Settings) {
            self.modal_focus = Some(Target::Settings(settings::SettingsTarget::Act(
                settings::RESET_ALL,
            )));
        }
        self.confirm_custom = None;
        self.confirm_alt = None;
        self.confirm_alt2 = None;
        self.confirm_detail_text = None;
        self.confirm_reset = true;
        self.confirm_destructive = None;
        self.open_modal(Modal::ConfirmTrash);
        self.modal_focus = Some(Target::ConfirmCancel);
    }

    /// The open confirmation accepted: it closes, then the settings are reset or its command
    /// is sent.
    fn confirm_accept(&mut self, st: &ViewerState) -> Response {
        let reset = self.confirm_reset;
        let c = self.confirm_command();
        self.close_modal();
        if reset {
            // One undo step: Ctrl+Z in Settings brings every value back.
            st.store
                .reset_all(specular_settings::Origin::SettingsWindow);
            self.announce(t!("settings-reset-all-done"));
            return Response::used();
        }
        Response::cmd(c)
    }

    /// What accepting the open confirmation sends.
    fn confirm_command(&self) -> Command {
        match &self.confirm_custom {
            Some((_, _, item)) => Command::Item(*item),
            None => Command::ConfirmTrash,
        }
    }

    /// The open confirmation's question and accept button.
    pub(crate) fn confirm_texts(&self) -> (String, String) {
        match &self.confirm_custom {
            Some((title, ok, _)) => (title.clone(), ok.clone()),
            None if self.confirm_reset => (
                t!("settings-reset-all-confirm"),
                t!("settings-reset-all-confirm-ok"),
            ),
            None => (
                t!("app-confirm-trash-title", file = self.confirm_file.clone()),
                t!("palette-move-to-trash"),
            ),
        }
    }

    /// The line under the open confirmation's question (`None`: none).
    pub(crate) fn confirm_detail(&self) -> Option<String> {
        if let Some(d) = self
            .confirm_destructive
            .as_ref()
            .or(self.confirm_detail_text.as_ref())
        {
            return Some(d.clone());
        }
        self.confirm_reset
            .then(|| t!("settings-reset-all-confirm-detail"))
    }

    /// Scrolls the open Settings page, or the info panel when it is open, down by `dy`
    /// logical px (screenshots; the next draw clamps it).
    #[doc(hidden)]
    pub fn scroll_settings(&mut self, dy: f32) {
        if self.info_open {
            self.info_scroll += dy;
        } else {
            self.settings.scroll += dy;
        }
    }

    /// Keeps every scrollbar shown instead of fading them out (screenshots).
    #[doc(hidden)]
    pub fn pin_scrollbars(&mut self) {
        self.bars.pinned = true;
    }

    /// Closes the open dialog, back to the one it was opened from if any, as that was left.
    pub fn close_modal(&mut self) {
        if self.modal == Some(Modal::DicomTags) {
            // The rows may hold patient data: none is kept once the panel is closed.
            self.dicom_tags = Default::default();
        }
        if self.modal == Some(Modal::Studies) {
            // The rows hold patient data: none is kept once the browser is closed.
            self.studies = Default::default();
        }
        if self.modal == Some(Modal::ConfirmTrash) {
            self.confirm_reset = false;
            self.confirm_destructive = None;
            self.confirm_alt = None;
            self.confirm_alt2 = None;
            self.confirm_detail_text = None;
        }
        let (modal, focus) = self.modal_parents.pop().unzip();
        self.modal = modal;
        self.modal_focus = focus.flatten();
        self.bars.grab = None;
        self.settings.dropdown = None;
    }

    pub fn toggle_info(&mut self) {
        self.info_open = !self.info_open;
        self.info_scroll = 0.0;
    }

    pub fn set_info(&mut self, open: bool, all: bool) {
        self.info_open = open;
        self.info_all = all;
    }

    /// Runs chrome actions; returns false for actions the application handles.
    pub fn handle_action(&mut self, a: Action) -> bool {
        let toggle = |me: &mut Chrome, m: Modal| {
            if me.modal == Some(m) {
                me.close_modal();
            } else {
                me.open_modal(m);
            }
        };
        match a {
            Action::ToggleInfo => self.toggle_info(),
            Action::Palette => toggle(self, Modal::Palette),
            Action::CheatSheet => toggle(self, Modal::CheatSheet),
            Action::About => toggle(self, Modal::About),
            Action::Help => {
                if self.modal == Some(Modal::Help) {
                    self.close_modal();
                } else {
                    self.show_help(self.help_ctx);
                }
            }
            Action::Settings => toggle(self, Modal::Settings),
            Action::Language => toggle(self, Modal::Language),
            Action::Hud => self.hud_open = !self.hud_open,
            Action::ToggleClipping => self.inspect.tools.clipping ^= true,
            Action::ToggleFocusPeaking => self.inspect.tools.peaking ^= true,
            Action::ToggleHistogram => self.inspect.tools.histogram ^= true,
            Action::TogglePixelInspector => self.inspect.tools.pixel ^= true,
            Action::ChannelRed => self.press_channel(ChannelKey::Red),
            Action::ChannelGreen => self.press_channel(ChannelKey::Green),
            Action::ChannelBlue => self.press_channel(ChannelKey::Blue),
            Action::ChannelAlpha => self.press_channel(ChannelKey::Alpha),
            Action::ChannelLuma => self.press_channel(ChannelKey::Luma),
            Action::ToggleFalseColour => self.inspect.tools.false_colour ^= true,
            Action::NextColourMap => {
                // Stepping the map shows it.
                let tools = &mut self.inspect.tools;
                tools.colour_map = tools.colour_map.next();
                tools.false_colour = true;
                self.announce(inspect::colour_map_name(self.inspect.tools.colour_map));
            }
            Action::NextChannelColours => {
                let tools = &mut self.inspect.tools;
                tools.channel_colours = tools.channel_colours.next();
                self.announce(inspect::channel_colours_label(
                    self.inspect.tools.channel_colours,
                ));
            }
            Action::ToggleHiLo => self.inspect.tools.hilo ^= true,
            Action::NextColourVision => self.set_cvd(self.inspect.tools.cvd.next()),
            Action::SimulateProtanopia => {
                self.set_cvd(self.inspect.tools.cvd.toggle(Cvd::Protanopia))
            }
            Action::SimulateDeuteranopia => {
                self.set_cvd(self.inspect.tools.cvd.toggle(Cvd::Deuteranopia))
            }
            Action::SimulateTritanopia => {
                self.set_cvd(self.inspect.tools.cvd.toggle(Cvd::Tritanopia))
            }
            Action::SimulateAchromatopsia => {
                self.set_cvd(self.inspect.tools.cvd.toggle(Cvd::Achromatopsia))
            }
            Action::CustomiseToolbar => self.customise_toolbar(),
            Action::CustomiseKeys => self.open_settings_at(keys_edit::KEY),
            Action::GoToImage => {
                self.goto = if self.goto.is_some() {
                    None
                } else {
                    Some(LineEdit::new("").with_filter(GOTO_FILTER))
                };
                self.goto_time = false;
                self.zoom_menu = false;
            }
            Action::GoToTime => self.open_goto_time(),
            Action::CastMenu => {
                if self.player.is_some() {
                    self.player_menu = match self.player_menu {
                        Some((PlayerMenu::Cast, _)) => None,
                        _ => Some((PlayerMenu::Cast, None)),
                    };
                    self.zoom_menu = false;
                    self.overflow_menu = None;
                }
            }
            Action::VideoPicture => {
                if self.player.as_ref().is_some_and(|p| p.picture_menu) {
                    self.player_menu = match self.player_menu {
                        Some((PlayerMenu::Picture, _)) => None,
                        _ => Some((PlayerMenu::Picture, None)),
                    };
                    self.zoom_menu = false;
                    self.overflow_menu = None;
                }
            }
            _ => return false,
        }
        true
    }

    /// Sets the colour-vision simulation (the application applies it in the compositor's
    /// final pass) and announces it.
    fn set_cvd(&mut self, c: Cvd) {
        self.inspect.tools.cvd = c;
        self.announce(inspect::cvd_label(c).unwrap_or_else(|| t!("palette-colour-vision-off")));
    }

    fn press_channel(&mut self, key: ChannelKey) {
        let tools = &mut self.inspect.tools;
        tools.channel = tools.channel.press(key);
    }

    /// The channel view to draw for the current image (`ChannelView::effective`).
    pub fn channel_view(&self, has_alpha: bool) -> ChannelView {
        self.inspect.tools.channel.effective(has_alpha)
    }

    /// Esc: closes the topmost overlay, or steps back inside the open dialog first (Settings'
    /// history, a typed filter; history.rs); a dialog opened from another returns to it.
    /// Returns false when there was none.
    pub fn back(&mut self) -> bool {
        // A menu on its way out is not what Esc is for.
        if self.menu.as_ref().is_some_and(|m| m.closing()) {
            self.menu = None;
        }
        let sub_open = self.menu.as_ref().is_some_and(|m| m.sub.is_some());
        if sub_open {
            // A submenu closes first, back to its row.
            if let Some(m) = self.menu.as_mut() {
                m.sub = None;
            }
        } else if self.menu.take().is_some() || self.bar_item_cancel() {
            // Closed by the condition.
        } else if self.goto.is_some() {
            self.goto = None;
        } else if self.zoom_menu {
            self.zoom_menu = false;
        } else if self.player_menu.is_some() {
            self.player_menu = None;
        } else if self.overflow_menu.is_some() {
            self.overflow_menu = None;
        } else if self.toolbar_menu.is_some() {
            self.toolbar_menu = None;
        } else if self.settings.dropdown.is_some() {
            self.close_dropdown();
        } else if self.toolbar_back()
            || (self.modal == Some(Modal::Settings) && self.keys_back())
            || self.dialog_back()
        {
        } else if self.modal == Some(Modal::ConfigEditor) {
            self.editor_back();
        } else if self.modal.is_some() {
            self.close_modal();
        } else if self.kbd.is_some() {
            // The keyboard goes back to the image.
            self.set_kbd(None);
        } else if self.info_open {
            self.info_open = false;
        } else {
            return false;
        }
        true
    }

    fn hit_at(&self, p: (f32, f32)) -> Option<Target> {
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|(_, t)| t.clone())
    }

    /// Feeds one input event.
    pub fn input(&mut self, ev: &Input, st: &ViewerState, now: f64) -> Response {
        let mark = self.settings_mark();
        let mut r = self.input_event(ev, st, now);
        // A field the keyboard left keeps its text (setting_field.rs).
        self.setting_field_sync(st);
        self.settings_record(mark);
        self.offer_primary_after(ev, now, &mut r);
        // Whatever moved an offset (wheel, keys, drags), it stays within the last layout's
        // range, so the next frame never shows content past either end.
        self.clamp_scrolls();
        r
    }

    fn input_event(&mut self, ev: &Input, st: &ViewerState, now: f64) -> Response {
        self.menu_chosen_at = None;
        // A key brings back the play controls of full-screen play mode.
        if self.video_full && matches!(ev, Input::Key { .. }) {
            self.last_activity = now;
        }
        match ev {
            Input::PointerMove { x, y } => {
                let from = self.pointer;
                self.pointer = Some((*x, *y));
                self.last_activity = now;
                // A scrollbar drag captures the pointer: the image never pans under it. So does
                // dragging a toolbar button or a row of the toolbar list.
                if let Some(r) = self.seeking {
                    let fraction = transport::seek_fraction(r, *x);
                    self.scrub = Some((fraction, f64::INFINITY));
                    return Response::cmd(Command::Seek {
                        fraction,
                        done: false,
                    });
                }
                if let Some(r) = self.volume_drag {
                    return self.volume_drag_to(r, *x, false, st, now);
                }
                if self.bar_drag(*y, now)
                    || self.field_drag((*x, *y))
                    || self.editor_drag((*x, *y))
                    || self.toolbar_drag_move((*x, *y))
                    || self.bar_item_move((*x, *y))
                    || self.plot_drag((*x, *y))
                    || self.queue_drag_move((*x, *y))
                {
                    return Response::used();
                }
                self.bars.pointer_moved((*x, *y), now);
                let h = self.hit_at((*x, *y));
                if h != self.hover {
                    self.hover = h;
                    self.hover_since = now;
                }
                if self.menu_open() {
                    self.menu_pointer(from, (*x, *y), now);
                }
                Response {
                    consumed: self.hover.as_ref().is_some_and(|t| !t.passive()),
                    commands: Vec::new(),
                }
            }
            Input::PointerLeave => {
                self.pointer = None;
                self.hover = None;
                Response::default()
            }
            Input::PointerButton { x, y, down, button } => {
                let h = self.hit_at((*x, *y));
                if !*down && let Some(r) = self.seeking.take() {
                    self.pressed = None;
                    let fraction = transport::seek_fraction(r, *x);
                    self.scrub = Some((fraction, now));
                    return Response::cmd(Command::Seek {
                        fraction,
                        done: true,
                    });
                }
                if !*down && let Some(r) = self.volume_drag.take() {
                    self.pressed = None;
                    return self.volume_drag_to(r, *x, true, st, now);
                }
                if *down
                    && *button == Button::Primary
                    && h == Some(Target::Volume)
                    && let Some(r) = self
                        .hits
                        .iter()
                        .rev()
                        .find(|(_, t)| *t == Target::Volume)
                        .map(|(r, _)| *r)
                {
                    self.last_activity = now;
                    self.pressed = h;
                    self.volume_drag = Some(r);
                    return self.volume_drag_to(r, *x, false, st, now);
                }
                if *down
                    && *button == Button::Primary
                    && h == Some(Target::SeekBar)
                    && let Some(r) = self
                        .hits
                        .iter()
                        .rev()
                        .find(|(_, t)| *t == Target::SeekBar)
                        .map(|(r, _)| *r)
                {
                    self.last_activity = now;
                    self.pressed = h;
                    self.seeking = Some(r);
                    let fraction = transport::seek_fraction(r, *x);
                    self.scrub = Some((fraction, f64::INFINITY));
                    return Response::cmd(Command::Seek {
                        fraction,
                        done: false,
                    });
                }
                if !*down && self.bar_release() {
                    self.pressed = None;
                    return Response::used();
                }
                if !*down {
                    self.editor.drag = None;
                    self.field_release();
                    if self.plot_release() {
                        self.pressed = None;
                        return Response::used();
                    }
                }
                // In a field of Settings, the keyboard goes there (and stays on the press).
                let on_setting = *down
                    && *button == Button::Primary
                    && self.setting_field_press((*x, *y), now, st);
                if *down && *button == Button::Primary && !on_setting {
                    self.field_press((*x, *y), now);
                }
                if *down
                    && *button == Button::Middle
                    && let Some(r) = self.field_middle((*x, *y))
                {
                    self.last_activity = now;
                    self.set_kbd(None);
                    return r;
                }
                if *down && let Some(r) = self.editor_middle((*x, *y), *button, h.as_ref()) {
                    self.last_activity = now;
                    self.set_kbd(None);
                    return r;
                }
                if !*down
                    && *button == Button::Primary
                    && let Some(r) = self.bar_item_release(st)
                {
                    self.pressed = None;
                    return r;
                }
                if !*down
                    && *button == Button::Primary
                    && let Some(r) = self.toolbar_release(st)
                {
                    self.pressed = None;
                    return r;
                }
                if !*down
                    && *button == Button::Primary
                    && let Some(r) = self.queue_release(h.as_ref(), now)
                {
                    self.pressed = None;
                    return r;
                }
                if *down {
                    // A press outside the right-click menu closes it. A left click does
                    // nothing else; a right click opens whatever menu belongs where it is.
                    if self.menu.as_ref().is_some_and(|m| m.closing()) {
                        self.menu = None;
                    }
                    if self.menu.is_some()
                        && !matches!(
                            h,
                            Some(Target::MenuItem(_) | Target::MenuSubItem(_) | Target::MenuBg)
                        )
                    {
                        self.menu = None;
                        if *button == Button::Primary {
                            self.pressed = None;
                            return Response::used();
                        }
                    }
                    self.last_activity = now;
                    self.pressed = h.clone();
                    // The pointer takes over from the keyboard.
                    self.set_kbd(None);
                    if !on_setting {
                        self.modal_focus = None;
                    }
                    self.zoom_sel = None;
                    if self.toolbar_menu.is_some() && h != Some(Target::ToolbarMenu) {
                        self.toolbar_menu = None;
                    }
                    // A press outside the More menu closes it (its button toggles it).
                    if self.overflow_menu.is_some()
                        && !matches!(h, Some(Target::ToolbarMore | Target::Toolbar(_)))
                    {
                        self.overflow_menu = None;
                    }
                    // Right-click on the info panel's title or a row copies it, as a click does
                    // (there is nothing else to offer there, and silence reads as broken).
                    if *button == Button::Secondary
                        && let Some(
                            t
                            @ (Target::InfoTitle | Target::InfoCopy(..) | Target::InfoSummary(..)),
                        ) = h.clone()
                    {
                        return self.activate(t, st);
                    }
                    // The toolbar's right-click menu.
                    if *button == Button::Secondary
                        && self.modal.is_none()
                        && matches!(
                            h,
                            Some(
                                Target::Toolbar(_)
                                    | Target::ToolbarMore
                                    | Target::ToolbarBg
                                    | Target::BarSlot(_)
                                    | Target::Position
                            )
                        )
                    {
                        self.zoom_menu = false;
                        self.overflow_menu = None;
                        self.goto = None;
                        self.toolbar_menu = Some((*x, *y));
                        return Response::used();
                    }
                    if *button == Button::Primary {
                        self.toolbar_press(h.as_ref(), (*x, *y));
                        self.queue_press(h.as_ref(), (*x, *y));
                        self.bar_item_press(h.as_ref(), (*x, *y), now, st);
                    }
                    self.bars.focus = match &h {
                        Some(
                            Target::InfoPanel
                            | Target::InfoTitle
                            | Target::InfoTab(_)
                            | Target::InfoPerf
                            | Target::InfoShowOnMap
                            | Target::InfoEditMetadata
                            | Target::InfoCopy(..)
                            | Target::InfoSummary(..)
                            | Target::Scrollbar(scrollbar::Area::Info),
                        ) => Some(scrollbar::Area::Info),
                        _ => None,
                    };
                    if let (Some(Target::Scrollbar(a)), Button::Primary) = (&h, button) {
                        self.bar_press(*a, *y, now);
                        return Response::used();
                    }
                    if let (
                        Some(Target::Editor(config_editor::EditorTarget::Text)),
                        Button::Primary,
                    ) = (&h, button)
                    {
                        self.editor_press((*x, *y), now);
                    }
                    if (&h, button) == (&Some(Target::PlotArea), &Button::Primary)
                        && self.plot_press((*x, *y))
                    {
                        return Response::used();
                    }
                    if self.zoom_menu
                        && !matches!(h, Some(Target::ZoomPreset(_)) | Some(Target::Toolbar(_)))
                    {
                        self.zoom_menu = false;
                    }
                    if self.player_menu.is_some()
                        && !matches!(
                            h,
                            Some(
                                Target::PlayerItem(..)
                                    | Target::PlayerButton(_)
                                    | Target::PlayerDelay(..)
                                    | Target::ToolbarBg
                            )
                        )
                    {
                        self.player_menu = None;
                    }
                    // A click anywhere else cancels the go-to field.
                    if self.goto.is_some() && h != Some(Target::Position) {
                        self.goto = None;
                    }
                    match h {
                        Some(t) if !t.passive() => Response::used(),
                        _ => Response::default(),
                    }
                } else {
                    let pressed = self.pressed.take();
                    match (h, pressed) {
                        (Some(t), Some(p)) if t == p && *button == Button::Primary => {
                            self.activate(t, st)
                        }
                        (Some(t), _) if !t.passive() => Response::used(),
                        _ => Response::default(),
                    }
                }
            }
            Input::Modifiers(m) => {
                self.mods = *m;
                Response::default()
            }
            // The right-click menu holds what is under it still, as the system's menus do. (A
            // trackpad's two-finger click often comes with a small scroll: closing the menu on
            // it would close it as it opens.)
            Input::Wheel { .. } if self.menu_open() => Response::used(),
            Input::Wheel { x, y, dy, .. }
                if self.modal.is_none() && self.hit_at((*x, *y)) == Some(Target::Volume) =>
            {
                self.volume_wheel_by(*dy, st, now)
            }
            Input::Wheel { x, y, dy, .. } => self.wheel((*x, *y), *dy),
            Input::Key { combo, repeat } => self.key(combo, *repeat, st, now),
            // While keys are recorded, a released Esc ends its press (a tap is a key).
            Input::KeyUp { combo } if self.recording_keys() => {
                self.keys_record_release(combo, st, now);
                Response::used()
            }
            Input::KeyUp { combo } => self.key_up(combo),
            // Recorded keys type nothing.
            Input::Text(_) | Input::Preedit(_) if self.recording_keys() => Response::used(),
            // A field of Settings has the keyboard: what is typed goes there.
            Input::Text(s) if self.setting_field_focus().is_some() => {
                self.with_field(|e| e.insert(s));
                Response::used()
            }
            // The Space that presses a focused control of a dialog (or toggles a row of the
            // toolbar list) also arrives as text; anything else typed goes back to the
            // dialog's field.
            Input::Text(s)
                if self.control_focused() && (s.trim().is_empty() || !self.typing_to_field()) =>
            {
                Response::used()
            }
            Input::Text(s) => match self.modal {
                Some(Modal::Palette) => {
                    self.palette.type_text(s);
                    Response::used()
                }
                Some(Modal::Settings) if self.settings.dropdown.is_none() => {
                    self.settings.type_text(s);
                    Response::used()
                }
                Some(Modal::Language) => {
                    self.language.type_text(s);
                    Response::used()
                }
                Some(Modal::DicomTags) => {
                    self.dicom_tags.type_text(s);
                    Response::used()
                }
                Some(Modal::Help) => {
                    self.help.type_text(s);
                    Response::used()
                }
                Some(Modal::ConfigEditor) => {
                    self.editor_text(s);
                    Response::used()
                }
                Some(_) => Response::used(),
                None if self.goto.is_some() => {
                    self.with_field(|e| e.insert(s));
                    Response::used()
                }
                None => Response::default(),
            },
            Input::Preedit(s) => {
                // Composing while a control of the dialog has the keyboard: the keyboard goes
                // back to the field, which shows the composition.
                if !s.is_empty() && self.setting_field_focus().is_none() {
                    self.typing_to_field();
                }
                self.preedit.clone_from(s);
                Response {
                    consumed: self.wants_text(),
                    commands: Vec::new(),
                }
            }
        }
    }

    fn wheel(&mut self, p: (f32, f32), dy: f32) -> Response {
        let d = dy * 48.0;
        match self.modal {
            Some(Modal::Settings) => {
                let a = if self.settings.dropdown.is_some() {
                    scrollbar::Area::Dropdown
                } else if self.settings.side.contains(p) {
                    scrollbar::Area::SettingsNav
                } else {
                    scrollbar::Area::Settings
                };
                self.scroll_by(a, -d);
                return Response::used();
            }
            Some(Modal::CheatSheet) => {
                self.scroll_by(scrollbar::Area::CheatSheet, -d);
                return Response::used();
            }
            Some(Modal::Palette) => {
                self.scroll_by(scrollbar::Area::Palette, -d);
                return Response::used();
            }
            Some(Modal::Language) => {
                self.scroll_by(scrollbar::Area::Language, -d);
                return Response::used();
            }
            Some(Modal::About) => {
                self.scroll_by(scrollbar::Area::About, -d);
                return Response::used();
            }
            Some(Modal::HwReport) => {
                self.scroll_by(scrollbar::Area::Report, -d);
                return Response::used();
            }
            Some(Modal::Insights) => {
                self.scroll_by(scrollbar::Area::Insights, -d);
                return Response::used();
            }
            Some(Modal::Work) => {
                self.scroll_by(scrollbar::Area::Work, -d);
                return Response::used();
            }
            Some(Modal::Downloads) => {
                self.scroll_by(scrollbar::Area::Downloads, -d);
                return Response::used();
            }
            Some(Modal::Queue) => {
                self.scroll_by(scrollbar::Area::Queue, -d);
                return Response::used();
            }
            Some(Modal::DicomTags) => {
                self.scroll_by(scrollbar::Area::DicomTags, -d);
                return Response::used();
            }
            Some(Modal::Help) => {
                self.scroll_by(scrollbar::Area::Help, -d);
                return Response::used();
            }
            Some(Modal::Crash) => {
                self.crash.scroll -= d;
                return Response::used();
            }
            Some(Modal::Diagnostics) => {
                if self.diag.list.is_some_and(|l| l.contains(p)) {
                    self.diag.list_scroll -= d;
                } else {
                    self.diag.scroll -= d;
                }
                return Response::used();
            }
            Some(Modal::Studies) => {
                self.scroll_by(scrollbar::Area::Studies, -d);
                return Response::used();
            }
            Some(Modal::ConfigEditor) => {
                self.scroll_by(scrollbar::Area::ConfigEditor, -d);
                self.editor_wheeled();
                return Response::used();
            }
            Some(Modal::ConfirmTrash | Modal::Profile | Modal::Plot) => return Response::used(),
            None => {}
        }
        match self.hit_at(p) {
            Some(
                Target::InfoPanel
                | Target::InfoTitle
                | Target::InfoTab(_)
                | Target::InfoPerf
                | Target::InfoShowOnMap
                | Target::InfoEditMetadata
                | Target::InfoCopy(..)
                | Target::InfoSummary(..)
                | Target::Scrollbar(scrollbar::Area::Info),
            ) => {
                self.bars.focus = Some(scrollbar::Area::Info);
                self.scroll_by(scrollbar::Area::Info, -d);
                Response::used()
            }
            Some(t) if !t.passive() => Response::used(),
            _ => Response::default(),
        }
    }

    /// Keys while the More menu is open: ↑/↓ (and Home/End) move between its rows, Enter or
    /// Space runs the one the keyboard is on (Esc closes it, in `back`). Any other key closes
    /// it and does what it usually does.
    fn overflow_key(&mut self, named: Option<NamedKey>, st: &ViewerState) -> Option<Response> {
        let n = self.toolbar_overflow.len();
        let sel = self.overflow_menu?.map(|s| s.min(n.saturating_sub(1)));
        if n == 0 {
            return None;
        }
        let to = match (named?, sel) {
            (NamedKey::ArrowDown, None) | (NamedKey::Home, _) => 0,
            (NamedKey::ArrowUp, None) | (NamedKey::End, _) => n - 1,
            (NamedKey::ArrowDown, Some(s)) => (s + 1) % n,
            (NamedKey::ArrowUp, Some(s)) => (s + n - 1) % n,
            (NamedKey::Enter | NamedKey::Space, Some(s)) => {
                let it = self.toolbar_overflow[s];
                let r = self.activate(Target::Toolbar(it), st);
                // The zoom readout's presets open with the keyboard on the first.
                if self.zoom_menu {
                    self.zoom_sel = Some(0);
                }
                return Some(r);
            }
            (NamedKey::Enter | NamedKey::Space, None) => return Some(Response::used()),
            _ => return None,
        };
        self.overflow_menu = Some(Some(to));
        Some(Response::used())
    }

    /// Keys while the zoom presets are open: ↑/↓ (and Home/End) move between them, Enter or
    /// Space picks the one the keyboard is on (Esc closes them, in `back`). Any other key
    /// closes them and does what it usually does.
    fn zoom_key(&mut self, named: Option<NamedKey>, st: &ViewerState) -> Option<Response> {
        let entries = toolbar::zoom_entries();
        let n = entries.len();
        let to = match (named?, self.zoom_sel.map(|s| s.min(n - 1))) {
            (NamedKey::ArrowDown, None) | (NamedKey::Home, _) => 0,
            (NamedKey::ArrowUp, None) | (NamedKey::End, _) => n - 1,
            (NamedKey::ArrowDown, Some(s)) => (s + 1) % n,
            (NamedKey::ArrowUp, Some(s)) => (s + n - 1) % n,
            (NamedKey::Enter | NamedKey::Space, Some(s)) => {
                return Some(self.activate(Target::ZoomPreset(entries[s].1), st));
            }
            (NamedKey::Enter | NamedKey::Space, None) => return Some(Response::used()),
            _ => return None,
        };
        self.zoom_sel = Some(to);
        Some(Response::used())
    }

    fn key(&mut self, combo: &KeyCombo, repeat: bool, st: &ViewerState, now: f64) -> Response {
        let named = match combo.key {
            Key::Named(n) if !combo.mods.any() => Some(n),
            _ => None,
        };
        // While the input method composes, its editing keys (Escape cancels, Enter commits,
        // arrows pick a candidate) are the composition's: one that also arrives here as a key
        // must not close the dialog or run the selected row.
        if !self.preedit.is_empty() && self.wants_text() && matches!(combo.key, Key::Named(_)) {
            return Response::used();
        }
        if self.menu.as_ref().is_some_and(|m| m.closing()) {
            self.menu = None;
        }
        if self.menu.is_some() {
            return self.menu_key(named);
        }
        // Keys being recorded (Settings → Keys) are recorded, whatever they are.
        if self.recording_keys() {
            if self.modal == Some(Modal::Settings) {
                return self.keys_record_press(combo, repeat, st, now);
            }
            self.settings.keys.recording = None;
        }
        let action = st.keymap.lookup(combo);
        if matches!(action, Some(Action::Quit)) {
            return if self.editor_quit() {
                Response::used()
            } else {
                Response::default()
            };
        }
        // A field of Settings with the keyboard takes its editing keys, Enter and Esc.
        if let Some(r) = self.setting_field_key(combo, st) {
            return r;
        }
        if named == Some(NamedKey::Escape)
            && self.modal == Some(Modal::Studies)
            && self.studies.sheet.cancellable
        {
            // The first Esc stops the reading (the list stays), the next closes.
            return Response::cmd(Command::StudiesCancel);
        }
        // F1 (Help): the help for what is on screen, from anywhere, dialogs included.
        if action == Some(Action::Help) && self.goto.is_none() {
            if !repeat {
                self.help_for_screen(st);
            }
            return Response::used();
        }
        if named == Some(NamedKey::Escape) {
            return if self.back() {
                Response::used()
            } else {
                Response::default()
            };
        }
        // Alt+←: back inside a dialog (history.rs).
        if combo.key == Key::Named(NamedKey::ArrowLeft)
            && combo.mods.alt
            && !(combo.mods.ctrl || combo.mods.shift || combo.mods.meta)
            && ((self.modal == Some(Modal::Help) && self.help.go_back()) || self.back_in_dialog())
        {
            return Response::used();
        }
        if self.modal.is_none() && self.goto.is_some() {
            if matches!(named, Some(NamedKey::Tab | NamedKey::F(6))) {
                // Leaving the field (like Esc, nothing is gone to); the key then moves the
                // focus as it would without it.
                self.goto = None;
            } else {
                return self.goto_key(combo, named, action, st);
            }
        }
        if self.modal.is_none() && self.overflow_menu.is_some() {
            if let Some(r) = self.overflow_key(named, st) {
                return r;
            }
            self.overflow_menu = None;
        }
        if self.modal.is_none()
            && self.toolbar_menu.is_some()
            && matches!(named, Some(NamedKey::Enter | NamedKey::Space))
        {
            self.customise_toolbar();
            return Response::used();
        }
        if self.modal.is_none() && self.zoom_menu {
            if let Some(r) = self.zoom_key(named, st) {
                return r;
            }
            self.zoom_menu = false;
        }
        if self.modal.is_none() && self.player_menu.is_some() {
            if let Some(r) = self.player_key(named) {
                return r;
            }
            if named.is_some_and(|n| {
                matches!(
                    n,
                    NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::Tab
                )
            }) {
                self.player_menu = None;
            }
        }
        if self.modal == Some(Modal::ConfigEditor) {
            return self.editor_key(combo, st);
        }
        if let Some(r) = self.focus_key(combo, named, repeat, st, now) {
            return r;
        }
        if self.modal == Some(Modal::Settings)
            && let Some(r) = self.settings_focus_key(combo, named, repeat, st)
        {
            return r;
        }
        if self.modal == Some(Modal::Queue)
            && let Some(r) = self.queue_key(combo, repeat)
        {
            return r;
        }
        if let Some(r) = self.modal_focus_key(combo, named, repeat, st) {
            return r;
        }
        if let Some(r) = self.field_dialog_focus_key(combo, named, repeat, st) {
            return r;
        }
        if !(self.modal == Some(Modal::Settings)
            && self.settings.dropdown.is_none()
            && self.settings.tools.focus.is_some())
            && let Some(r) = self.field_key(combo)
        {
            return r;
        }
        match self.modal {
            Some(Modal::DicomTags) => self.tags_key(combo, named, repeat, st),
            Some(Modal::Help) => self.help_key(combo, named, repeat, st),
            Some(Modal::Crash | Modal::Diagnostics) => {
                match named {
                    // Enter with nothing focused: the default button (Dismiss, Save…).
                    Some(NamedKey::Enter) if !repeat => {
                        let t = if self.modal == Some(Modal::Crash) {
                            Target::CrashDismiss
                        } else {
                            Target::DiagSave
                        };
                        return self.activate(t, st);
                    }
                    Some(NamedKey::ArrowDown) if self.modal == Some(Modal::Diagnostics) => {
                        let n = self.diag.sheet.entries.len();
                        self.diag.selected = (self.diag.selected + 1).min(n.saturating_sub(1));
                        self.diag.scroll = 0.0;
                        self.diag.reveal = true;
                    }
                    Some(NamedKey::ArrowUp) if self.modal == Some(Modal::Diagnostics) => {
                        self.diag.selected = self.diag.selected.saturating_sub(1);
                        self.diag.scroll = 0.0;
                        self.diag.reveal = true;
                    }
                    Some(NamedKey::PageDown) => {
                        self.crash.scroll += 300.0;
                        self.diag.scroll += 300.0;
                    }
                    Some(NamedKey::PageUp) => {
                        self.crash.scroll -= 300.0;
                        self.diag.scroll -= 300.0;
                    }
                    _ => {}
                }
                Response::used()
            }
            Some(Modal::Studies) => self.studies_key(combo, named, repeat, st),
            Some(Modal::Plot) => self.plot_key(combo, repeat),
            Some(Modal::Palette) => {
                if matches!(action, Some(Action::Palette)) {
                    self.close_modal();
                    return Response::used();
                }
                match named {
                    Some(NamedKey::ArrowDown) => self.palette.move_sel(1),
                    Some(NamedKey::ArrowUp) => self.palette.move_sel(-1),
                    Some(NamedKey::PageDown) => self.palette.move_sel(8),
                    Some(NamedKey::PageUp) => self.palette.move_sel(-8),
                    // As in the language list (the field has no caret to move).
                    Some(NamedKey::Home) => self.palette.move_sel(i32::MIN / 2),
                    Some(NamedKey::End) => self.palette.move_sel(i32::MAX / 2),
                    Some(NamedKey::Backspace) => self.palette.backspace(),
                    Some(NamedKey::Enter) => {
                        let sel = self.palette.selected;
                        return self.run_palette(sel, st);
                    }
                    _ => {}
                }
                Response::used()
            }
            Some(Modal::Settings) => {
                let ctrl = combo.mods.ctrl || combo.mods.meta;
                if self.settings.dropdown.is_none() && self.tools_key(combo, st) {
                    return Response::used();
                }
                if ctrl && combo.key == Key::Char('z') {
                    if combo.mods.shift {
                        st.store.redo()
                    } else {
                        st.store.undo()
                    };
                } else if ctrl && combo.key == Key::Char('y') {
                    st.store.redo();
                } else if matches!(action, Some(Action::Settings)) {
                    self.close_modal();
                } else if named == Some(NamedKey::Backspace) {
                    // Edits the search field, wherever the keyboard was.
                    self.modal_focus = None;
                    self.settings.backspace();
                } else if let Some(
                    n @ (NamedKey::PageUp | NamedKey::PageDown | NamedKey::Home | NamedKey::End),
                ) = named
                    && self.settings.dropdown.is_none()
                    && (self.settings.search.is_empty()
                        || !matches!(n, NamedKey::Home | NamedKey::End))
                {
                    self.bar_key(scrollbar::Area::Settings, n, now);
                } else if let Some(n) = named {
                    self.settings.key(n);
                }
                Response::used()
            }
            Some(Modal::Language) => {
                match named {
                    Some(NamedKey::ArrowDown) => self.language.move_sel(1),
                    Some(NamedKey::ArrowUp) => self.language.move_sel(-1),
                    Some(NamedKey::PageDown) => self.language.move_sel(8),
                    Some(NamedKey::PageUp) => self.language.move_sel(-8),
                    Some(NamedKey::Home) => self.language.move_sel(i32::MIN / 2),
                    Some(NamedKey::End) => self.language.move_sel(i32::MAX / 2),
                    Some(NamedKey::Backspace) => self.language.backspace(),
                    Some(NamedKey::Enter) => {
                        let sel = self.language.selected;
                        self.choose_language(sel, st);
                    }
                    _ => {}
                }
                Response::used()
            }
            Some(Modal::Profile) => {
                if named == Some(NamedKey::Enter) {
                    if self.modal_focus == Some(Target::ProfileSkip) {
                        return self.profile_skip();
                    }
                    return self.profile_accept(st);
                }
                Response::used()
            }
            Some(Modal::ConfirmTrash) => {
                // Resetting every setting has no default: Enter presses the focused button
                // only (modal_focus_key).
                if named == Some(NamedKey::Enter) && !self.confirm_reset {
                    return self.confirm_accept(st);
                }
                Response::used()
            }
            Some(Modal::CheatSheet) => {
                if matches!(action, Some(Action::CheatSheet)) {
                    self.close_modal();
                } else {
                    match named {
                        Some(NamedKey::ArrowDown) => self.cheat_scroll += 48.0,
                        Some(NamedKey::ArrowUp) => self.cheat_scroll -= 48.0,
                        // ←/→ go through the tabs (in reading order: mirrored right to left).
                        Some(n @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                            let next = (n == NamedKey::ArrowRight) != self.last_rtl;
                            self.cheat_step(next, st);
                        }
                        Some(n) => {
                            self.bar_key(scrollbar::Area::CheatSheet, n, now);
                        }
                        None => {}
                    }
                }
                Response::used()
            }
            Some(Modal::About) => {
                match named {
                    Some(NamedKey::Enter) => self.close_modal(),
                    Some(NamedKey::ArrowDown) => self.about_scroll += 48.0,
                    Some(NamedKey::ArrowUp) => self.about_scroll -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::About, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            Some(Modal::HwReport) => {
                match named {
                    Some(NamedKey::Enter) => self.close_modal(),
                    Some(NamedKey::ArrowDown) => self.report_scroll += 48.0,
                    Some(NamedKey::ArrowUp) => self.report_scroll -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::Report, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            Some(Modal::Insights) => {
                match named {
                    Some(NamedKey::Enter) => self.close_modal(),
                    Some(NamedKey::ArrowDown) => self.insights_scroll += 48.0,
                    Some(NamedKey::ArrowUp) => self.insights_scroll -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::Insights, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            Some(Modal::Work) => {
                match named {
                    Some(NamedKey::Enter) => self.close_modal(),
                    Some(NamedKey::ArrowDown) => self.work_scroll += 48.0,
                    Some(NamedKey::ArrowUp) => self.work_scroll -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::Work, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            Some(Modal::Downloads) => {
                match named {
                    Some(NamedKey::Enter) => self.close_modal(),
                    Some(NamedKey::ArrowDown) => self.downloads_scroll += 48.0,
                    Some(NamedKey::ArrowUp) => self.downloads_scroll -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::Downloads, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            Some(Modal::Queue) => {
                match named {
                    Some(NamedKey::ArrowDown) => *self.queue_scroll_mut() += 48.0,
                    Some(NamedKey::ArrowUp) => *self.queue_scroll_mut() -= 48.0,
                    Some(n) => {
                        self.bar_key(scrollbar::Area::Queue, n, now);
                    }
                    None => {}
                }
                Response::used()
            }
            // Handled above (`editor_key`).
            Some(Modal::ConfigEditor) => Response::used(),
            // The info panel, once clicked or scrolled, takes the paging keys.
            None if self.info_open && self.bars.focus == Some(scrollbar::Area::Info) => match named
            {
                Some(n) if self.bar_key(scrollbar::Area::Info, n, now) => Response::used(),
                _ => Response::default(),
            },
            None => Response::default(),
        }
    }

    /// Keys while the go-to field is open: Enter goes; the caret, selection, editing,
    /// clipboard and undo keys of a text field are the field's; other shortcuts with
    /// Ctrl/Cmd/Alt still work (and close the field); every other key is the field's.
    fn goto_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        action: Option<Action>,
        st: &ViewerState,
    ) -> Response {
        if named == Some(NamedKey::Enter) && self.goto_time {
            let Some(v) = st.playback.as_ref().and_then(|p| p.video) else {
                self.goto = None;
                return Response::used();
            };
            let text = self.goto.as_ref().map(|g| g.text().to_owned());
            // Something not understood stays to be put right.
            return match text.and_then(|t| goto_time::parse(&t, v.position, v.duration)) {
                Some(secs) if v.duration > 0.0 => {
                    self.goto = None;
                    Response::cmd(Command::Seek {
                        fraction: (secs / v.duration).clamp(0.0, 1.0),
                        done: true,
                    })
                }
                _ => Response::used(),
            };
        }
        if named == Some(NamedKey::Enter) {
            let n: usize = self
                .goto
                .take()
                .and_then(|g| g.text().parse().ok())
                .unwrap_or(0);
            if n == 0 || st.count == 0 {
                return Response::used();
            }
            return Response::cmd(Command::GoTo(n.min(st.count) - 1));
        }
        // The text field's keys, as in every other field. Digits run left to right in every
        // script: Left goes towards the start.
        if let Some(r) = self.field_edit_key(combo.key, combo.mods, false) {
            return r;
        }
        if combo.mods.ctrl || combo.mods.meta || combo.mods.alt {
            self.goto = None;
            // The go-to shortcut itself just closes the field.
            return if matches!(action, Some(Action::GoToImage | Action::GoToTime)) {
                Response::used()
            } else {
                Response::default()
            };
        }
        Response::used()
    }

    /// The toolbar over the Settings scrim, undimmed and outlined, while the toolbar list's
    /// moves show on it. It takes no clicks (the dialog is modal). `a`: the dialog's fade.
    fn draw_live_bar(&mut self, cx: &mut Ctx, st: &ViewerState, a: f32) {
        cx.dl.pop_opacity();
        let n = cx.hits_len();
        let menus = toolbar::draw(self, cx, st, self.toolbar_alpha);
        toolbar::draw_menus(self, cx, st, &menus);
        let bar = menus.bar;
        cx.truncate_hits(n);
        cx.dl.push_opacity(self.toolbar_alpha * a);
        let r = bar.inset(-4.0);
        cx.dl.rect_border(
            r,
            r.h * 0.5,
            Color::TRANSPARENT,
            cx.pal.accent.with_alpha(0.6),
            1.5,
        );
        cx.dl.pop_opacity();
        cx.dl.push_opacity(a);
    }

    fn activate(&mut self, t: Target, st: &ViewerState) -> Response {
        use specular_settings::ToolbarItem as T;
        // Disabled controls swallow the click and do nothing.
        if t.disabled(st).is_some() {
            return Response::used();
        }
        match t {
            Target::MenuItem(i) => self.menu_choose(i, false),
            Target::MenuSubItem(j) => self.menu_choose_sub(j),
            Target::MenuBg => Response::used(),
            Target::ProfileChip(capability) => Response::cmd(Command::Capability {
                capability,
                always: false,
                then: None,
            }),
            Target::ToolbarMore => {
                self.overflow_menu = match self.overflow_menu {
                    Some(_) => None,
                    None => Some(None),
                };
                self.zoom_menu = false;
                self.goto = None;
                Response::used()
            }
            Target::Toolbar(item) => {
                // A row of the More menu runs and closes it.
                self.overflow_menu = None;
                let a = match item {
                    T::Open => Action::Open,
                    T::Prev => Action::PrevImage,
                    T::Next => Action::NextImage,
                    T::ZoomOut => Action::ZoomOut,
                    T::ZoomIn => Action::ZoomIn,
                    T::ZoomLevel => {
                        self.zoom_menu = !self.zoom_menu;
                        self.zoom_sel = None;
                        return Response::used();
                    }
                    T::Fit => {
                        if st.at_fit {
                            Action::ZoomTo(100.0)
                        } else {
                            Action::ZoomFit
                        }
                    }
                    T::RotateLeft => Action::RotateLeft,
                    T::RotateRight => Action::RotateRight,
                    T::Info => Action::ToggleInfo,
                    T::Copy => Action::CopyImage,
                    T::Delete => Action::MoveToTrash,
                    T::Palette => Action::Palette,
                    T::Settings => Action::Settings,
                    T::Fullscreen => Action::ToggleFullscreen,
                    T::Slideshow => Action::Slideshow,
                    T::ExportSlideshow => Action::ExportSlideshow,
                    T::Mpr => Action::Mpr,
                    T::Discord => Action::JoinDiscord,
                    // Not controls.
                    T::Spacer | T::Separator => return Response::used(),
                };
                if self.handle_action(a) {
                    Response::used()
                } else {
                    Response::cmd(Command::Action(a))
                }
            }
            Target::ZoomPreset(a) => {
                self.zoom_menu = false;
                match a {
                    // The ticked choice stays chosen.
                    Action::ToggleFullscreen if st.fullscreen => Response::used(),
                    Action::ToggleFullscreen => Response::cmd(Command::EnterFullscreen),
                    _ => Response::cmd(Command::Action(a)),
                }
            }
            Target::Transport(Action::GoToTime) => {
                self.open_goto_time();
                Response::used()
            }
            Target::Transport(a) => Response::cmd(Command::Action(a)),
            Target::PlayerButton(m) => {
                self.toggle_player_menu(m);
                Response::used()
            }
            Target::PlayerItem(m, i) => {
                let cmd = self.player.as_ref().and_then(|v| player::stop_cmd(m, v, i));
                if !cmd.as_ref().is_some_and(player::keeps_open) {
                    self.player_menu = None;
                }
                cmd.map_or_else(Response::used, |c| self.player_choice(c))
            }
            Target::PlayerDelay(m, row, step) => player::step_cmd(m, row, step)
                .map_or_else(Response::used, |c| Response::cmd(Command::Player(c))),
            Target::Position => {
                if self.goto.is_none() {
                    self.handle_action(Action::GoToImage);
                }
                Response::used()
            }
            Target::EdgePrev => Response::cmd(Command::Action(Action::PrevImage)),
            Target::EdgeNext => Response::cmd(Command::Action(Action::NextImage)),
            Target::ViewerBack => Response::cmd(Command::BackToGallery),
            Target::Welcome(w) => self.welcome_activate(w, st),
            Target::CheatTab(m) => {
                self.cheat_pick(m);
                Response::used()
            }
            // Only there for its tooltip.
            Target::CheatLine(..) => Response::used(),
            Target::InfoTab(all) => {
                self.info_all = all;
                self.info_scroll = 0.0;
                Response::used()
            }
            Target::InfoClose => {
                self.info_open = false;
                Response::used()
            }
            Target::InfoPerf => {
                self.info_perf = !self.info_perf;
                Response::used()
            }
            Target::InfoShowOnMap => Response::cmd(Command::ShowOnMap),
            Target::InfoEditMetadata => Response::cmd(Command::EditMetadata),
            Target::InfoCopy(g, f) => match info::copy_text(st, g, f) {
                Some(s) => Response::cmd(Command::CopyText(s.to_owned())),
                None => Response::used(),
            },
            Target::InfoSummary(g, i) => match info::summary_text(st, g, i) {
                Some(s) => Response::cmd(Command::CopyText(s.to_owned())),
                None => Response::used(),
            },
            Target::InfoTitle => match st.file_name {
                Some(s) => Response::cmd(Command::CopyText(s.to_owned())),
                None => Response::used(),
            },
            Target::ToastUndo(id, undo) => {
                self.toasts.retain(|t| t.id != id);
                Response::cmd(Command::Undo(undo))
            }
            Target::ToastClose(id) => {
                self.toasts.retain(|t| t.id != id);
                Response::used()
            }
            Target::Editor(e) => self.editor_click(e, st),
            Target::ModalClose if self.modal == Some(Modal::ConfigEditor) => {
                self.editor_close();
                Response::used()
            }
            Target::Scrim => {
                if self.settings.dropdown.is_some() {
                    self.close_dropdown();
                } else {
                    self.close_modal();
                }
                Response::used()
            }
            Target::PaletteRow(i) => self.run_palette(i, st),
            Target::AboutHelp => {
                self.show_help("about");
                Response::used()
            }
            Target::AboutDiagnostics => {
                self.close_modal();
                Response::cmd(Command::Action(Action::SaveDiagnostics))
            }
            Target::AboutRenderer => {
                Response::cmd(Command::SettingAction(renderer_info::RENDERER_INFO))
            }
            // The browser opens; About stays.
            Target::AboutDiscord => Response::cmd(Command::Action(Action::JoinDiscord)),
            Target::HelpRow(i) => self.help_row_click(i),
            Target::HelpBack => {
                self.help.go_back();
                Response::used()
            }
            Target::HelpContents => {
                self.help.go(help::Loc::default());
                Response::used()
            }
            Target::CrashView | Target::CrashCopy | Target::CrashSave | Target::CrashDismiss => {
                self.crash_click(&t)
            }
            Target::DiagEntry(_) | Target::DiagNames | Target::DiagSave | Target::DiagCancel => {
                self.diag_click(&t)
            }
            Target::TagRow(i) => self.tag_row_click(i),
            Target::TagJump(i) => self.tag_jump(i),
            Target::PlotArea => Response::used(),
            // Pressing and dragging it is handled with the pointer (Command::Seek).
            Target::SeekBar | Target::Volume => Response::used(),
            Target::TagPatient => self.toggle_patient_data(),
            Target::StudyRow(i) => self.study_row_click(i),
            Target::StudiesPatient => self.toggle_studies_patient(),
            Target::StudiesCancel => Response::cmd(Command::StudiesCancel),
            Target::StudiesSend => Response::cmd(Command::StudiesSend(self.studies_selected())),
            Target::LanguageRow(i) => {
                self.choose_language(i, st);
                Response::used()
            }
            Target::ModalClose => {
                self.close_modal();
                Response::used()
            }
            Target::Settings(s) => self.settings_click(s, st),
            Target::ConfirmOk => self.confirm_accept(st),
            Target::ProfileCheck(c) => {
                self.profile_toggle(c);
                Response::used()
            }
            Target::ProfileDone => self.profile_accept(st),
            Target::ProfileStart(o) => {
                self.profile_start = o;
                Response::used()
            }
            Target::ProfileSkip => self.profile_skip(),
            Target::ConfirmCancel => {
                self.close_modal();
                Response::used()
            }
            Target::ConfirmAlt | Target::ConfirmAlt2 => {
                let alt = if t == Target::ConfirmAlt2 {
                    &self.confirm_alt2
                } else {
                    &self.confirm_alt
                };
                let item = alt.as_ref().map(|a| a.1);
                self.close_modal();
                match item {
                    Some(i) => Response::cmd(Command::Item(i)),
                    None => Response::used(),
                }
            }
            Target::ReportCopy => Response::cmd(Command::CopyReport),
            Target::DecoderCard(i) => match self.decoder_card_click(i) {
                Some(c) => Response::cmd(c),
                None => Response::used(),
            },
            Target::Motion => Response::cmd(Command::Motion),
            Target::PresentingPause(_) => Response::cmd(Command::PresentingPause),
            Target::PresentingStop(_) => Response::cmd(Command::PresentingStop),
            Target::CastStop => Response::cmd(Command::Player(PlayerCmd::CastStop)),
            t @ (Target::Insight
            | Target::InsightHide
            | Target::InsightAction(_)
            | Target::InsightDismiss
            | Target::InsightSwitch
            | Target::InsightRestore) => self.insight_click(&t),
            t @ (Target::Work | Target::WorkPause(_) | Target::WorkSettings(_)) => {
                self.work_click(&t)
            }
            Target::WorkBar(_) => Response::used(),
            t @ (Target::Downloads | Target::DownloadStop(_)) => self.downloads_click(&t),
            Target::Queue(q) => self.queue_click(q),
            Target::DownloadBar(_) => Response::used(),
            Target::ToolbarMenu => {
                self.customise_toolbar();
                Response::used()
            }
            Target::InfoPanel
            | Target::ToolbarBg
            | Target::BarSlot(_)
            | Target::ModalBody
            | Target::Toast
            | Target::Scrollbar(_) => Response::used(),
        }
    }

    fn run_palette(&mut self, i: usize, st: &ViewerState) -> Response {
        let Some(e) = self.palette.items.get(i).cloned() else {
            return Response::used();
        };
        // A command whose tools are off turns them on for the session, then runs.
        if let (palette::Run::Action(a), Some(c)) = (&e.run, e.hidden) {
            self.close_modal();
            return Response::cmd(Command::Capability {
                capability: c,
                always: false,
                then: Some(*a),
            });
        }
        // Disabled entries are listed (greyed, with the reason) so they can be found;
        // choosing one opens the manual where it says why (Esc comes back here).
        if let palette::Run::Action(a) = e.run
            && !a.enabled(&st.actions)
        {
            if let Some(d) = a.disabled(&st.actions) {
                self.show_help_topic(specular_help::disabled_topic(d));
            }
            return Response::used();
        }
        if e.unavailable.is_some() {
            return Response::used();
        }
        self.close_modal();
        match e.run {
            palette::Run::Action(a) => {
                if self.handle_action(a) {
                    Response::used()
                } else {
                    Response::cmd(Command::Action(a))
                }
            }
            palette::Run::Set(key, v) => {
                let _ = st.store.set(key, v, specular_settings::Origin::Palette);
                Response::used()
            }
            palette::Run::Item(id, _) => Response::cmd(Command::Item(id)),
            palette::Run::EditSettingsFile => {
                self.open_config_editor(st.store);
                Response::used()
            }
            palette::Run::EditKeymapFile => {
                self.open_keymap_editor(st.keymap);
                Response::used()
            }
            palette::Run::Capability(c, always) => Response::cmd(Command::Capability {
                capability: c,
                always,
                then: None,
            }),
            palette::Run::Help(page, anchor) => {
                self.show_help_at(Some(page), &anchor);
                Response::used()
            }
        }
    }

    fn choose_language(&mut self, i: usize, st: &ViewerState) {
        if let Some(id) = self.language.ids.get(i).cloned() {
            let _ = st
                .store
                .set_text("general.language", &id, specular_settings::Origin::Program);
            self.close_modal();
        }
    }

    /// Draws the chrome. Returns how long until it needs another frame (`Some(0.0)` while
    /// animating, `None` when idle).
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        st: &ViewerState,
        pal: &Palette,
        now: f64,
    ) -> Option<f64> {
        let dt = ((now - self.last_now) as f32).clamp(0.0, 0.1);
        self.last_now = now;
        let i18n = I18n::global();
        let generation = i18n.generation();
        if generation != self.i18n_gen {
            self.i18n_gen = generation;
            self.palette.dirty = true;
            self.language.dirty = true;
        }
        let rtl = i18n.direction().is_rtl();
        #[cfg(test)]
        let rtl = rtl || self.force_rtl;
        self.last_rtl = rtl;
        if !matches!(self.modal, Some(Modal::Palette | Modal::Help)) {
            self.help_ctx = self.help_context(st);
        }
        self.inspect.sync_settings(st.settings);
        if !self.zoom_menu {
            self.zoom_sel = None;
        }
        if self.gallery {
            // The gallery has its own focus; of the chrome only the indicators take it there.
            if !self
                .kbd
                .as_ref()
                .is_some_and(|k| self.indicator_targets().contains(k))
            {
                self.kbd = None;
            }
            self.kbd_hold = None;
        }
        let (w, h) = self.size;
        let mut wake: Option<f64> = None;
        // Paging repeat uses last frame's geometry; then every area records its own again.
        self.clamp_scrolls();
        self.bar_tick(now);
        let hold_wake = self.bar_hold_tick(now);
        let keys_wake = self.keys_tick(st, now);
        self.bars.begin_frame();
        let mut want = |d: f64| wake = Some(wake.map_or(d, |w: f64| w.min(d)));
        if let Some(d) = hold_wake {
            want(d);
        }
        if let Some(d) = keys_wake {
            want(d);
        }
        if let Some(d) = self.primary_tick(now) {
            want(d);
        }

        // Toolbar visibility.
        let s = st.settings;
        // While a held tool (loupe, zoom rectangle) follows the pointer, moving it does not
        // reveal the toolbar or the edge arrows.
        let tool = st.overlays.loupe.is_some() || st.overlays.zoom_rect.is_some();
        let near_bottom = !tool && self.pointer.is_some_and(|p| p.1 > h - 110.0);
        let delay = s.viewing.toolbar_autohide_delay;
        let idle = now - self.last_activity;
        // On the toolbar page of Settings, a bar the dialog leaves uncovered shows the list's
        // changes as they are made: it stays up (decided on the last frame, see `live` below).
        let pinned = matches!(self.modal, Some(Modal::Settings))
            && self.settings.tools.on_page
            && self.settings.tools.live
            && !self.gallery;
        let show_toolbar = !s.viewing.toolbar_autohide
            || self.accessibility
            || pinned
            || self.bar_busy()
            || !st.has_image
            || near_bottom
            || self.zoom_menu
            || self.overflow_menu.is_some()
            || self.goto.is_some()
            || self.toolbar_menu.is_some()
            || self
                .kbd
                .as_ref()
                .and_then(focus::region_of)
                .is_some_and(|r| {
                    matches!(
                        r,
                        focus::Region::Toolbar
                            | focus::Region::Transport
                            | focus::Region::Indicator
                    )
                })
            || !tool && idle < delay
            || !tool
                && self.hover.as_ref().is_some_and(|t| {
                    matches!(
                        t,
                        Target::Toolbar(_)
                            | Target::ToolbarMore
                            | Target::ProfileChip(_)
                            | Target::Position
                            | Target::ToolbarBg
                            | Target::BarSlot(_)
                            | Target::ZoomPreset(_)
                            | Target::Transport(_)
                            | Target::ToolbarMenu
                    )
                });
        if show_toolbar
            && s.viewing.toolbar_autohide
            && st.has_image
            && !near_bottom
            && idle < delay
        {
            want(delay - idle + 0.01);
        }
        let fade = |a: &mut f32, on: bool, dt: f32, rm: bool| -> bool {
            let target = if on { 1.0 } else { 0.0 };
            if rm {
                *a = target;
                return false;
            }
            let k = 1.0 - (-dt / (durations::FADE * 0.45)).exp();
            *a += (target - *a) * k;
            if (*a - target).abs() < 0.01 {
                *a = target;
                false
            } else {
                true
            }
        };
        if self.gallery {
            self.toolbar_menu = None;
            self.overflow_menu = None;
        }
        // Announcements stay in the tree long enough to be read out.
        if self.announce.as_ref().is_some_and(|a| now - a.2 > 5.0) {
            self.announce = None;
        }
        self.announce_zoom(st);
        self.announce_page(st);
        let rm = self.reduce_motion;
        let show_toolbar = show_toolbar && !self.presenting && !self.video_full;
        let mut animating = fade(&mut self.toolbar_alpha, show_toolbar, dt, rm);
        let dock = self.dock_wanted(st, tool);
        animating |= fade(&mut self.video_dock, dock, dt, rm);
        // Full-screen play mode: the transport over the picture.
        let playing = st.playback.as_ref().is_some_and(|p| p.playing);
        let play_on = self.video_full
            && st.playback.is_some()
            && (!playing
                || idle < PLAY_CONTROLS_IDLE
                || self.seeking.is_some()
                || self.player_menu.is_some()
                || self.goto.is_some() && self.goto_time
                || self.modal.is_some()
                || self
                    .kbd
                    .as_ref()
                    .and_then(focus::region_of)
                    .is_some_and(|r| r == focus::Region::Transport)
                || self.hover.as_ref().is_some_and(|t| {
                    matches!(
                        t,
                        Target::Transport(_)
                            | Target::SeekBar
                            | Target::Volume
                            | Target::ToolbarBg
                            | Target::PlayerButton(_)
                            | Target::PlayerItem(..)
                            | Target::PlayerDelay(..)
                    )
                }));
        if self.video_full && playing && idle < PLAY_CONTROLS_IDLE {
            want(PLAY_CONTROLS_IDLE - idle + 0.01);
        }
        animating |= fade(&mut self.play_alpha, play_on, dt, rm);
        let edge_zone = 140.0;
        // Edge arrows follow the navigation rule (no arrows to a dead end).
        let multi = Action::NextImage.enabled(&st.actions)
            && self.modal.is_none()
            && !self.gallery
            && !self.presenting
            && !self.video_full
            && !tool;
        // A panel whose action became disabled closes (the info panel after the folder's
        // last image is trashed). Not while the next file is opening: that is a moment.
        if st.actions.image == ImageState::None {
            if self.info_open && !Action::ToggleInfo.enabled(&st.actions) {
                self.info_open = false;
            }
            if self.zoom_menu && !Action::ZoomFit.enabled(&st.actions) {
                self.zoom_menu = false;
            }
        }
        let left_on = multi
            && self.pointer.is_some_and(|p| {
                let inset = self.info_inset_side(rtl, true);
                p.0 > inset && p.0 < edge_zone + inset && p.1 > 60.0 && p.1 < h - 90.0
            });
        let right_on = multi
            && self.pointer.is_some_and(|p| {
                // Over the image only, not over a panel at that side.
                let inset = self.info_inset_side(rtl, false);
                p.0 > w - edge_zone - inset && p.0 < w - inset && p.1 > 60.0 && p.1 < h - 90.0
            });
        animating |= fade(&mut self.edge_alpha[0], left_on, dt, rm);
        animating |= fade(&mut self.edge_alpha[1], right_on, dt, rm);
        animating |= fade(
            &mut self.info_alpha,
            self.info_open && !self.video_full,
            dt,
            rm,
        );
        animating |= fade(&mut self.modal_alpha, self.modal.is_some(), dt, rm);

        // Keep the hover target valid for the pointer when the layout moves.
        if let Some(p) = self.pointer {
            let hnow = self.hit_at(p);
            if hnow != self.hover {
                self.hover = hnow;
                self.hover_since = now;
            }
        }

        let mut cx = Ctx::new(
            dl,
            text,
            pal,
            rtl,
            w,
            h,
            self.text_scale,
            now,
            self.hover.clone(),
            self.pressed.clone(),
        );
        cx.preedit.clone_from(&self.preedit);
        cx.focus = match self.modal {
            None => self.kbd.clone(),
            Some(_) => self.modal_focus.clone(),
        };
        let mut glass = false;
        overlay::draw(&mut cx, &st.overlays);

        // Center states.
        self.welcome_drawn = false;
        if self.gallery {
        } else if let Some(err) = st.error {
            match self.decoder_card.clone() {
                Some(card) => decoder_card::draw(&mut cx, &card),
                None => self.draw_message(&mut cx, err, !self.notice),
            }
        } else if !st.has_image && !st.loading {
            welcome::draw(self, &mut cx, st);
            self.welcome_drawn = true;
        } else if st.loading {
            // DESIGN.md §3.2: spinners only after 300 ms (a quick open never flashes one);
            // a slow one then says what it is opening.
            let file = st.file_name.map(str::to_owned);
            let since = match &self.loading_since {
                Some((f, t)) if *f == file && *t <= now => *t,
                _ => {
                    self.loading_since = Some((file, now));
                    now
                }
            };
            let age = now - since;
            if age < SPINNER_DELAY {
                want(SPINNER_DELAY - age);
            } else {
                let (ccx, ccy) = (w * 0.5, h * 0.5);
                let a = (now * 5.0) as f32 % std::f32::consts::TAU;
                cx.dl.arc(
                    (ccx, ccy),
                    14.0,
                    a,
                    4.2,
                    2.4,
                    pal.text_muted.with_alpha(0.8),
                );
                want(1.0 / 60.0);
                if age >= OPENING_NAME_DELAY
                    && let Some(name) = st.file_name
                {
                    let msg = t!("viewer-opening", file = name);
                    let style = cx.style(metrics::FONT_BODY);
                    let maxw = (w - 80.0).min(cx.u(520.0));
                    let msg = cx.ellipsize(&msg, style, maxw, false);
                    let b = cx.layout(&msg, style, None);
                    let r = Rect::new((w - b.width) * 0.5, ccy + 30.0, b.width, b.height);
                    cx.block_in(&b, r, ctx::HAlign::Center, pal.text_muted);
                }
            }
        }
        if !st.loading {
            self.loading_since = None;
        }
        // Badges stack down from the top centre.
        let mut badge_y = 14.0;
        let channel = self.channel_view(st.actions.has_alpha);
        if channel != ChannelView::Normal && st.has_image && st.error.is_none() && !self.gallery {
            let label = inspect::channel_label(channel);
            let swatch = inspect::channel_swatch(channel);
            badge_y = self.draw_badge(&mut cx, badge_y, &label, Some(swatch)) + 6.0;
            glass = true;
        }
        // Display aids: the simulation covers the whole window (gallery too); HiLo, and false
        // colour while its bar is hidden, follow the image.
        if let Some(label) = inspect::cvd_label(self.inspect.tools.cvd) {
            badge_y = self.draw_badge(&mut cx, badge_y, &label, Some(Color::hex(0x9E9E9E))) + 6.0;
            glass = true;
        }
        if st.has_image && st.error.is_none() && !self.gallery {
            if self.inspect.tools.hilo {
                let hi = st.settings.inspection.highlight_color;
                let swatch = Color::rgba(
                    hi.r as f32 / 255.0,
                    hi.g as f32 / 255.0,
                    hi.b as f32 / 255.0,
                    1.0,
                );
                badge_y = self.draw_badge(&mut cx, badge_y, &t!("badge-hilo"), Some(swatch)) + 6.0;
                glass = true;
            }
            if let Some(map) = self.inspect.false_colour()
                && !inspect::colour_bar_shown(&self.inspect, st.settings)
            {
                let label = t!("badge-false-colour", map = inspect::colour_map_name(map));
                let c = map.sample(0.75);
                let swatch = Color::rgba(c[0], c[1], c[2], 1.0);
                badge_y = self.draw_badge(&mut cx, badge_y, &label, Some(swatch)) + 6.0;
                glass = true;
            }
        }
        if st.truncated && st.has_image && !self.gallery {
            badge_y = self.draw_badge(&mut cx, badge_y, &t!("app-truncated-badge"), None) + 6.0;
            glass = true;
        }
        if self.fullscreen_hint_until > now && st.fullscreen {
            let hint = if self.video_full {
                t!("fullscreen-exit-hint")
            } else {
                t!("fullscreen-back-hint")
            };
            self.draw_badge(&mut cx, badge_y, &hint, None);
            want(self.fullscreen_hint_until - now);
            glass = true;
        }

        // Edge arrows.
        for (i, a) in self.edge_alpha.into_iter().enumerate() {
            if a > 0.0 {
                let left = i == 0;
                // In RTL the left arrow goes forward.
                let next = left == rtl;
                toolbar::edge_arrow(&mut cx, left, next, a, self.info_inset_side(rtl, left));
                glass = true;
            }
        }
        // The back button comes and goes with the toolbar.
        let back_alpha = self.toolbar_alpha * (1.0 - self.video_dock);
        let has_back = self.back_to_gallery && !self.gallery && !self.video_full;
        if has_back && back_alpha > 0.01 {
            toolbar::back_button(&mut cx, back_alpha);
            glass = true;
        }

        if self.info_alpha > 0.0 && !self.gallery {
            info::draw(self, &mut cx, st, self.info_alpha);
            glass = true;
            if self.info_reveal > 0 {
                want(0.0);
            }
        } else {
            self.info_rows.clear();
            self.info_reveal = 0;
        }
        if self.hud_open && !self.video_full {
            // Below the back button (they share the top leading corner), also while the
            // button is faded out with the toolbar, so the panel doesn't jump.
            let top = if has_back {
                16.0 + toolbar::BACK_SIZE + 8.0
            } else {
                14.0
            };
            hud::draw(&mut cx, st.hud, self.info_inset_side(rtl, !rtl), top);
        }
        if st.has_image && st.error.is_none() && !self.gallery && !self.video_full {
            inspect::draw(self, &mut cx, st.settings);
        } else {
            self.inspect.loupe = None;
        }
        // Whether the real bar shows the toolbar list's changes in Settings: when it is on
        // screen and the dialog leaves it uncovered (else the list shows a preview of it).
        let bar_on = self.toolbar_alpha > 0.0 && !self.gallery;
        let live = matches!(self.modal, Some(Modal::Settings))
            && bar_on
            && !toolbar_edit::preview_needed(
                Some(toolbar::bar_rect(&mut cx, st)),
                settings::sheet_rect(&cx),
            );
        self.settings.tools.live = live;
        self.apply_show_drag();
        if self.bar_press.is_none() {
            self.bar_geom = None;
        }
        // A faded bar keeps its stops (Tab brings it back); the gallery has none.
        if self.gallery {
            self.bar_stops.clear();
        }
        let mut menus = None;
        // Toasts stack above the bar and whatever sits on it (transport, page stepper), so
        // they never cover a button.
        let mut toast_floor = cx.h - metrics::TOOLBAR_MARGIN - metrics::TOOLBAR_HEIGHT;
        self.toolbar_rect = None;
        self.player_anchors.clear();
        if bar_on {
            // A playing movie's toolbar gives its place to the transport (`video_dock`).
            let toolbar_alpha = self.toolbar_alpha * (1.0 - self.video_dock);
            // A live bar is drawn over the dialog's scrim instead (below). The start screen
            // has none: with no image, its controls would all be off.
            if !live && !self.welcome_drawn && toolbar_alpha > 0.01 {
                menus = Some(toolbar::draw(self, &mut cx, st, toolbar_alpha));
            }
            self.toolbar_rect = menus
                .as_ref()
                .filter(|_| toolbar_alpha > 0.5)
                .map(|m| m.bar);
            if let Some(pb) = &st.playback {
                let pb = self.scrubbed(pb, now, &mut want);
                let (top, anchors) = transport::draw(
                    &mut cx,
                    st,
                    &pb,
                    self.toolbar_alpha,
                    self.video_dock,
                    self.player.as_ref(),
                    self.goto.as_ref().filter(|_| self.goto_time),
                    self.pointer,
                    self.queue.order,
                );
                toast_floor = toast_floor.min(top);
                self.player_anchors = anchors;
            }
            if let Some(p) = &st.pages {
                toast_floor = toast_floor.min(pages::draw(&mut cx, st, p, self.toolbar_alpha));
            }
            if let Some(m) = &st.motion
                && !self.gallery
            {
                motion::draw(&mut cx, m, self.toolbar_alpha);
            }
            glass = true;
            let bar = match &menus {
                Some(m) => m.bar,
                None => toolbar::bar_rect(&mut cx, st),
            };
            let top = toast_floor.min(bar.y);
            self.bottom_bar = Some(Rect::new(bar.x, top, bar.w, bar.y + bar.h - top));
        } else if self.video_full
            && self.play_alpha > 0.0
            && let Some(pb) = &st.playback
        {
            let pb = self.scrubbed(pb, now, &mut want);
            let (top, anchors) = transport::draw(
                &mut cx,
                st,
                &pb,
                self.play_alpha,
                1.0,
                self.player.as_ref(),
                self.goto.as_ref().filter(|_| self.goto_time),
                self.pointer,
                self.queue.order,
            );
            toast_floor = toast_floor.min(top);
            self.player_anchors = anchors;
            glass = true;
            self.bottom_bar = None;
        } else {
            self.bottom_bar = None;
        }
        if self.insight_indicator.is_some() {
            // In the gallery it stays up (the gallery's bars do not fade).
            let alpha = if self.gallery {
                1.0
            } else if bar_on {
                self.toolbar_alpha
            } else {
                0.0
            };
            if alpha > 0.0 {
                self.draw_insight_indicator(&mut cx, alpha);
                glass = true;
            }
        }
        let chip_was = self.work_chip_w;
        self.work_chip_w = 0.0;
        if self.work_chip.is_some() {
            // In the gallery it stays up; in the viewer it comes and goes with the toolbar.
            let alpha = if self.gallery {
                1.0
            } else if bar_on {
                self.toolbar_alpha
            } else {
                0.0
            };
            if alpha > 0.0 {
                if self.draw_work_chip(&mut cx, alpha) {
                    // The spinner turns slowly: 20 frames a second are enough.
                    want(0.05);
                }
                glass = true;
            }
        }
        // What is downloading: in place beside the toolbar (or in the gallery's bottom
        // bar) when there is room, else the small indicator, which stays up with the
        // toolbar hidden.
        let dl_bar = self
            .bottom_bar
            .filter(|_| bar_on)
            .map(|b| (b, self.toolbar_alpha));
        if self.draw_downloads(&mut cx, dl_bar) {
            // A bar with no total moves: 20 frames a second are enough.
            want(0.05);
        }
        if self.downloads_form.is_some() {
            glass = true;
        }
        if self.work_chip_w != chip_was {
            // The gallery's breadcrumbs make room for it (or take it back) on the next frame.
            want(0.0);
        }
        if self.presenting_on.is_some() {
            self.draw_presenting(&mut cx);
            glass = true;
        }
        if self
            .player
            .as_ref()
            .is_some_and(|p| p.cast.playing_on.is_some())
        {
            self.draw_casting(&mut cx);
            glass = true;
        }
        self.draw_toasts(&mut cx, now, toast_floor, &mut want);
        if !self.toasts.is_empty() {
            glass = true;
        }
        // The bar's open menus go over the toasts and the indicator: nothing covers an open
        // menu or takes its clicks.
        if let Some(m) = &menus {
            toolbar::draw_menus(self, &mut cx, st, m);
        }
        if let (Some((m, sel)), Some(v)) = (self.player_menu, &self.player) {
            // The picture adjustments open over More, which offers them.
            let at = match m {
                PlayerMenu::Picture | PlayerMenu::Cast => PlayerMenu::More,
                PlayerMenu::Output => PlayerMenu::Sound,
                m => m,
            };
            match self.player_anchors.iter().find(|a| a.0 == at) {
                Some(&(_, anchor)) => player::menu(&mut cx, m, v, anchor, sel),
                None => self.player_menu = None,
            }
        }
        // The right-click menu goes over everything but a dialog, which closes it.
        if self.modal.is_some() {
            self.menu = None;
        }
        if let Some(t) = self.menu_tick(now) {
            want(t);
        }
        if let Some(m) = &mut self.menu {
            m.stamp(now);
            let (sub_rect, moving) = context_menu::draw(&mut cx, m, now);
            if let Some(s) = m.sub.as_mut() {
                s.rect = sub_rect;
            }
            if moving {
                want(0.0);
            } else if m.closing() {
                self.menu = None;
            }
            glass = true;
        }

        if self.modal_alpha > 0.0 || self.modal.is_some() {
            let a = self.modal_alpha;
            cx.dl.push_opacity(a);
            cx.dl.rect(Rect::new(0.0, 0.0, w, h), 0.0, pal.scrim);
            cx.hit(Rect::new(0.0, 0.0, w, h), Target::Scrim);
            match self.modal {
                Some(Modal::Palette) => palette::draw(self, &mut cx, st),
                Some(Modal::CheatSheet) => cheatsheet::draw(self, &mut cx, st),
                Some(Modal::Settings) => {
                    if live {
                        self.draw_live_bar(&mut cx, st, a);
                    }
                    settings::draw(self, &mut cx, st);
                    if self.settings.reveal > 0 {
                        want(0.0);
                    }
                }
                Some(Modal::Language) => settings::draw_language(self, &mut cx, st),
                Some(Modal::ConfirmTrash) => overlay::draw_confirm(self, &mut cx),
                Some(Modal::Profile) => profile_sheet::draw(self, &mut cx),
                Some(Modal::About) => about::draw(self, &mut cx),
                Some(Modal::HwReport) => report::draw(self, &mut cx),
                Some(Modal::Insights) => insights::draw(self, &mut cx),
                Some(Modal::Work) => work::draw(self, &mut cx),
                Some(Modal::Downloads) => {
                    downloads::draw(self, &mut cx);
                    if self.downloads_moving {
                        want(0.05);
                    }
                }
                Some(Modal::DicomTags) => dicom_tags::draw(self, &mut cx),
                Some(Modal::Queue) => queue_panel::draw(self, &mut cx),
                Some(Modal::Help) => help::draw(self, &mut cx, st),
                Some(Modal::Crash) => diag_sheet::draw_crash(self, &mut cx),
                Some(Modal::Diagnostics) => diag_sheet::draw_diag(self, &mut cx),
                Some(Modal::Studies) => studies::draw(self, &mut cx),
                Some(Modal::Plot) => plot::draw(self, &mut cx),
                Some(Modal::ConfigEditor) => config_editor::draw(self, &mut cx, st, &mut want),
                None => {}
            }
            cx.dl.pop_opacity();
            glass = true;
        }

        // Tooltip.
        let tip_ok = self.tooltip_allowed(&cx.popovers);
        let mut shown_tip = None;
        if let Some(tip) = self
            .hover
            .as_ref()
            .filter(|_| tip_ok)
            .and_then(|t| self.tooltip_of(t, st))
        {
            let wait = durations::TOOLTIP_DELAY as f64 - (now - self.hover_since);
            if wait <= 0.0 {
                if let Some(anchor) = cx.anchor_of(self.hover.as_ref()) {
                    cx.tooltip(&tip, anchor);
                    shown_tip = Some(tip);
                }
            } else {
                want(wait);
            }
        } else if let Some(tip) = self
            .kbd
            .as_ref()
            .filter(|_| tip_ok && self.hover.is_none() && self.modal.is_none())
            .filter(|_| !self.zoom_menu && self.overflow_menu.is_none())
            .and_then(|t| self.tooltip_of(t, st))
        {
            // The control the keyboard is on shows its tooltip too, after the same wait.
            let wait = durations::TOOLTIP_DELAY as f64 - (now - self.kbd_since);
            if wait <= 0.0 {
                if let Some(anchor) = cx.anchor_of(self.kbd.as_ref()) {
                    cx.tooltip(&tip, anchor);
                    shown_tip = Some(tip);
                }
            } else {
                want(wait);
            }
        }
        if specular_log::enabled!(target: specular_log::UI, specular_log::Level::DEBUG) {
            self.log_ui(shown_tip);
        }

        if let Some(d) = self.bars.take_wake() {
            want(d);
        }
        let next_text = cx.text.needs_redraw;
        let fields;
        (self.hits, fields) = cx.finish();
        if self.video_full {
            let mut t: Vec<Target> = Vec::new();
            for (_, h) in &self.hits {
                if matches!(h, Target::Transport(_) | Target::Volume) && !t.contains(h) {
                    t.push(h.clone());
                }
            }
            if !t.is_empty() {
                self.play_stops = t;
            }
        } else {
            self.play_stops.clear();
        }
        self.text_field = fields.focused;
        self.ime_field = fields.ime_only;
        self.ime_area = fields.ime_area;
        self.field_geom = fields.edit;
        // A composition belongs to the field it was typed in: once no field is on screen
        // (the dialog closed while composing), it must not reappear in the next one.
        if self.text_field.is_none() && self.ime_field.is_none() {
            self.preedit.clear();
        }
        self.glass_visible = glass;
        if animating || next_text {
            want(0.0);
        }
        wake
    }

    /// Logs modal, popover and tooltip changes (`SPECULAR_LOG=ui`).
    fn log_ui(&mut self, tip: Option<String>) {
        let owner = self.popover_owner();
        if self.logged.0 != self.modal {
            specular_log::debug!(target: specular_log::UI, "modal {:?} → {:?}", self.logged.0, self.modal);
        }
        if self.logged.1 != owner {
            match &owner {
                Some(o) => specular_log::debug!(target: specular_log::UI, "popover open: {o:?}"),
                None => {
                    specular_log::debug!(target: specular_log::UI, "popover closed: {:?}", self.logged.1)
                }
            }
        }
        if self.logged.2 != tip {
            match &tip {
                Some(t) => {
                    specular_log::debug!(target: specular_log::UI, "tooltip {t:?} for {:?}", self.hover)
                }
                None => specular_log::debug!(target: specular_log::UI, "tooltip hidden"),
            }
        }
        self.logged = (self.modal, owner, tip);
    }

    /// The control whose popover or menu is open, if any.
    fn popover_owner(&self) -> Option<Target> {
        if self.zoom_menu {
            Some(Target::Toolbar(specular_settings::ToolbarItem::ZoomLevel))
        } else if let (Some(Modal::Settings), Some(k)) = (self.modal, self.settings.dropdown) {
            Some(Target::Settings(settings::SettingsTarget::Dropdown(k)))
        } else {
            None
        }
    }

    /// TARGET's tooltip: [`Target::tooltip`], and those that need the chrome's own state.
    fn tooltip_of(&self, target: &Target, st: &ViewerState) -> Option<String> {
        match target {
            Target::Welcome(welcome::WelcomeTarget::Recent(i)) => welcome::recent_tooltip(self, *i),
            Target::Transport(Action::CycleRepeat | Action::ToggleShuffle) | Target::Queue(_) => {
                queue_panel::tooltip(self, target, st)
            }
            _ => target.tooltip(st),
        }
    }

    /// Tooltips never cover a popover or the control that opened it: none over a popover's
    /// area, none for its owner while it is open, and none for the owner after it closes
    /// until the pointer has left the owner and come back (so choosing a zoom preset does not
    /// pop up "Zoom level" at once).
    pub(crate) fn tooltip_allowed(&mut self, popovers: &[Rect]) -> bool {
        if let Some(owner) = self.popover_owner() {
            self.tip_latch = Some(owner);
        } else if self.tip_latch.is_some() && self.hover != self.tip_latch {
            self.tip_latch = None;
        }
        let over_popover = self
            .pointer
            .is_some_and(|p| popovers.iter().any(|r| r.contains(p)));
        !over_popover && (self.tip_latch.is_none() || self.hover != self.tip_latch)
    }

    /// Width taken by the info panel on the given side (`left` = physical left edge).
    fn info_inset_side(&self, rtl: bool, left: bool) -> f32 {
        // The panels sit on the end side: right in LTR, left in RTL.
        if left != rtl {
            return 0.0;
        }
        let info = if self.info_open && !self.video_full {
            info::panel_width(self.text_scale) + metrics::TOOLBAR_MARGIN
        } else {
            0.0
        };
        info + self.panel_inset
    }

    /// A pill at the top centre, `y` down; returns its bottom edge. `swatch` replaces the
    /// alert icon with a colour dot.
    fn draw_badge(&self, cx: &mut Ctx, y: f32, text: &str, swatch: Option<Color>) -> f32 {
        let style = cx.style(metrics::FONT_LABEL);
        // Wraps rather than running off a narrow window; grows with the text.
        let maxw = (cx.w - 32.0 - 46.0).max(80.0);
        let b = cx
            .text
            .layout(text, style, Some(maxw), specular_gfx::Align::Start);
        let bw = b.width + 24.0 + 22.0;
        let bh = (b.height + 10.0).max(30.0).round();
        let r = Rect::new(((cx.w - bw) * 0.5).round(), y, bw, bh);
        cx.dl.glass(r, 15.0, cx.pal.glass, cx.pal.glass_border, 1.0);
        let icon = Rect::new(r.x + 10.0, r.y + (bh - 16.0) * 0.5, 16.0, 16.0);
        let icon = cx.m_in(icon, r);
        match swatch {
            Some(c) => {
                cx.dl.circle(icon.center(), 6.0, cx.pal.glass_border);
                cx.dl.circle(icon.center(), 5.0, c);
            }
            None => specular_gfx::draw_icon(
                cx.dl,
                specular_gfx::Icon::Alert,
                icon,
                cx.pal.text_muted,
                1.5,
                false,
            ),
        }
        let tr = cx.m_in(Rect::new(r.x + 32.0, r.y, b.width, r.h), r);
        cx.block_in(&b, tr, ctx::HAlign::Start, cx.pal.text);
        r.bottom()
    }

    fn draw_message(&self, cx: &mut Ctx, msg: &str, error: bool) {
        let maxw = (cx.w - 80.0).min(560.0);
        let style = cx.style(metrics::FONT_BODY);
        let b = cx
            .text
            .layout(msg, style, Some(maxw), specular_gfx::Align::Center);
        let r = Rect::new(
            (cx.w - b.width) * 0.5 - 20.0,
            cx.h * 0.5 - b.height * 0.5 - 40.0,
            b.width + 40.0,
            b.height + 80.0,
        );
        let icon = Rect::new(r.center().0 - 14.0, r.y + 12.0, 28.0, 28.0);
        let c = if error {
            cx.pal.danger
        } else {
            cx.pal.text_muted
        };
        specular_gfx::draw_icon(cx.dl, specular_gfx::Icon::Alert, icon, c, 1.8, false);
        cx.text.draw(
            cx.dl,
            &b,
            (cx.w - b.width) * 0.5,
            r.y + 52.0,
            cx.pal.text_muted,
        );
    }

    fn draw_toasts(&mut self, cx: &mut Ctx, now: f64, floor: f32, want: &mut impl FnMut(f64)) {
        self.toasts.retain(|t| now - t.born < t.ttl);
        let mut y = floor - 14.0;
        for t in self.toasts.iter().rev() {
            let age = (now - t.born) as f32;
            let left = (t.ttl - (now - t.born)) as f32;
            let a = toast_opacity(age, left, self.reduce_motion);
            want(if a < 1.0 {
                0.0
            } else if self.reduce_motion {
                f64::from(left)
            } else {
                (left - durations::FADE) as f64
            });
            let style = cx.style(metrics::FONT_BODY);
            let b = cx
                .text
                .layout(&t.text, style, Some(cx.w * 0.6), specular_gfx::Align::Start);
            let undo_label = t
                .undo
                .map(|_| t.action.clone().unwrap_or_else(|| t!("toast-undo")));
            let ub = undo_label.as_ref().map(|u| {
                cx.text.layout(
                    u,
                    cx.style(metrics::FONT_LABEL).weight(600.0),
                    None,
                    specular_gfx::Align::Start,
                )
            });
            let uw = ub.as_ref().map_or(0.0, |b| b.width + 28.0);
            let th = b.height.max(18.0) + 20.0;
            let cw = if t.closable { th - 10.0 + 4.0 } else { 0.0 };
            let tw = b.width + 32.0 + uw + cw;
            y -= th;
            let r = Rect::new((cx.w - tw) * 0.5, y, tw, th);
            cx.dl.push_opacity(a);
            cx.dl
                .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 18.0, 6.0);
            cx.dl.glass(
                r,
                metrics::RADIUS_MD,
                cx.pal.glass,
                cx.pal.glass_border,
                1.0,
            );
            let tr = cx.m_in(Rect::new(r.x + 16.0, r.y, b.width, r.h), r);
            cx.block_in(&b, tr, ctx::HAlign::Start, cx.pal.text);
            cx.hit(r, Target::Toast);
            if let (Some(ub), Some(undo)) = (ub, t.undo) {
                let br = cx.m_in(
                    Rect::new(r.right() - uw - cw - 6.0, r.y + 5.0, uw, r.h - 10.0),
                    r,
                );
                let tgt = Target::ToastUndo(t.id, undo);
                if cx.is_hover(&tgt) {
                    cx.dl.rect(br, metrics::RADIUS_SM, cx.pal.surface_hover);
                }
                cx.block_in(&ub, br, ctx::HAlign::Center, cx.pal.accent);
                cx.ring_if(&tgt, br, metrics::RADIUS_SM);
                cx.hit(br, tgt);
            }
            if t.closable {
                let s = r.h - 10.0;
                let cr = cx.m_in(Rect::new(r.right() - s - 6.0, r.y + 5.0, s, s), r);
                let tgt = Target::ToastClose(t.id);
                if cx.is_hover(&tgt) {
                    cx.dl.rect(cr, metrics::RADIUS_SM, cx.pal.surface_hover);
                }
                let ic = cx.u(12.0).min(s);
                specular_gfx::draw_icon(
                    cx.dl,
                    specular_gfx::Icon::Close,
                    cr.centered(ic, ic),
                    cx.pal.text_muted,
                    1.5,
                    false,
                );
                cx.ring_if(&tgt, cr, metrics::RADIUS_SM);
                cx.hit(cr, tgt);
            }
            cx.dl.pop_opacity();
            y -= 8.0;
        }
    }
}

// ----- The dialogs' search fields --------------------------------------------------------

impl Chrome {
    /// `f` on the open dialog's search field (Settings', the language list's, the palette's,
    /// the DICOM tag browser's filter, the config editor's find field while it has the
    /// keyboard), which then updates what it filters, or without a dialog on the toolbar's
    /// go-to field; `None` when no such field is open.
    fn with_field<R>(&mut self, f: impl FnOnce(&mut LineEdit) -> R) -> Option<R> {
        let Some(modal) = self.modal else {
            // The toolbar's go-to field.
            return self.goto.as_mut().map(f);
        };
        let r = match modal {
            Modal::Settings if self.setting_field_focus().is_some() => {
                f(&mut self.settings.field.as_mut()?.edit)
            }
            Modal::Settings if self.settings.dropdown.is_none() => {
                let r = f(self.settings.edit());
                self.settings.edited();
                r
            }
            Modal::Language => {
                let r = f(self.language.edit());
                self.language.edited();
                r
            }
            Modal::Palette => {
                let r = f(self.palette.edit());
                self.palette.edited();
                r
            }
            Modal::Help => {
                let r = f(self.help.edit());
                self.help.edited();
                r
            }
            Modal::DicomTags => {
                let r = f(self.dicom_tags.edit());
                self.dicom_tags.edited();
                r
            }
            Modal::ConfigEditor => {
                let r = f(self.editor.find_edit()?);
                self.editor.find_edited();
                r
            }
            _ => return None,
        };
        Some(r)
    }

    /// The open dialog's search field, when its editing state is current.
    fn field_edit(&self) -> Option<&LineEdit> {
        let Some(modal) = self.modal else {
            return self.goto.as_ref();
        };
        match modal {
            Modal::Settings if self.setting_field_focus().is_some() => {
                self.settings.field.as_ref().map(|d| &d.edit)
            }
            Modal::Settings => self.settings.search_edit.of(&self.settings.search),
            Modal::Language => self.language.query_edit.of(&self.language.query),
            Modal::Palette => self.palette.query_edit.of(&self.palette.query),
            Modal::DicomTags => self.dicom_tags.query_edit.of(&self.dicom_tags.query),
            Modal::Help => self.help.query_edit.of(&self.help.query),
            Modal::ConfigEditor => self.editor.find_field(),
            _ => None,
        }
    }

    /// The selection of the open dialog's search field in characters (anchor, caret), for
    /// AccessKit; `None`: the caret at the end, or no such field.
    pub(crate) fn field_a11y_selection(&self) -> Option<(usize, usize)> {
        self.field_edit().map(LineEdit::a11y_selection)
    }

    /// AccessKit's SetTextSelection on the search field (anchor and focus in characters).
    pub(crate) fn field_set_a11y_selection(&mut self, anchor: usize, focus: usize) -> bool {
        let done = self
            .with_field(|e| e.set_a11y_selection(anchor, focus))
            .is_some();
        if done && self.setting_field_focus().is_none() {
            self.modal_focus = None;
            self.settings.tools.focus = None;
        }
        done
    }

    /// A key for the search field while it has the keyboard: the caret and selection keys,
    /// Backspace and Delete, the clipboard (copies and pastes go out as
    /// [`Command::EditorCopy`] and [`Command::EditorPaste`]) and undo. Up, Down, Enter and
    /// Escape stay the list's, so do Home and End in the palette, the language list and the
    /// DICOM tag browser (with Shift they select). In Settings, Home and End move the caret
    /// while there is a search, and Ctrl+Z and Ctrl+Y undo and redo the field while it has
    /// something to undo or redo, else the settings. In the DICOM tag browser, Left and Right
    /// move the caret while there is a filter, else they collapse and expand the tree (which
    /// has no tree to work while filtering). In the config editor's find field everything
    /// but Up, Down and Enter (the next and previous match) is the field's.
    fn field_key(&mut self, combo: &KeyCombo) -> Option<Response> {
        let modal = self.modal?;
        let on_field = match modal {
            Modal::Settings => self.settings_search_focused(),
            Modal::Language | Modal::Palette | Modal::DicomTags | Modal::Help => {
                self.modal_focus.is_none()
            }
            Modal::ConfigEditor => self.modal_focus.is_none() && self.editor.find_field_on(),
            _ => false,
        };
        if !on_field {
            return None;
        }
        let (key, m) = (combo.key, combo.mods);
        let edit = self.field_edit();
        match key {
            Key::Named(NamedKey::Home | NamedKey::End) if !m.shift => {
                let field = match modal {
                    Modal::Settings => !self.settings.search.is_empty(),
                    Modal::ConfigEditor => true,
                    _ => false,
                };
                if !field {
                    return None;
                }
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight)
                if modal == Modal::DicomTags && self.dicom_tags.query.is_empty() =>
            {
                return None;
            }
            Key::Char(c) if m.ctrl && modal == Modal::Settings => {
                let redo = matches!(c, 'y' | 'Y') || (m.shift && matches!(c, 'z' | 'Z'));
                let undo = matches!(c, 'z' | 'Z') && !m.shift;
                if (undo && !edit.is_some_and(LineEdit::can_undo))
                    || (redo && !edit.is_some_and(LineEdit::can_redo))
                {
                    return None;
                }
            }
            _ => {}
        }
        let rtl = self
            .field_geom
            .as_ref()
            .and_then(|g| g.text.as_ref())
            .map_or(self.last_rtl, |(b, _)| b.rtl);
        self.field_edit_key(key, m, rtl)
    }

    /// A key for the open text field ([`with_field`](Self::with_field): a dialog's search
    /// field or the go-to field) as [`LineEdit::key`] takes it, `rtl` when its text runs right
    /// to left: copies and pastes go out as [`Command::EditorCopy`] and
    /// [`Command::EditorPaste`]. `None` when the field doesn't take it (or none is open).
    fn field_edit_key(&mut self, key: Key, m: specular_input::Mods, rtl: bool) -> Option<Response> {
        let mut clip = PendingClipboard::default();
        let h = self.with_field(|e| e.key(key, m, rtl, &mut clip))?;
        if !h.used() {
            return None;
        }
        let mut r = Response::used();
        if let Some(text) = clip.copied {
            r.commands.push(Command::EditorCopy(text));
        }
        if clip.paste {
            r.commands.push(Command::EditorPaste);
        }
        Some(r)
    }

    /// Text from the clipboard for the open dialog's search field: one line, over the
    /// selection. False when no such field is open.
    fn field_paste(&mut self, s: &str) -> bool {
        self.with_field(|e| e.paste(s)).is_some()
    }

    /// The search field takes the keyboard (a press in it): from a control of the dialog,
    /// and in the config editor from the text.
    fn field_focus(&mut self) {
        match self.modal {
            Some(Modal::Settings) => self.settings.tools.focus = None,
            Some(Modal::ConfigEditor)
                if self.editor.find.is_some() && self.editor.discard.is_none() =>
            {
                self.editor.region = config_editor::Region::Find;
            }
            _ => {}
        }
        self.modal_focus = None;
    }

    /// Where a press at `p` goes in the search field as last drawn (a byte offset), when it
    /// is in it.
    fn field_hit(&self, p: (f32, f32)) -> Option<usize> {
        let g = self.field_geom.as_ref()?;
        (g.rect.contains(p) && self.preedit.is_empty()).then(|| g.byte_at(p.0))
    }

    /// A press at `p` in the search field as last drawn: the caret goes there (a double click
    /// selects a word, a triple click everything; a drag selects; with Shift the selection
    /// grows to there).
    fn field_press(&mut self, p: (f32, f32), now: f64) -> bool {
        let Some(at) = self.field_hit(p) else {
            return false;
        };
        let back = (self.modal_focus.clone(), self.editor.region);
        self.field_focus();
        let shift = self.mods.shift;
        let pressed = self.with_field(|e| e.press(at, p.0, now, shift)).is_some();
        if !pressed {
            (self.modal_focus, self.editor.region) = back;
        }
        pressed
    }

    /// A middle click at `p` in the search field (where there is a primary selection): the
    /// caret goes there and the primary selection is pasted at it ([`Command::PrimaryPaste`],
    /// which comes back through [`Chrome::paste_text`]).
    fn field_middle(&mut self, p: (f32, f32)) -> Option<Response> {
        if !self.primary_selection {
            return None;
        }
        let at = self.field_hit(p)?;
        let back = (self.modal_focus.clone(), self.editor.region);
        self.field_focus();
        let mut clip = PendingClipboard::default();
        if self
            .with_field(|e| e.paste_primary_at(at, &mut clip))
            .is_none()
        {
            (self.modal_focus, self.editor.region) = back;
            return None;
        }
        let mut r = Response::used();
        if clip.paste_primary {
            r.commands.push(Command::PrimaryPaste);
        }
        Some(r)
    }

    /// The pointer moved with the button held after a press in the search field.
    fn field_drag(&mut self, p: (f32, f32)) -> bool {
        if !self.field_edit().is_some_and(LineEdit::dragging) {
            return false;
        }
        // A field of Settings pressed since the last frame: where it was drawn.
        if let Some(g) = self.setting_field_geom().or(self.field_geom.as_ref()) {
            let at = g.byte_at(p.0);
            self.with_field(|e| e.drag_to(at));
        }
        true
    }

    fn field_release(&mut self) {
        if self.field_edit().is_some_and(LineEdit::dragging) {
            self.with_field(LineEdit::release);
        }
    }

    /// Copy-on-select: a new selection in the search field (or the config editor's text)
    /// becomes the primary selection now ([`Command::PrimaryCopy`] on `r`), once it is made
    /// (a drag offers it on release).
    pub(crate) fn field_offer_primary(&mut self, r: &mut Response) {
        self.offer_primary(Offer::Now, r);
    }

    /// Copy-on-select after an input event: keys put the offer off until they are let go
    /// (or rest, [`line_edit::PRIMARY_IDLE_S`]), so a held Shift+Arrow doesn't offer at
    /// every step; a press or release of a button, a key's release or a change of
    /// modifiers offers at once; anything else offers only what is due.
    fn offer_primary_after(&mut self, ev: &Input, now: f64, r: &mut Response) {
        let how = match ev {
            Input::Key { .. } | Input::Text(_) => Offer::Later(now),
            Input::PointerButton { .. } | Input::KeyUp { .. } | Input::Modifiers(_) => Offer::Now,
            Input::PointerMove { .. }
            | Input::PointerLeave
            | Input::Wheel { .. }
            | Input::Preedit(_) => Offer::IfDue(now),
        };
        self.offer_primary(how, r);
    }

    fn offer_primary(&mut self, how: Offer, r: &mut Response) {
        if !self.primary_selection {
            return;
        }
        let mut clip = PendingClipboard::default();
        if self.field_edit().is_some() {
            self.with_field(|e| match how {
                Offer::Now => {
                    e.offer_primary(&mut clip);
                }
                Offer::Later(now) => e.offer_primary_later(now),
                Offer::IfDue(now) => {
                    e.offer_primary_due(now, &mut clip);
                }
            });
        }
        if self.config_editor_open() {
            self.editor.offer_primary(how, &mut clip);
        }
        if let Some(text) = clip.primary {
            r.commands.push(Command::PrimaryCopy(text));
        }
    }

    /// Whether a selection made with the keys waits to be offered as the primary selection
    /// (the application then hands the chrome key releases, [`Input::KeyUp`]).
    pub fn primary_pending(&self) -> bool {
        self.primary_selection
            && (self.field_edit().is_some_and(LineEdit::primary_pending)
                || (self.config_editor_open() && self.editor.primary.pending()))
    }

    /// Offers the keyboard's selection once it is due (into [`Chrome::take_commands`]);
    /// seconds until it is, while one waits.
    pub(crate) fn primary_tick(&mut self, now: f64) -> Option<f64> {
        let mut r = Response::default();
        self.offer_primary(Offer::IfDue(now), &mut r);
        self.pending.extend(r.commands);
        if !self.primary_selection {
            return None;
        }
        let field = self.field_edit().and_then(|e| e.primary_wake(now));
        let editor = self
            .config_editor_open()
            .then(|| self.editor.primary.wake(now))
            .flatten();
        match (field, editor) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// When copy-on-select offers a new selection.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Offer {
    /// Now (a press, a release).
    Now,
    /// After keys (at this time): once they are let go or rest.
    Later(f64),
    /// Only when what keys selected is due (at this time).
    IfDue(f64),
}

#[cfg(test)]
mod dropdown_tests;
#[cfg(test)]
mod tests;

/// The go-to field takes decimal digits in any script, as ASCII digits: at most nine, never a
/// leading zero.
/// The empty window's hint at the cheat sheet and the palette, with their keys; `None`
/// when neither has a key.
pub fn empty_hint(km: &Keymap) -> Option<String> {
    let key = |a: Action| km.primary_key(a).map(|k| k.to_string());
    Some(match (key(Action::CheatSheet), key(Action::Palette)) {
        (Some(c), Some(p)) => t!("empty-hint-keys", cheatsheet = c, palette = p),
        (Some(c), None) => t!("empty-hint-cheatsheet", cheatsheet = c),
        (None, Some(p)) => t!("empty-hint-palette", palette = p),
        (None, None) => return None,
    })
}

/// How long a file opens with nothing on screen before the spinner shows (seconds).
pub const SPINNER_DELAY: f64 = 0.3;
/// Seconds the play controls of a movie's full-screen play mode (and the pointer) stay up
/// after the last move or key while it plays.
pub const PLAY_CONTROLS_IDLE: f64 = 2.0;

/// How long, at most, the seek bar keeps showing where it was let go while the movie gets
/// there (seconds).
const SCRUB_HOLD: f64 = 1.5;
/// How long before the spinner also names the file being opened (seconds).
pub const OPENING_NAME_DELAY: f64 = 1.5;

const GOTO_FILTER: Filter = Filter {
    map: goto_digit,
    valid: goto_valid,
};

fn goto_digit(c: char) -> Option<char> {
    digit_value(c).map(|d| char::from(b'0' + d))
}

fn goto_valid(t: &str) -> bool {
    t.len() <= 9 && !t.starts_with('0')
}

/// The value of a decimal digit in any script (`'7'`, `'٧'`, `'۷'`, `'７'`…).
pub(crate) fn digit_value(c: char) -> Option<u8> {
    if let Some(d) = c.to_digit(10) {
        return Some(d as u8);
    }
    // Zero of each block of ten decimal digits in common use for UI input.
    const ZEROS: [u32; 6] = [0x0660, 0x06F0, 0x0966, 0x09E6, 0x0E50, 0xFF10];
    let u = c as u32;
    ZEROS
        .iter()
        .find(|&&z| (z..z + 10).contains(&u))
        .map(|z| (u - z) as u8)
}

/// A toast's opacity `age` seconds after it appeared with `left` seconds to go: it fades in
/// and out, or with reduce motion is there or not.
pub(crate) fn toast_opacity(age: f32, left: f32, reduce_motion: bool) -> f32 {
    if reduce_motion {
        return if left > 0.0 { 1.0 } else { 0.0 };
    }
    (age / durations::FADE)
        .min(1.0)
        .min(left / durations::FADE)
        .clamp(0.0, 1.0)
}
