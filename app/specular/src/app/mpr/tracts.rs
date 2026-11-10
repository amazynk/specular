//! Streamlines over the MPR views: a brain volume opened in MPR (NIfTI and the other
//! research formats) looks for tractograms beside it (TrackVis .trk, MRtrix .tck;
//! [`specular_decode::neuro_dec::tracts_beside`]) and reads them off the UI thread. Each
//! view then draws the pieces of streamline within a slab about its plane (half the slab's
//! thickness either side), as lines in the 3D view's direction colours (|x| red, |y| green,
//! |z| blue). `Alt+S` steps the slab's thickness through [`SLABS`], then hides them.
//!
//! Streamlines are in RAS millimetres (TrackVis through its voxel-to-RAS matrix, MRtrix
//! scanner coordinates); the views are in DICOM's LPS, as the volume's affine is read, so a
//! point goes over as (−x, −y, z). Pieces are cut exactly at the slab's faces, so a
//! streamline crossing the plane at a slant shows as a short stroke and one running in the
//! plane as its whole path.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crossbeam_channel::Receiver;
use specular_decode::neuro_dec::{Tractogram, direction_colour, read_tracts, tracts_beside};
use specular_i18n::t;

use super::super::medical::number;
use super::draw::Layout;
use super::views::{R, V3, Views, dot, sub};

/// The slab thicknesses `Alt+S` steps through (mm); past the last the streamlines hide.
pub(crate) const SLABS: [f64; 6] = [1.0, 2.0, 4.0, 6.0, 10.0, 20.0];

/// The slab the streamlines start with: [`SLABS`]`[2]`, 4 mm (two voxels either side of
/// the usual 1–2 mm diffusion and anatomical grids).
const DEFAULT_SLAB: usize = 2;

/// At most this many pieces per view: past it every n-th is drawn.
pub(crate) const MAX_PIECES: usize = 60_000;

/// A piece of streamline in a view: its ends on screen (logical) and its colour.
pub(crate) type Piece = ((f32, f32), (f32, f32), [u8; 4]);

/// Each view's pieces.
pub(crate) type Pieces = [Arc<Vec<Piece>>; 3];

/// The tractograms found beside the volume, and how they are drawn.
pub(crate) struct Tracts {
    rx: Option<Receiver<Found>>,
    /// The files read (their names) and their streamlines.
    pub files: Vec<(String, Arc<Tractogram>)>,
    /// Index into [`SLABS`]; [`SLABS`]`.len()` hides them.
    slab: usize,
    /// The pieces last cut and what they were cut for.
    cache: Mutex<Option<(Key, Pieces)>>,
}

/// What the pieces depend on.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    views: Views,
    layout: Layout,
    slab: usize,
    files: usize,
}

/// One tractogram read off the UI thread, or why it was not.
pub(crate) type Found = (PathBuf, Result<Tractogram, String>);

impl Default for Tracts {
    fn default() -> Tracts {
        Tracts {
            rx: None,
            files: Vec::new(),
            slab: DEFAULT_SLAB,
            cache: Mutex::new(None),
        }
    }
}

impl Tracts {
    /// Looks for tractograms beside `volume` and reads them off the UI thread (through
    /// `spawn`, which runs its job in the background); `wake` once each is in.
    pub(crate) fn start(
        volume: &Path,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>),
        wake: impl Fn() + Send + 'static,
    ) -> Tracts {
        let found = tracts_beside(volume);
        if found.is_empty() {
            return Tracts::default();
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        spawn(Box::new(move || {
            for p in found {
                let r = read_tracts(&p).map_err(|e| e.to_string());
                if tx.send((p, r)).is_err() {
                    return;
                }
                wake();
            }
        }));
        Tracts {
            rx: Some(rx),
            ..Tracts::default()
        }
    }

    /// Takes the tractograms read since the last poll; true when one came in.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        let mut any = false;
        for (p, r) in rx.try_iter() {
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            match r {
                Ok(t) if t.streamlines() > 0 => {
                    specular_log::info!(
                        target: specular_log::DECODE,
                        "mpr: {} streamlines from {name} over the views",
                        t.streamlines()
                    );
                    self.files.push((name, Arc::new(t)));
                    any = true;
                }
                Ok(_) => {}
                Err(e) => {
                    specular_log::info!(target: specular_log::DECODE, "mpr: {name} not read: {e}");
                }
            }
        }
        any
    }

    /// The slab's thickness (mm), or `None` when the streamlines are hidden.
    pub(crate) fn thickness(&self) -> Option<f64> {
        SLABS.get(self.slab).copied()
    }

    /// The next thickness, then hidden, then the thinnest again.
    pub(crate) fn step(&mut self) {
        self.slab = (self.slab + 1) % (SLABS.len() + 1);
    }

    /// The panel's (and the toast's) line: the files and the slab, or `None` without any.
    pub(crate) fn text(&self) -> Option<String> {
        if self.files.is_empty() {
            return None;
        }
        let files = self
            .files
            .iter()
            .map(|(n, _)| specular_i18n::isolate_ltr(n))
            .collect::<Vec<_>>()
            .join(", ");
        Some(match self.thickness() {
            Some(t) => t!("mpr-tracts", files = files, thickness = number(t)),
            None => t!("mpr-tracts-off", files = files),
        })
    }

    /// Each view's pieces for `views` laid out in `layout` (cut again only when either
    /// changed).
    pub(crate) fn pieces(&self, views: &Views, layout: Layout) -> Pieces {
        let key = Key {
            views: views.clone(),
            layout,
            slab: self.slab,
            files: self.files.len(),
        };
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((k, p)) = cache.as_ref()
            && *k == key
        {
            return p.clone();
        }
        let p = match self.thickness() {
            Some(t) if !self.files.is_empty() => [0, 1, 2].map(|v| {
                let mut out = Vec::new();
                for (_, tr) in &self.files {
                    cut(&mut out, tr, views, v, layout.views[v], t / 2.0);
                }
                Arc::new(thin(out, MAX_PIECES))
            }),
            _ => [0; 3].map(|_| Arc::new(Vec::new())),
        };
        *cache = Some((key, p.clone()));
        p
    }
}

/// A RAS point in the views' LPS frame.
pub(crate) fn lps(p: [f32; 3]) -> V3 {
    [-f64::from(p[0]), -f64::from(p[1]), f64::from(p[2])]
}

/// Appends the pieces of `t`'s streamlines within `half` mm of view `v`'s plane, on the
/// screen of rectangle `r`.
pub(crate) fn cut(out: &mut Vec<Piece>, t: &Tractogram, views: &Views, v: usize, r: R, half: f64) {
    let n = views.normal(v);
    let c = dot(views.c, n);
    let to = |p: V3| {
        let q = views.to_screen(v, r, p);
        (q.0 as f32, q.1 as f32)
    };
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    for s in 0..t.streamlines() {
        let pts = t.streamline(s);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if !a.iter().chain(&b).all(|x| x.is_finite()) {
                continue;
            }
            let (pa, pb) = (lps(a), lps(b));
            let (da, db) = (dot(pa, n) - c, dot(pb, n) - c);
            // Both ends beyond the same face: nothing of it is in the slab.
            if (da > half && db > half) || (da < -half && db < -half) {
                continue;
            }
            // The part within the slab: t along a→b where |d| <= half.
            let (mut t0, mut t1) = (0.0f64, 1.0f64);
            let dd = db - da;
            if dd.abs() > 1e-12 {
                let ta = (-half - da) / dd;
                let tb = (half - da) / dd;
                let (lo, hi) = if ta < tb { (ta, tb) } else { (tb, ta) };
                t0 = t0.max(lo);
                t1 = t1.min(hi);
                if t0 > t1 {
                    continue;
                }
            }
            let d = sub(pb, pa);
            let qa = [0, 1, 2].map(|i| pa[i] + d[i] * t0);
            let qb = [0, 1, 2].map(|i| pa[i] + d[i] * t1);
            let (sa, sb) = (to(qa), to(qb));
            // Off the view entirely (both ends past the same edge).
            let off = |f: fn((f32, f32)) -> f64, lo: f64, hi: f64| {
                (f(sa) < lo && f(sb) < lo) || (f(sa) > hi && f(sb) > hi)
            };
            if off(|p| f64::from(p.0), x0, x1) || off(|p| f64::from(p.1), y0, y1) {
                continue;
            }
            out.push((sa, sb, direction_colour(a, b)));
        }
    }
}

/// At most `max` pieces: every n-th when there are more.
fn thin(pieces: Vec<Piece>, max: usize) -> Vec<Piece> {
    if pieces.len() <= max {
        return pieces;
    }
    let step = pieces.len().div_ceil(max);
    pieces.into_iter().step_by(step).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_dicom::volume::Geometry;

    /// A 1 mm grid of 101³ voxels centred on the origin (LPS).
    fn views() -> Views {
        let positions: Vec<[f64; 3]> = (0..101).map(|k| [-50.0, -50.0, k as f64 - 50.0]).collect();
        let g = Geometry::new(
            &positions,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0],
            [101, 101],
        );
        Views::new(&g)
    }

    fn tracts(lines: &[&[[f32; 3]]]) -> Tractogram {
        let mut t = Tractogram::default();
        t.offsets.push(0);
        for l in lines {
            t.points.extend_from_slice(l);
            t.offsets.push(t.points.len() as u32);
        }
        t
    }

    const R0: R = [0.0, 0.0, 400.0, 400.0];

    #[test]
    fn pieces_are_cut_at_the_slab_faces() {
        let v = views();
        // Along RAS z (axial normal) from -10 to +10 mm at RAS x = 10 (LPS x = -10).
        let t = tracts(&[&[[10.0, 0.0, -10.0], [10.0, 0.0, 10.0]]]);
        let mut out = Vec::new();
        cut(&mut out, &t, &v, 0, R0, 2.0);
        // In the axial view the piece is end-on: a point at LPS (-10, 0).
        assert_eq!(out.len(), 1);
        let ((ax, ay), (bx, by), c) = out[0];
        assert!((ax - bx).abs() < 1e-3 && (ay - by).abs() < 1e-3);
        let want = v.to_screen(0, R0, [-10.0, 0.0, 0.0]);
        assert!((f64::from(ax) - want.0).abs() < 1e-3, "{ax} vs {}", want.0);
        assert_eq!(c, [0, 0, 255, 255]);
        // In the coronal view (normal ±y, the crosshair at y = 0) it lies in the plane: all
        // 20 mm of it show.
        out.clear();
        cut(&mut out, &t, &v, 1, R0, 2.0);
        assert_eq!(out.len(), 1);
        let ((_, ay), (_, by), _) = out[0];
        let mm = v.mm(1, R0);
        assert!(((f64::from(by - ay)).abs() * mm - 20.0).abs() < 1e-3);
        // Moved out of the slab: none.
        let t = tracts(&[&[[10.0, 5.0, -10.0], [10.0, 5.0, 10.0]]]);
        out.clear();
        cut(&mut out, &t, &v, 1, R0, 2.0);
        assert!(out.is_empty());
    }

    #[test]
    fn a_slanted_piece_keeps_only_the_slab() {
        let v = views();
        // Through the axial plane at 45°: within ±2 mm of z = 0 the piece spans 4 mm in x.
        let t = tracts(&[&[[-10.0, 0.0, -10.0], [10.0, 0.0, 10.0]]]);
        let mut out = Vec::new();
        cut(&mut out, &t, &v, 0, R0, 2.0);
        assert_eq!(out.len(), 1);
        let ((ax, _), (bx, _), _) = out[0];
        let mm = v.mm(0, R0);
        assert!(((f64::from(bx - ax)).abs() * mm - 4.0).abs() < 1e-3);
    }

    /// A TrackVis 2 file (little-endian, RAS voxel order) of one streamline through
    /// `voxels`, with `vox_to_ras` its matrix and `vs` its voxel size.
    fn trk(vox_to_ras: [[f32; 4]; 4], vs: [f32; 3], voxels: &[[f32; 3]]) -> Vec<u8> {
        let mut h = vec![0u8; 1000];
        let put = |h: &mut Vec<u8>, at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
        put(&mut h, 0, b"TRACK\0");
        for (n, d) in [16i16, 16, 12].iter().enumerate() {
            put(&mut h, 6 + 2 * n, &d.to_le_bytes());
        }
        for (n, s) in vs.iter().enumerate() {
            put(&mut h, 12 + 4 * n, &s.to_le_bytes());
        }
        for (r, row) in vox_to_ras.iter().enumerate() {
            for (c, x) in row.iter().enumerate() {
                put(&mut h, 440 + 16 * r + 4 * c, &x.to_le_bytes());
            }
        }
        put(&mut h, 948, b"RAS");
        put(&mut h, 988, &1i32.to_le_bytes());
        put(&mut h, 992, &2i32.to_le_bytes());
        put(&mut h, 996, &1000i32.to_le_bytes());
        h.extend((voxels.len() as i32).to_le_bytes());
        // voxmm: a voxel's centre is half a voxel in from its corner.
        for v in voxels {
            for a in 0..3 {
                h.extend(((v[a] + 0.5) * vs[a]).to_le_bytes());
            }
        }
        h
    }

    /// An MRtrix .tck of one streamline through scanner (RAS) points `ras`.
    fn tck(ras: &[[f32; 3]]) -> Vec<u8> {
        let mut h = b"mrtrix tracks\ncount: 1\ndatatype: Float32LE\nfile: . 128\nEND\n".to_vec();
        h.resize(128, 0);
        let end = [[f32::NAN; 3], [f32::INFINITY; 3]];
        for p in ras.iter().chain(&end) {
            for x in p {
                h.extend(x.to_le_bytes());
            }
        }
        h
    }

    /// The streamlines' RAS points land on the volume's voxels in the views' LPS frame: a
    /// NIfTI with an oblique sform, a TrackVis file whose voxel-to-RAS matrix is that sform
    /// and an MRtrix file in the sform's scanner coordinates, each through the same voxels.
    #[test]
    fn streamlines_meet_the_volume_where_its_affine_puts_them() {
        use specular_decode::medvol;
        let (c, s) = (0.3f32.cos(), 0.3f32.sin());
        let vs = [2.0f32, 2.0, 2.5];
        let srow = [
            [vs[0] * c, -vs[1] * s, 0.0, -20.0],
            [vs[0] * s, vs[1] * c, 0.0, -30.0],
            [0.0, 0.0, vs[2], 12.0],
        ];
        let dims = [16u16, 16, 12];
        let n: usize = dims.iter().map(|&d| usize::from(d)).product();
        let nii = crate::app::medical::tests::nifti_i16(dims, srow, (0.0, 1.0), "t", &vec![0; n]);
        let voxels = [[1.0f32, 2.0, 3.0], [8.0, 5.0, 6.0], [15.0, 15.0, 11.0]];
        let ras: Vec<[f32; 3]> = voxels
            .iter()
            .map(|v| {
                [0, 1, 2]
                    .map(|r| srow[r][0] * v[0] + srow[r][1] * v[1] + srow[r][2] * v[2] + srow[r][3])
            })
            .collect();
        let mut m = [[0.0f32; 4]; 4];
        m[..3].copy_from_slice(&srow);
        m[3][3] = 1.0;
        let dir = std::env::temp_dir().join(format!("specular-mpr-tracts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("brain.nii"), nii).unwrap();
        std::fs::write(dir.join("a.trk"), trk(m, vs, &voxels)).unwrap();
        std::fs::write(dir.join("b.tck"), tck(&ras)).unwrap();
        let vol = medvol::open_path(&dir.join("brain.nii")).unwrap();
        let found = tracts_beside(&dir.join("brain.nii"));
        assert_eq!(found.len(), 2);
        for p in &found {
            let t = read_tracts(p).unwrap();
            assert_eq!(t.points.len(), voxels.len(), "{}", p.display());
            for (q, v) in t.points.iter().zip(&voxels) {
                let want = vol.header.to_patient(v.map(f64::from));
                let got = lps(*q);
                for a in 0..3 {
                    assert!(
                        (got[a] - want[a]).abs() < 1e-3,
                        "{}: voxel {v:?} at {got:?}, the volume says {want:?}",
                        p.display()
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_slab_steps_then_hides() {
        let mut t = Tracts::default();
        assert_eq!(t.thickness(), Some(4.0));
        let mut seen = Vec::new();
        for _ in 0..SLABS.len() + 1 {
            t.step();
            seen.push(t.thickness());
        }
        assert_eq!(seen[SLABS.len() - DEFAULT_SLAB - 1], None);
        assert_eq!(seen[SLABS.len()], Some(4.0));
        assert_eq!(thin(vec![((0.0, 0.0), (1.0, 1.0), [0; 4]); 10], 3).len(), 3);
    }
}
