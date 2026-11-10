//! Merge to panorama in the app (shared flow: [`super::merges`]).
//!
//! - **Sheet** ([`super::sheet`]): the photos (each can be left out), then crop to fill,
//!   the output format for JPEG/HEIC/TIFF photos (16-bit TIFF or lossless JPEG XL), open
//!   in Develop; the projection is the choice row (automatic by the field of view,
//!   rectilinear, cylindrical). Choices last for the run.
//! - **Job:** the frames are read, stitched ([`specular_develop::merge::pano`]) and saved
//!   as `<reference>-Pano.dng` (raws: a float DNG streamed to disk) or `-Pano.tif` / `.jxl`
//!   beside them, never over a file.

use std::path::{Path, PathBuf};

use specular_develop::merge::{self, Shot, dng, pano, project::Projection, rgb::RgbFormat};
use specular_i18n::t;

use super::merges::{Done, Failure, Kind};
use super::sheet::{DevelopSheet, Purpose};
use crate::app::App;

/// The projection choice's options, in order.
const PROJECTIONS: [Option<Projection>; 3] = [
    None,
    Some(Projection::Rectilinear),
    Some(Projection::Cylindrical),
];

/// The run's choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Prefs {
    /// Index into [`PROJECTIONS`].
    pub projection: usize,
    pub crop: bool,
    /// JPEG/HEIC/TIFF photos: lossless JPEG XL instead of 16-bit TIFF.
    pub jxl: bool,
    pub open: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            projection: 0,
            crop: true,
            jxl: RgbFormat::default() == RgbFormat::JxlLossless,
            open: true,
        }
    }
}

impl Prefs {
    fn format(&self) -> RgbFormat {
        if self.jxl {
            RgbFormat::JxlLossless
        } else {
            RgbFormat::Tiff16
        }
    }
}

/// Whether every photo is a raw (the panorama is a float DNG).
fn all_raw(shots: &[Shot]) -> bool {
    shots.iter().all(|s| super::controls::is_raw(&s.path))
}

/// The option rows after the photos: crop, (format), open.
fn options(raw: bool) -> usize {
    if raw { 2 } else { 3 }
}

/// The result's file name for `reference`.
fn result_name(reference: &Path, raw: bool, prefs: &Prefs) -> String {
    let ext = if raw { "dng" } else { prefs.format().ext() };
    dng::file_name_of(reference, dng::Kind::Pano, ext)
}

/// The middle photo (in capture order) names the panorama.
fn middle(shots: &[Shot]) -> Option<&Shot> {
    shots.get(shots.len() / 2)
}

/// The sheet for `shots` with `prefs`.
pub(super) fn sheet(shots: &[Shot], prefs: &Prefs) -> DevelopSheet {
    let raw = all_raw(shots);
    let mut rows: Vec<(String, bool)> = shots
        .iter()
        .map(|s| (super::merges::name_of(&s.path), true))
        .collect();
    let mut ends = Vec::with_capacity(rows.len() + 3);
    ends.resize(
        rows.len(),
        (t!("develop-sheet-included"), t!("develop-sheet-left-out")),
    );
    rows.push((t!("develop-pano-crop"), prefs.crop));
    ends.push((t!("develop-merge-on"), t!("develop-merge-off")));
    if !raw {
        rows.push((t!("develop-pano-format"), prefs.jxl));
        ends.push((
            t!("develop-pano-format-jxl"),
            t!("develop-pano-format-tiff"),
        ));
    }
    rows.push((t!("develop-merge-open"), prefs.open));
    ends.push((t!("develop-merge-on"), t!("develop-merge-off")));
    let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
    let name = middle(shots)
        .map(|s| result_name(&s.path, raw, prefs))
        .unwrap_or_default();
    DevelopSheet {
        purpose: Purpose::MergePano(paths),
        title: t!("develop-pano-title"),
        field: None,
        kinds: Vec::new(),
        mode: Some(prefs.projection.min(PROJECTIONS.len() - 1)),
        ok: t!("develop-pano-ok"),
        top: 0,
        note: Some(t!("develop-pano-note", name = name)),
        rows,
        ends,
        choice_text: Some((
            t!("develop-pano-projection"),
            vec![
                t!("develop-pano-projection-auto"),
                t!("develop-pano-projection-rectilinear"),
                t!("develop-pano-projection-cylindrical"),
            ],
        )),
    }
}

/// The sheet's choices: the photos kept and the prefs.
fn choices(s: &DevelopSheet, paths: &[PathBuf], raw: bool, old: &Prefs) -> (Vec<PathBuf>, Prefs) {
    let n = paths.len();
    let kept = paths
        .iter()
        .zip(&s.rows)
        .filter(|(_, r)| r.1)
        .map(|(p, _)| p.clone())
        .collect();
    let opt = |i: usize| s.rows.get(n + i).is_some_and(|r| r.1);
    let prefs = if raw {
        Prefs {
            projection: s.mode.unwrap_or(0),
            crop: opt(0),
            jxl: old.jxl,
            open: opt(1),
        }
    } else {
        Prefs {
            projection: s.mode.unwrap_or(0),
            crop: opt(0),
            jxl: opt(1),
            open: opt(2),
        }
    };
    debug_assert_eq!(s.rows.len(), n + options(raw));
    (kept, prefs)
}

/// The sheet for `shots` with the default choices (screenshot tests).
#[cfg(test)]
pub(super) fn sheet_for_shots(shots: &[Shot]) -> DevelopSheet {
    sheet(shots, &Prefs::default())
}

/// Stitches raws and saves the float DNG.
fn stitch_raws(
    paths: &[PathBuf],
    shots: &[Shot],
    o: &pano::PanoOptions,
    set: &(dyn Fn(f32) + Sync),
    cancel: &specular_core::CancelToken,
) -> Result<Done, Failure> {
    let n = paths.len();
    let mut frames = Vec::with_capacity(n);
    let mut focal = Vec::with_capacity(n);
    for (i, p) in paths.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(Failure::Merge(merge::Error::Cancelled));
        }
        let mut f = merge::Frame::read(p).map_err(Failure::Merge)?;
        if let Some(s) = shots.iter().find(|s| s.path == *p) {
            f.shot = s.clone();
        }
        let head = super::merges::head_of(p);
        focal.push(pano::focal_px(&head, f.sensor.crop.w, f.sensor.crop.h));
        frames.push(f);
        set(0.2 * (i + 1) as f32 / n as f32);
    }
    let p = pano::stitch(frames, &focal, o, &|f| set(0.2 + 0.75 * f), cancel)
        .map_err(Failure::Merge)?;
    let reference = &p.merged.shots[p.merged.reference].path;
    let ref_bytes = std::fs::read(reference).unwrap_or_default();
    let path = dng::save_streamed(
        &p.merged,
        &ref_bytes,
        &dng::DngOptions::default(),
        dng::Kind::Pano,
        Some(pano::xmp(&p)),
    )
    .map_err(|e| Failure::Write(e.to_string()))?;
    Ok(Done {
        path,
        sources: p.merged.shots.iter().map(|s| s.path.clone()).collect(),
        left_out: p.left_out,
    })
}

/// Stitches JPEG/HEIC/TIFF photos and saves them in `format`.
fn stitch_pictures(
    paths: &[PathBuf],
    shots: &[Shot],
    o: &pano::PanoOptions,
    format: RgbFormat,
    set: &(dyn Fn(f32) + Sync),
    cancel: &specular_core::CancelToken,
) -> Result<Done, Failure> {
    let n = paths.len();
    let mut images = Vec::with_capacity(n);
    for (i, p) in paths.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(Failure::Merge(merge::Error::Cancelled));
        }
        let (img, mut shot) = merge::rgb::read(p).map_err(Failure::Merge)?;
        if let Some(s) = shots.iter().find(|s| s.path == *p) {
            shot = s.clone();
        }
        images.push((img, shot));
        set(0.2 * (i + 1) as f32 / n as f32);
    }
    let focal: Vec<Option<f64>> = paths
        .iter()
        .zip(&images)
        .map(|(p, (img, _))| pano::focal_px(&super::merges::head_of(p), img.w, img.h))
        .collect();
    let (p, picture) = pano::stitch_rgb(images, &focal, o, &|f| set(0.2 + 0.75 * f), cancel)
        .map_err(Failure::Merge)?;
    let reference = p.merged.shots[p.merged.reference].path.clone();
    let ref_bytes = std::fs::read(&reference).ok();
    let bytes =
        merge::rgb::encode(&picture, format, ref_bytes.as_deref()).map_err(Failure::Write)?;
    let name = dng::file_name_of(&reference, dng::Kind::Pano, format.ext());
    let path =
        dng::write_beside(&reference, &name, &bytes).map_err(|e| Failure::Write(e.to_string()))?;
    Ok(Done {
        path,
        sources: p.merged.shots.iter().map(|s| s.path.clone()).collect(),
        left_out: p.left_out,
    })
}

impl App {
    /// "Merge to panorama…" on `targets`; `around` are the photos near a single target.
    /// Raws make a raw panorama; without raws among the targets, the other photos are
    /// stitched.
    pub(in crate::app) fn develop_pano_ask(&mut self, targets: Vec<PathBuf>, around: Vec<PathBuf>) {
        let keep: fn(&Path) -> bool = if targets.iter().any(|p| super::controls::is_raw(p)) {
            super::controls::is_raw
        } else {
            |p| !super::controls::is_raw(p)
        };
        self.develop_merges_ask(Kind::Pano, targets, around, keep);
    }

    /// The sheet's OK: the job starts.
    pub(super) fn develop_pano_ok(&mut self, s: &DevelopSheet, paths: Vec<PathBuf>) {
        let shots_all = self.develop.merges.shots.clone();
        let raw = all_raw(&shots_all);
        let (kept, prefs) = choices(s, &paths, raw, &self.develop.merges.pano);
        self.develop.merges.pano = prefs;
        if kept.len() < 2 {
            self.develop_say(t!("develop-merge-too-few"));
            return;
        }
        if kept.len() > pano::MAX_FRAMES {
            let why = super::merge::merge_reason(&merge::Error::Cap(pano::MAX_FRAMES));
            self.develop_say(t!("develop-pano-failed", reason = why));
            return;
        }
        let shots: Vec<Shot> = shots_all
            .into_iter()
            .filter(|s| kept.contains(&s.path))
            .collect();
        let name = middle(&shots)
            .map(|s| result_name(&s.path, raw, &prefs))
            .unwrap_or_default();
        let o = pano::PanoOptions {
            projection: PROJECTIONS[prefs.projection.min(PROJECTIONS.len() - 1)],
            crop: prefs.crop,
            ..pano::PanoOptions::default()
        };
        let format = prefs.format();
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        self.develop_merges_start(Kind::Pano, name, prefs.open, move |set, cancel| {
            if raw {
                stitch_raws(&paths, &shots, &o, set, cancel)
            } else {
                stitch_pictures(&paths, &shots, &o, format, set, cancel)
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(name: &str) -> Shot {
        Shot {
            path: PathBuf::from(format!("/p/{name}")),
            ..Shot::default()
        }
    }

    #[test]
    fn raw_sheet_lists_photos_then_options_and_reads_back() {
        let shots = vec![shot("a.CR2"), shot("b.CR2"), shot("c.CR2")];
        let mut s = sheet(&shots, &Prefs::default());
        assert_eq!(s.rows.len(), 3 + options(true));
        assert_eq!(
            s.note.as_deref().map(|n| n.contains("b-Pano.dng")),
            Some(true)
        );
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        s.toggle(1);
        s.toggle(3);
        s.mode = Some(2);
        let (kept, prefs) = choices(&s, &paths, true, &Prefs::default());
        assert_eq!(kept, vec![paths[0].clone(), paths[2].clone()]);
        assert!(!prefs.crop && prefs.open);
        assert_eq!(PROJECTIONS[prefs.projection], Some(Projection::Cylindrical));
    }

    #[test]
    fn picture_sheet_offers_the_format() {
        let shots = vec![shot("a.jpg"), shot("b.jpg")];
        let mut s = sheet(&shots, &Prefs::default());
        assert_eq!(s.rows.len(), 2 + options(false));
        // 16-bit TIFF by default.
        assert!(s.note.as_deref().is_some_and(|n| n.contains("b-Pano.tif")));
        assert!(!s.rows[3].1);
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        s.toggle(3);
        let (_, prefs) = choices(&s, &paths, false, &Prefs::default());
        assert_eq!(prefs.format(), RgbFormat::JxlLossless);
        assert_eq!(result_name(&paths[1], false, &prefs), "b-Pano.jxl");
    }
}
