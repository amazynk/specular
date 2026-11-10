//! Sky overlays of science mode: the credible regions of a gravitational-wave sky map
//! (contours at the 50 % and 90 % levels the decoder found, traced on the shown plane), and
//! source catalogues (VOTable, FITS table, CSV) placed on an image with celestial
//! coordinates, each source a ring, with the hovered source's fields in a tooltip.
//!
//! Catalogues are kept across images until removed; markers are placed through the image's
//! WCS (galactic catalogues on equatorial images and the reverse are converted), at most
//! [`MAX_MARKERS`] per catalogue on an image.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_astro::frames::{Rotation, Sky};
use specular_decode::science::catalogue::Catalogue;
use specular_decode::science::wcs::{Frame, Wcs};
use specular_decode::science::{self as dsci, Credible, NdPlane};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::Color;
use specular_theme::metrics::FONT_SMALL;

use super::{App, GridLine, wcs_rest};
use crate::loader::ImageId;

/// The most sources of one catalogue drawn on an image.
pub(crate) const MAX_MARKERS: usize = 20_000;
/// The largest catalogue file read (bytes).
const MAX_FILE: u64 = 512 << 20;
/// Ring radius of a marker, and how near the pointer must be to one to show its fields
/// (logical px).
const RING: f32 = 5.0;
const HOVER: f32 = 7.0;
/// The most fields in a tooltip.
const TIP_FIELDS: usize = 14;

/// Ink of the credible-region contours.
pub(crate) const CONTOUR_INK: Color = Color::rgba(1.0, 0.62, 0.25, 0.95);

/// Ink of the `k`-th catalogue's markers.
fn marker_ink(k: usize) -> Color {
    const INKS: [Color; 4] = [
        Color::rgba(1.0, 0.86, 0.2, 0.95),
        Color::rgba(0.35, 0.9, 1.0, 0.95),
        Color::rgba(1.0, 0.45, 0.9, 0.95),
        Color::rgba(0.55, 1.0, 0.45, 0.95),
    ];
    INKS[k % INKS.len()]
}

/// A catalogue read for the overlay, or what came of trying.
pub(crate) enum Loaded {
    Catalogue(String, Box<Catalogue>),
    Failed(String),
    /// A dropped file that is not a catalogue: opened as an image instead.
    Open(PathBuf),
}

/// A source on the image: its position (stored px, or logical px once placed), catalogue
/// and row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Marker {
    pub at: (f32, f32),
    pub cat: u16,
    pub row: u32,
}

/// Markers in stored px, for an image, the positions of its further WCS axes and the
/// number of catalogues.
pub(crate) type MarkerCache = (ImageId, Vec<usize>, usize, Arc<Vec<Marker>>);

/// The hovered source's fields, by the pointer (logical px).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tip {
    pub at: (f32, f32),
    pub lines: Vec<String>,
}

/// The catalogue file's name for toasts: the table's own name, else the file's.
fn display_name(path: &Path, cat: Option<&Catalogue>) -> String {
    cat.and_then(|c| c.name.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        })
}

/// Reads a catalogue file (on a background thread). `drop`: a dropped file, opened as an
/// image when it is not a catalogue.
pub(crate) fn load(path: PathBuf, drop: bool) -> Loaded {
    let bytes = std::fs::metadata(&path)
        .ok()
        .filter(|m| m.len() <= MAX_FILE)
        .and_then(|_| std::fs::read(&path).ok());
    match bytes.as_deref().and_then(dsci::catalogue_of) {
        Some(c) => Loaded::Catalogue(display_name(&path, Some(&c)), Box::new(c)),
        None if drop => Loaded::Open(path),
        None => Loaded::Failed(display_name(&path, None)),
    }
}

/// Whether a dropped file may be a catalogue, by its name.
fn catalogue_like(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "vot"
            | "votable"
            | "xml"
            | "csv"
            | "tsv"
            | "psv"
            | "txt"
            | "dat"
            | "ecsv"
            | "fits"
            | "fit"
            | "fts"
    )
}

/// The credible-region contours of the shown plane (stored px), the longest loop of each
/// level labelled.
pub(super) fn credible_lines(c: &Credible, plane: &NdPlane) -> Vec<GridLine> {
    let (w, h) = (plane.width as usize, plane.height as usize);
    if plane.values.len() != w * h {
        return Vec::new();
    }
    let mut out = Vec::new();
    for &(p, level) in &c.levels {
        let loops = dsci::contour_lines(&plane.values, w, h, level as f32);
        let longest = (0..loops.len()).max_by_key(|&k| loops[k].len());
        for (k, pts) in loops.into_iter().enumerate() {
            let label = if Some(k) == longest {
                t!(
                    "science-credible",
                    percent = format!("{}", (p * 100.0).round())
                )
            } else {
                String::new()
            };
            out.push(GridLine { pts, label });
        }
    }
    out
}

/// The frame a catalogue's positions are given in.
pub(crate) fn catalogue_frame(cat: &Catalogue) -> Sky {
    if cat.galactic {
        Sky::Galactic
    } else if cat.fk4 {
        Sky::Fk4
    } else {
        Sky::Icrs
    }
}

/// Readout notes for a B1950 (FK4) image whose positions are converted: catalogue markers
/// of ICRS or galactic catalogues (converted to FK4 to be placed) and, with `samp`
/// connected, SAMP positions (sent and taken as ICRS). Empty for other images, and for an
/// FK4 image with only B1950 catalogues and no SAMP: its own frame, nothing converted.
pub(crate) fn fk4_notes<'a>(
    wcs: &Wcs,
    cats: impl IntoIterator<Item = &'a Catalogue>,
    samp: bool,
) -> Vec<String> {
    if specular_astro::frames::of_wcs(wcs) != Some(Sky::Fk4) {
        return Vec::new();
    }
    let (mut icrs, mut galactic) = (false, false);
    for cat in cats.into_iter().filter(|c| !c.lng.is_empty()) {
        match catalogue_frame(cat) {
            Sky::Galactic => galactic = true,
            Sky::Fk4 => {}
            _ => icrs = true,
        }
    }
    let mut out = Vec::new();
    if icrs {
        out.push(t!("science-fk4-markers"));
    }
    if galactic {
        out.push(t!("science-fk4-markers-galactic"));
    }
    if samp {
        out.push(t!("science-fk4-samp"));
    }
    out
}

/// Places a catalogue's sources on an image of `w × h` (stored px) through its WCS: the
/// markers, at most [`MAX_MARKERS`], and how many sources fall on the image.
pub(crate) fn place(
    cat: &Catalogue,
    k: u16,
    wcs: &Wcs,
    w: u32,
    h: u32,
    rest: &[usize],
) -> (Vec<Marker>, usize) {
    let Some(c) = wcs.celestial.as_ref() else {
        return (Vec::new(), 0);
    };
    let galactic = match c.frame {
        Frame::Galactic => true,
        Frame::Equatorial { .. } => false,
        // Ecliptic and other frames: positions are not converted.
        _ => cat.galactic,
    };
    // The image's frame: galactic, FK4 B1950, or ICRS (FK5 J2000 taken as the same); the
    // catalogue's positions (galactic, FK4 B1950 or ICRS) are converted to it.
    let to = if galactic {
        Sky::Galactic
    } else if specular_astro::frames::of_wcs(wcs) == Some(Sky::Fk4) {
        Sky::Fk4
    } else {
        Sky::Icrs
    };
    // Ecliptic and other frames keep the catalogue's own numbers.
    let convertible = galactic || matches!(c.frame, Frame::Equatorial { .. });
    let fk4_cat = catalogue_frame(cat) == Sky::Fk4;
    // Galactic and ICRS catalogues meet galactic and ICRS images through the catalogue's
    // own (Hipparcos) rotation; FK4 on either side through the FK4 conversion.
    let rot = if fk4_cat {
        (convertible && to != Sky::Fk4).then(|| Rotation::new(Sky::Fk4, to))
    } else {
        (to == Sky::Fk4).then(|| Rotation::new(Sky::Icrs, Sky::Fk4))
    };
    let (wf, hf) = (f64::from(w), f64::from(h));
    let mut out = Vec::new();
    let mut on = 0;
    for row in 0..cat.lng.len() {
        let (a, b) = if fk4_cat {
            (cat.lng[row], cat.lat[row])
        } else {
            cat.sky(row, galactic)
        };
        let (a, b) = rot.map_or((a, b), |r| r.apply(a, b));
        let Some((x, y)) = wcs.display_of(a, b, rest) else {
            continue;
        };
        if !(x >= 0.0 && y >= 0.0 && x < wf && y < hf) {
            continue;
        }
        // A projection may map a point back onto the image from the far side of the sky.
        if wcs
            .sky_at(x, y, rest)
            .is_none_or(|(a2, b2)| angle_between(a, b, a2, b2) > 1e-3)
        {
            continue;
        }
        on += 1;
        if out.len() < MAX_MARKERS {
            out.push(Marker {
                at: (x as f32, y as f32),
                cat: k,
                row: row as u32,
            });
        }
    }
    (out, on)
}

/// Angle between two directions (degrees).
fn angle_between(a1: f64, b1: f64, a2: f64, b2: f64) -> f64 {
    let (a1, b1, a2, b2) = (
        a1.to_radians(),
        b1.to_radians(),
        a2.to_radians(),
        b2.to_radians(),
    );
    let c = b1.sin() * b2.sin() + b1.cos() * b2.cos() * (a1 - a2).cos();
    c.clamp(-1.0, 1.0).acos().to_degrees()
}

/// A source's fields as tooltip lines: the catalogue's name, then "column  value".
pub(crate) fn tip_lines(name: &str, cat: &Catalogue, row: usize) -> Vec<String> {
    let mut lines = vec![name.to_owned()];
    let Some(fields) = cat.rows.get(row) else {
        return lines;
    };
    lines.extend(
        cat.columns
            .iter()
            .zip(fields)
            .filter(|(_, v)| !v.trim().is_empty())
            .take(TIP_FIELDS)
            .map(|(c, v)| format!("{c}  {}", v.trim())),
    );
    lines
}

/// Draws the markers (logical px), clipped to `clip`.
pub(crate) fn draw_markers(
    dl: &mut DrawList,
    markers: &[Marker],
    clip: Rect,
    hovered: Option<usize>,
) {
    dl.push_clip(clip);
    let near = clip.inset(-RING);
    for (k, m) in markers.iter().enumerate() {
        if !near.contains(m.at) {
            continue;
        }
        let r = if Some(k) == hovered { RING + 2.0 } else { RING };
        let ring = Rect::new(m.at.0 - r, m.at.1 - r, 2.0 * r, 2.0 * r);
        dl.stroke_rect(ring, r, Color::rgba(0.0, 0.0, 0.0, 0.5), 3.0);
        dl.stroke_rect(ring, r, marker_ink(usize::from(m.cat)), 1.5);
    }
    dl.pop_clip();
}

/// Draws a tooltip by its point, kept within `area` (logical x, y, w, h).
pub(crate) fn draw_tip(
    dl: &mut DrawList,
    text: &mut Text,
    tip: &Tip,
    area: (f32, f32, f32, f32),
    ts: f32,
) {
    if tip.lines.is_empty() {
        return;
    }
    let st = TextStyle::new(FONT_SMALL * ts).tabular();
    let blocks: Vec<_> = tip
        .lines
        .iter()
        .map(|l| text.layout(l, st, Some(area.2 * 0.6), Align::Start))
        .collect();
    let w = blocks.iter().map(|b| b.width).fold(0.0f32, f32::max) + 12.0;
    let h = blocks.iter().map(|b| b.height).sum::<f32>() + 8.0;
    let mut x = tip.at.0 + 14.0;
    let mut y = tip.at.1 + 14.0;
    if x + w > area.0 + area.2 {
        x = (tip.at.0 - 14.0 - w).max(area.0);
    }
    if y + h > area.1 + area.3 {
        y = (tip.at.1 - 14.0 - h).max(area.1);
    }
    let r = Rect::new(x, y, w, h);
    dl.rect(r, 4.0, Color::rgba(0.05, 0.05, 0.07, 0.86));
    let mut ty = y + 4.0;
    for (k, b) in blocks.iter().enumerate() {
        let ink = if k == 0 {
            Color::rgba(1.0, 0.86, 0.45, 1.0)
        } else {
            Color::rgba(0.95, 0.95, 0.95, 1.0)
        };
        text.draw(dl, b, x + 6.0, ty, ink);
        ty += b.height;
    }
}

impl App {
    /// Asks for a catalogue file and reads it on the dialog's thread
    /// ([`App::science_catalogue_poll`] overlays it).
    pub(crate) fn science_catalogue_open(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let filter = t!("science-catalogue-filter");
        self.loader.background("dlg-sci-catalog", move |l| {
            let Some(p) = rfd::FileDialog::new()
                .add_filter(
                    filter,
                    &[
                        "vot", "votable", "xml", "fits", "fit", "fts", "csv", "tsv", "txt", "dat",
                    ],
                )
                .pick_file()
            else {
                return;
            };
            let _ = tx.send(load(p, false));
            l.waker()();
        });
        self.science.catalogue_rx = Some(rx);
    }

    /// A file dropped on the window: overlaid as a catalogue when the image shown has sky
    /// coordinates and the file is one (else opened as usual). True when taken.
    pub(crate) fn science_catalogue_drop(&mut self, path: &Path) -> bool {
        let sky = self.science_shown()
            && self
                .science
                .summary
                .as_ref()
                .and_then(|s| s.wcs.as_ref())
                .is_some_and(|w| w.celestial.is_some());
        if !sky || !catalogue_like(path) || self.science.catalogue_rx.is_some() {
            return false;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let p = path.to_path_buf();
        self.loader.background("sci-cat-drop", move |l| {
            let _ = tx.send(load(p, true));
            l.waker()();
        });
        self.science.catalogue_rx = Some(rx);
        true
    }

    /// Overlays a catalogue read from a file, once there.
    pub(crate) fn science_catalogue_poll(&mut self) {
        let Some(Ok(loaded)) = self.science.catalogue_rx.as_ref().map(|r| r.try_recv()) else {
            return;
        };
        self.science.catalogue_rx = None;
        let now = self.now();
        let (name, cat) = match loaded {
            Loaded::Open(p) => {
                self.open_target(p);
                return;
            }
            Loaded::Failed(name) => {
                let msg = t!("science-catalogue-failed", name = name);
                self.chrome.toast(msg, None, now);
                return;
            }
            Loaded::Catalogue(name, cat) => (name, Arc::new(*cat)),
        };
        // How many fall on the image shown.
        let on = self.science.summary.as_ref().and_then(|s| {
            let w = s.wcs.as_ref().filter(|w| w.celestial.is_some())?;
            let rest = wcs_rest(s, w, &self.science.index);
            Some(place(&cat, 0, w, s.width, s.height, &rest).1)
        });
        self.science.catalogues.push((name.clone(), cat));
        self.science.marker_cache = None;
        let msg = match on {
            Some(0) | None => t!("science-catalogue-none", name = name),
            Some(n) if n > MAX_MARKERS => t!(
                "science-catalogue-capped",
                name = name,
                shown = MAX_MARKERS,
                count = n
            ),
            Some(n) => t!("science-catalogue-toast", name = name, count = n),
        };
        self.chrome.toast(msg, None, now);
        self.science_readout();
        self.redraw = true;
    }

    /// Removes the overlaid catalogues.
    pub(crate) fn science_catalogues_clear(&mut self) {
        self.science.catalogues.clear();
        self.science.marker_cache = None;
        let now = self.now();
        self.chrome
            .toast(t!("science-catalogues-cleared"), None, now);
        self.science_readout();
        self.redraw = true;
    }

    /// The markers of every catalogue on the image (stored px), cached per image and
    /// position on the further WCS axes.
    pub(super) fn science_markers(&mut self, id: ImageId) -> Option<Arc<Vec<Marker>>> {
        if self.science.catalogues.is_empty() {
            return None;
        }
        let s = self.science.summary.clone()?;
        let w = s.wcs.as_ref().filter(|w| w.celestial.is_some())?;
        let rest = wcs_rest(&s, w, &self.science.index);
        let n = self.science.catalogues.len();
        if let Some((i, r, k, m)) = &self.science.marker_cache
            && *i == id
            && *r == rest
            && *k == n
        {
            return Some(m.clone());
        }
        let mut all = Vec::new();
        for (k, (_, cat)) in self.science.catalogues.iter().enumerate() {
            let k = u16::try_from(k).unwrap_or(u16::MAX);
            all.extend(place(cat, k, w, s.width, s.height, &rest).0);
        }
        let m = Arc::new(all);
        self.science.marker_cache = Some((id, rest, n, m.clone()));
        Some(m)
    }

    /// The credible-region contours (stored px) when the shown plane is the sky map's
    /// probability, cached per image and position.
    pub(super) fn science_contours(&mut self, id: ImageId) -> Option<Arc<Vec<GridLine>>> {
        let s = self.science.summary.clone()?;
        let c = s.credible.as_ref()?;
        let shown = self.science.shown.as_ref()?;
        if shown.composite.is_some() || shown.index != c.index {
            return None;
        }
        if let Some((i, x, l)) = &self.science.contour_cache
            && *i == id
            && *x == c.index
        {
            return Some(l.clone());
        }
        let plane = &self.science.planes.first()?.plane;
        let l = Arc::new(credible_lines(c, plane));
        self.science.contour_cache = Some((id, c.index.clone(), l.clone()));
        Some(l)
    }

    /// The hovered source's tooltip among `markers` (logical px), and its index.
    pub(super) fn science_tip(&self, markers: &[Marker]) -> Option<(usize, Tip)> {
        let p = self.logical(self.pointer?);
        let (k, d) = markers
            .iter()
            .enumerate()
            .map(|(k, m)| (k, (m.at.0 - p.0).hypot(m.at.1 - p.1)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        if d > HOVER {
            return None;
        }
        let m = markers[k];
        let (name, cat) = self.science.catalogues.get(usize::from(m.cat))?;
        Some((
            k,
            Tip {
                at: p,
                lines: tip_lines(name, cat, m.row as usize),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fits, open};
    use super::*;

    #[test]
    fn markers_fall_where_the_wcs_puts_them() {
        let s = open(fits(
            &[
                ("SIMPLE", "T"),
                ("BITPIX", "-32"),
                ("NAXIS", "2"),
                ("NAXIS1", "100"),
                ("NAXIS2", "100"),
                ("CTYPE1", "'RA---TAN'"),
                ("CTYPE2", "'DEC--TAN'"),
                ("CRVAL1", "10.0"),
                ("CRVAL2", "20.0"),
                ("CRPIX1", "50.5"),
                ("CRPIX2", "50.5"),
                ("CDELT1", "-0.001"),
                ("CDELT2", "0.001"),
            ],
            &[0.0; 100 * 100],
        ));
        let w = s.science().unwrap().wcs.clone().unwrap();
        let cat = Catalogue {
            columns: vec!["id".into(), "RA".into(), "Dec".into()],
            lng: vec![10.0, 10.01, 200.0, 10.0],
            lat: vec![20.0, 20.0, -40.0, 20.0],
            rows: vec![
                vec!["a".into(), "10".into(), "20".into()],
                vec!["b".into(), "10.01".into(), "20".into()],
                vec!["c".into(), "200".into(), "-40".into()],
                vec!["d".into(), String::new(), "20".into()],
            ],
            ..Default::default()
        };
        let (m, on) = place(&cat, 0, &w, 100, 100, &[]);
        assert_eq!(on, 3);
        // CRPIX 50.5 is the centre of the display (50, 50).
        assert!(
            (m[0].at.0 - 50.0).abs() < 1e-3 && (m[0].at.1 - 50.0).abs() < 1e-3,
            "{m:?}"
        );
        // Right ascension increases to the left.
        assert!(m[1].at.0 < m[0].at.0);
        assert_eq!(
            tip_lines("cat", &cat, 1),
            ["cat", "id  b", "RA  10.01", "Dec  20"]
        );
        assert_eq!(tip_lines("cat", &cat, 3), ["cat", "id  d", "Dec  20"]);
        // A galactic catalogue on the equatorial image: converted.
        let (l, b) = specular_decode::science::catalogue::icrs_to_galactic(10.0, 20.0);
        let gal = Catalogue {
            lng: vec![l],
            lat: vec![b],
            rows: vec![Vec::new()],
            galactic: true,
            ..Default::default()
        };
        let (g, _) = place(&gal, 1, &w, 100, 100, &[]);
        assert!(
            (g[0].at.0 - 50.0).abs() < 1e-3 && (g[0].at.1 - 50.0).abs() < 1e-3,
            "{g:?}"
        );
    }

    /// On a B1950 (FK4) image an ICRS source lands where its FK4 position is, not 0.7° off
    /// (astropy: ICRS 10.65897°, 20.27387° is FK4 B1950 10°, 20°).
    #[test]
    fn markers_on_a_b1950_image() {
        let s = open(fits(
            &[
                ("SIMPLE", "T"),
                ("BITPIX", "-32"),
                ("NAXIS", "2"),
                ("NAXIS1", "100"),
                ("NAXIS2", "100"),
                ("CTYPE1", "'RA---TAN'"),
                ("CTYPE2", "'DEC--TAN'"),
                ("CRVAL1", "10.0"),
                ("CRVAL2", "20.0"),
                ("CRPIX1", "50.5"),
                ("CRPIX2", "50.5"),
                ("CDELT1", "-0.001"),
                ("CDELT2", "0.001"),
                ("RADESYS", "'FK4'"),
                ("EQUINOX", "1950.0"),
            ],
            &[0.0; 100 * 100],
        ));
        let w = s.science().unwrap().wcs.clone().unwrap();
        let (ra, dec) = specular_astro::frames::convert(Sky::Fk4, Sky::Icrs, 10.0, 20.0);
        assert!((ra - 10.658_967).abs() < 1e-5 && (dec - 20.273_867).abs() < 1e-5);
        let cat = Catalogue {
            lng: vec![ra],
            lat: vec![dec],
            rows: vec![Vec::new()],
            ..Default::default()
        };
        let (m, on) = place(&cat, 0, &w, 100, 100, &[]);
        assert_eq!(on, 1);
        assert!(
            (m[0].at.0 - 50.0).abs() < 1e-3 && (m[0].at.1 - 50.0).abs() < 1e-3,
            "{m:?}"
        );
        // A B1950 catalogue on the B1950 image: placed as given.
        let b1950 = Catalogue {
            lng: vec![10.0],
            lat: vec![20.0],
            rows: vec![Vec::new()],
            fk4: true,
            ..Default::default()
        };
        let (m, _) = place(&b1950, 0, &w, 100, 100, &[]);
        assert!(
            (m[0].at.0 - 50.0).abs() < 1e-3 && (m[0].at.1 - 50.0).abs() < 1e-3,
            "{m:?}"
        );
    }

    /// A B1950 catalogue on an ICRS image is converted from FK4 (astropy: FK4 B1950 10°,
    /// 20° is ICRS 10.65897°, 20.27387°).
    #[test]
    fn b1950_catalogue_on_an_icrs_image() {
        let s = open(fits(
            &[
                ("SIMPLE", "T"),
                ("BITPIX", "-32"),
                ("NAXIS", "2"),
                ("NAXIS1", "100"),
                ("NAXIS2", "100"),
                ("CTYPE1", "'RA---TAN'"),
                ("CTYPE2", "'DEC--TAN'"),
                ("CRVAL1", "10.658967"),
                ("CRVAL2", "20.273867"),
                ("CRPIX1", "50.5"),
                ("CRPIX2", "50.5"),
                ("CDELT1", "-0.001"),
                ("CDELT2", "0.001"),
                ("RADESYS", "'ICRS'"),
            ],
            &[0.0; 100 * 100],
        ));
        let w = s.science().unwrap().wcs.clone().unwrap();
        let cat = Catalogue {
            lng: vec![10.0],
            lat: vec![20.0],
            rows: vec![Vec::new()],
            fk4: true,
            ..Default::default()
        };
        let (m, on) = place(&cat, 0, &w, 100, 100, &[]);
        assert_eq!(on, 1);
        // 1e-5° is 0.01 px at 0.001° per pixel.
        assert!(
            (m[0].at.0 - 50.0).abs() < 0.05 && (m[0].at.1 - 50.0).abs() < 0.05,
            "{m:?}"
        );
    }

    /// The WCS of a small TAN image with the given frame cards (equatorial unless they give
    /// the axis types).
    fn wcs_with(frame: &[(&str, &str)]) -> Arc<Wcs> {
        let own = frame.iter().any(|(k, _)| k.starts_with("CTYPE"));
        let axes = [("CTYPE1", "'RA---TAN'"), ("CTYPE2", "'DEC--TAN'")];
        let mut cards = vec![
            ("SIMPLE", "T"),
            ("BITPIX", "-32"),
            ("NAXIS", "2"),
            ("NAXIS1", "10"),
            ("NAXIS2", "10"),
        ];
        if !own {
            cards.extend_from_slice(&axes);
        }
        cards.extend_from_slice(&[
            ("CRVAL1", "10.0"),
            ("CRVAL2", "20.0"),
            ("CRPIX1", "5.5"),
            ("CRPIX2", "5.5"),
            ("CDELT1", "-0.001"),
            ("CDELT2", "0.001"),
        ]);
        cards.extend_from_slice(frame);
        let s = open(fits(&cards, &[0.0; 100]));
        s.science().unwrap().wcs.clone().unwrap()
    }

    fn cat_in(galactic: bool, fk4: bool) -> Catalogue {
        Catalogue {
            lng: vec![10.0],
            lat: vec![20.0],
            rows: vec![Vec::new()],
            galactic,
            fk4,
            ..Default::default()
        }
    }

    /// The readout notes a conversion only on a B1950 image, and only when one happens.
    #[test]
    fn fk4_notes_only_when_converted() {
        let fk4 = wcs_with(&[("RADESYS", "'FK4'"), ("EQUINOX", "1950.0")]);
        let icrs_cat = cat_in(false, false);
        let gal_cat = cat_in(true, false);
        let b1950_cat = cat_in(false, true);
        let markers = t!("science-fk4-markers");
        let galactic = t!("science-fk4-markers-galactic");
        let samp = t!("science-fk4-samp");
        // Shown in its own frame: nothing converted.
        assert!(fk4_notes(&fk4, [], false).is_empty());
        assert!(fk4_notes(&fk4, [&b1950_cat], false).is_empty());
        // An empty catalogue places nothing.
        assert!(fk4_notes(&fk4, [&Catalogue::default()], false).is_empty());
        assert_eq!(
            fk4_notes(&fk4, [&icrs_cat], false),
            std::slice::from_ref(&markers)
        );
        assert_eq!(
            fk4_notes(&fk4, [&gal_cat], false),
            std::slice::from_ref(&galactic)
        );
        assert_eq!(fk4_notes(&fk4, [], true), std::slice::from_ref(&samp));
        assert_eq!(
            fk4_notes(&fk4, [&b1950_cat, &gal_cat, &icrs_cat, &icrs_cat], true),
            [markers, galactic, samp]
        );
        // J2000, ICRS and galactic images: never.
        for frame in [
            &[("RADESYS", "'ICRS'")][..],
            &[("RADESYS", "'FK5'"), ("EQUINOX", "2000.0")][..],
            &[("CTYPE1", "'GLON-TAN'"), ("CTYPE2", "'GLAT-TAN'")][..],
        ] {
            let w = wcs_with(frame);
            assert!(
                fk4_notes(&w, [&icrs_cat, &gal_cat, &b1950_cat], true).is_empty(),
                "{frame:?}"
            );
        }
    }

    #[test]
    fn contours_of_the_credible_levels() {
        let (w, h) = (64usize, 32usize);
        let values: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 - 31.5, (i / w) as f32 - 15.5);
                (-(x * x + y * y) / 50.0).exp()
            })
            .collect();
        let plane = NdPlane {
            width: w as u32,
            height: h as u32,
            values,
            truncated: false,
        };
        let c = Credible {
            index: Vec::new(),
            levels: vec![(0.5, 0.5), (0.9, 0.1)],
        };
        let l = credible_lines(&c, &plane);
        assert_eq!(l.len(), 2);
        let bare = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(bare(&l[0].label), "50%");
        assert_eq!(bare(&l[1].label), "90%");
        assert!(l[0].pts.len() > 8 && l[0].pts.first() == l[0].pts.last());
    }
}
