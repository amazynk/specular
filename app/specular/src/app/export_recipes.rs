//! Export recipes (palette "Export with recipes…", in the gallery, the Library, the viewer and
//! Develop): saved ways to export, several run at once on the selection.
//!
//! The sheet lists the recipes with a check box each; New, Edit, Duplicate and Delete (which
//! asks first) change them, and they are kept in `export-recipes.txt` in the settings
//! folder (the shipped starters, web, print and email, until the user changes them). Export
//! asks for the folders of recipes that ask, settles every name on a background task, asks
//! before replacing files, then runs [`specular_convert::recipe::execute`] with progress and
//! Stop. Photos with a Develop edit are rendered through it; others are decoded as they are.
//! At the end it lists what failed, and opens the folders of recipes that say so.

pub(super) mod view;

#[cfg(test)]
mod shots;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use specular_convert::recipe::{
    self, After, Bits, Collision, Destination, Item, Keep, MarkKind, Medium, Plan, PlanError,
    Position, Recipe, RecipeReport, Size, Space, Strength,
};
use specular_convert::{Format, Outcome, Pattern, Progress};
use specular_core::CancelToken;
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use super::App;
use super::convert::{Source, eta_text, failure_text, file_name, pattern_error, source_label};
use super::integration::PasteTo;
use view::{Drawn, Field, FieldView, Hit, PhaseView, RecipeView, Row, RowView, View};

const FILE: &str = "export-recipes.txt";
/// Failures listed at the end before "and N more".
const MAX_FAILURES: usize = 8;
/// Files named in the replace question before "and N more".
const MAX_REPLACED: usize = 5;
/// Folders opened at the end at most.
const MAX_OPEN: usize = 3;
/// Pixel sizes the edge, width and height step through.
const SIZES: [u32; 27] = [
    256, 320, 480, 512, 640, 720, 800, 960, 1024, 1080, 1200, 1280, 1350, 1440, 1600, 1920, 2048,
    2160, 2400, 2560, 3000, 3200, 3840, 4096, 5120, 6000, 8192,
];
/// Megapixels (tenths) the megapixel row steps through.
const TENTHS: [u32; 14] = [5, 10, 15, 20, 24, 30, 40, 50, 60, 80, 100, 120, 160, 240];

// ----- Storage ----------------------------------------------------------------------------

fn recipes_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(specular_settings::Locations::detect().config_dir.join(FILE))
}

/// The user's recipes; the starters when there is no file yet.
pub(super) fn load_recipes() -> Vec<Recipe> {
    match recipes_file().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(text) => Recipe::parse_all(&text),
        None => Recipe::starters(),
    }
}

/// Writes the recipes (atomically).
fn save_recipes(all: &[Recipe]) {
    let Some(path) = recipes_file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("txt.tmp");
    if std::fs::write(&tmp, Recipe::write_all(all)).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

// ----- Words ------------------------------------------------------------------------------

/// The name a recipe shows (a starter's in the user's language while unnamed).
pub(super) fn recipe_name(r: &Recipe) -> String {
    if !r.name.trim().is_empty() {
        return r.name.clone();
    }
    match r.id.as_str() {
        "web" => t!("export-recipes-starter-web"),
        "print" => t!("export-recipes-starter-print"),
        "email" => t!("export-recipes-starter-email"),
        _ => t!("export-recipes-unnamed"),
    }
}

fn format_name(f: Format) -> String {
    match f {
        Format::Jpeg => "JPEG",
        Format::Png => "PNG",
        Format::Tiff => "TIFF",
        Format::Webp => "WebP",
        Format::Avif => "AVIF",
        Format::Jxl => "JPEG XL",
    }
    .to_owned()
}

fn space_name(s: Space) -> String {
    match s {
        Space::Srgb => t!("export-recipes-space-srgb"),
        Space::DisplayP3 => t!("export-recipes-space-p3"),
        Space::AdobeRgb => t!("export-recipes-space-adobe"),
        Space::ProPhoto => t!("export-recipes-space-prophoto"),
    }
}

fn yes_no(b: bool) -> String {
    if b {
        t!("export-recipes-yes")
    } else {
        t!("export-recipes-no")
    }
}

fn size_text(s: Size) -> String {
    match s {
        Size::Full => t!("export-recipes-size-full"),
        Size::LongEdge(n) => t!("export-recipes-size-long", px = n.to_string()),
        Size::ShortEdge(n) => t!("export-recipes-size-short", px = n.to_string()),
        Size::Megapixels(t) => t!(
            "export-recipes-size-mp",
            mp = format!("{}.{}", t / 10, t % 10)
        ),
        Size::Exact { width, height } => t!(
            "export-recipes-size-exact",
            width = width.to_string(),
            height = height.to_string()
        ),
        Size::Percent(p) => t!("export-recipes-size-percent", percent = i64::from(p)),
    }
}

fn folder_text(d: &Destination) -> String {
    match d {
        Destination::Same => t!("export-recipes-folder-same"),
        Destination::Subfolder(n) => t!("export-recipes-folder-sub", name = n.clone()),
        Destination::Fixed(p) => file_name(p),
        Destination::Ask => t!("export-recipes-folder-ask"),
    }
}

/// One line about a recipe for the list: format, size, space and folder.
pub(super) fn summary(r: &Recipe) -> String {
    let fmt = if recipe::lossy(r.format) && !r.lossless {
        format!("{} {}", format_name(r.format), r.quality)
    } else if r.bits == Bits::Sixteen && r.format.sixteen() {
        format!("{} 16-bit", format_name(r.format))
    } else {
        format_name(r.format)
    };
    let mut parts = vec![fmt, size_text(r.size), space_name(r.space)];
    if r.watermark.kind != MarkKind::None {
        parts.push(t!("export-recipes-summary-mark"));
    }
    parts.push(folder_text(&r.destination));
    parts.join(" · ")
}

fn row_label(row: Row) -> String {
    match row {
        Row::Images => t!("convert-images"),
        Row::Format => t!("export-recipes-format"),
        Row::Quality => t!("export-recipes-quality"),
        Row::Lossless => t!("export-recipes-lossless"),
        Row::Bits => t!("export-recipes-bits"),
        Row::Space => t!("export-recipes-space"),
        Row::Profile => t!("export-recipes-profile"),
        Row::Folder => t!("export-recipes-folder"),
        Row::Collision => t!("export-recipes-collision"),
        Row::After => t!("export-recipes-after"),
        Row::Resize => t!("export-recipes-resize"),
        Row::Edge => t!("export-recipes-edge"),
        Row::Width => t!("export-recipes-width"),
        Row::Height => t!("export-recipes-height"),
        Row::Megapixels => t!("export-recipes-megapixels"),
        Row::Percent => t!("export-recipes-percent"),
        Row::Enlarge => t!("export-recipes-enlarge"),
        Row::Sharpen => t!("export-recipes-sharpen"),
        Row::Amount => t!("export-recipes-amount"),
        Row::Metadata => t!("export-recipes-metadata"),
        Row::Location => t!("export-recipes-location"),
        Row::People => t!("export-recipes-people"),
        Row::Mark => t!("export-recipes-mark"),
        Row::Position => t!("export-recipes-position"),
        Row::Opacity => t!("export-recipes-opacity"),
        Row::Scale => t!("export-recipes-scale"),
    }
}

fn position_name(p: Position) -> String {
    match p {
        Position::TopLeft => t!("export-recipes-pos-top-left"),
        Position::Top => t!("export-recipes-pos-top"),
        Position::TopRight => t!("export-recipes-pos-top-right"),
        Position::Left => t!("export-recipes-pos-left"),
        Position::Centre => t!("export-recipes-pos-centre"),
        Position::Right => t!("export-recipes-pos-right"),
        Position::BottomLeft => t!("export-recipes-pos-bottom-left"),
        Position::Bottom => t!("export-recipes-pos-bottom"),
        Position::BottomRight => t!("export-recipes-pos-bottom-right"),
    }
}

fn row_value(r: &Recipe, row: Row) -> String {
    match row {
        Row::Images => String::new(),
        Row::Format => format_name(r.format),
        Row::Quality => r.quality.to_string(),
        Row::Lossless => yes_no(r.lossless),
        Row::Bits => match r.bits {
            Bits::Eight => t!("export-recipes-bits-8"),
            Bits::Sixteen => t!("export-recipes-bits-16"),
        },
        Row::Space => space_name(r.space),
        Row::Profile => {
            if r.embed_icc {
                t!("export-recipes-profile-embed")
            } else {
                t!("export-recipes-profile-none")
            }
        }
        Row::Folder => match &r.destination {
            Destination::Same => t!("export-recipes-folder-same"),
            Destination::Subfolder(_) => t!("export-recipes-folder-subfolder"),
            Destination::Fixed(p) => file_name(p),
            Destination::Ask => t!("export-recipes-folder-ask"),
        },
        Row::Collision => match r.collision {
            Collision::Unique => t!("export-recipes-collision-unique"),
            Collision::Overwrite => t!("export-recipes-collision-overwrite"),
            Collision::Skip => t!("export-recipes-collision-skip"),
        },
        Row::After => match r.after {
            After::Nothing => t!("export-recipes-after-nothing"),
            After::OpenFolder => t!("export-recipes-after-open"),
        },
        Row::Resize => match r.size {
            Size::Full => t!("export-recipes-size-full"),
            Size::LongEdge(_) => t!("export-recipes-resize-long"),
            Size::ShortEdge(_) => t!("export-recipes-resize-short"),
            Size::Megapixels(_) => t!("export-recipes-resize-mp"),
            Size::Exact { .. } => t!("export-recipes-resize-exact"),
            Size::Percent(_) => t!("export-recipes-resize-percent"),
        },
        Row::Edge => match r.size {
            Size::LongEdge(n) | Size::ShortEdge(n) => t!("export-recipes-px", px = n.to_string()),
            _ => String::new(),
        },
        Row::Width | Row::Height => match r.size {
            Size::Exact { width, height } => t!(
                "export-recipes-px",
                px = (if row == Row::Width { width } else { height }).to_string()
            ),
            _ => String::new(),
        },
        Row::Megapixels => match r.size {
            Size::Megapixels(t) => t!("export-recipes-mp", mp = format!("{}.{}", t / 10, t % 10)),
            _ => String::new(),
        },
        Row::Percent => match r.size {
            Size::Percent(p) => t!("export-recipes-pct", percent = i64::from(p)),
            _ => String::new(),
        },
        Row::Enlarge => yes_no(r.enlarge),
        Row::Sharpen => match r.sharpen.medium {
            Medium::Off => t!("export-recipes-sharpen-off"),
            Medium::Screen => t!("export-recipes-sharpen-screen"),
            Medium::Matte => t!("export-recipes-sharpen-matte"),
            Medium::Glossy => t!("export-recipes-sharpen-glossy"),
        },
        Row::Amount => match r.sharpen.strength {
            Strength::Low => t!("export-recipes-amount-low"),
            Strength::Standard => t!("export-recipes-amount-standard"),
            Strength::High => t!("export-recipes-amount-high"),
        },
        Row::Metadata => match r.metadata.keep {
            Keep::All => t!("export-recipes-meta-all"),
            Keep::Copyright => t!("export-recipes-meta-copyright"),
            Keep::CopyrightContact => t!("export-recipes-meta-contact"),
            Keep::None => t!("export-recipes-meta-none"),
        },
        Row::Location | Row::People => {
            let strip = if row == Row::Location {
                r.metadata.strip_gps
            } else {
                r.metadata.strip_people
            };
            if strip {
                t!("export-recipes-remove")
            } else {
                t!("export-recipes-keep")
            }
        }
        Row::Mark => match &r.watermark.kind {
            MarkKind::None => t!("export-recipes-mark-none"),
            MarkKind::Text(_) => t!("export-recipes-mark-text"),
            MarkKind::Image(p) if p.as_os_str().is_empty() => t!("export-recipes-mark-image"),
            MarkKind::Image(p) => file_name(p),
        },
        Row::Position => position_name(r.watermark.position),
        Row::Opacity => t!(
            "export-recipes-pct",
            percent = i64::from(r.watermark.opacity)
        ),
        Row::Scale => t!("export-recipes-pct", percent = i64::from(r.watermark.scale)),
    }
}

fn row_views(r: &Recipe, rows: &[Row]) -> Vec<RowView> {
    rows.iter()
        .map(|&row| RowView {
            row,
            label: row_label(row),
            value: row_value(r, row),
        })
        .collect()
}

/// The editor's three columns of rows for `r` (rows come and go with the choices).
pub(super) fn columns(r: &Recipe) -> [Vec<Row>; 3] {
    let mut a = vec![Row::Format];
    if recipe::lossy(r.format) && !r.lossless {
        a.push(Row::Quality);
    }
    if recipe::has_lossless(r.format) {
        a.push(Row::Lossless);
    }
    if r.format.sixteen() {
        a.push(Row::Bits);
    }
    a.extend([
        Row::Space,
        Row::Profile,
        Row::Folder,
        Row::Collision,
        Row::After,
    ]);
    let mut b = vec![Row::Resize];
    match r.size {
        Size::Full => {}
        Size::LongEdge(_) | Size::ShortEdge(_) => b.push(Row::Edge),
        Size::Exact { .. } => b.extend([Row::Width, Row::Height]),
        Size::Megapixels(_) => b.push(Row::Megapixels),
        Size::Percent(_) => b.push(Row::Percent),
    }
    if r.size != Size::Full {
        b.push(Row::Enlarge);
    }
    b.push(Row::Sharpen);
    if r.sharpen.medium != Medium::Off {
        b.push(Row::Amount);
    }
    let mut c = vec![Row::Metadata];
    if r.metadata.keep == Keep::All {
        c.extend([Row::Location, Row::People]);
    }
    c.push(Row::Mark);
    if r.watermark.kind != MarkKind::None {
        c.extend([Row::Position, Row::Opacity, Row::Scale]);
    }
    [a, b, c]
}

fn cycle<T: Copy + PartialEq>(all: &[T], v: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == v).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

/// The next value of a list of steps from `v` (the nearest step when `v` is between).
fn step_in(steps: &[u32], v: u32, d: i32) -> u32 {
    if d > 0 {
        steps.iter().copied().find(|&s| s > v).unwrap_or(v)
    } else {
        steps.iter().rev().copied().find(|&s| s < v).unwrap_or(v)
    }
}

// ----- The editor -------------------------------------------------------------------------

/// Sizes kept while the resize kind changes.
#[derive(Clone, Copy, Debug)]
struct Sizes {
    edge: u32,
    width: u32,
    height: u32,
    tenths: u32,
    percent: u32,
}

/// A recipe being edited.
pub(super) struct Editor {
    /// The recipe it changes; `None` for a new one.
    pub index: Option<usize>,
    pub r: Recipe,
    pub name: LineEdit,
    pub pattern: LineEdit,
    pub text: LineEdit,
    pub mark: LineEdit,
    pub sub: LineEdit,
    sizes: Sizes,
    /// Subfolder name and mark text kept while another kind is chosen.
    last_sub: String,
    last_mark: String,
}

impl Editor {
    pub fn new(index: Option<usize>, r: Recipe) -> Editor {
        let mut sizes = Sizes {
            edge: 2048,
            width: 1080,
            height: 1350,
            tenths: 24,
            percent: 50,
        };
        match r.size {
            Size::LongEdge(n) | Size::ShortEdge(n) => sizes.edge = n,
            Size::Exact { width, height } => {
                sizes.width = width;
                sizes.height = height;
            }
            Size::Megapixels(t) => sizes.tenths = t,
            Size::Percent(p) => sizes.percent = p,
            Size::Full => {}
        }
        let sub = match &r.destination {
            Destination::Subfolder(s) => s.clone(),
            _ => "export".to_owned(),
        };
        let mark = match &r.watermark.kind {
            MarkKind::Text(t) => t.clone(),
            _ => String::new(),
        };
        Editor {
            index,
            name: LineEdit::new(&recipe_name(&r)),
            pattern: LineEdit::new(&r.pattern),
            text: LineEdit::new(&r.text),
            mark: LineEdit::new(&mark),
            sub: LineEdit::new(&sub),
            r,
            sizes,
            last_sub: sub,
            last_mark: if mark.is_empty() {
                "©".to_owned()
            } else {
                mark
            },
        }
    }

    pub fn fields(&self) -> Vec<Field> {
        let mut v = vec![Field::Name, Field::Pattern, Field::Text];
        if matches!(self.r.watermark.kind, MarkKind::Text(_)) {
            v.push(Field::MarkText);
        }
        if matches!(self.r.destination, Destination::Subfolder(_)) {
            v.push(Field::Subfolder);
        }
        v
    }

    pub fn field(&self, f: Field) -> &LineEdit {
        match f {
            Field::Name => &self.name,
            Field::Pattern => &self.pattern,
            Field::Text => &self.text,
            Field::MarkText => &self.mark,
            Field::Subfolder => &self.sub,
        }
    }

    pub fn field_mut(&mut self, f: Field) -> &mut LineEdit {
        match f {
            Field::Name => &mut self.name,
            Field::Pattern => &mut self.pattern,
            Field::Text => &mut self.text,
            Field::MarkText => &mut self.mark,
            Field::Subfolder => &mut self.sub,
        }
    }

    /// The recipe with the fields' text in it.
    pub fn recipe(&self) -> Recipe {
        let mut r = self.r.clone();
        let name = self.name.text().trim().to_owned();
        // A starter keeps following the language while its name is left as it was.
        let starter = Recipe {
            name: String::new(),
            ..r.clone()
        };
        r.name = if !r.id.is_empty() && name == recipe_name(&starter) {
            String::new()
        } else {
            name
        };
        r.pattern = self.pattern.text().to_owned();
        r.text = self.text.text().to_owned();
        if let MarkKind::Text(_) = r.watermark.kind {
            r.watermark.kind = MarkKind::Text(self.mark.text().to_owned());
        }
        if let Destination::Subfolder(_) = r.destination {
            r.destination = Destination::Subfolder(self.sub.text().trim().to_owned());
        }
        r
    }

    /// What stops saving, if anything.
    pub fn problem(&self) -> Option<String> {
        if self.name.text().trim().is_empty() {
            return Some(t!("export-recipes-error-name"));
        }
        if let Err(e) = Pattern::parse(self.pattern.text()) {
            return Some(pattern_error(&e));
        }
        if matches!(self.r.watermark.kind, MarkKind::Text(_)) && self.mark.text().trim().is_empty()
        {
            return Some(t!("export-recipes-error-mark-text"));
        }
        if matches!(&self.r.watermark.kind, MarkKind::Image(p) if p.as_os_str().is_empty()) {
            return Some(t!("export-recipes-error-mark-image"));
        }
        if matches!(self.r.destination, Destination::Subfolder(_))
            && self.sub.text().trim().is_empty()
        {
            return Some(t!("export-recipes-error-subfolder"));
        }
        None
    }

    /// Steps `row` by `d`; true when something changed.
    pub fn step(&mut self, row: Row, d: i32) -> bool {
        let before = self.r.clone();
        let r = &mut self.r;
        let s = &mut self.sizes;
        match row {
            Row::Images => {}
            Row::Format => r.format = cycle(&recipe::FORMATS, r.format, d),
            Row::Quality => r.quality = (i32::from(r.quality) + 5 * d).clamp(5, 100) as u8,
            Row::Lossless => r.lossless = !r.lossless,
            Row::Bits => {
                r.bits = if r.bits == Bits::Eight {
                    Bits::Sixteen
                } else {
                    Bits::Eight
                }
            }
            Row::Space => r.space = cycle(&Space::ALL, r.space, d),
            Row::Profile => r.embed_icc = !r.embed_icc,
            Row::Folder => {
                let kinds = [0, 1, 2, 3];
                let k = match &r.destination {
                    Destination::Subfolder(_) => 0,
                    Destination::Same => 1,
                    Destination::Fixed(_) => 2,
                    Destination::Ask => 3,
                };
                if let Destination::Subfolder(n) = &r.destination {
                    self.last_sub = n.clone();
                }
                r.destination = match cycle(&kinds, k, d) {
                    0 => Destination::Subfolder(self.sub.text().to_owned()),
                    1 => Destination::Same,
                    2 => Destination::Fixed(PathBuf::new()),
                    _ => Destination::Ask,
                };
                if matches!(r.destination, Destination::Subfolder(_)) && self.sub.is_empty() {
                    self.sub.set_text(&self.last_sub);
                }
            }
            Row::Collision => {
                r.collision = cycle(
                    &[Collision::Unique, Collision::Overwrite, Collision::Skip],
                    r.collision,
                    d,
                )
            }
            Row::After => {
                r.after = if r.after == After::Nothing {
                    After::OpenFolder
                } else {
                    After::Nothing
                }
            }
            Row::Resize => {
                let kinds = [
                    Size::Full,
                    Size::LongEdge(s.edge),
                    Size::ShortEdge(s.edge),
                    Size::Megapixels(s.tenths),
                    Size::Exact {
                        width: s.width,
                        height: s.height,
                    },
                    Size::Percent(s.percent),
                ];
                let k = kinds
                    .iter()
                    .position(|x| std::mem::discriminant(x) == std::mem::discriminant(&r.size))
                    .unwrap_or(0) as i32;
                r.size = kinds[(k + d).rem_euclid(kinds.len() as i32) as usize];
            }
            Row::Edge => {
                s.edge = step_in(&SIZES, s.edge, d);
                r.size = match r.size {
                    Size::ShortEdge(_) => Size::ShortEdge(s.edge),
                    _ => Size::LongEdge(s.edge),
                };
            }
            Row::Width | Row::Height => {
                if row == Row::Width {
                    s.width = step_in(&SIZES, s.width, d);
                } else {
                    s.height = step_in(&SIZES, s.height, d);
                }
                r.size = Size::Exact {
                    width: s.width,
                    height: s.height,
                };
            }
            Row::Megapixels => {
                s.tenths = step_in(&TENTHS, s.tenths, d);
                r.size = Size::Megapixels(s.tenths);
            }
            Row::Percent => {
                s.percent = (s.percent as i32 + 5 * d).clamp(5, 400) as u32;
                r.size = Size::Percent(s.percent);
            }
            Row::Enlarge => r.enlarge = !r.enlarge,
            Row::Sharpen => {
                r.sharpen.medium = cycle(
                    &[Medium::Off, Medium::Screen, Medium::Matte, Medium::Glossy],
                    r.sharpen.medium,
                    d,
                )
            }
            Row::Amount => {
                r.sharpen.strength = cycle(
                    &[Strength::Low, Strength::Standard, Strength::High],
                    r.sharpen.strength,
                    d,
                )
            }
            Row::Metadata => {
                r.metadata.keep = cycle(
                    &[
                        Keep::All,
                        Keep::Copyright,
                        Keep::CopyrightContact,
                        Keep::None,
                    ],
                    r.metadata.keep,
                    d,
                )
            }
            Row::Location => r.metadata.strip_gps = !r.metadata.strip_gps,
            Row::People => r.metadata.strip_people = !r.metadata.strip_people,
            Row::Mark => {
                let k = match &r.watermark.kind {
                    MarkKind::None => 0,
                    MarkKind::Text(_) => 1,
                    MarkKind::Image(_) => 2,
                };
                if let MarkKind::Text(_) = r.watermark.kind
                    && !self.mark.is_empty()
                {
                    self.last_mark = self.mark.text().to_owned();
                }
                r.watermark.kind = match (k + d).rem_euclid(3) {
                    0 => MarkKind::None,
                    1 => MarkKind::Text(self.last_mark.clone()),
                    _ => MarkKind::Image(PathBuf::new()),
                };
                if let MarkKind::Text(t) = &r.watermark.kind
                    && self.mark.is_empty()
                {
                    self.mark.set_text(t);
                }
            }
            Row::Position => r.watermark.position = cycle(&Position::ALL, r.watermark.position, d),
            Row::Opacity => {
                r.watermark.opacity = (i32::from(r.watermark.opacity) + 5 * d).clamp(5, 100) as u8
            }
            Row::Scale => {
                r.watermark.scale = (i32::from(r.watermark.scale) + 5 * d).clamp(5, 100) as u8
            }
        }
        // Keep what the format can hold (16 bits, lossless).
        let n = self.r.clone().normalised();
        self.r.bits = n.bits;
        self.r.lossless = n.lossless;
        self.r != before
    }
}

// ----- State ------------------------------------------------------------------------------

/// What the background run tells the sheet.
enum Msg {
    /// Names settled: the files that would be replaced.
    Planned(Vec<PathBuf>),
    PlanFailed(String),
    Progress(Progress),
    Done(RecipeReport),
}

/// What a folder picker is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PickFor {
    /// A recipe's fixed folder (editor).
    Fixed,
    /// A watermark image (editor).
    Image,
    /// The folder of the run's recipe `n` that asks.
    Ask(usize),
}

pub(super) enum Phase {
    List,
    Edit(Box<Editor>),
    ConfirmDelete(usize),
    /// Folders being asked for (recipes that ask), then names settled.
    Asking,
    Planning,
    ConfirmReplace(Vec<PathBuf>),
    Running,
    Done(RecipeReport),
}

/// The open sheet.
pub(super) struct Dialog {
    pub sources: Vec<(Source, Vec<PathBuf>)>,
    pub source: usize,
    pub recipes: Vec<Recipe>,
    pub checked: Vec<bool>,
    pub current: usize,
    /// Opened from Develop: the image open there (its edit and turn).
    pub develop: Option<(
        PathBuf,
        specular_develop::model::Edit,
        specular_develop::preview::Orient,
    )>,
    pub phase: Phase,
    pub focus: Option<usize>,
    pub drawn: Drawn,
    pub preedit: String,
    /// The first image's facts (the editor's name preview).
    probe: Option<(PathBuf, Option<recipe::Facts>)>,
    probing: Option<Receiver<Option<recipe::Facts>>>,
    picking: Option<(PickFor, Receiver<Option<PathBuf>>)>,
    /// The recipes of the run being started (folders asked for filled in).
    run_recipes: Vec<Recipe>,
}

/// A run in the background.
pub(super) struct Run {
    cancel: CancelToken,
    msgs: Receiver<Msg>,
    confirm: Sender<bool>,
    last: Progress,
    stopping: bool,
    toast: Option<u64>,
    toast_at: f64,
    recipes: Vec<Recipe>,
    /// Started over the socket: its files (no sheet; nothing is replaced unasked).
    remote: Option<Vec<PathBuf>>,
}

/// Why an IPC `export` was not started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RemoteRefusal {
    /// Another export is running.
    Busy,
    /// No recipe of that name.
    Unknown,
    /// The recipe asks for its folder each time (only the window can ask).
    AsksForFolder,
}

/// The app's export recipes state.
#[derive(Default)]
pub(super) struct RecipesState {
    pub dialog: Option<Dialog>,
    pub run: Option<Run>,
    /// The recipes checked last time, by name.
    last_checked: Vec<String>,
}

impl std::fmt::Debug for RecipesState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecipesState")
            .field("open", &self.dialog.is_some())
            .field("running", &self.run.is_some())
            .finish_non_exhaustive()
    }
}

/// The run's status line.
pub(super) fn progress_text(p: &Progress) -> String {
    let base = t!(
        "export-recipes-progress",
        done = p.done as i64,
        total = p.total as i64
    );
    match p.eta {
        Some(eta) if p.done > 0 && p.done < p.total => format!("{base} · {}", eta_text(eta)),
        _ => base,
    }
}

/// The end report: a summary, then what was skipped or failed and why.
pub(super) fn report_lines(r: &RecipeReport, recipes: &[Recipe]) -> (String, Vec<String>) {
    let written = r.written() as i64;
    let summary = if r.cancelled {
        t!("export-recipes-stopped", count = written)
    } else {
        t!("export-recipes-done", count = written)
    };
    let mut lines = Vec::new();
    if r.skipped() > 0 {
        lines.push(t!(
            "export-recipes-done-skipped",
            count = r.skipped() as i64
        ));
    }
    if r.failed() > 0 {
        lines.push(t!("export-recipes-done-failed", count = r.failed() as i64));
    }
    let failures: Vec<_> = r
        .results
        .iter()
        .filter_map(|(p, k, o)| match o {
            Outcome::Failed(f) => Some((p, *k, f)),
            _ => None,
        })
        .collect();
    for (p, k, f) in failures.iter().take(MAX_FAILURES) {
        lines.push(t!(
            "export-recipes-failure",
            file = file_name(p),
            recipe = recipes.get(*k).map(recipe_name).unwrap_or_default(),
            reason = failure_text(f)
        ));
    }
    if failures.len() > MAX_FAILURES {
        lines.push(t!(
            "convert-report-more",
            count = (failures.len() - MAX_FAILURES) as i64
        ));
    }
    (summary, lines)
}

/// The replace question's lines.
pub(super) fn replace_lines(files: &[PathBuf]) -> Vec<String> {
    let mut v: Vec<String> = files
        .iter()
        .take(MAX_REPLACED)
        .map(|p| file_name(p))
        .collect();
    if files.len() > MAX_REPLACED {
        v.push(t!(
            "convert-report-more",
            count = (files.len() - MAX_REPLACED) as i64
        ));
    }
    v.push(t!("export-recipes-replace-detail"));
    v
}

/// "photo.heic → web/photo.jpg" for the first image, or what is wrong.
pub(super) fn preview(
    e: &Editor,
    first: Option<&Path>,
    facts: Option<&recipe::Facts>,
) -> Result<String, String> {
    if let Some(p) = e.problem() {
        return Err(p);
    }
    let r = e.recipe();
    let pat = Pattern::parse(&r.pattern).map_err(|e| pattern_error(&e))?;
    let Some(src) = first else {
        return Ok(String::new());
    };
    let f = facts.cloned().unwrap_or_default();
    let name = recipe::name_for(&pat, &r, src, 1, &f);
    let out = match recipe::folder(src, &r) {
        Some(d) if Some(d.as_path()) == src.parent() => name,
        Some(d) if d.as_os_str().is_empty() => name,
        Some(d) => format!("{}{}{name}", file_name(&d), std::path::MAIN_SEPARATOR),
        None => format!("…{}{name}", std::path::MAIN_SEPARATOR),
    };
    Ok(t!(
        "convert-name-preview",
        source = file_name(src),
        output = out
    ))
}

impl Dialog {
    fn view(&self, run: Option<&Run>) -> View {
        let phase = match &self.phase {
            Phase::List => {
                let (src, paths) = &self.sources[self.source];
                let blocked = if paths.is_empty() {
                    Some(t!("export-recipes-no-images"))
                } else if !self.checked.iter().any(|c| *c) {
                    Some(t!("export-recipes-none-checked"))
                } else {
                    None
                };
                PhaseView::List {
                    images: RowView {
                        row: Row::Images,
                        label: row_label(Row::Images),
                        value: source_label(*src, paths.len()),
                    },
                    recipes: self
                        .recipes
                        .iter()
                        .zip(&self.checked)
                        .map(|(r, c)| RecipeView {
                            name: recipe_name(r),
                            summary: summary(r),
                            checked: *c,
                        })
                        .collect(),
                    current: self.current,
                    blocked,
                }
            }
            Phase::Edit(e) => {
                let cols = columns(&e.r);
                let first = self.sources[self.source].1.first();
                let facts = self
                    .probe
                    .as_ref()
                    .filter(|(p, _)| Some(p) == first)
                    .and_then(|(_, f)| f.as_ref());
                PhaseView::Edit {
                    title: if e.index.is_some() {
                        t!("export-recipes-edit-title")
                    } else {
                        t!("export-recipes-new-title")
                    },
                    columns: cols.map(|c| row_views(&e.r, &c)),
                    fields: e
                        .fields()
                        .into_iter()
                        .map(|f| {
                            let le = e.field(f);
                            let sel = le.selection();
                            FieldView {
                                field: f,
                                label: field_label(f),
                                text: le.text().to_owned(),
                                caret: le.caret(),
                                selection: (sel.start, sel.end),
                                preedit: self.preedit.clone(),
                            }
                        })
                        .collect(),
                    preview: preview(e, first.map(PathBuf::as_path), facts),
                    choose_folder: matches!(e.r.destination, Destination::Fixed(_)),
                    choose_image: matches!(e.r.watermark.kind, MarkKind::Image(_)),
                }
            }
            Phase::ConfirmDelete(i) => PhaseView::Confirm {
                title: t!(
                    "export-recipes-delete-title",
                    name = self.recipes.get(*i).map(recipe_name).unwrap_or_default()
                ),
                lines: vec![t!("export-recipes-delete-detail")],
                confirm: t!("export-recipes-delete-confirm"),
                keep: t!("export-recipes-delete-keep"),
            },
            Phase::ConfirmReplace(files) => PhaseView::Confirm {
                title: t!("export-recipes-replace-title", count = files.len() as i64),
                lines: replace_lines(files),
                confirm: t!("export-recipes-replace-confirm"),
                keep: t!("app-cancel"),
            },
            Phase::Asking => PhaseView::Waiting {
                status: t!("export-recipes-asking"),
            },
            Phase::Planning => PhaseView::Waiting {
                status: t!("export-recipes-planning"),
            },
            Phase::Running => {
                let (p, stopping) =
                    run.map_or((Progress::default(), false), |r| (r.last, r.stopping));
                PhaseView::Running {
                    status: progress_text(&p),
                    progress: p.done as f32 / p.total.max(1) as f32,
                    stopping,
                }
            }
            Phase::Done(r) => {
                let (summary, lines) = report_lines(r, &self.run_recipes);
                PhaseView::Done {
                    summary,
                    lines,
                    can_open: r.written() > 0,
                }
            }
        };
        let mut v = View {
            phase,
            focus: self.focus,
        };
        let n = v.focusables().len();
        v.focus = v.focus.map(|i| i.min(n.saturating_sub(1)));
        v
    }

    fn editor(&mut self) -> Option<&mut Editor> {
        match &mut self.phase {
            Phase::Edit(e) => Some(e),
            _ => None,
        }
    }
}

pub(super) fn field_label(f: Field) -> String {
    match f {
        Field::Name => t!("export-recipes-name"),
        Field::Pattern => t!("export-recipes-pattern"),
        Field::Text => t!("export-recipes-text"),
        Field::MarkText => t!("export-recipes-mark-text-field"),
        Field::Subfolder => t!("export-recipes-subfolder"),
    }
}

impl RecipesState {
    fn view(&self) -> Option<View> {
        let d = self.dialog.as_ref()?;
        Some(d.view(self.run.as_ref()))
    }

    /// Draws the sheet (when open) over everything; returns what the accessibility tree
    /// needs.
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) -> Option<View> {
        let v = self.view()?;
        let drawn = view::draw(dl, text, pal, &v, size, rtl, ts);
        self.dialog.as_mut()?.drawn = drawn;
        Some(v)
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &View, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }

    /// The focused text field, when one has the keyboard.
    fn text_field(&self) -> Option<Field> {
        match self.view()?.focused()? {
            Hit::Field(f) => Some(f),
            _ => None,
        }
    }

    /// True while a text field has the keyboard.
    pub(super) fn wants_text(&self) -> bool {
        self.text_field().is_some()
    }

    /// Where the input method's window opens.
    pub(super) fn ime_area(&self) -> Option<Rect> {
        let f = self.text_field()?;
        let d = self.dialog.as_ref()?;
        d.drawn
            .ime
            .or_else(|| d.drawn.fields.iter().find(|x| x.0 == f).map(|x| x.1))
    }

    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            if wants {
                s.clone_into(&mut d.preedit);
            } else {
                d.preedit.clear();
            }
        }
    }
}

// ----- The app ----------------------------------------------------------------------------

impl App {
    /// Opens the sheet for the gallery's selection, the Library's, the viewer's image or the
    /// folder (or shows a running export).
    pub(super) fn recipes_open(&mut self) {
        if self.export_recipes.dialog.is_some() {
            return;
        }
        if self.export_recipes.run.is_some() {
            self.recipes_show_running();
            return;
        }
        let mut sources = Vec::new();
        if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
            sources.push((Source::Current, vec![p.clone()]));
        }
        let mut first = 0;
        if self.gallery_active() {
            let sel: Vec<PathBuf> = self
                .gallery
                .targets()
                .into_iter()
                .filter(|p| !p.is_dir())
                .collect();
            if !sel.is_empty() {
                sources.push((Source::Selection, sel));
                first = sources.len() - 1;
            }
        }
        if self.entries.len() > 1 {
            sources.push((Source::Folder, self.entries.clone()));
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, picks));
        }
        if sources.is_empty() {
            return;
        }
        self.recipes_show(sources, first, None);
    }

    /// Opens the sheet from Develop: the image open there (with its edit), the folder, the
    /// picks.
    pub(super) fn recipes_open_develop(&mut self) {
        if self.export_recipes.dialog.is_some() {
            return;
        }
        if self.export_recipes.run.is_some() {
            self.recipes_show_running();
            return;
        }
        let Some(cur) = self.develop_current() else {
            return;
        };
        let mut sources = vec![(Source::Current, vec![cur.0.clone()])];
        if self.entries.len() > 1 {
            sources.push((Source::Folder, self.entries.clone()));
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, picks));
        }
        self.recipes_show(sources, 0, Some(cur));
    }

    fn recipes_show(
        &mut self,
        sources: Vec<(Source, Vec<PathBuf>)>,
        source: usize,
        develop: Option<(
            PathBuf,
            specular_develop::model::Edit,
            specular_develop::preview::Orient,
        )>,
    ) {
        let recipes = load_recipes();
        let last = &self.export_recipes.last_checked;
        let mut checked: Vec<bool> = recipes
            .iter()
            .map(|r| last.contains(&recipe_name(r)))
            .collect();
        if !checked.iter().any(|c| *c)
            && let Some(c) = checked.first_mut()
        {
            *c = true;
        }
        self.export_recipes.dialog = Some(Dialog {
            sources,
            source,
            recipes,
            checked,
            current: 0,
            develop,
            phase: Phase::List,
            // On the first recipe.
            focus: Some(1),
            drawn: Drawn::default(),
            preedit: String::new(),
            probe: None,
            probing: None,
            picking: None,
            run_recipes: Vec::new(),
        });
        self.chrome.announce_status(view::title_text());
        self.redraw = true;
    }

    fn recipes_show_running(&mut self) {
        if let Some(r) = self.export_recipes.run.as_mut()
            && let Some(id) = r.toast.take()
        {
            self.chrome.dismiss_toast(id);
        }
        let recipes = self
            .export_recipes
            .run
            .as_ref()
            .map(|r| r.recipes.clone())
            .unwrap_or_default();
        self.export_recipes.dialog = Some(Dialog {
            sources: vec![(Source::Current, Vec::new())],
            source: 0,
            recipes: Vec::new(),
            checked: Vec::new(),
            current: 0,
            develop: None,
            phase: Phase::Running,
            focus: Some(1),
            drawn: Drawn::default(),
            preedit: String::new(),
            probe: None,
            probing: None,
            picking: None,
            run_recipes: recipes,
        });
        self.redraw = true;
    }

    fn recipes_view(&self) -> Option<View> {
        self.export_recipes.view()
    }

    fn recipes_set_focus(&mut self, h: Hit) {
        if let Some(v) = self.recipes_view()
            && let Some(i) = v.focusables().iter().position(|x| *x == h)
            && let Some(d) = self.export_recipes.dialog.as_mut()
        {
            d.focus = Some(i);
        }
    }

    fn recipes_save(&mut self) {
        if let Some(d) = self.export_recipes.dialog.as_ref()
            && self.args.screenshot.is_none()
        {
            save_recipes(&d.recipes);
        }
    }

    /// Reads the first image's facts (the editor's name preview) unless known.
    fn recipes_probe(&mut self) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let Some(first) = d.sources[d.source].1.first().cloned() else {
            return;
        };
        if d.probe.as_ref().is_some_and(|p| p.0 == first) || d.probing.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let registry = self.loader.registry();
        d.probe = Some((first.clone(), None));
        self.loader.background("xr-probe", move |l| {
            let files = recipe::Files { registry };
            let _ = tx.send(recipe::Render::facts(&files, &Item::new(first)).ok());
            l.waker()();
        });
        d.probing = Some(rx);
    }

    fn recipes_close(&mut self) {
        if let Some(d) = self.export_recipes.dialog.as_ref() {
            self.export_recipes.last_checked = d
                .recipes
                .iter()
                .zip(&d.checked)
                .filter(|(_, c)| **c)
                .map(|(r, _)| recipe_name(r))
                .collect();
        }
        self.export_recipes.dialog = None;
        self.redraw = true;
    }

    fn recipes_edit(&mut self, index: Option<usize>, copy: bool) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let r = match index.and_then(|i| d.recipes.get(i)) {
            Some(r) if copy => Recipe {
                id: String::new(),
                name: t!("export-recipes-copy-name", name = recipe_name(r)),
                ..r.clone()
            },
            Some(r) => r.clone(),
            None => Recipe {
                name: t!("export-recipes-new-name"),
                ..Recipe::default()
            },
        };
        let target = if copy { None } else { index };
        d.phase = Phase::Edit(Box::new(Editor::new(target, r)));
        d.focus = Some(0);
        self.recipes_probe();
        self.chrome.announce_status(if target.is_some() {
            t!("export-recipes-edit-title")
        } else {
            t!("export-recipes-new-title")
        });
    }

    fn recipes_save_edit(&mut self) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let Phase::Edit(e) = &d.phase else { return };
        if e.problem().is_some() {
            return;
        }
        let r = e.recipe().normalised();
        let name = recipe_name(&r);
        match e.index {
            Some(i) if i < d.recipes.len() => d.recipes[i] = r,
            _ => {
                d.recipes.push(r);
                d.checked.push(true);
                d.current = d.recipes.len() - 1;
            }
        }
        d.phase = Phase::List;
        d.focus = Some(1 + d.current);
        self.recipes_save();
        self.chrome
            .announce_status(t!("export-recipes-saved", name = name));
    }

    fn recipes_delete(&mut self) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let Phase::ConfirmDelete(i) = d.phase else {
            return;
        };
        let name = d.recipes.get(i).map(recipe_name).unwrap_or_default();
        if i < d.recipes.len() {
            d.recipes.remove(i);
            d.checked.remove(i);
        }
        d.current = d.current.min(d.recipes.len().saturating_sub(1));
        d.phase = Phase::List;
        d.focus = Some(1 + d.current);
        self.recipes_save();
        self.chrome
            .announce_status(t!("export-recipes-deleted", name = name));
    }

    fn recipes_pick(&mut self, what: PickFor) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        if d.picking.is_some() {
            return;
        }
        let start = d.sources[d.source]
            .1
            .first()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let (tx, rx) = crossbeam_channel::bounded(1);
        let title = match what {
            PickFor::Ask(k) => d
                .run_recipes
                .get(k)
                .map(|r| t!("export-recipes-ask-title", name = recipe_name(r))),
            _ => None,
        };
        self.loader.background("dlg-xrecipes", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start {
                dlg = dlg.set_directory(dir);
            }
            if let Some(t) = title {
                dlg = dlg.set_title(t);
            }
            let p = match what {
                PickFor::Image => dlg
                    .add_filter(
                        "Images",
                        &["png", "svg", "webp", "tif", "tiff", "jpg", "jpeg"],
                    )
                    .pick_file(),
                _ => dlg.pick_folder(),
            };
            let _ = tx.send(p);
            l.waker()();
        });
        d.picking = Some((what, rx));
    }

    /// Export: asks for folders of recipes that ask, then settles names in the background.
    fn recipes_start(&mut self) {
        if self.export_recipes.run.is_some() {
            return;
        }
        let Some(v) = self.recipes_view() else { return };
        if !v.can_start() {
            return;
        }
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        d.run_recipes = d
            .recipes
            .iter()
            .zip(&d.checked)
            .filter(|(_, c)| **c)
            .map(|(r, _)| r.clone())
            .collect();
        self.recipes_next_ask();
    }

    /// The next recipe that asks for its folder, or the run.
    fn recipes_next_ask(&mut self) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        if let Some(k) = d
            .run_recipes
            .iter()
            .position(|r| r.destination == Destination::Ask)
        {
            d.phase = Phase::Asking;
            d.focus = Some(0);
            self.recipes_pick(PickFor::Ask(k));
            return;
        }
        self.recipes_run();
    }

    /// Starts the background run: names settled, a question for replaced files, then the
    /// exports.
    fn recipes_run(&mut self) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let paths = d.sources[d.source].1.clone();
        let recipes = d.run_recipes.clone();
        let develop = d.develop.clone();
        d.phase = Phase::Planning;
        d.focus = Some(0);
        self.recipes_spawn(paths, recipes, develop, false);
        self.chrome.announce_status(t!("export-recipes-planning"));
        self.redraw = true;
    }

    /// IPC `export`: the recipe named `name` (as listed, or its id) on `paths`, in the
    /// background with no sheet. Files it would replace are not replaced (nobody was asked):
    /// the run ends there and says so in `exported`.
    pub(super) fn recipes_run_remote(
        &mut self,
        paths: Vec<PathBuf>,
        name: &str,
    ) -> Result<(), RemoteRefusal> {
        if self.export_recipes.run.is_some() {
            return Err(RemoteRefusal::Busy);
        }
        let all = load_recipes();
        let r = all
            .iter()
            .find(|r| recipe_name(r) == name || r.name == name || r.id == name)
            .cloned()
            .ok_or(RemoteRefusal::Unknown)?;
        if r.destination == Destination::Ask {
            return Err(RemoteRefusal::AsksForFolder);
        }
        self.recipes_spawn(paths, vec![r], None, true);
        Ok(())
    }

    /// The background run of `recipes` on `paths` (`remote`: started over the socket).
    fn recipes_spawn(
        &mut self,
        paths: Vec<PathBuf>,
        recipes: Vec<Recipe>,
        develop: Option<(
            PathBuf,
            specular_develop::model::Edit,
            specular_develop::preview::Orient,
        )>,
        remote: bool,
    ) {
        let run_recipes = recipes.clone();
        let remote = remote.then(|| paths.clone());
        let lib = self.gallery.library_handle();
        let gpu = self.gfx.as_ref().map(|g| {
            (
                g.gpu.device.clone(),
                g.gpu.queue.clone(),
                g.gpu.info.device_type != specular_gfx::wgpu::DeviceType::DiscreteGpu,
            )
        });
        if develop.is_some() || gpu.is_some() {
            super::develop::export::install_denoise_runner(
                gpu.as_ref().map(|g| (g.0.clone(), g.1.clone())),
            );
        }
        let registry = self.loader.registry();
        let wake = self.loader.waker();
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let (mtx, mrx) = crossbeam_channel::unbounded();
        let (ctx, crx) = crossbeam_channel::bounded::<bool>(1);
        let threads = std::thread::available_parallelism()
            .map_or(2, |n| n.get() / 2)
            .clamp(1, 4);
        specular_log::info!(
            "export recipes: {} image(s) through {} recipe(s)",
            paths.len(),
            recipes.len()
        );
        self.loader.background("xr-run", move |_| {
            let t0 = Instant::now();
            // Photos with an edit are developed (from Develop: all of them).
            let mut edits = HashMap::new();
            for p in &paths {
                match &develop {
                    Some((cp, e, o)) if cp == p => {
                        edits.insert(p.clone(), (e.clone(), Some(*o)));
                    }
                    _ => {
                        if let Some(e) =
                            super::develop::export::saved_edit(p, lib.as_deref(), develop.is_some())
                        {
                            edits.insert(p.clone(), (e, None));
                        }
                    }
                }
            }
            let plain = || -> Box<dyn recipe::Render> {
                Box::new(recipe::Files {
                    registry: registry.clone(),
                })
            };
            let render: Box<dyn recipe::Render> = if edits.is_empty() {
                plain()
            } else {
                let opened = match gpu {
                    Some((d, q, i)) => Ok((d, q, i)),
                    None => specular_develop::export::batch::open_device(),
                };
                match opened {
                    Ok((device, queue, integrated)) => {
                        Box::new(specular_develop::export::recipes::Developed::new(
                            &device,
                            &queue,
                            specular_develop::gpu::pool::Budget::for_device(integrated, None, None),
                            registry.clone(),
                            edits,
                        ))
                    }
                    Err(e) => {
                        specular_log::warn!("export recipes: no GPU ({e}), edits left out");
                        plain()
                    }
                }
            };
            let items: Vec<Item> = paths.iter().map(Item::new).collect();
            let plan: Plan = match recipe::plan(items, recipes.clone(), render.as_ref(), &c) {
                Ok(p) => p,
                Err(PlanError::Cancelled) => {
                    let _ = mtx.send(Msg::Done(RecipeReport {
                        cancelled: true,
                        ..RecipeReport::default()
                    }));
                    wake();
                    return;
                }
                Err(PlanError::Pattern { recipe, error }) => {
                    let name = recipes.get(recipe).map(recipe_name).unwrap_or_default();
                    let _ = mtx.send(Msg::PlanFailed(t!(
                        "export-recipes-plan-pattern",
                        name = name,
                        reason = pattern_error(&error)
                    )));
                    wake();
                    return;
                }
                Err(PlanError::NoFolder { .. }) => {
                    let _ = mtx.send(Msg::PlanFailed(t!("export-recipes-no-folder")));
                    wake();
                    return;
                }
            };
            let replacing: Vec<PathBuf> = plan
                .replacing()
                .into_iter()
                .map(Path::to_path_buf)
                .collect();
            let ask = !replacing.is_empty();
            let _ = mtx.send(Msg::Planned(replacing));
            wake();
            if ask && !crx.recv().unwrap_or(false) {
                return;
            }
            let w = wake.clone();
            let tx = mtx.clone();
            let send = move |p: &Progress| {
                let _ = tx.send(Msg::Progress(*p));
                w();
            };
            let report =
                recipe::execute(&plan, render.as_ref(), Some(&registry), threads, &c, &send);
            specular_log::info!(
                "export recipes: {} written, {} skipped, {} failed in {:.1} s",
                report.written(),
                report.skipped(),
                report.failed(),
                t0.elapsed().as_secs_f64()
            );
            let _ = mtx.send(Msg::Done(report));
            wake();
        });
        self.export_recipes.run = Some(Run {
            cancel,
            msgs: mrx,
            confirm: ctx,
            last: Progress::default(),
            stopping: false,
            toast: None,
            toast_at: 0.0,
            recipes: run_recipes,
            remote,
        });
    }

    /// `exported`, as done by whoever started the run (the job of a client's).
    fn recipes_exported(&mut self, results: &[super::events::Exported]) {
        match self.remote.events.exports.take() {
            Some((job, src)) => {
                let e = super::events::exported(results, Some(job));
                self.emit_as(e, &src);
            }
            None => self.emit_as(
                super::events::exported(results, None),
                &super::events::Source::User,
            ),
        }
    }

    fn recipes_stop(&mut self) {
        if let Some(r) = self.export_recipes.run.as_mut() {
            r.cancel.cancel();
            r.stopping = true;
            let _ = r.confirm.try_send(false);
        }
        self.redraw = true;
    }

    fn recipes_hide(&mut self) {
        self.export_recipes.dialog = None;
        let now = self.now();
        if let Some(r) = self.export_recipes.run.as_mut() {
            r.toast = Some(self.chrome.toast(progress_text(&r.last), None, now));
            r.toast_at = now;
        }
        self.redraw = true;
    }

    /// Replies of the probe, the pickers and the run.
    pub(super) fn recipes_tick(&mut self) {
        let mut picked = None;
        if let Some(d) = self.export_recipes.dialog.as_mut() {
            if let Some(rx) = &d.probing
                && let Ok(f) = rx.try_recv()
            {
                if let Some(p) = d.probe.as_mut() {
                    p.1 = f;
                }
                d.probing = None;
                self.redraw = true;
            }
            if let Some((what, rx)) = &d.picking
                && let Ok(p) = rx.try_recv()
            {
                picked = Some((*what, p));
                d.picking = None;
            }
        }
        if let Some((what, p)) = picked {
            self.recipes_picked(what, p);
        }
        let now = self.now();
        let Some(r) = self.export_recipes.run.as_mut() else {
            return;
        };
        let mut done = None;
        let mut planned = None;
        let mut failed = None;
        let mut changed = false;
        while let Ok(m) = r.msgs.try_recv() {
            match m {
                Msg::Progress(p) => {
                    r.last = p;
                    changed = true;
                }
                Msg::Planned(files) => planned = Some(files),
                Msg::PlanFailed(e) => failed = Some(e),
                Msg::Done(rep) => done = Some(rep),
            }
        }
        if changed {
            self.redraw = true;
            if let Some(id) = r.toast
                && now - r.toast_at >= 1.0
            {
                r.toast = Some(self.chrome.update_toast(id, progress_text(&r.last), now));
                r.toast_at = now;
            }
        }
        // Over the socket nobody can be asked: files that exist stay, and the run ends.
        if let Some(files) = planned.as_ref().filter(|f| !f.is_empty())
            && let Some(sources) = r.remote.clone()
        {
            let _ = r.confirm.try_send(false);
            let recipes = r.recipes.clone();
            self.export_recipes.run = None;
            let why = t!("export-recipes-remote-exists", count = files.len() as i64);
            let name = recipes.first().map(recipe_name).unwrap_or_default();
            let results: Vec<super::events::Exported> = sources
                .into_iter()
                .map(|p| (p, name.clone(), Err(why.clone())))
                .collect();
            self.recipes_exported(&results);
            return;
        }
        if let Some(files) = planned
            && let Some(d) = self.export_recipes.dialog.as_mut()
        {
            if files.is_empty() {
                d.phase = Phase::Running;
                d.focus = Some(1);
                self.chrome.announce_status(t!("export-recipes-running"));
            } else {
                self.chrome.announce_status(t!(
                    "export-recipes-replace-title",
                    count = files.len() as i64
                ));
                d.phase = Phase::ConfirmReplace(files);
                // On Cancel: replacing needs a deliberate choice.
                d.focus = Some(1);
            }
            self.redraw = true;
        }
        if let Some(e) = failed {
            if let Some(sources) = self
                .export_recipes
                .run
                .as_ref()
                .and_then(|r| r.remote.clone())
            {
                let name = self
                    .export_recipes
                    .run
                    .as_ref()
                    .and_then(|r| r.recipes.first().map(recipe_name))
                    .unwrap_or_default();
                let results: Vec<super::events::Exported> = sources
                    .into_iter()
                    .map(|p| (p, name.clone(), Err(e.clone())))
                    .collect();
                self.recipes_exported(&results);
            }
            self.export_recipes.run = None;
            if let Some(d) = self.export_recipes.dialog.as_mut() {
                d.phase = Phase::List;
                d.focus = Some(1 + d.current);
            }
            let now = self.now();
            self.chrome.warning(e, now);
            self.redraw = true;
            return;
        }
        if let Some(rep) = done {
            self.recipes_finished(rep);
        }
    }

    fn recipes_picked(&mut self, what: PickFor, p: Option<PathBuf>) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        match what {
            PickFor::Fixed | PickFor::Image => {
                let (Some(p), Some(e)) = (p, d.editor()) else {
                    return;
                };
                let row = if what == PickFor::Fixed {
                    e.r.destination = Destination::Fixed(p);
                    Row::Folder
                } else {
                    e.r.watermark.kind = MarkKind::Image(p);
                    Row::Mark
                };
                let v = row_value(&e.r, row);
                self.chrome.announce_status(v);
            }
            PickFor::Ask(k) => match p {
                Some(p) => {
                    if let Some(r) = d.run_recipes.get_mut(k) {
                        r.destination = Destination::Fixed(p);
                    }
                    self.recipes_next_ask();
                }
                None => {
                    d.phase = Phase::List;
                    d.focus = Some(1 + d.current);
                    self.chrome.announce_status(t!("export-recipes-no-folder"));
                }
            },
        }
        self.redraw = true;
    }

    fn recipes_finished(&mut self, report: RecipeReport) {
        let Some(run) = self.export_recipes.run.take() else {
            return;
        };
        let now = self.now();
        if let Some(id) = run.toast {
            self.chrome.dismiss_toast(id);
        }
        for (p, k, o) in &report.results {
            if let Outcome::Failed(f) = o {
                specular_log::warn!("export recipes: {} (recipe {k}): {f}", p.display());
            }
        }
        if self.events_listening("exported") {
            let results: Vec<super::events::Exported> = report
                .results
                .iter()
                .filter_map(|(p, k, o)| {
                    let name = run.recipes.get(*k).map(recipe_name).unwrap_or_default();
                    let r = match o {
                        Outcome::Converted { out, .. } => Ok(out.clone()),
                        Outcome::Failed(f) => Err(failure_text(f)),
                        Outcome::Skipped { existing } => Err(t!(
                            "export-recipes-remote-skipped",
                            file = existing.display().to_string()
                        )),
                        Outcome::Cancelled => return None,
                    };
                    Some((p.clone(), name, r))
                })
                .collect();
            self.recipes_exported(&results);
        }
        // Post-export: the folders of recipes that open theirs.
        if !report.cancelled {
            let mut shown: Vec<PathBuf> = Vec::new();
            for (_, k, o) in &report.results {
                if let Outcome::Converted { out, .. } = o
                    && run
                        .recipes
                        .get(*k)
                        .is_some_and(|r| r.after == After::OpenFolder)
                    && let Some(dir) = out.parent()
                    && !shown.iter().any(|d| d == dir)
                    && shown.len() < MAX_OPEN
                {
                    shown.push(dir.to_path_buf());
                    self.recipes_reveal(out.clone());
                }
            }
        }
        let (summary, lines) = report_lines(&report, &run.recipes);
        match self.export_recipes.dialog.as_mut() {
            Some(d) => {
                d.run_recipes = run.recipes;
                d.phase = Phase::Done(report);
                d.focus = Some(0);
                self.chrome.announce_status(summary);
            }
            None => {
                let mut msg = summary;
                for l in lines.iter().take(3) {
                    msg = format!("{msg}. {l}");
                }
                if report.failed() > 0 {
                    self.chrome.warning(msg, now);
                } else {
                    self.chrome.toast(msg, None, now);
                }
            }
        }
        self.redraw = true;
    }

    fn recipes_reveal(&mut self, p: PathBuf) {
        if self.args.screenshot.is_some() {
            return;
        }
        self.loader
            .background("int-reveal", move |_| super::gallery::reveal(&p));
    }

    /// Open folder in the end report: the first file of each folder written to.
    fn recipes_open_folders(&mut self) {
        let Some(Phase::Done(r)) = self.export_recipes.dialog.as_ref().map(|d| &d.phase) else {
            return;
        };
        let mut shown: Vec<PathBuf> = Vec::new();
        let mut files = Vec::new();
        for (_, _, o) in &r.results {
            if let Outcome::Converted { out, .. } = o
                && let Some(dir) = out.parent()
                && !shown.iter().any(|d| d == dir)
                && shown.len() < MAX_OPEN
            {
                shown.push(dir.to_path_buf());
                files.push(out.clone());
            }
        }
        for f in files {
            self.recipes_reveal(f);
        }
    }

    fn recipes_step(&mut self, row: Row, delta: i32) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        if row == Row::Images {
            if matches!(d.phase, Phase::List) && d.sources.len() > 1 {
                d.source = (d.source as i32 + delta).rem_euclid(d.sources.len() as i32) as usize;
                let (src, paths) = &d.sources[d.source];
                let label = source_label(*src, paths.len());
                self.chrome.announce_status(label);
            }
            return;
        }
        let Some(e) = d.editor() else { return };
        if e.step(row, delta) {
            let v = row_value(&e.r, row);
            self.recipes_set_focus(Hit::Row(row));
            self.chrome.announce_status(v);
        }
    }

    fn recipes_hit(&mut self, h: Hit) {
        let Some(v) = self.recipes_view() else { return };
        match h {
            Hit::Dec(r) => self.recipes_step(r, -1),
            Hit::Inc(r) => self.recipes_step(r, 1),
            Hit::Row(_) | Hit::Field(_) => self.recipes_set_focus(h),
            Hit::Recipe(i) => {
                if let Some(d) = self.export_recipes.dialog.as_mut() {
                    d.current = i;
                }
                self.recipes_set_focus(h);
            }
            Hit::Check(i) => {
                if let Some(d) = self.export_recipes.dialog.as_mut()
                    && let Some(c) = d.checked.get_mut(i)
                {
                    *c = !*c;
                    d.current = i;
                    let msg = if *c {
                        t!("export-recipes-checked", name = recipe_name(&d.recipes[i]))
                    } else {
                        t!(
                            "export-recipes-unchecked",
                            name = recipe_name(&d.recipes[i])
                        )
                    };
                    self.chrome.announce_status(msg);
                }
                self.recipes_set_focus(Hit::Recipe(i));
            }
            Hit::New => self.recipes_edit(None, false),
            Hit::Edit | Hit::Duplicate | Hit::Delete => {
                let Some(d) = self.export_recipes.dialog.as_mut() else {
                    return;
                };
                if d.recipes.is_empty() {
                    return;
                }
                let i = d.current.min(d.recipes.len() - 1);
                match h {
                    Hit::Edit => self.recipes_edit(Some(i), false),
                    Hit::Duplicate => self.recipes_edit(Some(i), true),
                    _ => {
                        d.phase = Phase::ConfirmDelete(i);
                        // On Keep.
                        d.focus = Some(1);
                        let name = recipe_name(&d.recipes[i]);
                        self.chrome
                            .announce_status(t!("export-recipes-delete-title", name = name));
                    }
                }
            }
            Hit::ChooseFolder => {
                self.recipes_set_focus(h);
                self.recipes_pick(PickFor::Fixed);
            }
            Hit::ChooseImage => {
                self.recipes_set_focus(h);
                self.recipes_pick(PickFor::Image);
            }
            Hit::Cancel => match &v.phase {
                PhaseView::Edit { .. } => {
                    if let Some(d) = self.export_recipes.dialog.as_mut() {
                        d.phase = Phase::List;
                        d.focus = Some(1 + d.current);
                    }
                }
                PhaseView::Waiting { .. } => {
                    self.recipes_stop();
                    self.export_recipes.run = None;
                    if let Some(d) = self.export_recipes.dialog.as_mut() {
                        d.phase = Phase::List;
                        d.picking = None;
                        d.focus = Some(1 + d.current);
                    }
                }
                _ => self.recipes_close(),
            },
            Hit::Start => self.recipes_start(),
            Hit::Save => self.recipes_save_edit(),
            Hit::Confirm => match self.export_recipes.dialog.as_ref().map(|d| &d.phase) {
                Some(Phase::ConfirmDelete(_)) => self.recipes_delete(),
                Some(Phase::ConfirmReplace(_)) => {
                    if let Some(r) = self.export_recipes.run.as_ref() {
                        let _ = r.confirm.try_send(true);
                    }
                    if let Some(d) = self.export_recipes.dialog.as_mut() {
                        d.phase = Phase::Running;
                        d.focus = Some(1);
                    }
                }
                _ => {}
            },
            Hit::Keep => match self.export_recipes.dialog.as_ref().map(|d| &d.phase) {
                Some(Phase::ConfirmReplace(_)) => {
                    if let Some(r) = self.export_recipes.run.take() {
                        let _ = r.confirm.try_send(false);
                    }
                    if let Some(d) = self.export_recipes.dialog.as_mut() {
                        d.phase = Phase::List;
                        d.focus = Some(1 + d.current);
                    }
                    self.chrome
                        .announce_status(t!("export-recipes-replace-kept"));
                }
                _ => {
                    if let Some(d) = self.export_recipes.dialog.as_mut() {
                        d.phase = Phase::List;
                        d.focus = Some(1 + d.current);
                    }
                }
            },
            Hit::Stop => self.recipes_stop(),
            Hit::Hide => self.recipes_hide(),
            Hit::OpenFolder => self.recipes_open_folders(),
            Hit::Close => self.recipes_close(),
        }
        self.redraw = true;
    }

    /// Text into the focused field (typed keys and input-method commits).
    pub(super) fn recipes_text(&mut self, s: &str) {
        let Some(f) = self.export_recipes.text_field() else {
            return;
        };
        if let Some(e) = self.export_recipes.dialog.as_mut().and_then(Dialog::editor)
            && e.field_mut(f).insert(s)
        {
            self.redraw = true;
        }
    }

    /// Text pasted into the focused field.
    pub(super) fn recipes_paste(&mut self, s: &str) {
        let Some(f) = self.export_recipes.text_field() else {
            return;
        };
        if let Some(e) = self.export_recipes.dialog.as_mut().and_then(Dialog::editor) {
            e.field_mut(f).paste(s);
            self.redraw = true;
        }
    }

    /// The pointer moved: a press in a field drags a selection.
    pub(super) fn recipes_drag(&mut self, p: (f32, f32)) {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return;
        };
        let fields = d.drawn.fields.clone();
        let Some(e) = d.editor() else { return };
        for (f, _, block) in &fields {
            if e.field(*f).dragging()
                && let Some((block, x0)) = block
            {
                e.field_mut(*f).drag_to(block.hit_byte(p.0 - x0));
                self.redraw = true;
            }
        }
    }

    fn recipes_field_key(
        &mut self,
        f: Field,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
    ) -> bool {
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return false;
        };
        let rtl = d
            .drawn
            .fields
            .iter()
            .find(|x| x.0 == f)
            .and_then(|x| x.2.as_ref())
            .is_some_and(|(b, _)| b.rtl);
        let Some(e) = d.editor() else { return false };
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            let handled = e.field_mut(f).key(c.key, c.mods, rtl, &mut clip).used();
            if handled {
                self.field_clipboard(clip, PasteTo::ExportRecipes);
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.recipes_text(&s);
        true
    }

    /// Keys while the sheet is open (it takes them all).
    pub(super) fn recipes_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(v) = self.recipes_view() else {
            return false;
        };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focused();
        if let Some(Hit::Field(f)) = focus
            && self.recipes_field_key(f, combo, text)
        {
            return true;
        }
        let Some(combo) = combo else { return true };
        let rtl = self.rtl();
        let move_focus = |me: &mut App, d: i32| {
            if let Some(dl) = me.export_recipes.dialog.as_mut() {
                let i = dl
                    .focus
                    .map_or(0, |i| (i as i32 + d).rem_euclid(n.max(1) as i32) as usize);
                dl.focus = Some(i);
                if let Some(Hit::Recipe(r)) = focusables.get(i) {
                    dl.current = *r;
                }
            }
        };
        let plain = combo.mods == Mods::NONE;
        match combo.key {
            Key::Named(NamedKey::Escape) => self.recipes_hit(match v.phase {
                PhaseView::Running { .. } => Hit::Hide,
                PhaseView::Confirm { .. } => Hit::Keep,
                _ => Hit::Cancel,
            }),
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Row(r)) => self.recipes_step(r, if forward { 1 } else { -1 }),
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Enter) => match focus {
                Some(Hit::Row(_) | Hit::Field(_) | Hit::Recipe(_)) | None => {
                    self.recipes_hit(v.default_hit())
                }
                Some(h) => self.recipes_hit(h),
            },
            Key::Named(NamedKey::Space) => match focus {
                Some(Hit::Row(r)) => self.recipes_step(r, 1),
                Some(Hit::Recipe(i)) => self.recipes_hit(Hit::Check(i)),
                Some(Hit::Field(_)) | None => {}
                Some(h) => self.recipes_hit(h),
            },
            Key::Named(NamedKey::Delete) if matches!(focus, Some(Hit::Recipe(_))) => {
                self.recipes_hit(Hit::Delete)
            }
            Key::Char('s' | 'S') if combo.mods.ctrl => self.recipes_hit(v.default_hit()),
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the sheet is open (it takes them all).
    pub(super) fn recipes_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let Some(d) = self.export_recipes.dialog.as_mut() else {
            return false;
        };
        let fields = d.drawn.fields.clone();
        if !down {
            if let Some(e) = d.editor() {
                for f in e.fields() {
                    e.field_mut(f).release();
                }
            }
            return true;
        }
        // Buttons and check boxes before the rows they sit in.
        let hit = d
            .drawn
            .hits
            .iter()
            .find(|(r, h)| r.contains(p) && !matches!(h, Hit::Row(_) | Hit::Recipe(_)))
            .or_else(|| d.drawn.hits.iter().find(|(r, _)| r.contains(p)))
            .map(|x| x.1);
        if let Some(Hit::Field(f)) = hit
            && let Some((_, _, Some((block, x0)))) = fields.iter().find(|x| x.0 == f)
            && let Some(e) = d.editor()
        {
            let at = block.hit_byte(p.0 - x0);
            e.field_mut(f).press(at, p.0, now, shift);
        }
        if let Some(h) = hit {
            self.recipes_hit(h);
        }
        true
    }

    /// A screen reader's request on one of the sheet's nodes; false when not ours.
    pub(super) fn recipes_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.recipes_view() else {
            return false;
        };
        let Some(hit) = view::a11y_target(&v, req.target_node.0) else {
            return (view::A11Y_BASE..view::A11Y_BASE + 0x1000).contains(&req.target_node.0);
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Row(r)) => self.recipes_step(r, 1),
            (accesskit::Action::Decrement, Hit::Row(r)) => self.recipes_step(r, -1),
            (accesskit::Action::Click, Hit::Recipe(i)) => self.recipes_hit(Hit::Check(i)),
            (accesskit::Action::SetValue, Hit::Field(f)) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data
                    && let Some(e) = self.export_recipes.dialog.as_mut().and_then(Dialog::editor)
                {
                    e.field_mut(f).set_text(s);
                }
            }
            (accesskit::Action::SetTextSelection, Hit::Field(f)) => {
                if let Some(accesskit::ActionData::SetTextSelection(sel)) = &req.data
                    && let Some(e) = self.export_recipes.dialog.as_mut().and_then(Dialog::editor)
                {
                    e.field_mut(f)
                        .set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                }
                self.recipes_set_focus(Hit::Field(f));
            }
            (accesskit::Action::Click, h) => self.recipes_hit(h),
            (accesskit::Action::Focus, h) => self.recipes_set_focus(h),
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// The run is lost (its task panicked).
    pub(super) fn recipes_lost(&mut self, task: &str) {
        match task {
            "xr-run" => {
                self.export_recipes.run = None;
                if let Some(d) = self.export_recipes.dialog.as_mut()
                    && matches!(
                        d.phase,
                        Phase::Planning | Phase::Running | Phase::ConfirmReplace(_)
                    )
                {
                    d.phase = Phase::List;
                    d.focus = Some(1 + d.current);
                }
            }
            "xr-probe" => {
                if let Some(d) = self.export_recipes.dialog.as_mut() {
                    d.probing = None;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row steps within what the format holds; rows come and go with the choices.
    #[test]
    fn editor_rows_follow_the_choices() {
        let mut e = Editor::new(None, Recipe::default());
        assert!(columns(&e.r)[0].contains(&Row::Quality));
        assert!(!columns(&e.r)[0].contains(&Row::Bits));
        e.step(Row::Format, 1);
        assert_eq!(e.r.format, Format::Png);
        assert!(columns(&e.r)[0].contains(&Row::Bits));
        assert!(!columns(&e.r)[0].contains(&Row::Quality));
        e.step(Row::Bits, 1);
        assert_eq!(e.r.bits, Bits::Sixteen);
        // To WebP: no 16 bits there.
        e.step(Row::Format, 1);
        e.step(Row::Format, 1);
        assert_eq!(e.r.format, Format::Webp);
        assert_eq!(e.r.bits, Bits::Eight);
        // Resize kinds and their values.
        e.step(Row::Resize, 1);
        assert_eq!(e.r.size, Size::LongEdge(2048));
        e.step(Row::Edge, 1);
        assert_eq!(e.r.size, Size::LongEdge(2160));
        e.step(Row::Resize, 3);
        assert_eq!(
            e.r.size,
            Size::Exact {
                width: 1080,
                height: 1350
            }
        );
        assert!(columns(&e.r)[1].contains(&Row::Height));
        // Watermark rows only with a watermark; a text mark needs its text.
        assert!(!columns(&e.r)[2].contains(&Row::Position));
        e.step(Row::Mark, 1);
        assert!(columns(&e.r)[2].contains(&Row::Position));
        assert!(e.fields().contains(&Field::MarkText));
        e.mark.set_text("  ");
        assert!(e.problem().is_some());
        e.mark.set_text("© Ana");
        assert!(e.problem().is_none());
        assert_eq!(e.recipe().watermark.kind, MarkKind::Text("© Ana".into()));
        // An image mark needs its file.
        e.step(Row::Mark, 1);
        assert!(e.problem().is_some());
        // Folders: a subfolder has a name field.
        assert!(e.fields().contains(&Field::Subfolder));
        e.sub.set_text("client");
        e.step(Row::Mark, 1);
        assert_eq!(
            e.recipe().destination,
            Destination::Subfolder("client".into())
        );
        e.step(Row::Folder, 3);
        assert_eq!(e.r.destination, Destination::Ask);
        // A bad pattern stops saving with its reason.
        e.pattern.set_text("{nope}");
        assert!(e.problem().is_some());
    }

    /// A starter left as it was keeps following the language; renamed, it keeps its name.
    #[test]
    fn starter_names() {
        let web = Recipe::starters().remove(0);
        let e = Editor::new(Some(0), web.clone());
        assert_eq!(e.name.text(), t!("export-recipes-starter-web"));
        assert_eq!(e.recipe().name, "");
        let mut e = Editor::new(Some(0), web);
        e.name.set_text("Blog");
        assert_eq!(e.recipe().name, "Blog");
        assert_eq!(recipe_name(&e.recipe()), "Blog");
        assert!(summary(&Recipe::starters()[0]).contains("2048"));
    }

    #[test]
    fn report_lists_failures_plainly() {
        let recipes = Recipe::starters();
        let rep = RecipeReport {
            results: vec![
                (
                    PathBuf::from("/p/a.jpg"),
                    0,
                    Outcome::Converted {
                        out: PathBuf::from("/p/web/a.jpg"),
                        bytes: 1,
                    },
                ),
                (
                    PathBuf::from("/p/b.cr3"),
                    1,
                    Outcome::Failed(specular_convert::Failure::Decode("truncated".into())),
                ),
            ],
            cancelled: false,
            elapsed: std::time::Duration::from_secs(1),
        };
        let (summary, lines) = report_lines(&rep, &recipes);
        assert_eq!(summary, t!("export-recipes-done", count = 1));
        assert!(lines.iter().any(|l| l.contains("b.cr3")
            && l.contains(&t!("export-recipes-starter-print"))
            && l.contains("truncated")));
    }
}
