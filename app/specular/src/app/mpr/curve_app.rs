//! Curved MPR, the dental layout and the segments' export in the running app: their
//! actions, controls, pointer and wheel, dialogs and background work ([`super::curved`],
//! [`super::segments`]).

use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::{Receiver, TryRecvError, bounded};
use specular_i18n::t;
use specular_input::Action;
use specular_render_volume::arch;
use specular_widgets::Button;

use super::super::App;
use super::curved::{self, Curved, Dental, POINT_HIT, UNDO_KEY};
use super::draw::{Control, contains};
use super::load::lock;
use super::segments::{self, DETAILS};
use super::views::V3;
use super::{Drag, Session};

/// Curved MPR's dialogs and background work.
#[derive(Default)]
pub(crate) struct Io {
    /// The sidecar written (its path), or why not.
    save_rx: Option<Receiver<Result<PathBuf, String>>>,
    /// A sidecar read (its text), or why not.
    load_rx: Option<Receiver<Result<String, String>>>,
    /// The folder chosen for the segments, then the export's result.
    seg_dir_rx: Option<Receiver<PathBuf>>,
    seg_done: Option<Receiver<Result<segments::Report, String>>>,
    /// The triangle count per segment ([`DETAILS`]).
    pub detail: usize,
    /// The dental arch suggested.
    arch_rx: Option<Receiver<Option<arch::Arch>>>,
    /// The centrelines of the sidecars beside the volume (read when MPR opens).
    pub sidecars_rx: Option<Receiver<Vec<curved::Loaded>>>,
}

/// The other centrelines of the volume besides the chosen one ([`Session::curved`]), and
/// the chosen one's place among them: the list is `others[..at]`, the chosen one,
/// `others[at..]`. With no chosen one there are no others.
#[derive(Default)]
pub(crate) struct Centrelines {
    pub others: Vec<Curved>,
    pub at: usize,
    /// The chosen one's name being typed, and an input method's composition.
    pub rename: Option<(specular_widgets::line_edit::LineEdit, String)>,
    /// The chip with the keyboard focus (list order); `None`: the keys are the viewer's.
    pub focus: Option<usize>,
}

impl Centrelines {
    /// A press or release at `p` (logical px) over the chosen centreline `c`'s name field
    /// while it is typed: a press puts the caret (Shift extends, two select the word, three
    /// all) and drops the composition, the release ends a drag. False when not the field's.
    pub(crate) fn field_button(
        &mut self,
        c: Option<&Curved>,
        p: (f32, f32),
        down: bool,
        now: f64,
        shift: bool,
    ) -> bool {
        let Some((e, pre)) = self.rename.as_mut() else {
            return false;
        };
        if !down {
            let was = e.dragging();
            e.release();
            return was;
        }
        let Some(c) = c else { return false };
        let in_field = c
            .placed
            .iter()
            .any(|(ctl, r, _)| *ctl == Control::CurveRenameField && r.contains(p));
        let Some((block, x0)) = c.field_text.as_ref().filter(|_| in_field) else {
            return false;
        };
        pre.clear();
        let at = block.hit_byte(p.0 - x0).min(e.text().len());
        e.press(at, p.0, now, shift);
        true
    }

    /// The pointer at logical `x` with the button held after a press in `c`'s name field:
    /// the selection runs from the press to there. False when no press is being dragged.
    pub(crate) fn field_drag(&mut self, c: Option<&Curved>, x: f32) -> bool {
        let Some((e, _)) = self.rename.as_mut().filter(|(e, _)| e.dragging()) else {
            return false;
        };
        let Some((block, x0)) = c.and_then(|c| c.field_text.as_ref()) else {
            return false;
        };
        let at = block.hit_byte(x - x0).min(e.text().len());
        e.drag_to(at);
        true
    }

    /// Makes centreline `i` (list order) the chosen one; the layout stays as it was.
    pub(crate) fn select(&mut self, curved: &mut Option<Curved>, i: usize) -> bool {
        let n = self.others.len() + usize::from(curved.is_some());
        if i >= n || i == self.at || curved.is_none() {
            return false;
        }
        self.rename = None;
        let Some(mut cur) = curved.take() else {
            return false;
        };
        let shown = std::mem::replace(&mut cur.shown, false);
        let at = self.at.min(self.others.len());
        self.others.insert(at, cur);
        let mut next = self.others.remove(i);
        next.shown = shown;
        next.placing = next.points().len() < 2;
        *curved = Some(next);
        self.at = i;
        true
    }

    /// A new centreline after the others, chosen and shown, its points placed in view `v`.
    pub(crate) fn add_new(&mut self, curved: &mut Option<Curved>, v: usize, up: V3) {
        self.rename = None;
        if let Some(mut cur) = curved.take() {
            cur.shown = false;
            let at = self.at.min(self.others.len());
            self.others.insert(at, cur);
        }
        self.at = self.others.len();
        *curved = Some(Curved::new(v, up));
    }

    /// Removes the chosen centreline; its neighbour is chosen (shown if it was).
    pub(crate) fn delete(&mut self, curved: &mut Option<Curved>) -> Option<Curved> {
        self.rename = None;
        let gone = curved.take()?;
        if !self.others.is_empty() {
            let at = self.at.min(self.others.len() - 1);
            let mut next = self.others.remove(at);
            next.shown = gone.shown;
            *curved = Some(next);
            self.at = at;
        } else {
            self.at = 0;
        }
        Some(gone)
    }

    /// Adds centrelines after the others (the first chosen when none is).
    pub(crate) fn add(&mut self, curved: &mut Option<Curved>, mut add: Vec<Curved>) {
        if add.is_empty() {
            return;
        }
        if curved.is_none() {
            let first = add.remove(0);
            self.at = self.others.len();
            *curved = Some(first);
        }
        // Only the chosen one can be shown.
        self.others.extend(add.into_iter().map(|mut c| {
            c.shown = false;
            c
        }));
    }
}

/// The sidecars beside `path` (`<stem>-curve*.json` in its folder), read off the UI
/// thread: their centrelines in name order of the files (dental arches left out).
pub(crate) fn read_sidecars(path: &std::path::Path) -> Vec<curved::Loaded> {
    let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) else {
        return Vec::new();
    };
    let prefix = format!("{}-curve", stem.to_string_lossy());
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.starts_with(&prefix) && n.to_ascii_lowercase().ends_with(".json")
            })
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for f in files.into_iter().take(16) {
        if std::fs::metadata(&f).is_ok_and(|m| m.len() <= 8 << 20)
            && let Ok(text) = std::fs::read_to_string(&f)
            && let Ok(curves) = curved::from_json_all(&text)
        {
            out.extend(curves.into_iter().filter(|c| c.dental.is_none()));
        }
        if out.len() >= curved::MAX_CURVES {
            out.truncate(curved::MAX_CURVES);
            break;
        }
    }
    out
}

/// A centreline from a sidecar's curve, placed in view `v` (`up` its normal unless the
/// sidecar gives one).
pub(crate) fn centreline_of(l: curved::Loaded, v: usize, up: V3) -> Curved {
    let mut c = Curved::new(v, l.up.unwrap_or(up));
    c.name = l.name;
    c.kind = l.kind;
    c.angle = l.angle;
    c.marks = l.marks;
    c.placing = false;
    c.shown = false;
    c.set_points(l.points);
    c
}

/// The curve shown: the dental layout's or curved MPR's (one at a time).
pub(crate) fn shown<'a>(
    curved: &'a mut Option<Curved>,
    dental: &'a mut Option<Curved>,
) -> Option<&'a mut Curved> {
    match dental.as_mut().filter(|c| c.shown) {
        Some(d) => Some(d),
        None => curved.as_mut().filter(|c| c.shown),
    }
}

impl Session {
    /// The centrelines' names in list order, numbered where they have none.
    pub(crate) fn curve_names(&self) -> Vec<String> {
        let cl = &self.lines;
        let mut v: Vec<&Curved> = cl.others.iter().collect();
        if let Some(c) = &self.curved {
            v.insert(cl.at.min(v.len()), c);
        }
        v.iter()
            .enumerate()
            .map(|(i, c)| match c.name.trim() {
                "" => t!("cpr-centreline", number = (i + 1) as i64),
                n => n.to_owned(),
            })
            .collect()
    }

    /// Hands the list to the chosen centreline for drawing.
    pub(crate) fn sync_curve_list(&mut self) {
        let names = self.curve_names();
        let at = self.lines.at;
        let rename = self
            .lines
            .rename
            .as_ref()
            .map(|(e, pre)| (e.text().to_owned(), e.caret(), pre.clone()));
        let selection = self
            .lines
            .rename
            .as_ref()
            .map_or(0..0, |(e, _)| e.selection());
        let n = names.len();
        if self.lines.focus.is_some_and(|f| f >= n) {
            self.lines.focus = n.checked_sub(1);
        }
        let focus = self.lines.focus;
        if let Some(c) = self.curved.as_mut() {
            c.list = names;
            c.index = at.min(c.list.len().saturating_sub(1));
            c.rename = rename;
            c.rename_selection = selection;
            c.focus = focus;
        }
    }

    /// A press or release at logical `lp` over the centreline's name field while it is
    /// typed ([`Centrelines::field_button`]). False when not the field's.
    pub(crate) fn rename_button(
        &mut self,
        lp: (f64, f64),
        down: bool,
        now: f64,
        shift: bool,
    ) -> bool {
        let p = (lp.0 as f32, lp.1 as f32);
        let c = self.curved.as_ref().filter(|c| c.shown);
        self.lines.field_button(c, p, down, now, shift)
    }

    /// A drag from a press in the name field ([`Centrelines::field_drag`]).
    pub(crate) fn rename_drag(&mut self, x: f32) -> bool {
        self.lines.field_drag(self.curved.as_ref(), x)
    }

    /// Makes centreline `i` (list order) the chosen one; the layout stays as it was.
    pub(crate) fn curve_select(&mut self, i: usize) -> bool {
        self.lines.select(&mut self.curved, i)
    }

    /// A new centreline after the others, chosen and shown, its points placed in view `v`.
    pub(crate) fn curve_new(&mut self, v: usize, up: V3) {
        self.lines.add_new(&mut self.curved, v, up);
    }

    /// Removes the chosen centreline; its neighbour is chosen (shown if it was).
    pub(crate) fn curve_delete(&mut self) -> Option<Curved> {
        self.lines.delete(&mut self.curved)
    }

    /// Adds centrelines after the others (the first chosen when none is).
    pub(crate) fn curve_add(&mut self, add: Vec<Curved>) {
        self.lines.add(&mut self.curved, add);
    }

    /// The curve shown, if any.
    pub(crate) fn curve(&self) -> Option<&Curved> {
        self.dental
            .as_ref()
            .filter(|c| c.shown)
            .or_else(|| self.curved.as_ref().filter(|c| c.shown))
    }

    /// There are segments to export (segmentations found, or a label volume).
    pub(crate) fn has_segments(&self) -> bool {
        if self
            .analysis
            .sets
            .iter()
            .any(|s| !s.labels.segments.is_empty())
        {
            return true;
        }
        self.load.info.as_ref().is_some_and(|i| {
            i.unit.is_none()
                && i.seismic.is_none()
                && i.range.0 >= 0.0
                && i.range.1 >= 1.0
                && i.range.1 <= 4096.0
                && i.range.0.fract() == 0.0
                && i.range.1.fract() == 0.0
        })
    }
}

impl super::Mpr {
    /// Where a centreline's name field wants the input method's candidate window: under
    /// the field.
    pub(in crate::app) fn rename_ime_area(&self) -> Option<specular_gfx::Rect> {
        let s = self.s.as_ref()?;
        s.lines.rename.as_ref()?;
        s.curve()?
            .placed
            .iter()
            .find(|(c, ..)| *c == Control::CurveRenameField)
            .map(|x| x.1)
    }
}

/// The chip a key moves the focus to from chip `at` of `n`: the next one with → (← in a
/// right-to-left layout, where the chips run from the right) or ↓, the one before with the
/// other arrows, wrapping round; the ends with Home and End.
pub(crate) fn chip_step(
    key: &specular_input::Key,
    rtl: bool,
    at: usize,
    n: usize,
) -> Option<usize> {
    use specular_input::{Key, NamedKey};
    if n == 0 {
        return None;
    }
    let (fwd, bwd) = if rtl {
        (NamedKey::ArrowLeft, NamedKey::ArrowRight)
    } else {
        (NamedKey::ArrowRight, NamedKey::ArrowLeft)
    };
    match *key {
        Key::Named(k) if k == fwd || k == NamedKey::ArrowDown => Some((at + 1) % n),
        Key::Named(k) if k == bwd || k == NamedKey::ArrowUp => Some((at + n - 1) % n),
        Key::Named(NamedKey::Home) => Some(0),
        Key::Named(NamedKey::End) => Some(n - 1),
        _ => None,
    }
}

/// The confirmation's id for deleting a centreline.
pub(crate) const DELETE_ITEM: u64 = crate::app::items::CPR_DELETE;

impl App {
    /// Deleting the chosen centreline was confirmed.
    pub(in crate::app) fn mpr_curve_delete_confirmed(&mut self) {
        let now = self.now();
        let Some(s) = self.mpr.s.as_mut() else { return };
        let name = s.curve_names().get(s.lines.at).cloned().unwrap_or_default();
        if s.curve_delete().is_some() {
            self.chrome.toast(t!("cpr-deleted", name = name), None, now);
            self.mpr.rev += 1;
            self.redraw = true;
        }
    }

    /// The centreline chips on screen: the curved reformat shows its list (not while a
    /// name is typed, and not on a dental panorama).
    fn mpr_chips_shown(&self) -> bool {
        self.mpr.s.as_ref().is_some_and(|s| {
            s.lines.rename.is_none()
                && s.curve()
                    .is_some_and(|c| c.dental.is_none() && !c.list.is_empty())
        })
    }

    /// Keys for the centreline chips (curved MPR): Tab from the image (when the keymap
    /// leaves it free and the chrome has no focus) enters them at the chosen one; ←/→
    /// (mirrored in right-to-left layouts), Home and End move between them; Enter or Space
    /// chooses one, F2 renames it, Delete asks to delete it; Tab goes on to the chrome,
    /// Shift+Tab and Esc back to the image. False when the key is not theirs.
    pub(in crate::app) fn mpr_chips_key(&mut self, c: &specular_input::KeyCombo) -> bool {
        use specular_input::{Key, NamedKey};
        if !self.mpr_chips_shown() {
            if let Some(s) = self.mpr.s.as_mut() {
                s.lines.focus = None;
            }
            return false;
        }
        let rtl = self.rtl();
        // Tab: Some(false), Shift+Tab: Some(true).
        let tab =
            (c.key == Key::Named(NamedKey::Tab) && !c.mods.ctrl && !c.mods.alt && !c.mods.meta)
                .then_some(c.mods.shift);
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let n = s.curve_names().len();
        if n == 0 {
            return false;
        }
        let Some(at) = s.lines.focus else {
            // Entering from the image.
            if tab == Some(false)
                && !self.chrome.has_keyboard_focus()
                && self.keymap.lookup(c).is_none()
            {
                s.lines.focus = Some(s.lines.at.min(n - 1));
                self.mpr.rev += 1;
                self.redraw = true;
                let name = s.curve_names().get(s.lines.at.min(n - 1)).cloned();
                self.chrome.announce_status(name.unwrap_or_default());
                return true;
            }
            return false;
        };
        if c.mods.ctrl || c.mods.alt || c.mods.meta {
            return false;
        }
        if let Some(to) = chip_step(&c.key, rtl, at, n) {
            s.lines.focus = Some(to);
            let name = s.curve_names().get(to).cloned().unwrap_or_default();
            self.chrome.announce_status(name);
            self.mpr.rev += 1;
            self.redraw = true;
            return true;
        }
        match (c.key, tab) {
            // Tab goes on to the chrome (its first stop), as from the image.
            (_, Some(false)) => {
                s.lines.focus = None;
                self.mpr.rev += 1;
                self.redraw = true;
                false
            }
            (_, Some(true)) | (Key::Named(NamedKey::Escape), _) => {
                s.lines.focus = None;
                self.mpr.rev += 1;
                self.redraw = true;
                true
            }
            (Key::Named(NamedKey::Enter | NamedKey::Space), _) => {
                self.mpr_curve_control(Control::CurveName(at));
                self.mpr.rev += 1;
                self.redraw = true;
                true
            }
            (Key::Named(NamedKey::F(2)), _) => {
                self.mpr_curve_control(Control::CurveName(at));
                self.mpr_curve_control(Control::CurveRename);
                self.mpr.rev += 1;
                self.redraw = true;
                true
            }
            (Key::Named(NamedKey::Delete), _) => {
                // Chosen first: the confirmation names it, and deletes the chosen one.
                self.mpr_curve_control(Control::CurveName(at));
                self.mpr_curve_control(Control::CurveDelete);
                self.mpr.rev += 1;
                self.redraw = true;
                true
            }
            _ => false,
        }
    }

    /// A screen reader's request on a curved MPR control: Click presses it, Focus on a
    /// centreline chip gives it the keyboard focus. False when the node is not one.
    pub(in crate::app) fn mpr_curve_a11y_request(
        &mut self,
        req: &specular_widgets::accesskit::ActionRequest,
    ) -> bool {
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let Some(ctl) = s
            .curve()
            .and_then(|c| curved::a11y_control(&c.placed, req.target_node.0))
        else {
            return false;
        };
        match (req.action, ctl) {
            (specular_widgets::accesskit::Action::Focus, Control::CurveName(i)) => {
                s.lines.focus = Some(i);
            }
            (specular_widgets::accesskit::Action::Click, _) => {
                if let Control::CurveName(i) = ctl {
                    s.lines.focus = Some(i);
                }
                self.mpr_curve_control(ctl);
            }
            _ => return false,
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// A centreline's name is being typed: the keys go to its field.
    pub(in crate::app) fn mpr_renaming(&self) -> bool {
        self.mpr
            .s
            .as_ref()
            .is_some_and(|s| s.lines.rename.is_some() && s.curve().is_some())
    }

    /// Keys while a centreline's name is typed: Enter keeps it, Esc leaves it as it was.
    pub(in crate::app) fn mpr_rename_key(
        &mut self,
        combo: Option<&specular_input::KeyCombo>,
        text: Option<&str>,
    ) {
        use specular_input::{Key, NamedKey};
        let rtl = self.rtl();
        let Some(s) = self.mpr.s.as_mut() else { return };
        self.mpr.rev += 1;
        self.redraw = true;
        if let Some(c) = combo {
            match c.key {
                Key::Named(NamedKey::Enter) => {
                    if let Some((e, _)) = s.lines.rename.take()
                        && let Some(cur) = s.curved.as_mut()
                    {
                        let name: String = e.text().trim().chars().take(80).collect();
                        cur.name = name;
                    }
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    s.lines.rename = None;
                    return;
                }
                _ => {}
            }
            let mut clip = specular_widgets::line_edit::PendingClipboard::default();
            if let Some((e, _)) = s.lines.rename.as_mut()
                && e.key(c.key, c.mods, rtl, &mut clip).used()
            {
                return;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return;
            }
        }
        if let Some(t) = text
            && let Some((e, _)) = s.lines.rename.as_mut()
        {
            e.insert(&specular_widgets::line_edit::single_line(t));
        }
    }

    /// An input method's composition (empty: none) for the name field.
    pub(in crate::app) fn mpr_rename_preedit(&mut self, s: &str) {
        if let Some((_, pre)) = self.mpr.s.as_mut().and_then(|x| x.lines.rename.as_mut()) {
            *pre = s.to_owned();
            self.mpr.rev += 1;
        }
    }

    /// An input method's commit, typed into the name field.
    pub(in crate::app) fn mpr_rename_commit(&mut self, s: &str) {
        if let Some((e, pre)) = self.mpr.s.as_mut().and_then(|x| x.lines.rename.as_mut()) {
            pre.clear();
            e.insert(&specular_widgets::line_edit::single_line(s));
            self.mpr.rev += 1;
            self.redraw = true;
        }
    }

    /// Curved MPR's actions; true when `a` was one.
    pub(super) fn mpr_curve_action(&mut self, a: Action) -> bool {
        let now = self.now();
        match a {
            Action::MprCurve => self.mpr_curve_toggle(),
            Action::MprDental => self.mpr_dental_toggle(),
            Action::MprCurveLayout => {
                let Some(s) = self.mpr.s.as_mut() else {
                    return true;
                };
                let Some(c) = s.curved.as_mut().filter(|c| c.shown) else {
                    self.chrome.toast(t!("cpr-none"), None, now);
                    return true;
                };
                c.kind = c.kind.next();
                c.pan = [0.0; 2];
                let k = c.kind.id();
                self.chrome.toast(t!("cpr-kind", kind = k), None, now);
            }
            Action::MprCurveSave => self.mpr_curve_save(),
            Action::MprCurveLoad => self.mpr_curve_load(),
            Action::MprExportSegments => self.mpr_segments_export(),
            Action::MprNerve => {
                let Some(s) = self.mpr.s.as_mut() else {
                    return true;
                };
                let Some(d) = s
                    .dental
                    .as_mut()
                    .filter(|c| c.shown)
                    .and_then(|c| c.dental.as_mut())
                else {
                    self.chrome.toast(t!("dental-off"), None, now);
                    return true;
                };
                d.tracing = !d.tracing;
                let msg = if d.tracing {
                    t!("dental-nerve-hint")
                } else {
                    t!("dental-nerve-done")
                };
                self.chrome.toast(msg, None, now);
            }
            _ => return false,
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// Curved MPR on (a new curve placed in the view last used, or the one kept), or off
    /// (the curve stays for next time).
    fn mpr_curve_toggle(&mut self) {
        let now = self.now();
        let Some(s) = self.mpr.s.as_mut() else { return };
        let Some(views) = s.views.as_ref() else {
            return;
        };
        if let Some(d) = s.dental.as_mut() {
            d.shown = false;
        }
        match s.curved.as_mut() {
            Some(c) => c.shown = !c.shown,
            None => {
                let v = s.active.min(2);
                s.curved = Some(Curved::new(v, views.normal(v)));
                let view = super::view_name(v, s.oriented(), views.oblique(v));
                self.chrome
                    .toast(t!("cpr-hint-start", view = view), None, now);
            }
        }
        s.maximised = None;
        s.drag = None;
    }

    /// The dental layout on (the arch suggested on a thread the first time) or off.
    fn mpr_dental_toggle(&mut self) {
        let now = self.now();
        let Some(s) = self.mpr.s.as_mut() else { return };
        let Some(views) = s.views.as_ref() else {
            return;
        };
        if let Some(c) = s.curved.as_mut() {
            c.shown = false;
        }
        s.maximised = None;
        s.drag = None;
        if let Some(d) = s.dental.as_mut() {
            d.shown = !d.shown;
            return;
        }
        let vendor = s.load.info.as_ref().and_then(|i| i.vendor);
        let mut c = Curved::new(0, views.normal(0));
        c.placing = false;
        c.dental = Some(Dental::new(vendor));
        s.dental = Some(c);
        let volume = s.load.volume.clone();
        let (tx, rx) = bounded(1);
        self.loader.background("mpr-arch", move |l| {
            let a = arch::suggest(&lock(&volume));
            let _ = tx.send(a);
            l.waker()();
        });
        s.curve_io.arch_rx = Some(rx);
        self.chrome.toast(t!("dental-suggesting"), None, now);
    }

    /// A control over the reformat or the slot's curved MPR buttons; true when taken.
    pub(super) fn mpr_curve_control(&mut self, ctl: Control) -> bool {
        let now = self.now();
        let a = match ctl {
            Control::Curve | Control::CurveDone => Some(Action::MprCurve),
            Control::Dental => Some(Action::MprDental),
            Control::ExportSegments => Some(Action::MprExportSegments),
            Control::CurveKind(_) => Some(Action::MprCurveLayout),
            Control::CurveSave => Some(Action::MprCurveSave),
            Control::CurveLoad => Some(Action::MprCurveLoad),
            Control::Nerve => Some(Action::MprNerve),
            _ => None,
        };
        if let Some(a) = a {
            // Done leaves the layout shown: dental or curved.
            if ctl == Control::CurveDone
                && self
                    .mpr
                    .s
                    .as_ref()
                    .and_then(|s| s.dental.as_ref())
                    .is_some_and(|d| d.shown)
            {
                return self.mpr_curve_action(Action::MprDental);
            }
            return self.mpr_curve_action(a);
        }
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        match ctl {
            Control::CurveName(i) => {
                if s.curve_select(i) {
                    let name = s.curve_names().get(i).cloned().unwrap_or_default();
                    self.chrome.announce_status(name);
                }
            }
            Control::CurveNew => {
                let Some(views) = s.views.as_ref() else {
                    return true;
                };
                let v = s.curved.as_ref().map_or(s.active.min(2), |c| c.view);
                let up = views.normal(v);
                let view = super::view_name(v, s.oriented(), views.oblique(v));
                s.curve_new(v, up);
                self.chrome
                    .toast(t!("cpr-hint-start", view = view), None, now);
            }
            Control::CurveRename => {
                if s.curved.is_some() && s.lines.rename.is_none() {
                    let name = s
                        .curved
                        .as_ref()
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    let mut e = specular_widgets::line_edit::LineEdit::new(&name);
                    e.select_all();
                    s.lines.rename = Some((e, String::new()));
                }
            }
            Control::CurveDelete => {
                let names = s.curve_names();
                if let Some(name) = names.get(s.lines.at).cloned() {
                    self.chrome.confirm_destructive(
                        t!("cpr-delete-title", name = name),
                        t!("cpr-delete-detail"),
                        t!("cpr-delete"),
                        DELETE_ITEM,
                    );
                }
            }
            Control::SegmentDetail(_) => {
                s.curve_io.detail = (s.curve_io.detail + 1) % DETAILS.len();
                let msg = detail_text(DETAILS[s.curve_io.detail]);
                self.chrome.toast(msg, None, now);
            }
            _ => {
                let Some(c) = shown(&mut s.curved, &mut s.dental) else {
                    return false;
                };
                match ctl {
                    Control::CurvePlacing => c.placing = !c.placing,
                    Control::CurveMark => {
                        let set = c.toggle_mark();
                        let msg = match c.mark_at_cursor() {
                            Some(i) if set => t!("cpr-mark-set", n = (i + 1) as i64),
                            _ => t!("cpr-mark-removed"),
                        };
                        self.chrome.announce_status(msg);
                    }
                    Control::CurveUndo => {
                        let tracing = c.dental.as_ref().is_some_and(|d| d.tracing);
                        match c.dental.as_mut().filter(|_| tracing) {
                            Some(d) => {
                                let mut n = d.nerve.clone();
                                n.pop();
                                d.set_nerve(n);
                            }
                            None => {
                                c.pop();
                            }
                        }
                    }
                    Control::CurveClear => {
                        if !c.points().is_empty() {
                            c.cleared = Some(c.points().to_vec());
                            c.set_points(Vec::new());
                            c.placing = true;
                            self.chrome.toast(t!("cpr-cleared"), Some(UNDO_KEY), now);
                            self.last_undo = Some(UNDO_KEY);
                        }
                    }
                    Control::PanoSlab(_) => {
                        if let Some(d) = c.dental.as_mut() {
                            d.slab = (d.slab + 1) % curved::PANO_SLABS.len();
                        }
                    }
                    Control::Spacing(_) => {
                        if let Some(d) = c.dental.as_mut() {
                            d.spacing = (d.spacing + 1) % curved::SPACINGS.len();
                        }
                    }
                    _ => return false,
                }
            }
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// The Undo of a cleared curve's toast.
    pub(in crate::app) fn mpr_curve_undo(&mut self, key: u64) -> bool {
        if key != UNDO_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        if self.last_undo == Some(key) {
            self.last_undo = None;
        }
        if let Some(s) = self.mpr.s.as_mut() {
            for c in [s.curved.as_mut(), s.dental.as_mut()].into_iter().flatten() {
                if let Some(p) = c.cleared.take() {
                    c.set_points(p);
                }
            }
            self.mpr.rev += 1;
            self.redraw = true;
        }
        true
    }

    /// A press or release at logical `lp` over curved MPR's views; `None` when it is not
    /// curved MPR's to take.
    pub(super) fn mpr_curve_button(
        &mut self,
        lp: (f64, f64),
        down: bool,
        b: Button,
    ) -> Option<bool> {
        let scale = f64::from(self.dl.scale.max(0.1));
        let shift = self.mods.shift_key();
        let hit = POINT_HIT * f64::from(self.chrome.text_scale);
        let now = self.now();
        let s = self.mpr.s.as_mut()?;
        s.curve()?;
        if b == Button::Primary && s.rename_button(lp, down, now, shift) {
            self.mpr.rev += 1;
            self.redraw = true;
            return Some(true);
        }
        if !down {
            return match s.drag {
                Some(Drag::CurvePoint(_) | Drag::CprPan(_)) => {
                    s.drag = None;
                    self.mpr.rev += 1;
                    self.redraw = true;
                    Some(true)
                }
                _ => None,
            };
        }
        let window = s.current_window();
        let measuring = s.measuring;
        let c = shown(&mut s.curved, &mut s.dental)?;
        if b == Button::Primary
            && let Some(&(ctl, _, _)) = c
                .placed
                .iter()
                .find(|(_, r, _)| r.contains((lp.0 as f32, lp.1 as f32)))
        {
            self.mpr_curve_control(ctl);
            return Some(true);
        }
        let rects = c.rects.clone()?;
        let views = s.views.as_ref()?;
        let in_xsec = std::iter::once(rects.xsec)
            .chain(rects.cells.iter().copied())
            .any(|r| contains(r, lp));
        let mut drag = None;
        if contains(rects.cpr, lp) {
            match b {
                Button::Secondary => drag = Some(Drag::Window(lp, window)),
                Button::Middle => drag = Some(Drag::CprPan(lp)),
                Button::Primary if shift => drag = Some(Drag::CprPan(lp)),
                Button::Primary => match c.dental.as_mut().filter(|d| d.tracing) {
                    Some(_) => {
                        if let Some(p) = c.cpr_point(lp, scale)
                            && let Some(d) = c.dental.as_mut()
                        {
                            let mut n = d.nerve.clone();
                            n.push(p);
                            d.set_nerve(n);
                        }
                    }
                    None => {
                        if let Some(at) = c.cpr_s(lp.0, scale) {
                            c.cursor = at;
                        }
                    }
                },
            }
        } else if in_xsec {
            let plane = c.xsec_at(lp, scale, window, views);
            match b {
                Button::Secondary => drag = Some(Drag::Window(lp, window)),
                Button::Primary => {
                    let Some(plane) = plane else {
                        return Some(true);
                    };
                    let p: V3 = plane.at(lp.0 * scale, lp.1 * scale);
                    if measuring {
                        match s.draft.take() {
                            Some(a) => s.measures.push([a, p]),
                            None => s.draft = Some(p),
                        }
                    } else if let Some(d) = c.dental.as_mut().filter(|d| d.tracing) {
                        let mut n = d.nerve.clone();
                        n.push(p);
                        d.set_nerve(n);
                    } else if c.dental.is_some() {
                        // A click on a cell centres the cells on it.
                        let cells: Vec<_> = rects
                            .cells
                            .iter()
                            .copied()
                            .zip(c.cell_positions())
                            .collect();
                        if let Some((_, at)) = cells.into_iter().find(|(r, _)| contains(*r, lp)) {
                            let l = c.spline().map_or(0.0, |sp| sp.length());
                            c.cursor = at.clamp(0.0, l);
                        }
                    }
                }
                Button::Middle => {}
            }
        } else {
            let v = c.view.min(2);
            let r = s.layout?.views[v];
            if !contains(r, lp) || b != Button::Primary || shift || !c.placing || measuring {
                return None;
            }
            match c.point_at(views, v, r, lp, hit) {
                Some(i) => drag = Some(Drag::CurvePoint(i)),
                None => c.push(views.to_patient(v, r, lp)),
            }
            s.active = v;
        }
        if drag.is_some() {
            s.drag = drag;
        }
        self.mpr.rev += 1;
        self.redraw = true;
        Some(true)
    }

    /// Pointer motion at logical `lp` during curved MPR's drags.
    pub(super) fn mpr_curve_motion(&mut self, lp: (f64, f64)) {
        let scale = f64::from(self.dl.scale.max(0.1));
        let Some(s) = self.mpr.s.as_mut() else { return };
        let drag = s.drag;
        let (Some(views), Some(l)) = (s.views.as_ref(), s.layout) else {
            return;
        };
        let Some(c) = shown(&mut s.curved, &mut s.dental) else {
            return;
        };
        match drag {
            Some(Drag::CurvePoint(i)) => {
                let v = c.view.min(2);
                c.move_point(i, views.to_patient(v, l.views[v], lp));
            }
            Some(Drag::CprPan(last)) => {
                let mm = c.cpr.as_ref().map_or(1.0, |p| p.mm) * scale;
                c.pan[0] -= (lp.0 - last.0) * mm;
                c.pan[1] += (lp.1 - last.1) * mm;
                s.drag = Some(Drag::CprPan(lp));
            }
            _ => {}
        }
    }

    /// A wheel turn over the reformat or a cross-section: the cursor moves along the curve;
    /// Ctrl zooms; Shift turns the reformat about the curve. `None` elsewhere.
    pub(super) fn mpr_curve_wheel(&mut self, dy: f64, lp: (f64, f64)) -> Option<bool> {
        let ctrl = self.mods.control_key();
        let shift = self.mods.shift_key();
        let s = self.mpr.s.as_mut()?;
        let c = shown(&mut s.curved, &mut s.dental)?;
        let rects = c.rects.clone()?;
        let in_cpr = contains(rects.cpr, lp);
        let in_xsec = std::iter::once(rects.xsec)
            .chain(rects.cells.iter().copied())
            .any(|r| contains(r, lp));
        if !in_cpr && !in_xsec {
            return None;
        }
        if ctrl {
            let f = 1.2f64.powf(dy);
            if in_cpr {
                c.zoom = (c.zoom * f).clamp(0.25, 40.0);
            } else {
                c.xsec_zoom = (c.xsec_zoom * f).clamp(0.25, 20.0);
            }
        } else if shift && in_cpr && c.dental.is_none() {
            c.angle = (c.angle + dy * 5f64.to_radians()).rem_euclid(std::f64::consts::TAU);
        } else {
            s.wheel += -dy;
            let steps = s.wheel.trunc().clamp(-10.0, 10.0);
            s.wheel -= s.wheel.trunc();
            if steps != 0.0 {
                c.step(steps);
            }
        }
        self.mpr.rev += 1;
        self.redraw = true;
        Some(true)
    }

    /// Esc in curved MPR: tracing stops, then the layout closes (the curve stays).
    pub(super) fn mpr_curve_back(&mut self) -> bool {
        let Some(s) = self.mpr.s.as_mut() else {
            return false;
        };
        let Some(c) = shown(&mut s.curved, &mut s.dental) else {
            return false;
        };
        if let Some(d) = c.dental.as_mut().filter(|d| d.tracing) {
            d.tracing = false;
        } else {
            c.shown = false;
        }
        self.mpr.rev += 1;
        self.redraw = true;
        true
    }

    /// The file name stem of the image MPR is on.
    fn mpr_stem(&self) -> String {
        self.current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| p.file_stem())
            .map_or_else(|| "volume".to_owned(), |s| s.to_string_lossy().into_owned())
    }

    fn mpr_curve_save(&mut self) {
        let now = self.now();
        let dir = self.folder.clone();
        let stem = self.mpr_stem();
        let Some(s) = self.mpr.s.as_mut() else { return };
        let Some(c) = s.curve().filter(|c| c.spline().is_some()) else {
            self.chrome.toast(t!("cpr-none"), None, now);
            return;
        };
        if s.curve_io.save_rx.is_some() {
            return;
        }
        // The dental arch alone; else every centreline of two points or more, in order.
        let json = if c.dental.is_some() {
            curved::to_json_all(&[c], None)
        } else {
            let mut all: Vec<&Curved> = s.lines.others.iter().collect();
            all.insert(s.lines.at.min(all.len()), c);
            all.retain(|c| c.spline().is_some());
            curved::to_json_all(&all, None)
        };
        let title = t!("palette-mpr-curve-save");
        let (tx, rx) = bounded(1);
        self.loader.background("mpr-curve-dlg", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .set_title(title)
                .add_filter("JSON", &["json"])
                .set_file_name(format!("{stem}-curve.json"));
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(mut p) = dlg.save_file() {
                if p.extension().is_none() {
                    p.set_extension("json");
                }
                let r = std::fs::write(&p, json.as_bytes())
                    .map(|()| p)
                    .map_err(|e| crate::error_text::io_reason(&e));
                let _ = tx.send(r);
            }
            drop(tx);
            l.waker()();
        });
        s.curve_io.save_rx = Some(rx);
    }

    fn mpr_curve_load(&mut self) {
        let dir = self.folder.clone();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if s.curve_io.load_rx.is_some() {
            return;
        }
        let title = t!("palette-mpr-curve-load");
        let (tx, rx) = bounded(1);
        self.loader.background("mpr-curve-dlg", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .set_title(title)
                .add_filter("JSON", &["json"]);
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_file() {
                let r = match std::fs::metadata(&p) {
                    Ok(m) if m.len() > 8 << 20 => Err(t!("cpr-load-not-curves")),
                    Ok(_) => {
                        std::fs::read_to_string(&p).map_err(|e| crate::error_text::io_reason(&e))
                    }
                    Err(e) => Err(crate::error_text::io_reason(&e)),
                };
                let _ = tx.send(r);
            }
            drop(tx);
            l.waker()();
        });
        s.curve_io.load_rx = Some(rx);
    }

    /// Applies a sidecar read: a dental arch to the dental layout; centrelines added to the
    /// volume's list (the first of them chosen and shown).
    fn mpr_curve_apply(&mut self, text: &str) {
        let now = self.now();
        let mut all = match curved::from_json_all(text) {
            Ok(l) => l,
            Err(e) => {
                self.chrome
                    .toast(t!("cpr-load-failed", reason = e), None, now);
                return;
            }
        };
        let Some(s) = self.mpr.s.as_mut() else { return };
        let Some(views) = s.views.as_ref() else {
            return;
        };
        if all.iter().all(|l| l.dental.is_none()) {
            let v = s.active.min(2);
            let up = views.normal(v);
            let n = all.len();
            let lines: Vec<Curved> = all.into_iter().map(|l| centreline_of(l, v, up)).collect();
            if let Some(d) = s.dental.as_mut() {
                d.shown = false;
            }
            let first = s.lines.others.len() + usize::from(s.curved.is_some());
            let was = s.curved.is_some();
            s.curve_add(lines);
            if was {
                s.curve_select(first);
            }
            if let Some(c) = s.curved.as_mut() {
                c.shown = true;
            }
            s.maximised = None;
            self.chrome
                .toast(t!("cpr-loaded-count", count = n as i64), None, now);
            return;
        }
        let loaded = all.remove(0);
        let target = match (loaded.dental.is_some(), &mut s.dental, &mut s.curved) {
            (true, d, _) => {
                if let Some(c) = s.curved.as_mut() {
                    c.shown = false;
                }
                d.get_or_insert_with(|| {
                    let mut c = Curved::new(0, views.normal(0));
                    c.dental = Some(Dental::new(None));
                    c
                })
            }
            (false, _, c) => {
                if let Some(d) = s.dental.as_mut() {
                    d.shown = false;
                }
                let v = s.active.min(2);
                c.get_or_insert_with(|| Curved::new(v, views.normal(v)))
            }
        };
        target.shown = true;
        target.placing = false;
        if let Some(up) = loaded.up {
            target.up = up;
        }
        target.kind = loaded.kind;
        target.angle = loaded.angle;
        target.set_points(loaded.points);
        target.marks = loaded.marks;
        if let (Some(d), Some((nerve, spacing))) = (target.dental.as_mut(), loaded.dental) {
            d.set_nerve(nerve);
            d.spacing = curved::SPACINGS
                .iter()
                .position(|x| (x - spacing).abs() < 1e-6)
                .unwrap_or(0);
        }
        s.maximised = None;
        self.chrome.toast(t!("cpr-loaded"), None, now);
    }

    fn mpr_segments_export(&mut self) {
        let now = self.now();
        let dir = self.folder.clone();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if !s.has_segments() {
            self.chrome.toast(t!("seg-stl-none"), None, now);
            return;
        }
        if s.curve_io.seg_dir_rx.is_some() || s.curve_io.seg_done.is_some() {
            return;
        }
        let title = t!("palette-mpr-export-segments");
        let (tx, rx) = bounded(1);
        self.loader.background("mpr-segs-dlg", move |l| {
            let mut dlg = rfd::FileDialog::new().set_title(title);
            if let Some(dir) = dir {
                dlg = dlg.set_directory(dir);
            }
            if let Some(p) = dlg.pick_folder() {
                let _ = tx.send(p);
            }
            drop(tx);
            l.waker()();
        });
        s.curve_io.seg_dir_rx = Some(rx);
    }

    /// The dialogs' and the background work's results.
    pub(super) fn mpr_curve_tick(&mut self) {
        let now = self.now();
        let stem = self.mpr_stem();
        let Some(s) = self.mpr.s.as_mut() else { return };
        if let (Some(views), Some(rx)) = (s.views.as_ref(), &s.curve_io.sidecars_rx)
            && let Ok(found) = rx.try_recv()
        {
            s.curve_io.sidecars_rx = None;
            let v = s.active.min(2);
            let up = views.normal(v);
            let n = found.len();
            let lines: Vec<Curved> = found.into_iter().map(|l| centreline_of(l, v, up)).collect();
            s.curve_add(lines);
            if n > 0 {
                self.chrome
                    .toast(t!("cpr-sidecars-found", count = n as i64), None, now);
            }
            self.mpr.rev += 1;
            self.redraw = true;
        }
        if let Some(rx) = &s.curve_io.arch_rx
            && let Ok(a) = rx.try_recv()
        {
            s.curve_io.arch_rx = None;
            if let Some(c) = s.dental.as_mut() {
                match a {
                    Some(a) => {
                        c.set_points(a.points);
                        if let Some(views) = s.views.as_mut()
                            && let Some(p) = c.points().first()
                        {
                            // The axial view at the arch's level.
                            views.c[2] = p[2];
                        }
                        self.chrome.toast(t!("dental-suggested"), None, now);
                    }
                    None => {
                        c.placing = true;
                        self.chrome.toast(t!("dental-no-arch"), None, now);
                    }
                }
            }
            self.mpr.rev += 1;
            self.redraw = true;
        }
        match s.curve_io.save_rx.as_ref().map(Receiver::try_recv) {
            Some(Ok(r)) => {
                s.curve_io.save_rx = None;
                let msg = match r {
                    Ok(p) => t!("cpr-saved", name = file_name(&p)),
                    Err(e) => t!("cpr-save-failed", reason = e),
                };
                self.chrome.toast(msg, None, now);
            }
            Some(Err(TryRecvError::Disconnected)) => s.curve_io.save_rx = None,
            _ => {}
        }
        match s.curve_io.load_rx.as_ref().map(Receiver::try_recv) {
            Some(Ok(r)) => {
                s.curve_io.load_rx = None;
                match r {
                    Ok(text) => self.mpr_curve_apply(&text),
                    Err(e) => {
                        self.chrome
                            .toast(t!("cpr-load-failed", reason = e), None, now);
                    }
                }
                self.mpr.rev += 1;
                self.redraw = true;
                return;
            }
            Some(Err(TryRecvError::Disconnected)) => s.curve_io.load_rx = None,
            _ => {}
        }
        match s.curve_io.seg_dir_rx.as_ref().map(Receiver::try_recv) {
            Some(Ok(dir)) => {
                s.curve_io.seg_dir_rx = None;
                let target = DETAILS[s.curve_io.detail % DETAILS.len()];
                let smooth = s.vr.smooth;
                let labels: Vec<_> = s.analysis.sets.iter().map(|x| &x.labels).collect();
                let bytes: u64 = labels.iter().map(|l| l.bytes()).sum();
                let volume = s.load.volume.clone();
                let reserve = if labels.is_empty() {
                    lock(&volume).bytes()
                } else {
                    bytes
                };
                let reserved = match specular_core::budget::reserve(reserve) {
                    Ok(r) => r,
                    Err(e) => {
                        let reason = crate::error_text::needs_memory_reason(e);
                        self.chrome
                            .toast(t!("seg-stl-failed", reason = reason), None, now);
                        return;
                    }
                };
                let jobs = (!labels.is_empty()).then(|| segments::jobs_of_labels(&labels));
                let (tx, rx) = bounded(1);
                self.loader.background("mpr-segments", move |l| {
                    let _reserved = reserved;
                    let jobs = match jobs {
                        Some(j) => Ok(j),
                        None => {
                            let v = Arc::new(lock(&volume).clone());
                            segments::jobs_of_label_volume(v)
                        }
                    };
                    let r = jobs.and_then(|j| segments::export(&j, &dir, &stem, smooth, target));
                    let _ = tx.send(r);
                    l.waker()();
                });
                s.curve_io.seg_done = Some(rx);
                self.chrome.toast(t!("seg-stl-exporting"), None, now);
            }
            Some(Err(TryRecvError::Disconnected)) => s.curve_io.seg_dir_rx = None,
            _ => {}
        }
        if let Some(rx) = &s.curve_io.seg_done
            && let Ok(r) = rx.try_recv()
        {
            s.curve_io.seg_done = None;
            let msg = match r {
                Ok(r) if r.written == 0 => t!("seg-stl-empty"),
                Ok(r) => {
                    let mut m = t!(
                        "seg-stl-saved",
                        count = r.written as i64,
                        folder = file_name(&r.dir)
                    );
                    if r.open > 0 {
                        m = format!("{m} {}", t!("seg-stl-open", count = r.open as i64));
                    }
                    m
                }
                Err(e) => t!("seg-stl-failed", reason = e),
            };
            self.chrome.toast(msg, None, now);
            self.redraw = true;
        }
    }
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The segments' triangle count, as the button and toast say it.
pub(crate) fn detail_text(target: Option<usize>) -> String {
    match target {
        Some(n) => t!("seg-stl-detail", count = n as i64),
        None => t!("seg-stl-detail-full"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arrows move the focus between the centreline chips in reading order (mirrored
    /// in a right-to-left layout), wrapping round; Home and End go to the ends; other keys
    /// are not moves.
    #[test]
    fn chip_focus_follows_the_arrows() {
        use specular_input::{Key, NamedKey};
        let k = |n| Key::Named(n);
        assert_eq!(chip_step(&k(NamedKey::ArrowRight), false, 0, 3), Some(1));
        assert_eq!(chip_step(&k(NamedKey::ArrowRight), false, 2, 3), Some(0));
        assert_eq!(chip_step(&k(NamedKey::ArrowLeft), false, 0, 3), Some(2));
        assert_eq!(chip_step(&k(NamedKey::ArrowLeft), true, 0, 3), Some(1));
        assert_eq!(chip_step(&k(NamedKey::ArrowRight), true, 1, 3), Some(0));
        assert_eq!(chip_step(&k(NamedKey::ArrowDown), true, 1, 3), Some(2));
        assert_eq!(chip_step(&k(NamedKey::ArrowUp), false, 1, 3), Some(0));
        assert_eq!(chip_step(&k(NamedKey::Home), false, 2, 3), Some(0));
        assert_eq!(chip_step(&k(NamedKey::End), false, 0, 3), Some(2));
        assert_eq!(chip_step(&k(NamedKey::Enter), false, 0, 3), None);
        assert_eq!(chip_step(&Key::Char('a'), false, 0, 3), None);
        assert_eq!(chip_step(&k(NamedKey::ArrowRight), false, 0, 0), None);
    }

    fn named(n: &str) -> Curved {
        let mut c = Curved::new(0, [0.0, 0.0, 1.0]);
        c.name = n.into();
        c.set_points(vec![[0.0; 3], [5.0, 0.0, 0.0]]);
        c
    }

    fn order(cur: &Option<Curved>, l: &Centrelines) -> Vec<String> {
        let mut v: Vec<String> = l.others.iter().map(|c| c.name.clone()).collect();
        if let Some(c) = cur {
            v.insert(l.at, format!("*{}", c.name));
        }
        v
    }

    /// Several centrelines per volume: added, chosen, a new one placed, the chosen one
    /// deleted (its neighbour chosen), the list order kept throughout.
    #[test]
    fn centrelines_are_listed_chosen_and_deleted() {
        let mut l = Centrelines::default();
        let mut cur = None;
        l.add(&mut cur, vec![named("a"), named("b"), named("c")]);
        assert_eq!(order(&cur, &l), ["*a", "b", "c"]);
        if let Some(c) = cur.as_mut() {
            c.shown = true;
        }
        assert!(l.select(&mut cur, 2));
        assert_eq!(order(&cur, &l), ["a", "b", "*c"]);
        assert!(
            cur.as_ref().is_some_and(|c| c.shown),
            "the layout stays shown"
        );
        assert!(!l.others.iter().any(|c| c.shown));
        assert!(!l.select(&mut cur, 2) && !l.select(&mut cur, 9));
        l.add_new(&mut cur, 1, [0.0, 1.0, 0.0]);
        assert_eq!(order(&cur, &l), ["a", "b", "c", "*"]);
        assert!(
            cur.as_ref()
                .is_some_and(|c| c.placing && c.points().is_empty())
        );
        assert!(l.select(&mut cur, 1));
        let gone = l.delete(&mut cur).expect("deleted");
        assert_eq!(gone.name, "b");
        assert_eq!(order(&cur, &l), ["a", "*c", ""]);
        l.delete(&mut cur);
        l.delete(&mut cur);
        assert_eq!(order(&cur, &l), ["*a"]);
        l.delete(&mut cur);
        assert!(cur.is_none() && l.others.is_empty());
        assert!(l.delete(&mut cur).is_none());
    }

    /// A press in the name field being typed puts the caret where it lands (dropping a
    /// composition) and a drag selects; presses elsewhere, or with no name typed, are not
    /// the field's.
    #[test]
    fn the_name_field_takes_presses_and_drags() {
        let name = "Left coronary";
        let mut text = specular_gfx::Text::bundled_only();
        text.begin_frame(1.0);
        let block = text.layout(
            name,
            specular_gfx::TextStyle::new(14.0),
            None,
            specular_gfx::Align::Start,
        );
        let x0 = 108.0;
        let x_of = |b: usize| x0 + block.caret_x(b);
        let mut c = named("a");
        let field = specular_gfx::Rect::new(100.0, 10.0, 200.0, 24.0);
        c.placed = vec![(Control::CurveRenameField, field, true)];
        c.field_text = Some((block.clone(), x0));
        let mut l = Centrelines::default();
        assert!(!l.field_button(Some(&c), (x_of(4), 20.0), true, 1.0, false));
        l.rename = Some((
            specular_widgets::line_edit::LineEdit::new(name),
            "ご".into(),
        ));
        assert!(!l.field_button(Some(&c), (x_of(4), 60.0), true, 1.0, false));
        assert!(!l.field_drag(Some(&c), x_of(2)), "no press");
        assert!(l.field_button(Some(&c), (x_of(5), 20.0), true, 1.0, false));
        let (e, pre) = l.rename.as_ref().expect("typed");
        assert_eq!((e.caret(), pre.as_str()), (5, ""));
        assert!(l.field_drag(Some(&c), x_of(13)));
        assert_eq!(
            l.rename.as_ref().expect("typed").0.selected_text(),
            "coronary"
        );
        assert!(l.field_button(Some(&c), (x_of(13), 20.0), false, 1.1, false));
        assert!(!l.field_drag(Some(&c), x_of(0)));
    }

    /// Sidecars beside the volume (`<stem>-curve*.json`) give all their centrelines; other
    /// files, and dental arches, are left out.
    #[test]
    fn sidecars_beside_the_volume_are_read() {
        let dir =
            std::env::temp_dir().join(format!("specular-cpr-sidecars-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = named("LAD");
        a.kind = specular_render_volume::CprKind::Stretched;
        let b = named("");
        std::fs::write(
            dir.join("heart-curve.json"),
            curved::to_json_all(&[&a, &b], None),
        )
        .unwrap();
        let mut d = named("arch");
        d.dental = Some(curved::Dental::new(None));
        std::fs::write(
            dir.join("heart-curve-2.json"),
            curved::to_json_all(&[&d], None),
        )
        .unwrap();
        std::fs::write(
            dir.join("other-curve.json"),
            curved::to_json_all(&[&a], None),
        )
        .unwrap();
        let found = read_sidecars(&dir.join("heart.nii"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "LAD");
        assert_eq!(found[0].kind, specular_render_volume::CprKind::Stretched);
        let c = centreline_of(found[0].clone(), 1, [0.0, 1.0, 0.0]);
        assert!(!c.shown && !c.placing && c.spline().is_some());
    }
}
