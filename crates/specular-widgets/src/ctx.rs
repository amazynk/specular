//! Per-frame drawing context: the draw list, text system, palette, direction and the hit
//! rectangles recorded for the next input event.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Action, Disabled};
use specular_settings::ToolbarItem;
use specular_theme::{Color, Palette, metrics};

use crate::ViewerState;
use crate::line_edit::LineEdit;
use crate::settings::SettingsTarget;

/// How far a dialog button's focus ring sits outside it: the 2 px ring plus a 2 px gap.
pub(crate) const BUTTON_RING_OFFSET: f32 = 4.0;

/// Development aid (debug builds): with `SPECULAR_UI_OVERFLOW=1`, text drawn wider than its
/// box (or, centred, taller) is reported on stderr, for the overflow sweep over long
/// translations and large text scales. Labels are named; bare blocks show their box.
fn overflow_check(s: Option<&str>, b: &TextBlock, r: Rect, centred: bool) {
    #[cfg(debug_assertions)]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*ON.get_or_init(|| std::env::var_os("SPECULAR_UI_OVERFLOW").is_some()) {
            return;
        }
        let wide = b.width > r.w + 0.5;
        let tall = centred && b.lines == 1 && b.height > r.h + 1.0;
        if wide || tall {
            eprintln!(
                "ui-overflow: {:?} {:.0}x{:.0} in box {:.0},{:.0} {:.0}x{:.0}",
                s.unwrap_or("<block>"),
                b.width,
                b.height,
                r.x,
                r.y,
                r.w,
                r.h
            );
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = (s, b, r, centred);
}

/// Something the pointer can hit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Toolbar(ToolbarItem),
    /// The More button at the end of a bar too long for the window (opens the More menu,
    /// whose rows are `Toolbar` targets).
    ToolbarMore,
    /// The chip above the toolbar that turns a profile's tools on ("Medical tools"), shown
    /// while an image of a profile that is off is on screen.
    ProfileChip(specular_input::Capability),
    /// The position readout ("12 / 348"): opens the go-to field.
    Position,
    ToolbarBg,
    /// A spacer or separator of the toolbar, by position in its list: it does nothing, but
    /// it can be dragged to rearrange the bar.
    BarSlot(usize),
    ZoomPreset(Action),
    /// A button of the animation transport bar.
    Transport(Action),
    /// A movie's seek bar (press or drag to move the playhead).
    SeekBar,
    /// The volume slider beside the mute button (press, drag, wheel or arrows).
    Volume,
    /// A player menu's button on the transport (subtitles, sound tracks, chapters).
    PlayerButton(crate::player::PlayerMenu),
    /// A row of a player menu, by position.
    PlayerItem(crate::player::PlayerMenu, usize),
    /// A stepper row of a player menu (a delay, a picture adjustment), by position among
    /// its steppers: −1 less (earlier), +1 more (later), 0 reset.
    PlayerDelay(crate::player::PlayerMenu, usize, i64),
    /// The motion badge of a Motion Photo or Live Photo (plays the clip, or back to the
    /// still).
    Motion,
    /// The "presenting on" control's Pause / Resume button (the show is paused) and its Stop
    /// button (a slideshow, else the viewer on the other display).
    PresentingPause(bool),
    PresentingStop(bool),
    /// The "Playing on" card's "Stop casting".
    CastStop,
    EdgePrev,
    EdgeNext,
    /// The viewer's back button (top leading corner): back to the gallery.
    ViewerBack,
    /// A control of the start screen.
    Welcome(crate::welcome::WelcomeTarget),
    /// A button of the "needs a free decoder" card, by position.
    DecoderCard(u8),
    InfoPanel,
    InfoTab(bool),
    InfoClose,
    /// The info panel title when it is cut short: its tooltip shows the whole file name.
    InfoTitle,
    /// The header of the info panel's "Performance" section (expands and collapses it).
    InfoPerf,
    /// "Show on map" under the info panel's location: the gallery's map, on this photo.
    InfoShowOnMap,
    /// "Edit metadata…" at the foot of the info panel's summary: the metadata editor.
    InfoEditMetadata,
    /// A row of the "All fields" list (group, field index in [`FieldGroup::fields`]): clicking
    /// it copies the whole value (not the shortened text shown).
    ///
    /// [`FieldGroup::fields`]: specular_metadata::FieldGroup::fields
    InfoCopy(usize, usize),
    /// A row of the Summary (group, item in [`SummaryGroup::items`]): clicking it copies the
    /// value shown.
    ///
    /// [`SummaryGroup::items`]: specular_metadata::SummaryGroup::items
    InfoSummary(usize, usize),
    Toast,
    ToastUndo(u64, u64),
    /// The close (×) button of a hint ([`crate::Chrome::hint`]).
    ToastClose(u64),
    Scrim,
    ModalBody,
    ModalClose,
    /// A tab of the cheat sheet: the keys of one mode.
    CheatTab(crate::cheatsheet::SheetMode),
    /// A cheat sheet line cut short (tab, group, line): its tooltip is the whole line.
    CheatLine(crate::cheatsheet::SheetMode, u16, u16),
    PaletteRow(usize),
    LanguageRow(usize),
    /// A row of the DICOM tag browser, by position in the rows listed.
    TagRow(usize),
    /// The jump button of a tag browser row (a report item that shows an image).
    TagJump(usize),
    /// The page of the plot sheet (calipers are drawn on it).
    PlotArea,
    /// The tag browser's "Show patient data" switch.
    TagPatient,
    /// A row of the studies browser, by index in [`crate::Studies::rows`].
    StudyRow(usize),
    /// The studies browser's "Show patient data" switch.
    StudiesPatient,
    /// The studies browser's Cancel, while the medium is read.
    StudiesCancel,
    /// The studies browser's "Send to PACS…", once the medium is read.
    StudiesSend,
    Settings(SettingsTarget),
    ConfirmOk,
    ConfirmCancel,
    /// The other choice of a confirmation that offers two ([`crate::Chrome::confirm_choice`]).
    ConfirmAlt,
    /// The third choice of a confirmation that offers three ([`crate::Chrome::confirm_choice3`]).
    ConfirmAlt2,
    /// A check box of the first-run question.
    ProfileCheck(specular_input::Capability),
    /// Continue, under the first-run question.
    ProfileDone,
    /// A choice of what a start without a file shows, on the first-run sheet.
    ProfileStart(specular_settings::OnStart),
    /// Skip, on the first-run sheet: keeps every default.
    ProfileSkip,
    /// The Copy button of the hardware report.
    ReportCopy,
    /// The performance insights indicator ("Why is this slow?") and its close button.
    Insight,
    InsightHide,
    /// In the insights panel: the button of a "what you can do now" item (its index), "Don't
    /// show again", the global switch, and "Show hidden insights again".
    InsightAction(usize),
    InsightDismiss,
    InsightSwitch,
    InsightRestore,
    /// The background-work indicator (it opens the panel).
    Work,
    /// In the background-work panel: a job's progress bar (for screen readers), its Pause or
    /// Resume button and its settings button, by row.
    WorkBar(usize),
    WorkPause(usize),
    WorkSettings(usize),
    /// What is downloading, in place or as the small indicator (it opens the panel).
    Downloads,
    /// In the "Downloads" panel: a download's progress bar (for screen readers) and its
    /// Stop button, by row.
    DownloadBar(usize),
    DownloadStop(usize),
    /// A control of the play queue panel.
    Queue(crate::queue_panel::QueueTarget),
    /// The scrollbar of a scrolling area (its whole hit strip).
    Scrollbar(crate::scrollbar::Area),
    /// A part of the settings.toml editor.
    Editor(crate::config_editor::EditorTarget),
    /// "Customise toolbar…" in the toolbar's right-click menu.
    ToolbarMenu,
    /// A row of the application's right-click menu (`context_menu.rs`), a row of its
    /// submenu, and the background of either.
    MenuItem(usize),
    MenuSubItem(usize),
    MenuBg,
    /// A row of the help panel: a result, a page of the contents, a link of the page.
    HelpRow(usize),
    /// The help panel's Back and Contents.
    HelpBack,
    HelpContents,
    /// The crash prompt's buttons.
    CrashView,
    CrashCopy,
    CrashSave,
    CrashDismiss,
    /// The diagnostics dialog: a file of the bundle (shows it), "Include file names", Save…
    /// and Cancel.
    DiagEntry(usize),
    DiagNames,
    DiagSave,
    DiagCancel,
    /// About's Help and Save diagnostics bundle….
    AboutHelp,
    AboutDiagnostics,
    AboutRenderer,
    /// About's Discord button (opens the Specular Discord invite).
    AboutDiscord,
}

impl Target {
    /// Targets that let pointer events through to the image. None do at the moment; the
    /// hook keeps decorative regions possible.
    pub(crate) fn passive(&self) -> bool {
        false
    }

    /// The action this control runs, when it is an action button (the enabled rule and the
    /// shortcut shown in its tooltip come from it).
    pub(crate) fn action(&self, st: &ViewerState) -> Option<Action> {
        match self {
            Target::Toolbar(item) => crate::toolbar::item_action(*item, st),
            Target::ZoomPreset(a) | Target::Transport(a) => Some(*a),
            Target::Position => Some(Action::GoToImage),
            Target::EdgePrev => Some(Action::PrevImage),
            Target::EdgeNext => Some(Action::NextImage),
            Target::Welcome(crate::welcome::WelcomeTarget::Open) => Some(Action::Open),
            _ => None,
        }
    }

    /// Why this control is unavailable (`Action::disabled` of its action), if it is.
    pub(crate) fn disabled(&self, st: &ViewerState) -> Option<Disabled> {
        self.action(st).and_then(|a| a.disabled(&st.actions))
    }

    /// Tooltip text (label and shortcut, or why it is unavailable) for buttons that have one.
    pub(crate) fn tooltip(&self, st: &ViewerState) -> Option<String> {
        let (label, action) = match self {
            Target::InfoTitle => return st.file_name.map(str::to_owned),
            Target::CheatLine(m, g, l) => {
                return crate::cheatsheet::line_text(*m, st.keymap, *g as usize, *l as usize);
            }
            Target::InfoCopy(..) | Target::InfoSummary(..) => (t!("app-info-copy-value"), None),
            Target::Toolbar(item) => crate::toolbar::item_label(*item, st),
            Target::ToolbarMore => (t!("toolbar-more"), None),
            Target::AboutDiscord => (t!("toolbar-discord"), Some(Action::JoinDiscord)),
            Target::ProfileChip(c) => (crate::palette::turn_on(*c, false), None),
            Target::Position => (t!("toolbar-go-to"), Some(Action::GoToImage)),
            Target::Transport(a) => (crate::transport::label(*a, st), Some(*a)),
            // The time under the pointer shows instead (player.rs).
            Target::SeekBar => return None,
            // Its readout shows instead, at once (volume.rs).
            Target::Volume => return None,
            Target::PlayerButton(m) => (crate::player::label(*m), None),
            // The shortcut shown is the one that starts it (Play/Pause pauses a playing clip).
            Target::Motion => {
                let m = st.motion.as_ref()?;
                let key = (!m.playing).then_some(Action::PlayPause);
                (crate::motion::label(m), key)
            }
            Target::PresentingPause(paused) => (crate::presenting::pause_label(*paused), None),
            Target::PresentingStop(show) => (crate::presenting::stop_label(*show), None),
            Target::CastStop => (t!("cast-stop"), None),
            Target::EdgePrev => (t!("toolbar-prev"), Some(Action::PrevImage)),
            Target::EdgeNext => (t!("toolbar-next"), Some(Action::NextImage)),
            Target::ViewerBack => (t!("toolbar-back-gallery"), Some(Action::Back)),
            Target::InfoClose | Target::ModalClose => (t!("app-close"), Some(Action::Back)),
            Target::Settings(SettingsTarget::Reset(_)) => (t!("settings-reset"), None),
            Target::Settings(SettingsTarget::ToolUp(_)) => (t!("settings-toolbar-move-up"), None),
            Target::Settings(SettingsTarget::ToolDown(_)) => {
                (t!("settings-toolbar-move-down"), None)
            }
            Target::Settings(SettingsTarget::ToolMenu(i)) => {
                let list = crate::toolbar_edit::current(st.store);
                let (it, _) = crate::toolbar_edit::row_item(&list, *i)?;
                (
                    t!(
                        "settings-toolbar-row-menu",
                        item = crate::toolbar::item_name(it)
                    ),
                    None,
                )
            }
            Target::InsightHide => (t!("open-report-insights-hide"), None),
            Target::Work => (t!("work-show"), None),
            Target::Downloads => (t!("net-show"), None),
            _ => return None,
        };
        if let Some(why) = self.disabled(st) {
            let reason = specular_i18n::I18n::global().tr(why.message_id());
            return Some(t!("tooltip-unavailable", label = label, reason = reason));
        }
        let key = action.and_then(|a| st.keymap.primary_key(a));
        Some(match key {
            Some(k) => t!(
                "tooltip-with-shortcut",
                label = label,
                shortcut = k.to_string()
            ),
            None => label,
        })
    }
}

/// Draws the chrome's tooltip with `s` near `anchor`, inside `bounds` (logical px, usually
/// the window) at text scale `ts`: centred on the anchor, above it when the anchor is in the
/// lower half of `bounds` (so near the bottom edge it flips up), below it otherwise, and
/// kept 6 px inside the sides. Returns where it went. Also used by mode chrome drawn
/// outside the [`Ctx`] (cull mode's strip), so every tooltip looks the same.
pub fn tooltip(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    s: &str,
    anchor: Rect,
    bounds: Rect,
    ts: f32,
) -> Rect {
    let style = TextStyle::new((metrics::FONT_SMALL + 0.5) * ts).break_anywhere();
    let max_w = (320.0 * ts.max(1.0)).round().min(bounds.w - 16.0);
    let b = text.layout(s, style, Some(max_w), Align::Start);
    let w = b.width + 16.0;
    let h = b.height + 10.0;
    let above = anchor.center().1 > bounds.y + bounds.h * 0.5;
    let y = if above {
        anchor.y - h - 8.0
    } else {
        anchor.bottom() + 8.0
    };
    let y = y.clamp(bounds.y, (bounds.bottom() - h).max(bounds.y));
    let x = (anchor.center().0 - w * 0.5).clamp(
        bounds.x + 6.0,
        (bounds.right() - w - 6.0).max(bounds.x + 6.0),
    );
    let r = Rect::new(x, y, w, h);
    dl.shadow(r, metrics::RADIUS_SM, pal.shadow, 10.0, 3.0);
    dl.rect(r, metrics::RADIUS_SM, pal.tooltip_bg);
    text.draw(dl, &b, r.x + 8.0, r.y + 5.0, pal.tooltip_text);
    r
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HAlign {
    Start,
    Center,
    End,
}

/// The text fields a frame drew (logical pixels).
#[derive(Clone, Debug, Default)]
pub(crate) struct Fields {
    /// The focused field (caret, accent border; the accessible focus).
    pub focused: Option<Rect>,
    /// A field drawn unfocused that still takes what is typed and composed (a control of
    /// its dialog has the keyboard): the input method stays on for it.
    pub ime_only: Option<Rect>,
    /// Where the input method's candidate window goes: the composition, or the caret.
    pub ime_area: Option<Rect>,
    /// The field with an editing state ([`LineEdit`]) as drawn: where a click goes.
    pub edit: Option<FieldGeom>,
}

/// A text field with an editing state as drawn, focused or not: where a click in it goes.
#[derive(Clone, Debug)]
pub(crate) struct FieldGeom {
    pub rect: Rect,
    /// Its text as drawn (composition included) and the x it starts at; `None` while empty.
    pub text: Option<(Arc<TextBlock>, f32)>,
    /// What is drawn when it is not the field's text: a number of the go-to field in the
    /// language's digits and grouping, whose digits are the text's ASCII digits in order.
    pub digits: Option<String>,
}

impl FieldGeom {
    /// The byte of the text under `x` (0 in an empty field).
    pub fn byte_at(&self, x: f32) -> usize {
        let Some((b, x0)) = &self.text else { return 0 };
        let at = b.hit_byte(x - x0);
        match &self.digits {
            // The digits drawn before it (one byte each in the text).
            Some(shown) => digits_before(shown, at),
            None => at,
        }
    }
}

/// How many digits `shown` has before byte `at`.
pub(crate) fn digits_before(shown: &str, at: usize) -> usize {
    shown
        .char_indices()
        .take_while(|&(i, _)| i < at)
        .filter(|&(_, c)| c.is_numeric())
        .count()
}

/// Where each digit of `shown` starts (then where the text ends), and where each ends.
pub(crate) fn digit_bytes(shown: &str) -> (Vec<usize>, Vec<usize>) {
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for (i, c) in shown.char_indices().filter(|&(_, c)| c.is_numeric()) {
        starts.push(i);
        ends.push(i + c.len_utf8());
    }
    starts.push(shown.len());
    (starts, ends)
}

pub struct Ctx<'a> {
    pub(crate) dl: &'a mut DrawList,
    pub(crate) text: &'a mut Text,
    pub(crate) pal: &'a Palette,
    pub(crate) rtl: bool,
    pub(crate) w: f32,
    pub(crate) h: f32,
    ts: f32,
    #[allow(dead_code)]
    pub(crate) now: f64,
    hover: Option<Target>,
    pressed: Option<Target>,
    hits: Vec<(Rect, Target)>,
    /// IME composition shown after the focused field's text.
    pub(crate) preedit: String,
    /// The focused text field drawn this frame (its accessibility bounds).
    pub(crate) text_field: Option<Rect>,
    /// A text field drawn unfocused that still takes what is typed and composed: the input
    /// method stays on for it (`field` with `ime`).
    pub(crate) ime_field: Option<Rect>,
    /// Where the IME candidate window goes: the composition, or the caret when nothing is
    /// being composed (a line tall, inside the focused field).
    pub(crate) ime_area: Option<Rect>,
    /// The field with an editing state as drawn (composition included).
    pub(crate) field_geom: Option<FieldGeom>,
    /// Popovers and menus drawn this frame; no tooltip shows over them.
    pub(crate) popovers: Vec<Rect>,
    /// The control with the keyboard focus: it gets a focus ring.
    pub(crate) focus: Option<Target>,
}

impl<'a> Ctx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        dl: &'a mut DrawList,
        text: &'a mut Text,
        pal: &'a Palette,
        rtl: bool,
        w: f32,
        h: f32,
        ts: f32,
        now: f64,
        hover: Option<Target>,
        pressed: Option<Target>,
    ) -> Ctx<'a> {
        Ctx {
            dl,
            text,
            pal,
            rtl,
            w,
            h,
            ts,
            now,
            hover,
            pressed,
            hits: Vec::new(),
            preedit: String::new(),
            text_field: None,
            ime_field: None,
            ime_area: None,
            field_geom: None,
            popovers: Vec::new(),
            focus: None,
        }
    }

    /// What the frame recorded: the hit rectangles and the [`Fields`].
    pub(crate) fn finish(self) -> (Vec<(Rect, Target)>, Fields) {
        (
            self.hits,
            Fields {
                focused: self.text_field,
                ime_only: self.ime_field,
                ime_area: self.ime_area,
                edit: self.field_geom,
            },
        )
    }

    pub(crate) fn style(&self, size: f32) -> TextStyle {
        TextStyle::new(size * self.ts)
    }

    /// How much boxes sized for text grow with the text scale: as much as the text, but never
    /// below the standard size (smaller text keeps the pointer targets).
    pub(crate) fn grow(&self) -> f32 {
        self.ts.max(1.0)
    }

    /// `px` (a height, padding or icon size laid out around text at the standard size) at the
    /// current text scale, rounded to whole logical pixels.
    pub(crate) fn u(&self, px: f32) -> f32 {
        (px * self.grow()).round()
    }

    pub(crate) fn hit(&mut self, r: Rect, t: Target) {
        self.hits.push((r, t));
    }

    /// How many hit rectangles have been recorded so far this frame.
    pub(crate) fn hits_len(&self) -> usize {
        self.hits.len()
    }

    /// Forgets the hit rectangles recorded after the first `n` (something drawn only to be
    /// seen, not used).
    pub(crate) fn truncate_hits(&mut self, n: usize) {
        self.hits.truncate(n);
    }

    pub(crate) fn is_hover(&self, t: &Target) -> bool {
        self.hover.as_ref() == Some(t)
    }

    pub(crate) fn is_pressed(&self, t: &Target) -> bool {
        self.pressed.as_ref() == Some(t) && self.is_hover(t)
    }

    pub(crate) fn is_focused(&self, t: &Target) -> bool {
        self.focus.as_ref() == Some(t)
    }

    /// The keyboard focus ring: a 2 px outline just inside `r`, with corners of `radius`.
    pub(crate) fn focus_ring(&mut self, r: Rect, radius: f32) {
        self.dl
            .rect_border(r, radius, Color::TRANSPARENT, self.pal.focus_ring, 2.0);
    }

    /// The keyboard focus ring of a dialog's push button `r` (corner radius `radius`): 2 px,
    /// set off from the button by a 2 px gap of the sheet, so it reads on a filled (default)
    /// button too and never as the button's own edge. Its contrast is the ring against the
    /// sheet (≥ 3:1 in both themes, `tests::keyboard_focus::focus_ring_contrast`).
    pub(crate) fn button_ring(&mut self, r: Rect, radius: f32) {
        self.focus_ring(r.inset(-BUTTON_RING_OFFSET), radius + BUTTON_RING_OFFSET);
    }

    /// A dialog's Close (×) button at `r`: an icon button whose keyboard focus is the push
    /// buttons' ring, set off from it ([`button_ring`](Self::button_ring)).
    pub(crate) fn close_button(&mut self, r: Rect) {
        let t = Target::ModalClose;
        let focused = self.is_focused(&t);
        // Drawn without the icon buttons' inner ring: this one has the offset ring.
        let focus = self.focus.take();
        self.icon_button(r, Icon::Close, t, self.pal.text_muted, false, false);
        self.focus = focus;
        if focused {
            self.button_ring(r, metrics::RADIUS_SM + 2.0);
        }
    }

    /// The focus ring around `r` when `t` has the keyboard focus.
    pub(crate) fn ring_if(&mut self, t: &Target, r: Rect, radius: f32) {
        if self.is_focused(t) {
            self.focus_ring(r, radius);
        }
    }

    pub(crate) fn anchor_of(&self, t: Option<&Target>) -> Option<Rect> {
        let t = t?;
        self.hits
            .iter()
            .rev()
            .find(|(_, h)| h == t)
            .map(|(r, _)| *r)
    }

    /// Mirrors `r` horizontally inside the window when the layout is right-to-left.
    pub(crate) fn m(&self, r: Rect) -> Rect {
        if self.rtl { r.mirror_x(self.w) } else { r }
    }

    /// Mirrors `r` inside `container` when the layout is right-to-left.
    pub(crate) fn m_in(&self, r: Rect, container: Rect) -> Rect {
        if self.rtl {
            Rect {
                x: container.x + (container.right() - r.right()),
                ..r
            }
        } else {
            r
        }
    }

    pub(crate) fn layout(
        &mut self,
        s: &str,
        style: TextStyle,
        max_w: Option<f32>,
    ) -> Arc<TextBlock> {
        self.text.layout(s, style, max_w, Align::Start)
    }

    /// Draws `b` inside `r`, vertically centred. `Start`/`End` follow the layout direction.
    pub(crate) fn block_in(&mut self, b: &TextBlock, r: Rect, align: HAlign, color: Color) {
        overflow_check(None, b, r, true);
        self.place_in(b, r, align, color);
    }

    fn place_in(&mut self, b: &TextBlock, r: Rect, align: HAlign, color: Color) {
        let x = match (align, self.rtl) {
            (HAlign::Center, _) => r.x + (r.w - b.width) * 0.5,
            (HAlign::Start, false) | (HAlign::End, true) => r.x,
            (HAlign::Start, true) | (HAlign::End, false) => r.right() - b.width,
        };
        let y = r.y + (r.h - b.height) * 0.5;
        self.text.draw(self.dl, b, x, y, color);
    }

    /// Draws `b` at the top of `r`, aligned to the start or end side.
    pub(crate) fn block_top(&mut self, b: &TextBlock, r: Rect, align: HAlign, color: Color) {
        overflow_check(None, b, r, false);
        let x = match (align, self.rtl) {
            (HAlign::Center, _) => r.x + (r.w - b.width) * 0.5,
            (HAlign::Start, false) | (HAlign::End, true) => r.x,
            (HAlign::Start, true) | (HAlign::End, false) => r.right() - b.width,
        };
        self.text.draw(self.dl, b, x, r.y, color);
    }

    /// A name or label for a button or menu row, isolated for the UI's direction so its
    /// words keep their order (see [`specular_i18n::isolate_label`]).
    pub(crate) fn name(&self, s: &str) -> String {
        let dir = if self.rtl {
            specular_i18n::Direction::Rtl
        } else {
            specular_i18n::Direction::Ltr
        };
        specular_i18n::isolate_label(s, dir)
    }

    /// One line of text in `r`.
    pub(crate) fn label(
        &mut self,
        s: &str,
        style: TextStyle,
        r: Rect,
        align: HAlign,
        color: Color,
    ) -> Arc<TextBlock> {
        let b = self.layout(s, style, None);
        overflow_check(Some(s), &b, r, true);
        self.place_in(&b, r, align, color);
        b
    }

    /// One line of text in `r`, shortened with an ellipsis when it does not fit (`middle`
    /// keeps both ends, for file names).
    pub(crate) fn label_fit(
        &mut self,
        s: &str,
        style: TextStyle,
        r: Rect,
        align: HAlign,
        color: Color,
        middle: bool,
    ) -> Arc<TextBlock> {
        let s = self.ellipsize(s, style, r.w, middle);
        self.label(&s, style, r, align, color)
    }

    /// The longest form of `s` (whole, or cut with "…") that fits in `max_w`.
    pub(crate) fn ellipsize(
        &mut self,
        s: &str,
        style: TextStyle,
        max_w: f32,
        middle: bool,
    ) -> String {
        let text = &mut *self.text;
        specular_i18n::ellipsize_to_fit(s, middle, |t| text.measure(t, style) <= max_w)
    }

    /// A square icon button with hover and pressed states.
    pub(crate) fn icon_button(
        &mut self,
        r: Rect,
        icon: Icon,
        t: Target,
        color: Color,
        mirror: bool,
        active: bool,
    ) {
        self.icon_button_e(r, icon, t, color, mirror, active, true);
    }

    /// An icon button that may be disabled: dimmed, with no hover or pressed effect. It still
    /// records its hit rectangle so its tooltip can say why it is unavailable.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn icon_button_e(
        &mut self,
        r: Rect,
        icon: Icon,
        t: Target,
        color: Color,
        mirror: bool,
        active: bool,
        enabled: bool,
    ) {
        let (color, active) = if enabled {
            (color, active)
        } else {
            (self.pal.text_faint.with_alpha(0.55), false)
        };
        let bg = if !enabled {
            None
        } else if self.is_pressed(&t) {
            Some(self.pal.surface_pressed)
        } else if self.is_hover(&t) {
            Some(self.pal.surface_hover)
        } else if active {
            Some(self.pal.accent_soft)
        } else {
            None
        };
        if let Some(bg) = bg {
            self.dl.rect(r, metrics::RADIUS_SM + 2.0, bg);
        }
        let ic = r.centered(metrics::ICON, metrics::ICON);
        draw_icon(
            self.dl,
            icon,
            ic,
            if active { self.pal.accent } else { color },
            metrics::ICON_STROKE,
            mirror,
        );
        self.ring_if(&t, r, metrics::RADIUS_SM + 2.0);
        self.hit(r, t);
    }

    /// A text field look (rounded box, icon, text or placeholder, caret, selection).
    ///
    /// The placeholder shows only while the field is empty and nothing is being composed; it
    /// is shortened with "…" and clipped to the field, and drawn in `text_muted` (WCAG AA on
    /// `surface_raised`). The caret sits just before where the first character will go, so it
    /// never covers the placeholder, and it is as tall as the text at any text scale. Callers
    /// give the field a height of `u(…)` so it grows with the text scale.
    ///
    /// `edit` puts the caret and the selection where the field's editing state has them
    /// (when its text is `value`; else the caret is at the end), and records the field as
    /// drawn for the pointer (`Fields::edit`). A composition is shown at the caret. A text
    /// too long for the field scrolls to keep the caret in view.
    ///
    /// `ime` keeps the input method while the field is drawn unfocused (`ime` without
    /// `focused`): a control of the dialog has the keyboard, but what is typed, composed
    /// text included, still goes to the field. No caret is drawn
    /// and the field is not the focused one (`text_field`); it is recorded as `ime_field`
    /// with the area at its caret's place, so the input method stays on and its candidate
    /// window opens where the text will go.
    pub(crate) fn field(
        &mut self,
        r: Rect,
        value: &str,
        edit: Option<&LineEdit>,
        placeholder: &str,
        focused: bool,
        ime: bool,
    ) {
        self.field_with(
            r,
            value,
            edit,
            placeholder,
            focused,
            ime,
            Some(Icon::Search),
        );
    }

    /// [`field`](Self::field) with `icon` at its start (`None`: none, a value's field rather
    /// than a search). Returns where its text was drawn, `edit` or not (Settings' fields
    /// place the caret of the one a press focuses).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn field_with(
        &mut self,
        r: Rect,
        value: &str,
        edit: Option<&LineEdit>,
        placeholder: &str,
        focused: bool,
        ime: bool,
        icon: Option<Icon>,
    ) -> FieldGeom {
        let ime = ime || focused;
        self.dl.rect_border(
            r,
            metrics::RADIUS_SM + 1.0,
            self.pal.surface_raised,
            if focused {
                self.pal.focus_ring.with_alpha(0.7)
            } else {
                self.pal.separator
            },
            1.0,
        );
        // The icon and the padding grow with the text (callers size the field with `u`).
        let lead = match icon {
            Some(icon) => {
                let size = self.u(16.0);
                let ic = self
                    .m_in(Rect::new(r.x + self.u(8.0), r.y, size + 2.0, r.h), r)
                    .centered(size, size);
                draw_icon(self.dl, icon, ic, self.pal.text_faint, 1.5, false);
                self.u(32.0)
            }
            None => self.u(8.0),
        };
        let trail = self.u(8.0);
        let tr = self.m_in(Rect::new(r.x + lead, r.y, r.w - lead - trail, r.h), r);
        let style = self.style(metrics::FONT_BODY);
        // The caret and the composition underline follow the text's line box (centred, as
        // `block_in` draws it), not fixed offsets from the field's edges.
        let line_h = self.layout("Ag", style, None).height.min(r.h - 4.0);
        let line = Rect::new(tr.x, r.y + (r.h - line_h) * 0.5, tr.w, line_h);
        let caret_r = |x: f32| Rect::new(x, line.y + 1.0, 1.5, (line.h - 2.0).max(1.0));
        let (caret, sel) = match edit.and_then(|e| e.of(value)) {
            Some(e) => (e.caret(), e.selection()),
            None => (value.len(), value.len()..value.len()),
        };
        // The composition (empty when nothing is being composed) sits at the caret.
        let preedit = if focused {
            self.preedit.clone()
        } else {
            String::new()
        };
        let pre = preedit.len();
        if focused {
            self.text_field = Some(r);
        } else if ime {
            self.ime_field = Some(r);
        }
        if value.is_empty() && pre == 0 {
            self.dl.push_clip(tr);
            self.label_fit(
                placeholder,
                style,
                tr,
                HAlign::Start,
                self.pal.text_muted,
                false,
            );
            self.dl.pop_clip();
            if ime {
                // Before the first character: outside the placeholder's box.
                let x = if self.rtl {
                    tr.right() + 0.5
                } else {
                    tr.x - 2.0
                };
                if focused {
                    self.dl.rect(caret_r(x), 0.0, self.pal.accent);
                }
                self.ime_area = Some(caret_r(x));
            }
            let g = FieldGeom {
                rect: r,
                text: None,
                digits: None,
            };
            if edit.is_some() {
                self.field_geom = Some(g.clone());
            }
            return g;
        }
        let shown = format!("{}{preedit}{}", &value[..caret], &value[caret..]);
        let b = self.layout(&shown, style, None);
        let cx = b.caret_x(caret + pre);
        // At the start side while it fits; scrolled to keep the caret in view when not.
        let x0 = crate::line_edit::field_x0(&b, cx, tr, self.rtl);
        self.dl.push_clip(tr.inset_xy(-2.0, 0.0));
        if focused && pre == 0 {
            for (a, z) in b.range_spans(sel) {
                let s = Rect::new(x0 + a, line.y, z - a, line.h);
                self.dl.rect(s, 0.0, self.pal.selection);
            }
        }
        self.text
            .draw(self.dl, &b, x0, r.y + (r.h - b.height) * 0.5, self.pal.text);
        // The composition is underlined.
        let (pa, pb) = (x0 + b.caret_x(caret), x0 + cx);
        if pre > 0 {
            self.dl.rect(
                Rect::new(pa.min(pb), line.bottom() - 1.0, (pa - pb).abs(), 1.0),
                0.0,
                self.pal.text_muted,
            );
        }
        if ime {
            let c = caret_r((x0 + cx - 0.75).round().clamp(tr.x - 1.0, tr.right() - 0.5));
            if focused {
                self.dl.rect(c, 0.0, self.pal.accent);
            }
            // The candidate window opens under the composition and the caret.
            let (x0, x1) = (pa.min(pb).min(c.x), pa.max(pb).max(c.right()));
            let (x0, x1) = (x0.max(tr.x - 2.0), x1.min(tr.right() + 2.0));
            self.ime_area = Some(Rect::new(x0, c.y, (x1 - x0).max(1.0), c.h));
        }
        self.dl.pop_clip();
        let g = FieldGeom {
            rect: r,
            text: Some((b, x0)),
            digits: None,
        };
        if edit.is_some() {
            self.field_geom = Some(g.clone());
        }
        g
    }

    /// A keycap ("Ctrl+K") drawn as a small rounded box on the end side of `row`, `pad`
    /// from its edge; returns its width.
    pub(crate) fn keycap_end(&mut self, s: &str, row: Rect, pad: f32) -> f32 {
        let style = self.style(metrics::FONT_SMALL).weight(500.0);
        let b = self.layout(s, style, None);
        let w = b.width + 12.0;
        let x = if self.rtl {
            row.x + pad
        } else {
            row.right() - pad - w
        };
        let h = self.u(20.0);
        let r = Rect::new(x, row.center().1 - h * 0.5, w, h);
        // A firmer edge than a separator's: on a light card a white key would vanish.
        let edge = self.pal.separator.with_alpha(self.pal.separator.a * 2.0);
        self.dl
            .rect_border(r, 4.0, self.pal.surface_raised, edge, 1.0);
        self.text.draw(
            self.dl,
            &b,
            r.x + 6.0,
            r.y + (r.h - b.height) * 0.5,
            self.pal.text_muted,
        );
        w
    }

    /// Draws a tooltip near `anchor`.
    pub(crate) fn tooltip(&mut self, s: &str, anchor: Rect) {
        let bounds = Rect::new(0.0, 0.0, self.w, self.h);
        tooltip(self.dl, self.text, self.pal, s, anchor, bounds, self.ts);
    }

    /// A small downward chevron (dropdowns).
    pub(crate) fn chevron_down(&mut self, c: (f32, f32), color: Color) {
        self.dl
            .line((c.0 - 4.0, c.1 - 2.0), (c.0, c.1 + 2.0), 1.5, color);
        self.dl
            .line((c.0, c.1 + 2.0), (c.0 + 4.0, c.1 - 2.0), 1.5, color);
    }

    /// A modal sheet: shadow, opaque surface, a hit region that swallows clicks.
    pub(crate) fn sheet(&mut self, r: Rect) {
        self.dl
            .shadow(r, metrics::RADIUS_LG, self.pal.shadow, 40.0, 14.0);
        self.dl.rect_border(
            r,
            metrics::RADIUS_LG,
            self.pal.surface,
            self.pal.glass_border,
            1.0,
        );
        self.hit(r, Target::ModalBody);
    }
}
