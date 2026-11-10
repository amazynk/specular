//! The inspection report: the thermal report's dialog and layout for any photo (thermal or
//! visible), with the measurements drawn on it and listed, its facts and place, and the
//! folder's other photos when asked; saved as PDF, and on request also as a Word document
//! and a KMZ of the photos' places beside it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Orientation};
use specular_decode::{DecodeRequest, Session};
use specular_i18n::{I18n, t};
use specular_markup::report;
use specular_measure::geom::Shape;
use specular_widgets::line_edit::LineEdit;

use super::super::super::App;
use super::super::super::gallery::drone;
use super::{Dialog, MAX_IMAGES, PHOTO_W, folder_images, view};

/// The photo shown, as it goes into the report.
pub(super) struct PhotoCurrent {
    path: PathBuf,
    session: Arc<dyn Session>,
    /// Number, name, value, scale source and shape of each measurement (stored pixels).
    rows: Vec<(u32, String, String, String, Shape)>,
}

/// The words of the inspection report, taken on the UI thread.
#[derive(Clone)]
struct Words {
    caption: String,
    no_table: String,
    head: Vec<String>,
}

/// Where a stored pixel lands once the image is turned upright (`w` × `h` stored).
fn upright(p: (f64, f64), w: f64, h: f64, o: Orientation) -> (f64, f64) {
    if o == Orientation::NORMAL {
        return p;
    }
    let (q, flip) = o.to_rotation_flip();
    let (mut x, mut y, mut cw, mut ch) = (p.0, p.1, w, h);
    if flip {
        x = cw - x;
    }
    for _ in 0..q {
        let nx = ch - y;
        y = x;
        x = nx;
        std::mem::swap(&mut cw, &mut ch);
    }
    (x, y)
}

/// The photo upright, at most [`PHOTO_W`] wide, and the stored pixels per its pixel.
fn picture(session: &dyn Session) -> Option<(report::Image, f64)> {
    let info = session.info();
    let scale = [1u32, 2, 4, 8, 16]
        .into_iter()
        .find(|s| info.width.max(info.height) / s <= PHOTO_W)
        .unwrap_or(16);
    let r = session
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .ok()?;
    let k = f64::from(info.width) / f64::from(r.width.max(1));
    let (w, h, rgba) = crate::clipboard::oriented_rgba(&r, info.orientation);
    Some((
        report::Image {
            rgb: rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect(),
            width: w as u32,
            height: h as u32,
        },
        k,
    ))
}

/// The facts of a photo: camera, lens, date, place and drone groups of its metadata.
fn facts(session: &dyn Session, lang: &specular_i18n::LanguageIdentifier) -> Vec<(String, String)> {
    let loc = crate::loader::meta_localizer(lang.clone());
    let (meta, _) = crate::loader::metadata_report(session, &loc);
    let mut out: Vec<(String, String)> = meta
        .summary
        .iter()
        .filter(|g| matches!(g.key, "camera" | "lens" | "date" | "location" | "drone"))
        .flat_map(|g| g.items.iter().map(|i| (i.label.clone(), i.value.clone())))
        .take(14)
        .collect();
    if let Some(g) = &meta.gps
        && !out
            .iter()
            .any(|(_, v)| v.contains(&format!("{:.4}", g.lat)))
    {
        out.push((
            t!("thermal-report-place"),
            format!("{:.5}, {:.5}", g.lat, g.lon),
        ));
    }
    out
}

/// One photo's page.
fn entry(
    w: &Words,
    path: &Path,
    session: &dyn Session,
    rows: &[(u32, String, String, String, Shape)],
    lang: &specular_i18n::LanguageIdentifier,
) -> Option<report::Entry> {
    let (image, k) = picture(session)?;
    let info = session.info();
    let (sw, sh) = (f64::from(info.width), f64::from(info.height));
    let mut table = vec![w.head.clone()];
    let mut marks = Vec::new();
    for (id, name, value, source, shape) in rows {
        table.push(vec![
            id.to_string(),
            name.clone(),
            value.clone(),
            source.clone(),
        ]);
        let pts: Vec<(f64, f64)> = match shape {
            Shape::Point(p) => vec![(p.x, p.y)],
            s => s.outline().iter().map(|p| (p.x, p.y)).collect(),
        };
        marks.push(report::Mark {
            points: pts
                .into_iter()
                .map(|p| {
                    let (x, y) = upright(p, sw, sh, info.orientation);
                    (x / k, y / k)
                })
                .collect(),
            closed: shape.is_area(),
            label: id.to_string(),
        });
    }
    Some(report::Entry {
        title: path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        thermal: image,
        marks,
        scale: None,
        visible: None,
        thermal_caption: if rows.is_empty() {
            String::new()
        } else {
            w.caption.clone()
        },
        visible_caption: String::new(),
        facts: facts(session, lang),
        table,
        no_table: w.no_table.clone(),
    })
}

/// Writes the PDF to `out`, and beside it the Word document and the KMZ when asked; the
/// file name and the number of pages, or why it failed.
pub(super) fn write_outputs(
    out: &Path,
    r: &report::Report,
    pdf: &[u8],
    docx: bool,
    kmz: Option<&[PathBuf]>,
) -> Result<(String, usize), String> {
    let io = |e: std::io::Error| crate::error_text::io_reason(&e);
    std::fs::write(out, pdf).map_err(io)?;
    if docx {
        let bytes = specular_markup::docx::write(r);
        if bytes.is_empty() {
            return Err(t!("inspection-report-docx-failed"));
        }
        std::fs::write(out.with_extension("docx"), bytes).map_err(io)?;
    }
    if let Some(paths) = kmz {
        let marks: Vec<drone::Placemark> =
            paths.iter().filter_map(|p| drone::placemark(p)).collect();
        if !marks.is_empty() {
            let f = std::fs::File::create(out.with_extension("kmz")).map_err(io)?;
            drone::write_kmz(
                std::io::BufWriter::new(f),
                &r.title,
                &marks,
                drone::Pictures::Thumbnails,
                drone::thumbnail,
            )
            .map_err(io)?;
        }
    }
    let name = out
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Ok((name, r.entries.len()))
}

impl App {
    /// Opens the report dialog for the photo shown (not a thermal image).
    pub(in crate::app) fn inspection_report_open(&mut self) {
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.thermal_report.dialog = Some(Dialog {
            title: LineEdit::new(&t!("inspection-report-default-title")),
            notes: LineEdit::new(""),
            photo: None,
            folder: false,
            docx: false,
            kmz: false,
            inspection: true,
            focus: view::Part::Title,
            preedit: String::new(),
            subtitle: name,
            drawn: view::Drawn::default(),
        });
        self.redraw = true;
    }

    fn inspection_current(&self) -> Option<PhotoCurrent> {
        let path = self.current.and_then(|i| self.entries.get(i)).cloned()?;
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.clone())?;
        Some(PhotoCurrent {
            path,
            session,
            rows: self.measure_report_rows(),
        })
    }

    /// Asks where to save, then writes the inspection report in the background.
    pub(super) fn inspection_report_save(&mut self) {
        let Some(cur) = self.inspection_current() else {
            let now = self.now();
            self.chrome.toast(t!("thermal-report-nothing"), None, now);
            return;
        };
        let Some(d) = self.thermal_report.dialog.as_ref() else {
            return;
        };
        let rtl = self.rtl();
        let lang = I18n::global().localizer().format_language().clone();
        let words = Words {
            caption: t!("inspection-report-caption"),
            no_table: t!("thermal-report-no-measurements"),
            head: vec![
                "#".into(),
                t!("thermal-report-col-measurement"),
                t!("inspection-report-col-value"),
                t!("inspection-report-col-scale"),
            ],
        };
        let title = d.title.text().trim().to_owned();
        let title = if title.is_empty() {
            t!("inspection-report-default-title")
        } else {
            title
        };
        let date = specular_markup::now_local();
        let subtitle = date.get(..16).unwrap_or(&date).replacen('T', " ", 1);
        let created = specular_markup::pdf_date(&date);
        let notes = d.notes.text().to_owned();
        let (folder, docx, kmz) = (d.folder, d.docx, d.kmz);
        let notes_heading = t!("thermal-report-notes");
        let table_heading = t!("thermal-report-measurements");
        let stem = cur.path.file_stem().map_or_else(
            || "inspection".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-inspect-rep", move |l| {
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
            let mut paths = vec![cur.path.clone()];
            let mut entries: Vec<report::Entry> =
                entry(&words, &cur.path, cur.session.as_ref(), &cur.rows, &lang)
                    .into_iter()
                    .collect();
            if folder {
                let reg = specular_decode::Registry::new();
                for p in folder_images(&cur.path) {
                    if entries.len() > MAX_IMAGES {
                        break;
                    }
                    let Ok(s) = reg.open_path(&p) else { continue };
                    if s.info().width == 0 {
                        continue;
                    }
                    if let Some(e) = entry(&words, &p, s.as_ref(), &[], &lang) {
                        entries.push(e);
                        paths.push(p);
                    }
                }
            }
            if entries.is_empty() {
                let _ = tx.send(Err(t!("inspection-report-unreadable")));
                l.waker()();
                return;
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
            let outlines = |s: &str, size: f64, bold: bool| {
                super::super::super::markup::outline_text(&text, s, size, bold)
            };
            let pdf = report::write(&report, Some(&outlines));
            let r = write_outputs(&out, &report, &pdf, docx, kmz.then_some(paths.as_slice()));
            specular_log::debug!(target: specular_log::UI, "inspection report: {n} images in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            let _ = tx.send(r);
            l.waker()();
        });
        self.thermal_report.job = Some(rx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_turn_upright() {
        // Orientation 6: a quarter turn clockwise; a 4 × 2 image becomes 2 × 4.
        let o = Orientation::from_exif(6);
        assert_eq!(upright((0.0, 0.0), 4.0, 2.0, o), (2.0, 0.0));
        assert_eq!(upright((4.0, 2.0), 4.0, 2.0, o), (0.0, 4.0));
        assert_eq!(
            upright((1.0, 1.0), 4.0, 2.0, Orientation::NORMAL),
            (1.0, 1.0)
        );
    }

    /// The PDF, the Word document and the KMZ land side by side; a photo without a place
    /// is left out of the KMZ.
    #[test]
    fn outputs_land_beside_the_pdf() {
        use std::io::Read;
        let dir = std::env::temp_dir().join(format!("specular-insp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.jpg");
        std::fs::write(&a, drone::tests::fixture(Some(45.0), None)).unwrap();
        let b = dir.join("b.jpg");
        std::fs::write(&b, [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        let img = report::Image {
            rgb: vec![128; 8 * 6 * 3],
            width: 8,
            height: 6,
        };
        let r = report::Report {
            title: "T".into(),
            subtitle: String::new(),
            notes: String::new(),
            notes_heading: "Notes".into(),
            table_heading: "Measurements".into(),
            entries: vec![report::Entry {
                title: "a.jpg".into(),
                thermal: img,
                marks: Vec::new(),
                scale: None,
                visible: None,
                thermal_caption: String::new(),
                visible_caption: String::new(),
                facts: Vec::new(),
                table: vec![vec!["#".into()]],
                no_table: "None".into(),
            }],
            page_labels: vec!["1".into()],
            rtl: false,
            created: None,
            producer: "test".into(),
        };
        let out = dir.join("r.pdf");
        let (name, n) =
            write_outputs(&out, &r, b"%PDF-1.4", true, Some(&[a.clone(), b.clone()])).unwrap();
        assert_eq!((name.as_str(), n), ("r.pdf", 1));
        assert!(dir.join("r.docx").is_file());
        let mut z = zip::ZipArchive::new(std::fs::File::open(dir.join("r.kmz")).unwrap()).unwrap();
        let mut kml = String::new();
        z.by_name("doc.kml")
            .unwrap()
            .read_to_string(&mut kml)
            .unwrap();
        assert_eq!(kml.matches("<Placemark>").count(), 1);
        assert!(kml.contains("<heading>45.0</heading>"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
