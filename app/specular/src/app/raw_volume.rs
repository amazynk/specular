//! The raw volume dialog: how a headerless `.raw` / `.vol` file (no header beside it, no
//! format of its own) is laid out. Asked when such a file fails to open for want of a layout;
//! offers the layouts its size and name suggest ([`medvol::raw::guesses`]) and lets every
//! field be stepped or typed: width, height, slices, sample type, byte order, header offset,
//! pixel size and slice spacing. A line under the fields says whether the layout fits the
//! file; Open is off until it does. Open gives the layout to the volume reader and reopens
//! the file.
//!
//! The layout is remembered per file (by path and size) in `raw-volumes.tsv` in the data
//! folder, so the file opens straight away next time; a file whose size changed is asked
//! about again.

#[cfg(test)]
mod shots;
mod view;

use std::path::{Path, PathBuf};

use specular_decode::medvol::DataType;
use specular_decode::medvol::raw::{self, RawLayout};
use specular_gfx::{DrawList, Text};
use specular_i18n::{ByteUnits, I18n, t};
use specular_input::{Key, KeyCombo, NamedKey};
use specular_log::{UI, info, warn};
use specular_theme::Palette;
use specular_widgets::accesskit;

use self::view::{A11Y_BASE, DialogView, Drawn, Hit, Part, Row, RowView};
use super::App;
use crate::loader::ImageId;

/// Remembered layouts at most; the oldest go first.
const MAX_REMEMBERED: usize = 256;

const TYPES: [DataType; 10] = [
    DataType::U8,
    DataType::I8,
    DataType::U16,
    DataType::I16,
    DataType::U32,
    DataType::I32,
    DataType::U64,
    DataType::I64,
    DataType::F32,
    DataType::F64,
];

/// Spacings the arrows step through, in millimetres.
const SPACINGS: [f64; 13] = [
    0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0,
];

/// An open dialog.
pub(super) struct Dialog {
    pub file: PathBuf,
    /// The file's size in bytes.
    len: u64,
    guesses: Vec<RawLayout>,
    /// The guess shown, until a field is changed.
    guess: Option<usize>,
    layout: RawLayout,
    focus: Part,
    /// Digits being typed into the focused row.
    edit: Option<String>,
    drawn: Drawn,
}

#[derive(Default)]
pub(super) struct RawVolume {
    pub dialog: Option<Dialog>,
}

impl RawVolume {
    pub(super) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn view(&self) -> Option<DialogView> {
        self.dialog.as_ref().map(build_view)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&DialogView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }
}

impl Dialog {
    fn new(file: &Path, len: u64) -> Dialog {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let guesses = raw::guesses(&name, len);
        let layout = guesses.first().copied().unwrap_or(RawLayout {
            size: [1, 1, 1],
            dtype: DataType::U16,
            big_endian: false,
            offset: 0,
            spacing: [1.0; 3],
        });
        Dialog {
            file: file.to_path_buf(),
            len,
            guess: (!guesses.is_empty()).then_some(0),
            guesses,
            layout,
            focus: Part::Row(Row::Width),
            edit: None,
            drawn: Drawn::default(),
        }
    }

    fn fits(&self) -> bool {
        self.layout.fits(self.len)
    }

    /// Steps `row` one way (`by` is ±1, ±10 with Shift).
    fn step(&mut self, row: Row, by: i64) {
        self.commit();
        let l = &mut self.layout;
        let add = |x: usize| (x as i64 + by).clamp(1, 1 << 20) as usize;
        match row {
            Row::Guess => {
                let n = self.guesses.len();
                if n == 0 {
                    return;
                }
                let i = match self.guess {
                    Some(i) => (i as i64 + by.signum()).rem_euclid(n as i64) as usize,
                    None if by > 0 => 0,
                    None => n - 1,
                };
                // A guess keeps the spacing already set.
                let spacing = self.layout.spacing;
                self.layout = RawLayout {
                    spacing,
                    ..self.guesses[i]
                };
                self.guess = Some(i);
                return;
            }
            Row::Width => l.size[0] = add(l.size[0]),
            Row::Height => l.size[1] = add(l.size[1]),
            Row::Slices => l.size[2] = add(l.size[2]),
            Row::Type => {
                let i = TYPES.iter().position(|t| *t == l.dtype).unwrap_or(0) as i64;
                l.dtype = TYPES[(i + by.signum()).rem_euclid(TYPES.len() as i64) as usize];
            }
            Row::Order => l.big_endian = !l.big_endian,
            Row::Offset => {
                // By a sample, or a slice with Shift.
                let unit = if by.abs() >= 10 {
                    (l.size[0] * l.size[1] * l.dtype.size()) as i64
                } else {
                    l.dtype.size() as i64
                };
                l.offset = (l.offset as i64 + by.signum() * unit).clamp(0, self.len as i64) as u64;
            }
            Row::Pixel => {
                let s = step_spacing(l.spacing[0], by > 0);
                l.spacing[0] = s;
                l.spacing[1] = s;
            }
            Row::Spacing => l.spacing[2] = step_spacing(l.spacing[2], by > 0),
        }
        self.changed();
    }

    /// A field was changed: the layout is the person's own unless it is one of the guesses.
    fn changed(&mut self) {
        let l = self.layout;
        self.guess = self.guesses.iter().position(|g| {
            g.size == l.size
                && g.dtype == l.dtype
                && g.big_endian == l.big_endian
                && g.offset == l.offset
        });
    }

    /// Puts typed digits into the field they were typed in.
    fn commit(&mut self) {
        let (Some(s), Part::Row(row)) = (self.edit.take(), self.focus) else {
            return;
        };
        if set_field(&mut self.layout, row, &s) {
            self.changed();
        }
    }

    fn type_char(&mut self, ch: char) -> bool {
        let Part::Row(row) = self.focus else {
            return false;
        };
        if !row.typed() {
            return false;
        }
        let decimal = matches!(row, Row::Pixel | Row::Spacing);
        // Full-width digits and marks (an East Asian input method's) count as typed.
        let ch = match ch {
            '０'..='９' => char::from(b'0' + (ch as u32 - '０' as u32) as u8),
            ',' | '．' | '，' | '。' => '.',
            c => c,
        };
        let ok = ch.is_ascii_digit() || (decimal && ch == '.');
        if !ok {
            return false;
        }
        let e = self.edit.get_or_insert_with(String::new);
        if e.len() < 16 && !(ch == '.' && e.contains('.')) {
            e.push(ch);
        }
        true
    }

    fn focus(&mut self, p: Part) {
        if self.focus != p {
            self.commit();
            self.focus = p;
        }
    }
}

/// Sets `row` from typed text; false when it is not a value the row takes.
fn set_field(l: &mut RawLayout, row: Row, s: &str) -> bool {
    let int = || s.parse::<u64>().ok();
    let mm = || {
        s.parse::<f64>()
            .ok()
            .filter(|x| x.is_finite() && *x > 0.0 && *x < 1e6)
    };
    match row {
        Row::Width | Row::Height | Row::Slices => {
            let Some(n) = int().filter(|n| (1..=1 << 20).contains(n)) else {
                return false;
            };
            let a = match row {
                Row::Width => 0,
                Row::Height => 1,
                _ => 2,
            };
            l.size[a] = n as usize;
        }
        Row::Offset => {
            let Some(n) = int() else { return false };
            l.offset = n;
        }
        Row::Pixel => {
            let Some(x) = mm() else { return false };
            l.spacing[0] = x;
            l.spacing[1] = x;
        }
        Row::Spacing => {
            let Some(x) = mm() else { return false };
            l.spacing[2] = x;
        }
        Row::Guess | Row::Type | Row::Order => return false,
    }
    true
}

/// The next spacing in [`SPACINGS`] above or below `x`.
fn step_spacing(x: f64, up: bool) -> f64 {
    if up {
        SPACINGS
            .iter()
            .copied()
            .find(|s| *s > x * (1.0 + 1e-9))
            .unwrap_or(x)
    } else {
        SPACINGS
            .iter()
            .rev()
            .copied()
            .find(|s| *s < x * (1.0 - 1e-9))
            .unwrap_or(x)
    }
}

fn type_name(t: DataType) -> String {
    let bits = t.size() * 8;
    match t {
        DataType::F32 | DataType::F64 => t!("raw-volume-type-float", bits = bits as i64),
        DataType::I8 | DataType::I16 | DataType::I32 | DataType::I64 => {
            t!("raw-volume-type-signed", bits = bits as i64)
        }
        _ => t!("raw-volume-type-unsigned", bits = bits as i64),
    }
}

fn build_view(d: &Dialog) -> DialogView {
    let loc = I18n::global().localizer();
    let l = &d.layout;
    let name = d.file.file_name().map_or_else(
        || d.file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mm = |x: f64| t!("raw-volume-mm", value = loc.number(x, 4));
    let rows = Row::ALL
        .iter()
        .map(|&row| {
            let (label, value) = match row {
                Row::Guess => (
                    t!("raw-volume-guess-label"),
                    match d.guess {
                        Some(i) => t!(
                            "raw-volume-guess",
                            index = (i + 1) as i64,
                            count = d.guesses.len() as i64
                        ),
                        None => t!("raw-volume-custom"),
                    },
                ),
                Row::Width => (t!("raw-volume-width"), loc.integer_plain(l.size[0] as i64)),
                Row::Height => (t!("raw-volume-height"), loc.integer_plain(l.size[1] as i64)),
                Row::Slices => (t!("raw-volume-slices"), loc.integer_plain(l.size[2] as i64)),
                Row::Type => (t!("raw-volume-type"), type_name(l.dtype)),
                Row::Order => (
                    t!("raw-volume-order"),
                    if l.big_endian {
                        t!("raw-volume-big")
                    } else {
                        t!("raw-volume-little")
                    },
                ),
                Row::Offset => (
                    t!("raw-volume-offset"),
                    t!("raw-volume-bytes", count = l.offset as i64),
                ),
                Row::Pixel => (t!("raw-volume-pixel"), mm(l.spacing[0])),
                Row::Spacing => (t!("raw-volume-spacing"), mm(l.spacing[2])),
            };
            // Digits being typed show as typed.
            let value = match (&d.edit, d.focus) {
                (Some(e), Part::Row(r)) if r == row => format!("{e}▏"),
                _ => value,
            };
            RowView { row, label, value }
        })
        .collect();
    let need = l.bytes().map_or(u64::MAX, |b| b.saturating_add(l.offset));
    let size = |b: u64| loc.bytes(b, ByteUnits::Binary);
    let status = if need == d.len {
        t!("raw-volume-exact", size = size(d.len))
    } else if d.fits() {
        t!(
            "raw-volume-fits",
            size = size(need),
            file = size(d.len),
            left = size(d.len - need)
        )
    } else {
        t!(
            "raw-volume-too-big",
            size = if need == u64::MAX {
                "∞".to_owned()
            } else {
                size(need)
            },
            file = size(d.len)
        )
    };
    DialogView {
        title: t!("raw-volume-title"),
        subtitle: t!("raw-volume-file", file = name),
        rows,
        status,
        bad: !d.fits(),
        cancel: t!("raw-volume-cancel"),
        open: t!("raw-volume-open"),
        focus: d.focus,
    }
}

/// Where remembered layouts are kept.
fn store_path() -> PathBuf {
    specular_settings::Locations::detect()
        .data_dir
        .join("raw-volumes.tsv")
}

/// One remembered layout: path, size in bytes, then the layout.
fn format_line(path: &Path, len: u64, l: &RawLayout) -> Option<String> {
    let p = path.to_str()?;
    if p.contains(['\t', '\n', '\r']) {
        return None;
    }
    Some(format!(
        "{p}\t{len}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        l.size[0],
        l.size[1],
        l.size[2],
        l.dtype.name(),
        u8::from(l.big_endian),
        l.offset,
        l.spacing[0],
        l.spacing[1],
        l.spacing[2]
    ))
}

fn parse_line(line: &str) -> Option<(PathBuf, u64, RawLayout)> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() != 11 {
        return None;
    }
    let n = |i: usize| f[i].parse::<usize>().ok().filter(|n| *n > 0);
    let x = |i: usize| {
        f[i].parse::<f64>()
            .ok()
            .filter(|x| x.is_finite() && *x > 0.0)
    };
    let dtype = TYPES.iter().copied().find(|t| t.name() == f[5])?;
    Some((
        PathBuf::from(f[0]),
        f[1].parse().ok()?,
        RawLayout {
            size: [n(2)?, n(3)?, n(4)?],
            dtype,
            big_endian: f[6] == "1",
            offset: f[7].parse().ok()?,
            spacing: [x(8)?, x(9)?, x(10)?],
        },
    ))
}

/// The layout remembered for `path` at `len` bytes, from the store's `text`.
fn find(text: &str, path: &Path, len: u64) -> Option<RawLayout> {
    text.lines()
        .filter_map(parse_line)
        .filter(|(p, n, _)| p == path && *n == len)
        .map(|x| x.2)
        .next_back()
}

/// The store's text with `path`'s layout put last (replacing any it had), the oldest
/// dropped past [`MAX_REMEMBERED`].
fn with_entry(text: &str, path: &Path, len: u64, l: &RawLayout) -> Option<String> {
    let line = format_line(path, len, l)?;
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|s| parse_line(s).is_some_and(|(p, ..)| p != path))
        .collect();
    lines.push(&line);
    let skip = lines.len().saturating_sub(MAX_REMEMBERED);
    let mut out = lines[skip..].join("\n");
    out.push('\n');
    Some(out)
}

impl App {
    /// An image failed to open: when it is a headerless volume wanting a layout, opens it
    /// with the one remembered for it, or asks for one.
    pub(super) fn raw_volume_failed(&mut self, id: ImageId) {
        if self.current_id() != Some(id) || self.raw_volume.is_open() {
            return;
        }
        let Some(path) = self.images.get(&id).map(|i| i.path.clone()) else {
            return;
        };
        use specular_decode::{Format, ident};
        // Data chosen as raw samples ("Which kind of file is …?") wants a layout too, and
        // any other name with a layout remembered for it was chosen so before.
        let chosen = ident::chosen(&path) == Some(Format::RawVolume);
        let other_name = !raw::is_raw_name(&path) && super::file_kind::headerless(&path);
        if raw::layout_of(&path).is_some() || !(raw::needs_layout(&path) || other_name) {
            return;
        }
        let Ok(len) = std::fs::metadata(&path).map(|m| m.len()) else {
            return;
        };
        let text = std::fs::read_to_string(store_path()).unwrap_or_default();
        if let Some(l) = find(&text, &path, len).filter(|l| l.fits(len)) {
            info!(target: UI, "raw volume {}: remembered layout", path.display());
            if other_name {
                ident::choose(&path, Format::RawVolume);
            }
            raw::remember(&path, l);
            self.reload_path(&path);
            return;
        }
        if other_name && !chosen {
            return;
        }
        self.raw_volume_open(&path, len);
    }

    pub(super) fn raw_volume_open(&mut self, path: &Path, len: u64) {
        self.raw_volume.dialog = Some(Dialog::new(path, len));
        self.redraw = true;
    }

    pub(super) fn raw_volume_view(&self) -> Option<DialogView> {
        self.raw_volume.view()
    }

    /// Keys while the dialog is open (it takes them all); `text` is what the key types.
    pub(super) fn raw_volume_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        let rtl = self.rtl();
        let Some(d) = self.raw_volume.dialog.as_mut() else {
            return;
        };
        self.redraw = true;
        let order = Part::order();
        let Some(c) = combo else {
            if let Some(t) = text {
                for ch in t.chars() {
                    d.type_char(ch);
                }
            }
            return;
        };
        let move_focus = |d: &mut Dialog, by: isize| {
            let i = order.iter().position(|p| *p == d.focus).unwrap_or(0) as isize;
            d.focus(order[(i + by).rem_euclid(order.len() as isize) as usize]);
        };
        let by = if c.mods.shift { 10 } else { 1 };
        match c.key {
            Key::Named(NamedKey::Escape) if d.edit.is_some() => d.edit = None,
            Key::Named(NamedKey::Escape) => self.raw_volume_cancel(),
            Key::Named(NamedKey::Enter) => match d.focus {
                Part::Cancel => self.raw_volume_cancel(),
                _ => self.raw_volume_submit(),
            },
            Key::Named(NamedKey::Space) if d.focus == Part::Cancel => self.raw_volume_cancel(),
            Key::Named(NamedKey::Space) if d.focus == Part::Open => self.raw_volume_submit(),
            Key::Named(NamedKey::Tab) => move_focus(d, if c.mods.shift { -1 } else { 1 }),
            Key::Named(NamedKey::ArrowUp) => move_focus(d, -1),
            Key::Named(NamedKey::ArrowDown) => move_focus(d, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match d.focus {
                    Part::Row(row) => d.step(row, if forward { by } else { -by }),
                    _ => d.focus(if forward { Part::Open } else { Part::Cancel }),
                }
            }
            Key::Named(NamedKey::PageUp) | Key::Named(NamedKey::PageDown) => {
                if let Part::Row(row) = d.focus {
                    let up = c.key == Key::Named(NamedKey::PageUp);
                    d.step(row, if up { 10 } else { -10 });
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(e) = d.edit.as_mut() {
                    e.pop();
                } else if let Part::Row(row) = d.focus
                    && row.typed()
                {
                    // Starts typing afresh.
                    d.edit = Some(String::new());
                }
            }
            Key::Char(ch) if !(c.mods.ctrl || c.mods.meta) => {
                d.type_char(ch);
            }
            _ => {
                if let Some(t) = text {
                    for ch in t.chars() {
                        d.type_char(ch);
                    }
                }
            }
        }
    }

    pub(super) fn raw_volume_click(&mut self, p: (f32, f32), down: bool) {
        if !down {
            return;
        }
        let Some(d) = self.raw_volume.dialog.as_mut() else {
            return;
        };
        let hit = d
            .drawn
            .hits
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return };
        self.redraw = true;
        match h {
            Hit::Dec(row) => {
                d.focus(Part::Row(row));
                d.step(row, -1);
            }
            Hit::Inc(row) => {
                d.focus(Part::Row(row));
                d.step(row, 1);
            }
            Hit::Part(Part::Cancel) => self.raw_volume_cancel(),
            Hit::Part(Part::Open) => self.raw_volume_submit(),
            Hit::Part(p) => d.focus(p),
        }
    }

    /// The wheel over the dialog steps the focused row.
    pub(super) fn raw_volume_wheel(&mut self, up: bool) {
        if let Some(d) = self.raw_volume.dialog.as_mut()
            && let Part::Row(row) = d.focus
        {
            d.step(row, if up { 1 } else { -1 });
            self.redraw = true;
        }
    }

    pub(super) fn raw_volume_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(d) = self.raw_volume.dialog.as_mut() else {
            return false;
        };
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return (A11Y_BASE..A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        self.redraw = true;
        match (req.action, part) {
            (accesskit::Action::Increment, Part::Row(row)) => {
                d.focus(part);
                d.step(row, 1);
            }
            (accesskit::Action::Decrement, Part::Row(row)) => {
                d.focus(part);
                d.step(row, -1);
            }
            (accesskit::Action::SetValue, Part::Row(row)) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data {
                    d.focus(part);
                    if set_field(&mut d.layout, row, s.trim()) {
                        d.changed();
                    }
                }
            }
            (accesskit::Action::Click, Part::Cancel) => self.raw_volume_cancel(),
            (accesskit::Action::Click, Part::Open) => self.raw_volume_submit(),
            (accesskit::Action::Focus | accesskit::Action::Click, p) => d.focus(p),
            _ => {}
        }
        true
    }

    /// Open: gives the layout to the reader, remembers it and reopens the file.
    fn raw_volume_submit(&mut self) {
        let Some(d) = self.raw_volume.dialog.as_mut() else {
            return;
        };
        d.commit();
        if !d.fits() {
            return;
        }
        let Some(d) = self.raw_volume.dialog.take() else {
            return;
        };
        self.redraw = true;
        raw::remember(&d.file, d.layout);
        let store = store_path();
        let text = std::fs::read_to_string(&store).unwrap_or_default();
        if let Some(new) = with_entry(&text, &d.file, d.len, &d.layout)
            && let Err(e) = specular_settings::write_atomic(&store, &new)
        {
            warn!(target: UI, "raw volume layouts: {e}");
        }
        self.reload_path(&d.file);
    }

    /// Cancel: the file stays shown as not opened.
    fn raw_volume_cancel(&mut self) {
        self.raw_volume.dialog = None;
        self.redraw = true;
    }

    /// `--show raw-volume`: the dialog for a 64 MiB `part.raw`. False for any other scene.
    pub(super) fn raw_volume_show(&mut self, scene: &str) -> bool {
        if scene != "raw-volume" {
            return false;
        }
        self.raw_volume_open(Path::new("part_512x512x128.raw"), 512 * 512 * 128 * 2);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> RawLayout {
        RawLayout {
            size: [64, 32, 10],
            dtype: DataType::U16,
            big_endian: true,
            offset: 512,
            spacing: [0.05, 0.05, 0.1],
        }
    }

    /// Numbers use tabular figures; the words of the type and byte-order rows don't, since
    /// a tabular hyphen spaced out "16-bit" and "Little-endian" in a live run.
    #[test]
    fn only_number_rows_use_tabular_figures() {
        let words: Vec<Row> = Row::ALL.into_iter().filter(|r| !r.figures()).collect();
        assert_eq!(words, [Row::Type, Row::Order]);
        assert!(Row::ALL.into_iter().filter(|r| r.typed()).all(Row::figures));
    }

    #[test]
    fn layouts_round_trip_and_stay_bounded() {
        let p = Path::new("/scans/part.raw");
        let line = format_line(p, 41472, &layout()).unwrap();
        let (q, n, l) = parse_line(&line).unwrap();
        assert_eq!((q.as_path(), n, l), (p, 41472, layout()));
        assert!(format_line(Path::new("/a\tb.raw"), 1, &layout()).is_none());
        assert!(parse_line("garbage").is_none());

        let mut text = String::new();
        for i in 0..MAX_REMEMBERED + 5 {
            text = with_entry(&text, Path::new(&format!("/s/{i}.raw")), 9, &layout()).unwrap();
        }
        assert_eq!(text.lines().count(), MAX_REMEMBERED);
        assert!(find(&text, Path::new("/s/0.raw"), 9).is_none());
        let last = format!("/s/{}.raw", MAX_REMEMBERED + 4);
        assert_eq!(find(&text, Path::new(&last), 9), Some(layout()));
        // A file whose size changed is asked about again.
        assert!(find(&text, Path::new(&last), 10).is_none());
        // Saving again replaces the entry.
        let mut other = layout();
        other.offset = 0;
        let text = with_entry(&text, Path::new(&last), 9, &other).unwrap();
        assert_eq!(text.lines().count(), MAX_REMEMBERED);
        assert_eq!(find(&text, Path::new(&last), 9), Some(other));
    }

    /// What an input method commits (full-width digits and marks) types like the keys.
    #[test]
    fn full_width_digits_type() {
        let mut d = Dialog::new(Path::new("ct_100x80x20.raw"), 100 * 80 * 20 * 2);
        d.focus(Part::Row(Row::Width));
        d.edit = Some(String::new());
        for ch in "１２０".chars() {
            assert!(d.type_char(ch));
        }
        assert!(!d.type_char('ア'));
        d.focus(Part::Row(Row::Pixel));
        assert_eq!(d.layout.size[0], 120);
        d.edit = Some(String::new());
        for ch in "０．２５".chars() {
            d.type_char(ch);
        }
        d.commit();
        assert_eq!(d.layout.spacing[0], 0.25);
    }

    #[test]
    fn fields_step_type_and_guess() {
        let mut d = Dialog::new(Path::new("ct_100x80x20.raw"), 100 * 80 * 20 * 2);
        assert_eq!(d.guess, Some(0));
        assert_eq!(d.layout.size, [100, 80, 20]);
        assert!(d.fits());
        d.step(Row::Width, 1);
        assert_eq!(d.layout.size[0], 101);
        assert_eq!(d.guess, None);
        assert!(!d.fits());
        d.focus(Part::Row(Row::Width));
        d.edit = Some(String::new());
        for ch in "1x00".chars() {
            d.type_char(ch);
        }
        d.focus(Part::Row(Row::Pixel));
        assert_eq!(d.layout.size[0], 100);
        assert_eq!(d.guess, Some(0));
        for ch in "0,03".chars() {
            d.type_char(ch);
        }
        d.commit();
        assert_eq!(d.layout.spacing, [0.03, 0.03, 1.0]);
        d.step(Row::Pixel, 1);
        assert_eq!(d.layout.spacing[0], 0.05);
        d.step(Row::Spacing, -1);
        assert_eq!(d.layout.spacing[2], 0.5);
        d.step(Row::Type, 1);
        assert_eq!(d.layout.dtype, DataType::I16);
        d.step(Row::Type, -1);
        d.step(Row::Order, 1);
        assert!(d.layout.big_endian);
        d.step(Row::Order, 1);
        // Stepping the guesses keeps the spacing.
        d.step(Row::Guess, 1);
        assert_eq!(d.layout.spacing[0], 0.05);
        assert!(d.fits());
    }

    #[test]
    fn the_status_says_whether_it_fits() {
        let mut d = Dialog::new(Path::new("x.raw"), 4096 + 100);
        d.layout = RawLayout {
            size: [32, 32, 2],
            dtype: DataType::U16,
            big_endian: false,
            offset: 0,
            spacing: [1.0; 3],
        };
        assert!(!build_view(&d).bad);
        d.layout.offset = 101;
        assert!(build_view(&d).bad);
    }
}
