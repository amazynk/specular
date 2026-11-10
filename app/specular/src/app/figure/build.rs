//! The pure part of figures and key images: from what the viewer captured (pixels read back
//! from the display path, measurements in stored pixels, the calibration and the display
//! settings) to a [`Spec`], and from key images to slides, a folder or one tiled picture.
//! No GPU and no app state here, so the tests run on synthetic inputs.

use std::fmt::Write as _;

use specular_dicom::mask::Rect as Area;
use specular_figure::compose::{Mark, ScaleSpec};
use specular_figure::{El, Figure, Item, Picture, Rgb16, Rgba, Role, pptx};
use specular_measure::calib::Calibration;

/// A 2-D affine map `[a, b, c, d, e, f]`: (x, y) → (a·x + b·y + c, d·x + e·y + f).
pub(in crate::app) type Map = [f64; 6];

pub(in crate::app) fn apply(m: &Map, p: (f64, f64)) -> (f64, f64) {
    (
        m[0] * p.0 + m[1] * p.1 + m[2],
        m[3] * p.0 + m[4] * p.1 + m[5],
    )
}

/// A measurement in stored pixels: its outline, whether it is closed, the headline value and
/// the lines a notes page lists.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Measured {
    pub outline: Vec<(f64, f64)>,
    pub closed: bool,
    pub label: String,
    pub lines: Vec<String>,
}

/// Measurements mapped by `m` (stored → panel px) into figure marks.
pub(in crate::app) fn marks(ms: &[Measured], m: &Map) -> Vec<Mark> {
    ms.iter()
        .filter(|x| !x.outline.is_empty())
        .map(|x| {
            let mut path: Vec<El> = Vec::with_capacity(x.outline.len() + 1);
            for (i, p) in x.outline.iter().enumerate() {
                let q = apply(m, *p);
                let q = [q.0 as f32, q.1 as f32];
                path.push(if i == 0 { El::Move(q) } else { El::Line(q) });
            }
            if x.closed {
                path.push(El::Close);
            }
            if x.outline.len() == 1 {
                // A point: a small cross.
                let q = apply(m, x.outline[0]);
                let (cx, cy) = (q.0 as f32, q.1 as f32);
                path = vec![
                    El::Move([cx - 4.0, cy]),
                    El::Line([cx + 4.0, cy]),
                    El::Move([cx, cy - 4.0]),
                    El::Line([cx, cy + 4.0]),
                ];
            }
            Mark {
                path,
                closed: x.closed,
                label: (!x.label.is_empty()).then(|| x.label.clone()),
            }
        })
        .collect()
}

/// The scale bar's calibration along the panel's horizontal: units per panel px, from the
/// calibration and `stored_from_panel` (the panel's px back to stored px).
pub(in crate::app) fn scale_spec(cal: &Calibration, stored_from_panel: &Map) -> Option<ScaleSpec> {
    if !cal.is_calibrated() {
        return None;
    }
    let a = apply(stored_from_panel, (0.0, 0.0));
    let b = apply(stored_from_panel, (1.0, 0.0));
    let d = cal.map(b.0 - a.0, b.1 - a.1);
    let per_px = (d.x * d.x + d.y * d.y).sqrt();
    (per_px.is_finite() && per_px > 0.0).then(|| ScaleSpec {
        per_px,
        unit: cal.unit.symbol().to_owned(),
        metres: cal.unit.metres(),
        approximate: cal.source.is_approximate(),
    })
}

/// A colour ramp of 256 entries from an sRGB sampler (0‥1).
pub(in crate::app) fn ramp(sample: impl Fn(f32) -> [f32; 3]) -> Vec<[u16; 3]> {
    (0..256)
        .map(|i| sample(i as f32 / 255.0).map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16))
        .collect()
}

/// `stored_from_panel` for a map the other way.
pub(in crate::app) fn invert(m: &Map) -> Map {
    let det = m[0] * m[4] - m[1] * m[3];
    if det == 0.0 || !det.is_finite() {
        return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    }
    let (a, b, c, d, e, f) = (m[0] / det, m[1] / det, m[2], m[3] / det, m[4] / det, m[5]);
    [e, -b, b * f - e * c, -d, a, d * c - a * f]
}

/// An RFC 3339 time (UTC) from Unix seconds.
pub(in crate::app) fn rfc3339(unix: u64) -> String {
    let [y, mo, d, h, mi, s] = specular_convert::civil_from_unix(unix as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// A PDF date from Unix seconds.
pub(in crate::app) fn pdf_date(unix: u64) -> String {
    let [y, mo, d, h, mi, s] = specular_convert::civil_from_unix(unix as i64);
    format!("D:{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z")
}

// ---------------------------------------------------------------------------------------------
// Key images.

/// A key image: the view as it was when marked (pixels through the display path), its
/// measurements and what titles it.
#[derive(Clone, Debug)]
pub(in crate::app) struct KeyImage {
    pub path: std::path::PathBuf,
    /// The page or frame.
    pub frame: u32,
    /// The title on its slide (file or series).
    pub title: String,
    /// A title with nothing about the patient (modality, body part, frame), for teaching.
    pub anon_title: String,
    pub px: Rgb16,
    /// Captured px from stored px.
    pub from_stored: Map,
    pub stored: (u32, u32),
    pub measured: Vec<Measured>,
    /// A DICOM file (teaching goes through the anonymiser).
    pub dicom: bool,
}

impl KeyImage {
    /// The notes page: one line per measurement.
    pub fn notes(&self) -> Vec<String> {
        self.measured
            .iter()
            .flat_map(|m| {
                if m.lines.is_empty() {
                    vec![m.label.clone()]
                } else {
                    m.lines.clone()
                }
            })
            .filter(|l| !l.is_empty())
            .collect()
    }
}

/// Blacks out `areas` (stored px) in the captured pixels: every captured pixel whose centre
/// maps into an area, plus a pixel of margin.
pub(in crate::app) fn mask(k: &KeyImage, areas: &[Area]) -> Rgb16 {
    let mut px = k.px.clone();
    for a in areas {
        let corners = [
            (f64::from(a.x), f64::from(a.y)),
            (f64::from(a.x + a.w), f64::from(a.y)),
            (f64::from(a.x), f64::from(a.y + a.h)),
            (f64::from(a.x + a.w), f64::from(a.y + a.h)),
        ]
        .map(|p| apply(&k.from_stored, p));
        let x0 = corners.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor() - 1.0;
        let x1 = corners.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil() + 1.0;
        let y0 = corners.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() - 1.0;
        let y1 = corners.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() + 1.0;
        let (w, h) = (f64::from(px.width), f64::from(px.height));
        for y in y0.max(0.0) as u32..y1.min(h).max(0.0) as u32 {
            for x in x0.max(0.0) as u32..x1.min(w).max(0.0) as u32 {
                let i = (y as usize * px.width as usize + x as usize) * 3;
                px.data[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    }
    px
}

/// One key image as a picture: masked, with its measurements drawn when `overlays`.
pub(in crate::app) fn key_picture(k: &KeyImage, areas: &[Area], overlays: bool) -> Rgb16 {
    let px = mask(k, areas);
    if !overlays || k.measured.is_empty() {
        return px;
    }
    let (w, h) = (px.width, px.height);
    let s = specular_figure::compose::unit((w, h));
    let mut items = Vec::new();
    for m in marks(&k.measured, &k.from_stored) {
        items.push(Item::stroke(
            m.path.clone(),
            Rgba::BLACK.with_alpha(0.7),
            s * 0.3,
            Role::Plain,
        ));
        items.push(Item::stroke(
            m.path,
            Rgba::rgb(255, 214, 0),
            s * 0.14,
            Role::Measurement,
        ));
    }
    let f = Figure {
        width: w,
        height: h,
        background: Rgba::BLACK,
        pictures: vec![Picture {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
            px,
            smooth: false,
            name: String::new(),
        }],
        items,
        meta: Vec::new(),
        title: String::new(),
        deep: false,
    };
    specular_figure::raster::render(&f, 1.0)
}

/// The deck: one slide per key image, titled, notes with the measurements; the case notes
/// lead the first slide's notes.
pub(in crate::app) fn deck(
    keys: &[(&KeyImage, Vec<Area>)],
    teaching: bool,
    overlays: bool,
    case_notes: &str,
    title: &str,
) -> Result<pptx::Deck, String> {
    let mut slides = Vec::new();
    for (i, (k, areas)) in keys.iter().enumerate() {
        let px = key_picture(k, areas, overlays);
        let png = specular_figure::png(&px, false, &[])?;
        let mut notes = Vec::new();
        if i == 0 {
            notes.extend(
                case_notes
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned),
            );
        }
        notes.extend(k.notes());
        let t = if teaching { &k.anon_title } else { &k.title };
        slides.push(pptx::Slide {
            title: t.clone(),
            alt: t.clone(),
            png,
            size: (px.width, px.height),
            notes,
        });
    }
    Ok(pptx::Deck {
        title: title.to_owned(),
        slides,
        dark: teaching || keys.iter().any(|(k, _)| k.dicom),
    })
}

/// The files of a key-image folder: `key-01.png`, … and `notes.txt` (the case notes, then
/// each image's title and measurements).
pub(in crate::app) fn folder(
    keys: &[(&KeyImage, Vec<Area>)],
    teaching: bool,
    overlays: bool,
    case_notes: &str,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    let mut notes = String::new();
    let case = case_notes.trim();
    if !case.is_empty() {
        notes.push_str(case);
        notes.push_str("\n\n");
    }
    for (i, (k, areas)) in keys.iter().enumerate() {
        let name = format!("key-{:02}.png", i + 1);
        let px = key_picture(k, areas, overlays);
        files.push((name.clone(), specular_figure::png(&px, false, &[])?));
        let t = if teaching { &k.anon_title } else { &k.title };
        let _ = writeln!(notes, "{name}: {t}");
        for l in k.notes() {
            let _ = writeln!(notes, "  {l}");
        }
    }
    files.push(("notes.txt".to_owned(), notes.into_bytes()));
    Ok(files)
}

/// The key images side by side (at most four to a row) on black, as one picture for the
/// clipboard; scaled down to at most `max_px` pixels.
pub(in crate::app) fn tiled(keys: &[(&KeyImage, Vec<Area>)], overlays: bool, max_px: u64) -> Rgb16 {
    let pics: Vec<Rgb16> = keys
        .iter()
        .map(|(k, a)| key_picture(k, a, overlays))
        .collect();
    let gap = 8.0f32;
    let cols = pics.len().clamp(1, 4);
    let mut pictures = Vec::new();
    let (mut x, mut y, mut row_h, mut width) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for (i, p) in pics.into_iter().enumerate() {
        if i > 0 && i % cols == 0 {
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        let (w, h) = (p.width as f32, p.height as f32);
        pictures.push(Picture {
            x,
            y,
            w,
            h,
            px: p,
            smooth: false,
            name: String::new(),
        });
        x += w + gap;
        width = width.max(x - gap);
        row_h = row_h.max(h);
    }
    let height = y + row_h;
    let f = Figure {
        width: width.max(1.0) as u32,
        height: height.max(1.0) as u32,
        background: Rgba::BLACK,
        pictures,
        items: Vec::new(),
        meta: Vec::new(),
        title: String::new(),
        deep: false,
    };
    let px = u64::from(f.width) * u64::from(f.height);
    let scale = if px > max_px {
        (max_px as f64 / px as f64).sqrt() as f32
    } else {
        1.0
    };
    specular_figure::raster::render(&f, scale)
}

/// RGBA8 (opaque) from RGB16, for the clipboard.
pub(in crate::app) fn rgba8(px: &Rgb16) -> Vec<u8> {
    let mut o = Vec::with_capacity(px.width as usize * px.height as usize * 4);
    for p in px.data.chunks_exact(3) {
        o.extend(p.iter().map(|&v| specular_figure::to8(v)));
        o.push(255);
    }
    o
}

/// Texts of a teaching case folder, localised by the caller.
pub(in crate::app) struct CaseTexts {
    /// The README's text.
    pub readme: String,
    /// "Case notes" heading, "Findings" heading, "No findings recorded".
    pub notes: String,
    pub findings: String,
    pub none: String,
}

/// A teaching case as an IHE TCE-style folder, the layout Radiopaedia's uploader takes:
///
/// - `README.txt`: what the folder holds;
/// - `case.txt`: the case notes, then each key image with its findings;
/// - `case.json`: the same for scripts (`notes`, `images[].{image, dicom, frame, title,
///   findings}`);
/// - `images/key-NN.png`: the key images, burned-in text masked;
/// - `DICOM/key-NN.dcm`: the anonymised copies (`copies`, by key index), and
///   `DICOM/teaching-manifest.dcm`, a Key Object Selection document "For Teaching"
///   (DCM 113004) with the case notes as its Key Object Description, referencing them.
///
/// Every key has passed the teaching checks before (no patient details in titles, masks).
pub(in crate::app) fn tce(
    keys: &[(&KeyImage, Vec<Area>)],
    copies: &[(usize, Vec<u8>)],
    overlays: bool,
    case_notes: &str,
    texts: &CaseTexts,
    when: (&str, &str),
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    let mut txt = String::new();
    let case = case_notes.trim();
    let _ = writeln!(txt, "{}:", texts.notes);
    let _ = writeln!(
        txt,
        "{}\n",
        if case.is_empty() {
            texts.none.as_str()
        } else {
            case
        }
    );
    let mut images = Vec::new();
    for (i, (k, areas)) in keys.iter().enumerate() {
        let png = format!("images/key-{:02}.png", i + 1);
        let px = key_picture(k, areas, overlays);
        files.push((png.clone(), specular_figure::png(&px, false, &[])?));
        let dicom = copies
            .iter()
            .any(|(j, _)| *j == i)
            .then(|| format!("DICOM/key-{:02}.dcm", i + 1));
        let findings = k.notes();
        let _ = writeln!(txt, "{png}: {}", k.anon_title);
        let _ = writeln!(txt, "  {}:", texts.findings);
        if findings.is_empty() {
            let _ = writeln!(txt, "    {}", texts.none);
        }
        for l in &findings {
            let _ = writeln!(txt, "    {l}");
        }
        images.push(serde_json::json!({
            "image": png,
            "dicom": dicom,
            "frame": k.frame,
            "title": k.anon_title,
            "findings": findings,
        }));
    }
    let mut sources = Vec::new();
    for (i, bytes) in copies {
        files.push((format!("DICOM/key-{:02}.dcm", i + 1), bytes.clone()));
        if let Ok(s) = specular_dicom::kos::source(bytes) {
            sources.push(s);
        }
    }
    // One manifest per study (key images may come from several).
    let mut studies: Vec<String> = sources.iter().map(|s| s.reference.study.clone()).collect();
    studies.dedup();
    studies.sort();
    studies.dedup();
    for (n, st) in studies.iter().enumerate() {
        let of: Vec<_> = sources
            .iter()
            .filter(|s| s.reference.study == *st)
            .cloned()
            .collect();
        let doc = specular_dicom::kos::document(&of, ("113004", "For Teaching"), case, when)
            .map_err(|e| e.to_string())?;
        let name = if studies.len() == 1 {
            "DICOM/teaching-manifest.dcm".to_owned()
        } else {
            format!("DICOM/teaching-manifest-{}.dcm", n + 1)
        };
        files.push((name, doc));
    }
    let json = serde_json::json!({
        "format": "specular-teaching-case",
        "version": 1,
        "anonymised": true,
        "notes": case,
        "images": images,
    });
    files.push((
        "case.json".to_owned(),
        serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?,
    ));
    files.push(("case.txt".to_owned(), txt.into_bytes()));
    files.push((
        "README.txt".to_owned(),
        format!("{}\n", texts.readme.trim()).into_bytes(),
    ));
    Ok(files)
}

/// A teaching case folder read back for the quiz: its notes and its key images (from
/// `case.json`).
pub(in crate::app) struct Case {
    pub notes: String,
    pub images: Vec<CaseImage>,
}

/// One key image of a teaching case folder.
pub(in crate::app) struct CaseImage {
    /// The PNG.
    pub png: std::path::PathBuf,
    /// The anonymised DICOM copy, when the folder has one, and the frame marked in it.
    pub dicom: Option<std::path::PathBuf>,
    pub frame: u32,
    pub title: String,
    pub findings: Vec<String>,
}

/// Reads `dir/case.json` (as [`tce`] writes it).
pub(in crate::app) fn read_case(dir: &std::path::Path) -> Option<Case> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("case.json")).ok()?).ok()?;
    if v.get("format")?.as_str()? != "specular-teaching-case" {
        return None;
    }
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_owned();
    let images = v
        .get("images")?
        .as_array()?
        .iter()
        .filter_map(|i| {
            // Only files inside the folder.
            let inside = |rel: &str| {
                (!rel.contains("..") && !std::path::Path::new(rel).is_absolute())
                    .then(|| dir.join(rel))
            };
            let png = inside(i.get("image")?.as_str()?)?;
            let dicom = i.get("dicom").and_then(|d| d.as_str()).and_then(inside);
            let findings = i
                .get("findings")
                .and_then(|f| f.as_array())
                .map(|a| a.iter().map(s).collect())
                .unwrap_or_default();
            Some(CaseImage {
                png,
                dicom,
                frame: i
                    .get("frame")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|f| u32::try_from(f).ok())
                    .unwrap_or(0),
                title: s(i.get("title")?),
                findings,
            })
        })
        .collect();
    Some(Case {
        notes: v.get("notes").map(s).unwrap_or_default(),
        images,
    })
}
