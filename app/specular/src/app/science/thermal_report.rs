//! The thermal report ([`Action::ThermalReport`]): a PDF of the thermal image shown, and on
//! request of the other thermal stills in its folder, laid out by
//! [`specular_markup::report`].
//!
//! - The dialog asks for the report's title and notes, whether to include the visible photo
//!   and whether to add the folder's other thermal images.
//! - The image shown goes in as it looks: the frame shown, its palette between the limits
//!   shown, the measurements drawn on it with their statistics in the table (spots, lines,
//!   areas, and the whole image), in the temperature unit shown, with the object parameters
//!   used. Other images go in with their own parameters and the whole image's statistics.
//! - Facts: camera and lens, date, place (GPS), object parameters.
//! - Text Helvetica cannot write is set as outlines by the viewer's text system, so Hebrew
//!   and Arabic reports read right to left with nothing embedded.
//!
//! [`Action::ThermalReport`]: specular_input::Action::ThermalReport

mod inspection;
#[cfg(test)]
mod shots;
pub(in crate::app) mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::Receiver;
use specular_core::CancelToken;
use specular_decode::science::{ColourTable, NdPlane, TemperatureUnit, ThermalConditions};
use specular_decode::{DecodeRequest, PageLabel, Session};
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_markup::report;
use specular_measure::geom::Shape;
use specular_measure::stats::ChannelStats;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use self::view::{FieldView, Part, ReportView};
use super::super::App;
use super::trimmed;

/// Most other images a report takes from the folder.
const MAX_IMAGES: usize = 60;
/// Widest a visible photo is put in the report, pixels.
const PHOTO_W: u32 = 1600;

/// The open dialog.
pub(in crate::app) struct Dialog {
    title: LineEdit,
    notes: LineEdit,
    photo: Option<bool>,
    folder: bool,
    /// Also save a Word document, and a KMZ of the photos' places, beside the PDF.
    docx: bool,
    kmz: bool,
    /// The inspection report of any photo (not the thermal image's).
    inspection: bool,
    focus: Part,
    /// An input method's composition for the focused field (empty: none).
    preedit: String,
    /// The image's name (and frame), for the subtitle.
    subtitle: String,
    pub drawn: view::Drawn,
}

/// The dialog and a report being written.
#[derive(Default)]
pub(in crate::app) struct ReportState {
    pub dialog: Option<Dialog>,
    job: Option<Receiver<Result<(String, usize), String>>>,
}

impl ReportState {
    pub(in crate::app) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// An input method's composition (empty: none) for the field with the keys; nothing
    /// composes when a field does not have them.
    pub(in crate::app) fn set_preedit(&mut self, s: &str) {
        if let Some(d) = self.dialog.as_mut() {
            d.preedit = if d.focus.is_field() {
                s.to_owned()
            } else {
                String::new()
            };
        }
    }

    /// Where the input method's candidate window opens: the focused field's composition.
    pub(in crate::app) fn ime_area(&self) -> Option<specular_gfx::Rect> {
        let d = self.dialog.as_ref()?;
        d.focus.is_field().then_some(d.drawn.ime).flatten()
    }
}

/// The image shown, as it goes into the report.
struct Current {
    path: PathBuf,
    session: Arc<dyn Session>,
    plane: Arc<NdPlane>,
    limits: (f32, f32),
    lut: Arc<ColourTable>,
    unit: TemperatureUnit,
    conditions: ThermalConditions,
    regions: Vec<(u32, String, Shape, Option<ChannelStats>)>,
    /// "Frame 3 of 7 · 0.12 s" for a sequence.
    frame: Option<String>,
}

/// The words the report uses, taken on the UI thread (the job has no localiser).
#[derive(Clone)]
struct Words {
    thermal_caption: String,
    visible_caption: String,
    no_table: String,
    head: Vec<String>,
    whole: String,
    camera: String,
    lens: String,
    date: String,
    place: String,
    frame: String,
    emissivity: String,
    reflected: String,
    distance: String,
    humidity: String,
    range: String,
    format: String,
}

impl Words {
    fn new() -> Words {
        Words {
            thermal_caption: t!("thermal-report-thermal-caption"),
            visible_caption: t!("thermal-report-visible-caption"),
            no_table: t!("thermal-report-no-measurements"),
            head: vec![
                "#".into(),
                t!("thermal-report-col-measurement"),
                t!("thermal-report-col-min"),
                t!("thermal-report-col-max"),
                t!("thermal-report-col-mean"),
                t!("thermal-report-col-emissivity"),
            ],
            whole: t!("thermal-report-whole-image"),
            camera: t!("thermal-report-camera"),
            lens: t!("thermal-report-lens"),
            date: t!("thermal-report-date"),
            place: t!("thermal-report-place"),
            frame: t!("thermal-report-frame"),
            emissivity: t!("thermal-report-emissivity"),
            reflected: t!("thermal-report-reflected"),
            distance: t!("thermal-report-distance"),
            humidity: t!("thermal-report-humidity"),
            range: t!("thermal-report-range"),
            format: t!("thermal-report-format"),
        }
    }
}

/// A value with its unit ("36.4 °C"), NaN as a dash.
fn temp(v: f64, unit: TemperatureUnit) -> String {
    if v.is_finite() {
        format!("{} {}", trimmed(v, 1), unit.symbol())
    } else {
        "–".to_owned()
    }
}

/// The plane's pixels through `lut` between `lo` and `hi`, as RGB.
fn colour(p: &NdPlane, lut: &ColourTable, (lo, hi): (f32, f32)) -> report::Image {
    let k = if hi > lo { 255.0 / (hi - lo) } else { 0.0 };
    let mut rgb = Vec::with_capacity(p.values.len() * 3);
    for v in &p.values {
        if v.is_finite() {
            rgb.extend_from_slice(&lut[((v - lo) * k).round().clamp(0.0, 255.0) as usize]);
        } else {
            rgb.extend_from_slice(&[0, 0, 0]);
        }
    }
    report::Image {
        rgb,
        width: p.width,
        height: p.height,
    }
}

/// The visible photo (page 2 of a thermal still), at most [`PHOTO_W`] wide.
fn photo(session: &dyn Session) -> Option<report::Image> {
    if session.page_count() < 2
        || session.page_label() != Some(PageLabel::Thermal { visible: false })
    {
        return None;
    }
    let page = session.open_page(1).ok()?;
    let w = page.info().width;
    let scale = [1, 2, 4, 8]
        .into_iter()
        .find(|s| w / s <= PHOTO_W)
        .unwrap_or(8);
    let r = page
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .ok()?;
    let (w, h, rgba) = crate::clipboard::oriented_rgba(&r, page.info().orientation);
    Some(report::Image {
        rgb: rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect(),
        width: w as u32,
        height: h as u32,
    })
}

/// The whole plane's minimum, maximum and mean.
fn whole(p: &NdPlane) -> (f64, f64, f64) {
    let (mut lo, mut hi, mut sum, mut n) = (f64::MAX, f64::MIN, 0.0, 0u64);
    for &v in p.values.iter().filter(|v| v.is_finite()) {
        let v = f64::from(v);
        lo = lo.min(v);
        hi = hi.max(v);
        sum += v;
        n += 1;
    }
    if n == 0 {
        (f64::NAN, f64::NAN, f64::NAN)
    } else {
        (lo, hi, sum / n as f64)
    }
}

/// One image's page of the report.
#[allow(clippy::too_many_arguments)]
fn entry(
    w: &Words,
    path: &Path,
    session: &dyn Session,
    plane: &NdPlane,
    limits: (f32, f32),
    lut: &ColourTable,
    unit: TemperatureUnit,
    c: &ThermalConditions,
    regions: &[(u32, String, Shape, Option<ChannelStats>)],
    frame: Option<&str>,
    with_photo: bool,
    lang: &specular_i18n::LanguageIdentifier,
) -> report::Entry {
    let sci = session.science();
    let t = sci.and_then(|s| s.thermal.clone());
    let e = |v: Option<f64>| v.map_or_else(|| "–".to_owned(), |v| trimmed(v, 3));
    let mut table = vec![w.head.clone()];
    let (lo, hi, mean) = whole(plane);
    table.push(vec![
        String::new(),
        w.whole.clone(),
        temp(lo, unit),
        temp(hi, unit),
        temp(mean, unit),
        e(c.emissivity),
    ]);
    let mut marks = Vec::new();
    for (id, name, shape, st) in regions {
        let (lo, hi, mean) = st
            .as_ref()
            .map_or((f64::NAN, f64::NAN, f64::NAN), |s| (s.min, s.max, s.mean));
        table.push(vec![
            id.to_string(),
            name.clone(),
            temp(lo, unit),
            temp(hi, unit),
            temp(mean, unit),
            e(c.emissivity),
        ]);
        let pts: Vec<(f64, f64)> = match shape {
            Shape::Point(p) => vec![(p.x, p.y)],
            s => s.outline().iter().map(|p| (p.x, p.y)).collect(),
        };
        marks.push(report::Mark {
            points: pts,
            closed: shape.is_area(),
            label: id.to_string(),
        });
    }
    let mut facts = Vec::new();
    if let Some(t) = &t {
        let cam = t.camera();
        if !cam.model.is_empty() {
            facts.push((w.camera.clone(), cam.model.clone()));
        }
        if !cam.lens.is_empty() {
            facts.push((w.lens.clone(), cam.lens.clone()));
        }
        if let Some(d) = &cam.date {
            facts.push((w.date.clone(), d.clone()));
        }
        facts.push((w.format.clone(), t.kind_name().to_owned()));
    }
    let loc = crate::loader::meta_localizer(lang.clone());
    let (meta, _) = crate::loader::metadata_report(session, &loc);
    if let Some(g) = &meta.gps {
        facts.push((w.place.clone(), format!("{:.5}, {:.5}", g.lat, g.lon)));
    }
    if let Some(f) = frame {
        facts.push((w.frame.clone(), f.to_owned()));
    }
    if t.as_ref().is_some_and(|t| t.adjustable()) {
        facts.push((w.emissivity.clone(), e(c.emissivity)));
        if let Some(r) = c.reflected {
            facts.push((w.reflected.clone(), temp(unit.from_celsius(r), unit)));
        }
        if let Some(d) = c.distance {
            facts.push((
                w.distance.clone(),
                format!("{} {}", trimmed(d, 2), specular_i18n::unit_symbol("m")),
            ));
        }
        if let Some(h) = c.humidity {
            facts.push((
                w.humidity.clone(),
                specular_i18n::I18n::global().localizer().percent(h, 1),
            ));
        }
    }
    facts.push((
        w.range.clone(),
        format!("{} – {}", temp(lo, unit), temp(hi, unit)),
    ));
    report::Entry {
        title: path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        thermal: colour(plane, lut, limits),
        marks,
        scale: Some(report::Scale {
            colours: lut.to_vec(),
            low: temp(f64::from(limits.0), unit),
            high: temp(f64::from(limits.1), unit),
        }),
        visible: if with_photo { photo(session) } else { None },
        thermal_caption: w.thermal_caption.clone(),
        visible_caption: w.visible_caption.clone(),
        facts,
        table,
        no_table: w.no_table.clone(),
    }
}

/// The folder's other thermal stills, by name, at most [`MAX_IMAGES`].
fn folder_images(current: &Path) -> Vec<PathBuf> {
    let Some(dir) = current.parent() else {
        return Vec::new();
    };
    let reg = specular_decode::Registry::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| p != current && p.is_file() && reg.supports_extension(p))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files.truncate(MAX_IMAGES * 4);
    files
}

impl App {
    /// Opens the report dialog for the thermal image shown.
    pub(super) fn thermal_report_open(&mut self) {
        let Some(t) = self.science.thermal().cloned() else {
            return;
        };
        let name = self
            .science
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let photo = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .is_some_and(|i| {
                i.page_label == Some(PageLabel::Thermal { visible: false }) && i.pages > 1
            })
            .then_some(true);
        let subtitle = match self.thermal_seq_panel() {
            Some(p) if t.frame_count() > 1 => {
                specular_i18n::dot_join::<&str>(&[&name, &p.position])
            }
            _ => name,
        };
        self.thermal_report.dialog = Some(Dialog {
            title: LineEdit::new(&t!("thermal-report-default-title")),
            notes: LineEdit::new(""),
            photo,
            folder: false,
            docx: false,
            kmz: false,
            inspection: false,
            focus: Part::Title,
            preedit: String::new(),
            subtitle,
            drawn: view::Drawn::default(),
        });
        self.redraw = true;
    }

    /// The dialog as drawn.
    pub(in crate::app) fn thermal_report_view(&self) -> Option<ReportView> {
        let d = self.thermal_report.dialog.as_ref()?;
        Some(build_view(d, self.thermal_report.job.is_some()))
    }

    /// Keys while the dialog is open (it takes them all); `text` is what the key types.
    pub(in crate::app) fn thermal_report_key(
        &mut self,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
    ) {
        let Some(d) = self.thermal_report.dialog.as_ref() else {
            return;
        };
        let parts = build_view(d, false).parts();
        let focus = d.focus;
        if focus.is_field() && self.thermal_report_field_key(combo, text) {
            return;
        }
        let Some(combo) = combo else { return };
        let step = |me: &mut App, k: i32| {
            if let Some(d) = me.thermal_report.dialog.as_mut() {
                let i = parts.iter().position(|p| *p == d.focus).unwrap_or(0) as i32;
                d.focus = parts[(i + k).rem_euclid(parts.len() as i32) as usize];
                d.preedit.clear();
            }
        };
        match combo.key {
            Key::Named(NamedKey::Escape) => self.thermal_report_hit(Part::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => step(self, -1),
            Key::Named(NamedKey::Tab) => step(self, 1),
            Key::Named(NamedKey::ArrowUp) if combo.mods == Mods::NONE => step(self, -1),
            Key::Named(NamedKey::ArrowDown) if combo.mods == Mods::NONE => step(self, 1),
            Key::Named(NamedKey::Enter) => {
                self.thermal_report_hit(if focus.is_field() { Part::Save } else { focus })
            }
            Key::Named(NamedKey::Space) if !focus.is_field() => self.thermal_report_hit(focus),
            _ => {}
        }
        self.redraw = true;
    }

    /// Editing keys in the focused field; false when the key is not one.
    fn thermal_report_field_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(d) = self.thermal_report.dialog.as_mut() else {
            return false;
        };
        let focus = d.focus;
        let rtl = d
            .drawn
            .fields
            .iter()
            .find(|(p, ..)| *p == focus)
            .is_some_and(|(_, b, _)| b.rtl);
        let field = if focus == Part::Title {
            &mut d.title
        } else {
            &mut d.notes
        };
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            if field.key(c.key, c.mods, rtl, &mut clip).used() {
                self.field_clipboard(clip, super::super::integration::PasteTo::ThermalReport);
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
        self.thermal_report_text(&s);
        true
    }

    /// Whether typed text (input-method commits) goes to the dialog.
    pub(in crate::app) fn thermal_report_wants_text(&self) -> bool {
        self.thermal_report
            .dialog
            .as_ref()
            .is_some_and(|d| d.focus.is_field())
    }

    /// Text into the focused field (typed keys, input-method commits, pastes).
    pub(in crate::app) fn thermal_report_text(&mut self, s: &str) {
        if let Some(d) = self.thermal_report.dialog.as_mut() {
            d.preedit.clear();
            let used = match d.focus {
                Part::Title => d.title.insert(s),
                Part::Notes => d.notes.insert(s),
                _ => false,
            };
            self.redraw |= used;
        }
    }

    /// Text pasted into the focused field.
    pub(in crate::app) fn thermal_report_paste(&mut self, s: &str) {
        if let Some(d) = self.thermal_report.dialog.as_mut() {
            match d.focus {
                Part::Title => d.title.paste(s),
                Part::Notes => d.notes.paste(s),
                _ => false,
            };
            self.redraw = true;
        }
    }

    /// A click (logical px) while the dialog is open.
    /// A press in a field puts the caret (Shift extends, two select the word, three all);
    /// a drag ([`App::thermal_report_drag`]) selects.
    pub(in crate::app) fn thermal_report_click(&mut self, p: (f32, f32), down: bool) {
        let now = self.now();
        let shift = self.mods.shift_key();
        let Some(d) = self.thermal_report.dialog.as_mut() else {
            return;
        };
        if !down {
            d.title.release();
            d.notes.release();
            return;
        }
        let Some(&(_, part)) = d.drawn.hits.iter().find(|(r, _)| r.contains(p)) else {
            return;
        };
        if part != d.focus {
            d.preedit.clear();
        }
        if part.is_field() {
            d.focus = part;
            if let Some((_, block, x0)) = d.drawn.fields.iter().find(|(q, ..)| *q == part) {
                let at = block.hit_byte(p.0 - x0);
                let f = if part == Part::Title {
                    &mut d.title
                } else {
                    &mut d.notes
                };
                f.press(at.min(f.text().len()), p.0, now, shift);
            }
            self.redraw = true;
            return;
        }
        self.thermal_report_hit(part);
    }

    /// The pointer moved while the dialog is open: a press in a field drags a selection.
    pub(in crate::app) fn thermal_report_drag(&mut self, p: (f32, f32)) {
        let Some(d) = self.thermal_report.dialog.as_mut() else {
            return;
        };
        for (part, block, x0) in &d.drawn.fields {
            let f = if *part == Part::Title {
                &mut d.title
            } else {
                &mut d.notes
            };
            if f.dragging() {
                let at = block.hit_byte(p.0 - x0).min(f.text().len());
                f.drag_to(at);
                self.redraw = true;
            }
        }
    }

    /// A screen reader's request on the dialog; true when the node is the dialog's.
    pub(in crate::app) fn thermal_report_a11y_request(
        &mut self,
        req: &accesskit::ActionRequest,
    ) -> bool {
        if self.thermal_report.dialog.is_none() {
            return false;
        }
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return false;
        };
        match req.action {
            accesskit::Action::Focus => {
                if let Some(d) = self.thermal_report.dialog.as_mut() {
                    d.focus = part;
                    d.preedit.clear();
                }
            }
            accesskit::Action::Click => self.thermal_report_hit(part),
            accesskit::Action::SetValue => {
                if let (Some(d), Some(accesskit::ActionData::Value(v))) =
                    (self.thermal_report.dialog.as_mut(), req.data.as_ref())
                {
                    match part {
                        Part::Title => d.title.set_text(v),
                        Part::Notes => d.notes.set_text(v),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A button or checkbox.
    fn thermal_report_hit(&mut self, part: Part) {
        let Some(d) = self.thermal_report.dialog.as_mut() else {
            return;
        };
        d.focus = part;
        match part {
            Part::Photo => d.photo = d.photo.map(|on| !on),
            Part::Folder => d.folder = !d.folder,
            Part::Docx => d.docx = !d.docx,
            Part::Kmz => d.kmz = !d.kmz,
            Part::Cancel => self.thermal_report.dialog = None,
            Part::Save if self.thermal_report.job.is_none() => {
                if d.inspection {
                    self.inspection_report_save();
                } else {
                    self.thermal_report_save();
                }
            }
            _ => {}
        }
        self.redraw = true;
    }

    /// Asks where to save, then writes the report on the dialog's thread.
    fn thermal_report_save(&mut self) {
        let Some(cur) = self.thermal_report_current() else {
            let now = self.now();
            self.chrome.toast(t!("thermal-report-nothing"), None, now);
            return;
        };
        let Some(d) = self.thermal_report.dialog.as_ref() else {
            return;
        };
        let rtl = self.rtl();
        let lang = I18n::global().localizer().format_language().clone();
        let words = Words::new();
        let title = d.title.text().trim().to_owned();
        let title = if title.is_empty() {
            t!("thermal-report-default-title")
        } else {
            title
        };
        let date = specular_markup::now_local();
        // "2026-10-25 14:03" (the local time the report was made).
        let subtitle = date.get(..16).unwrap_or(&date).replacen('T', " ", 1);
        let created = specular_markup::pdf_date(&date);
        let notes = d.notes.text().to_owned();
        let with_photo = d.photo.unwrap_or(false);
        let folder = d.folder;
        let (docx, kmz) = (d.docx, d.kmz);
        let notes_heading = t!("thermal-report-notes");
        let table_heading = t!("thermal-report-measurements");
        let stem = cur.path.file_stem().map_or_else(
            || "thermal".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-therm-rep", move |l| {
            let Some(out) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("{stem}-report.pdf"))
                .save_file()
            else {
                let _ = tx.send(Err(String::new()));
                l.waker()();
                return;
            };
            let t0 = Instant::now();
            let mut entries = vec![entry(
                &words,
                &cur.path,
                cur.session.as_ref(),
                &cur.plane,
                cur.limits,
                &cur.lut,
                cur.unit,
                &cur.conditions,
                &cur.regions,
                cur.frame.as_deref(),
                with_photo,
                &lang,
            )];
            let mut paths = vec![cur.path.clone()];
            if folder {
                let reg = specular_decode::Registry::new();
                for p in folder_images(&cur.path) {
                    if entries.len() > MAX_IMAGES {
                        break;
                    }
                    let Ok(s) = reg.open_path(&p) else { continue };
                    let Some(sci) = s.science() else { continue };
                    let (Some(t), Some(lut)) = (sci.thermal.clone(), sci.luts.first().cloned())
                    else {
                        continue;
                    };
                    if t.frame_count() > 1 {
                        continue;
                    }
                    let c = t.file_conditions();
                    let Ok(plane) = t.plane(&c, cur.unit, &CancelToken::new()) else {
                        continue;
                    };
                    let limits = specular_decode::science::ThermalImage::range_of(&plane)
                        .unwrap_or((0.0, 1.0));
                    entries.push(entry(
                        &words, &p, s.as_ref(), &plane, limits, &lut, cur.unit, &c, &[], None,
                        with_photo, &lang,
                    ));
                    paths.push(p);
                }
            }
            let n = entries.len();
            let report = report::Report {
                title,
                subtitle,
                notes,
                notes_heading,
                table_heading,
                page_labels: (1..=n)
                    .map(|i| t!("thermal-report-page", page = i as i64, pages = n as i64))
                    .collect(),
                entries,
                rtl,
                created: Some(created),
                producer: format!("Specular {}", env!("CARGO_PKG_VERSION")),
            };
            let text = std::cell::RefCell::new(None);
            let outlines =
                |s: &str, size: f64, bold: bool| super::super::markup::outline_text(&text, s, size, bold);
            let pdf = report::write(&report, Some(&outlines));
            let r = inspection::write_outputs(
                &out,
                &report,
                &pdf,
                docx,
                kmz.then_some(paths.as_slice()),
            );
            specular_log::debug!(target: specular_log::UI, "thermal report: {n} images in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            let _ = tx.send(r);
            l.waker()();
        });
        self.thermal_report.job = Some(rx);
    }

    /// What of the image shown goes into the report.
    fn thermal_report_current(&self) -> Option<Current> {
        let sc = &self.science;
        let summary = sc.summary.as_ref()?;
        let pl = sc.planes.first()?;
        let (conditions, unit) = sc.shown.as_ref()?.thermal?;
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())?;
        let limits = pl.limits.unwrap_or_else(|| {
            specular_decode::science::ThermalImage::range_of(&pl.plane).unwrap_or((0.0, 1.0))
        });
        Some(Current {
            path: sc.path.clone()?,
            session,
            plane: pl.plane.clone(),
            limits,
            lut: summary.luts.first()?.clone(),
            unit,
            conditions,
            regions: self.measure_regions(),
            frame: self.thermal_seq_panel().map(|p| p.position),
        })
    }

    /// Reports a finished report and closes the dialog.
    pub(in crate::app) fn thermal_report_poll(&mut self) {
        let Some(Ok(r)) = self.thermal_report.job.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.thermal_report.job = None;
        let now = self.now();
        match r {
            Ok((name, n)) => {
                self.thermal_report.dialog = None;
                self.chrome.toast(
                    t!("thermal-report-saved", name = name, count = n as i64),
                    None,
                    now,
                );
            }
            // The save dialog was closed: the report dialog stays.
            Err(e) if e.is_empty() => {}
            Err(e) => {
                self.chrome
                    .toast(t!("thermal-report-failed", reason = e), None, now);
            }
        }
        self.redraw = true;
    }

    /// `--show thermal-report` (screenshots): the dialog for the image shown.
    pub(in crate::app) fn thermal_report_show(&mut self, what: &str) -> bool {
        if what != "thermal-report" {
            return false;
        }
        self.thermal_report_open();
        true
    }
}

fn field(label: String, f: &LineEdit, preedit: &str) -> FieldView {
    FieldView {
        label,
        text: f.text().to_owned(),
        caret: f.caret(),
        selection: f.selection(),
        preedit: preedit.to_owned(),
    }
}

fn build_view(d: &Dialog, busy: bool) -> ReportView {
    ReportView {
        heading: if d.inspection {
            t!("inspection-report-heading")
        } else {
            t!("thermal-report-heading")
        },
        subtitle: d.subtitle.clone(),
        title: field(
            t!("thermal-report-title-label"),
            &d.title,
            if d.focus == Part::Title {
                &d.preedit
            } else {
                ""
            },
        ),
        notes: field(
            t!("thermal-report-notes-label"),
            &d.notes,
            if d.focus == Part::Notes {
                &d.preedit
            } else {
                ""
            },
        ),
        photo: d.photo.map(|on| (t!("thermal-report-photo"), on)),
        folder: (
            if d.inspection {
                t!("inspection-report-folder")
            } else {
                t!("thermal-report-folder")
            },
            d.folder,
        ),
        docx: (t!("inspection-report-docx"), d.docx),
        kmz: (t!("inspection-report-kmz"), d.kmz),
        busy: busy.then(|| t!("thermal-report-busy")),
        cancel: t!("app-cancel"),
        save: t!("thermal-report-save"),
        focus: d.focus,
    }
}
