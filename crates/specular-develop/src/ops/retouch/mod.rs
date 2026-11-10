//! Retouch: heal, clone, fill and blur spots, each
//! on the whole picture or on one layer of a frequency separation.
//!
//! - **Spots** live in source coordinates (fractions of the frame, sizes as fractions of its
//!   long side), like mask shapes, so they follow crop, straighten and lens changes and
//!   export at any size. [`Op::place`] maps them through the warp onto the graph's picture
//!   once per graph build: centres and brush paths by [`Plan::to_output`], radii by the
//!   local scale, and the source offset rounded to whole pixels (no resampling blur).
//! - **Heal** is Poisson image editing (Pérez, Gangnet and Blake 2003): the source patch with
//!   a harmonic correction `g` (`Δg = 0` inside, `g = target − source` on the edge), so the
//!   patch keeps its texture and takes the target's tones at the edge. `g` is solved per spot
//!   by multigrid on the GPU ([`layout`], [`shader`]); a CPU reference checks it.
//! - **Clone** copies the source as it is. **Fill** takes its tones from the surroundings
//!   only (a membrane `Δf = 0` from the edge, a diffusion fill) and its fine grain from the
//!   source; no structure is copied. **Blur** is a Gaussian of the layer under the spot.
//! - **Frequency separation** splits the picture at `split_radius` (σ in source pixels) into
//!   a low layer `G ⊛ I` and a high layer `I − G ⊛ I`; a spot on one layer retouches only
//!   that layer and keeps the other.
//! - Every spot has a size, a feather (soft band inside the edge) and an opacity; spots are
//!   applied in order, each mixed over the result so far.
//!
//! **Tiles.** Each spot is solved over its whole extent in every tile that shows part of it,
//! with the same arithmetic, so tiles have no seams. The halo covers the farthest pixel a
//! spot reads (its source, the blur and split reach).

pub mod layout;
pub mod patch;
pub mod search;
pub mod shader;

#[cfg(test)]
pub(crate) mod cpu_ref;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::graph::{NodeKind, Op, OpData, Placement, Program, Roi};
use crate::mask::Geometry;
use crate::model::fields::{Choice, choice};
use crate::model::{Edit, Fields, ModuleKind, Params, Value};

/// Spots one module holds; more are left out.
pub const MAX_SPOTS: usize = 512;
/// Brush path points one module holds, all spots together.
pub const MAX_PATH_POINTS: usize = 8192;

choice! {
    /// What a spot does.
    SpotMode {
        /// Source texture, target tones at the edge (Poisson).
        Heal = "heal",
        /// Plain copy of the source.
        Clone = "clone",
        /// Tones from the surroundings, grain from the source.
        Fill = "fill",
        /// Gaussian blur of the area.
        Blur = "blur",
        /// "Remove (AI)": a heal whose source is the Remove feature's inpainted
        /// patch ([`patch`]); without its patch, a heal from the automatic source.
        Remove = "remove",
    }
}

choice! {
    /// Which frequency layer a spot acts on.
    Layer {
        All = "all",
        Low = "low",
        High = "high",
    }
}

impl SpotMode {
    /// Whether the spot reads a source.
    pub fn has_source(self) -> bool {
        !matches!(self, SpotMode::Blur)
    }

    fn index(self) -> u32 {
        match self {
            SpotMode::Heal => 0,
            SpotMode::Clone => 1,
            SpotMode::Fill => 2,
            SpotMode::Blur => 3,
            // Placed as a heal ([`Placed::new`]).
            SpotMode::Remove => 0,
        }
    }
}

impl Layer {
    fn index(self) -> u32 {
        match self {
            Layer::All => 0,
            Layer::Low => 1,
            Layer::High => 2,
        }
    }
}

/// One spot, in source coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub mode: SpotMode,
    pub layer: Layer,
    /// Centre of the area (fractions of the frame). For a brushed area, the point the
    /// source offset is measured from.
    pub target: [f32; 2],
    /// Where the source's centre is (fractions of the frame).
    pub source: [f32; 2],
    /// Radius (fraction of the frame's long side); for a brushed area, the brush radius.
    pub size: f32,
    /// Soft band inside the edge, 0 (hard) … 1 (soft from the centre).
    pub feather: f32,
    /// 0 … 1.
    pub opacity: f32,
    /// Blur σ (fraction of the frame's long side), blur spots only.
    pub blur: f32,
    /// A brushed area: the brush path (fractions of the frame). Empty: a round spot.
    pub path: Vec<[f32; 2]>,
    /// Removal spots: the inpainting model (the manifest id); empty for other spots.
    pub model: String,
}

impl Default for Spot {
    fn default() -> Self {
        Spot {
            mode: SpotMode::Heal,
            layer: Layer::All,
            target: [0.5, 0.5],
            source: [0.5, 0.5],
            size: 0.02,
            feather: 0.5,
            opacity: 1.0,
            blur: 0.004,
            path: Vec::new(),
            model: String::new(),
        }
    }
}

impl Spot {
    /// The source minus the target (fractions of the frame).
    pub fn offset(&self) -> [f32; 2] {
        [
            self.source[0] - self.target[0],
            self.source[1] - self.target[1],
        ]
    }

    /// Whether `p` (fractions of the frame) is on the spot's area; `aspect` is the frame's
    /// width over its height. Brushed areas count their path.
    pub fn covers(&self, p: [f32; 2], aspect: f32) -> bool {
        self.distance(p, self.target, aspect) <= self.size
    }

    /// Whether `p` is on the spot's source area.
    pub fn covers_source(&self, p: [f32; 2], aspect: f32) -> bool {
        self.mode.has_source() && self.distance(p, self.source, aspect) <= self.size
    }

    /// Distance from `p` to the area placed with its reference point at `at`, in fractions
    /// of the long side.
    fn distance(&self, p: [f32; 2], at: [f32; 2], aspect: f32) -> f32 {
        let (ax, ay) = if aspect >= 1.0 {
            (1.0, 1.0 / aspect)
        } else {
            (aspect, 1.0)
        };
        let d = |a: [f32; 2], b: [f32; 2]| {
            let (x, y) = ((a[0] - b[0]) * ax, (a[1] - b[1]) * ay);
            (x * x + y * y).sqrt()
        };
        if self.path.is_empty() {
            return d(p, at);
        }
        let shift = [at[0] - self.target[0], at[1] - self.target[1]];
        let pts: Vec<[f32; 2]> = self
            .path
            .iter()
            .map(|q| [q[0] + shift[0], q[1] + shift[1]])
            .collect();
        if pts.len() == 1 {
            return d(p, pts[0]);
        }
        let mut best = f32::MAX;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = [(b[0] - a[0]) * ax, (b[1] - a[1]) * ay];
            let ap = [(p[0] - a[0]) * ax, (p[1] - a[1]) * ay];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1];
            let t = if l2 > 0.0 {
                ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (x, y) = (ap[0] - ab[0] * t, ap[1] - ab[1] * t);
            best = best.min((x * x + y * y).sqrt());
        }
        best
    }
}

/// Retouch params (fields of [`ModuleKind::RETOUCH`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Retouch {
    /// Frequency separation σ in source pixels, 0.5 … 100.
    pub split_radius: f32,
    pub spots: Vec<Spot>,
}

impl Default for Retouch {
    fn default() -> Self {
        Retouch {
            split_radius: 4.0,
            spots: Vec::new(),
        }
    }
}

fn finite_or(v: f32, d: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

impl Retouch {
    pub fn default_fields() -> Fields {
        Retouch::default().to_fields()
    }

    pub fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }

    /// Columns: one word or number per spot in each `spot_*` field; brush paths as point
    /// counts (`spot_points`) and the points one spot after another (`spot_path`).
    pub fn to_fields(&self) -> Fields {
        let spots = &self.spots;
        let words =
            |f: &dyn Fn(&Spot) -> &'static str| spots.iter().map(f).collect::<Vec<_>>().join(" ");
        let col = |f: &dyn Fn(&Spot) -> Vec<f32>| spots.iter().flat_map(f).collect::<Vec<f32>>();
        // Models only when a spot names one (older edits keep their fields and hashes).
        let models = spots.iter().any(|s| !s.model.is_empty()).then(|| {
            spots
                .iter()
                .map(|s| {
                    if s.model.is_empty() || s.model.contains(char::is_whitespace) {
                        "-"
                    } else {
                        s.model.as_str()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        let f = Fields::new()
            .with("split_radius", Value::Float(self.split_radius))
            .with("spot_mode", Value::Text(words(&|s| s.mode.name())))
            .with("spot_layer", Value::Text(words(&|s| s.layer.name())))
            .with("spot_target", Value::Floats(col(&|s| s.target.to_vec())))
            .with("spot_source", Value::Floats(col(&|s| s.source.to_vec())))
            .with("spot_size", Value::Floats(col(&|s| vec![s.size])))
            .with("spot_feather", Value::Floats(col(&|s| vec![s.feather])))
            .with("spot_opacity", Value::Floats(col(&|s| vec![s.opacity])))
            .with("spot_blur", Value::Floats(col(&|s| vec![s.blur])))
            .with(
                "spot_points",
                Value::Floats(col(&|s| vec![s.path.len() as f32])),
            )
            .with(
                "spot_path",
                Value::Floats(col(&|s| s.path.iter().flatten().copied().collect())),
            );
        match models {
            Some(m) => f.with("spot_model", Value::Text(m)),
            None => f,
        }
    }

    /// Reads the columns; the spot count is the number of modes, and a short column gives
    /// its default to the spots it misses.
    pub fn from_fields(f: &Fields) -> Self {
        let d = Retouch::default();
        let ds = Spot::default();
        let floats = |n: &str| match f.get(n) {
            Some(Value::Floats(v)) => v.clone(),
            Some(Value::Float(v)) => vec![*v],
            _ => Vec::new(),
        };
        let modes: Vec<SpotMode> = f
            .text("spot_mode")
            .unwrap_or("")
            .split_ascii_whitespace()
            .take(MAX_SPOTS)
            .map(|w| SpotMode::from_name(w).unwrap_or_default())
            .collect();
        let models: Vec<&str> = f
            .text("spot_model")
            .unwrap_or("")
            .split_ascii_whitespace()
            .collect();
        let layers: Vec<Layer> = f
            .text("spot_layer")
            .unwrap_or("")
            .split_ascii_whitespace()
            .map(|w| Layer::from_name(w).unwrap_or_default())
            .collect();
        let (target, source, size, feather, opacity, blur, points, path) = (
            floats("spot_target"),
            floats("spot_source"),
            floats("spot_size"),
            floats("spot_feather"),
            floats("spot_opacity"),
            floats("spot_blur"),
            floats("spot_points"),
            floats("spot_path"),
        );
        let at = |v: &[f32], i: usize, dv: f32| v.get(i).copied().unwrap_or(dv);
        let mut used = 0usize;
        let spots = modes
            .iter()
            .enumerate()
            .map(|(i, &mode)| {
                let pt = |v: &[f32], dv: [f32; 2]| {
                    [
                        finite_or(at(v, 2 * i, dv[0]), dv[0], -1.0, 2.0),
                        finite_or(at(v, 2 * i + 1, dv[1]), dv[1], -1.0, 2.0),
                    ]
                };
                let target = pt(&target, ds.target);
                let n = at(&points, i, 0.0);
                let n = if n.is_finite() {
                    n.max(0.0) as usize
                } else {
                    0
                };
                let n = n.min(MAX_PATH_POINTS - used.min(MAX_PATH_POINTS));
                let mut p = Vec::with_capacity(n);
                for k in 0..n {
                    let j = 2 * (used + k);
                    if let (Some(&x), Some(&y)) = (path.get(j), path.get(j + 1))
                        && x.is_finite()
                        && y.is_finite()
                    {
                        p.push([x.clamp(-1.0, 2.0), y.clamp(-1.0, 2.0)]);
                    }
                }
                used += n;
                let model = models
                    .get(i)
                    .filter(|m| **m != "-")
                    .map_or_else(String::new, |m| (*m).to_owned());
                Spot {
                    mode,
                    // A removal works on the whole picture.
                    layer: if mode == SpotMode::Remove {
                        Layer::All
                    } else {
                        layers.get(i).copied().unwrap_or_default()
                    },
                    model,
                    target,
                    source: pt(&source, target),
                    size: finite_or(at(&size, i, ds.size), ds.size, 0.0005, 0.5),
                    feather: finite_or(at(&feather, i, ds.feather), ds.feather, 0.0, 1.0),
                    opacity: finite_or(at(&opacity, i, ds.opacity), ds.opacity, 0.0, 1.0),
                    blur: finite_or(at(&blur, i, ds.blur), ds.blur, 0.0, 0.05),
                    path: p,
                }
            })
            .collect();
        Retouch {
            split_radius: finite_or(
                f.f32_or("split_radius", d.split_radius),
                d.split_radius,
                0.5,
                100.0,
            ),
            spots,
        }
    }

    /// Whether the module changes nothing.
    pub fn is_identity(&self) -> bool {
        self.spots.iter().all(|s| s.opacity <= 0.0)
    }
}

/// Whether `edit` has an enabled retouch module with spots: its graph needs the warp's
/// geometry ([`crate::graph::Graph::build_masked`]) as masks do.
pub fn needs_geometry(edit: &Edit) -> bool {
    edit.modules.iter().any(|m| {
        m.enabled && m.kind == ModuleKind::RETOUCH && !Retouch::of(&m.params).is_identity()
    })
}

/// Where a frame point (fractions) lands on the graph's picture (pixels).
pub fn frame_to_image(g: &Geometry<'_>, image: (u32, u32), p: [f32; 2]) -> Option<[f64; 2]> {
    let (iw, ih) = (f64::from(image.0.max(1)), f64::from(image.1.max(1)));
    let (u, v) = (f64::from(p[0]), f64::from(p[1]));
    match g {
        Geometry::Frame { .. } => Some([u * iw, v * ih]),
        Geometry::Warp(plan) => {
            let [fx, fy, fw, fh] = plan.frame;
            let o = plan.to_output((fx + u * fw, fy + v * fh), 1.0)?;
            let c = plan.crop_rect();
            Some([o.0 / c[2].max(1e-9) * iw, o.1 / c[3].max(1e-9) * ih])
        }
    }
}

/// The frame point (fractions) shown at picture pixel `p` (the inverse of
/// [`frame_to_image`]).
pub fn image_to_frame(g: &Geometry<'_>, image: (u32, u32), p: [f64; 2]) -> [f32; 2] {
    let (iw, ih) = (f64::from(image.0.max(1)), f64::from(image.1.max(1)));
    let (x, y) = g.frame_point(p[0] / iw, p[1] / ih);
    [x as f32, y as f32]
}

/// Picture pixels per frame long-side unit near frame point `p`.
fn local_scale(g: &Geometry<'_>, image: (u32, u32), p: [f32; 2], c: [f64; 2]) -> f64 {
    let (fw, fh) = match g {
        Geometry::Frame { w, h } => (w.max(1.0), h.max(1.0)),
        Geometry::Warp(plan) => (plan.frame[2].max(1.0), plan.frame[3].max(1.0)),
    };
    let long = fw.max(fh);
    // A step of 1 % of the long side, across and down; their mean length.
    let e = 0.01;
    let dx = (e * long / fw) as f32;
    let dy = (e * long / fh) as f32;
    let mut sum = 0.0;
    let mut n = 0.0;
    for q in [[p[0] + dx, p[1]], [p[0], p[1] + dy]] {
        if let Some(o) = frame_to_image(g, image, q) {
            sum += ((o[0] - c[0]).powi(2) + (o[1] - c[1]).powi(2)).sqrt() / e;
            n += 1.0;
        }
    }
    if n > 0.0 {
        sum / n
    } else {
        f64::from(image.0.max(image.1))
    }
}

/// Retouch params resolved onto one picture ([`Op::place`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub image: (u32, u32),
    /// Frequency separation σ in picture pixels.
    pub split: f32,
    pub spots: Vec<PlacedSpot>,
    /// A quick render while a spot is dragged ([`Placement::draft`]).
    pub draft: bool,
}

/// A spot on the picture: pixels of the graph's image.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedSpot {
    pub mode: SpotMode,
    pub layer: Layer,
    pub centre: [f32; 2],
    /// Source minus target, whole pixels.
    pub offset: [i32; 2],
    pub radius: f32,
    pub feather: f32,
    pub opacity: f32,
    /// Blur σ in pixels.
    pub blur: f32,
    /// Brush path in pixels; empty for a round spot.
    pub path: Vec<[f32; 2]>,
    /// A removal spot's patch: its key, size and the map from picture pixels to its
    /// texels ([`patch::affine`]). The spot is then a heal from the patch.
    pub patch: Option<PlacedPatch>,
}

/// A placed removal patch.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedPatch {
    pub key: [u8; 32],
    pub w: u32,
    pub h: u32,
    pub affine: [f32; 6],
}

const PLACED: &str = "_placed";
const DRAFT: &str = "_draft";
/// Floats per placed spot: 11, then a patch flag, its key as 16 halves, size and map.
const SPOT_FLOATS: usize = 11 + 1 + 16 + 2 + 6;

impl Placed {
    /// Spots of `r` on the picture `at` describes; spots that leave it (a source or centre
    /// the warp cannot reach) are left out.
    pub fn new(r: &Retouch, at: &Placement<'_>) -> Placed {
        let g = &at.geometry;
        let mut spots = Vec::new();
        let mut px_per_long = None;
        for s in &r.spots {
            if s.opacity <= 0.0 {
                continue;
            }
            let Some(c) = frame_to_image(g, at.image, s.target) else {
                continue;
            };
            let k = local_scale(g, at.image, s.target, c);
            px_per_long.get_or_insert(k);
            let off = if s.mode.has_source() {
                match frame_to_image(g, at.image, s.source) {
                    Some(o) => [(o[0] - c[0]).round() as i32, (o[1] - c[1]).round() as i32],
                    None => continue,
                }
            } else {
                [0, 0]
            };
            let mut path: Vec<[f32; 2]> = s
                .path
                .iter()
                .filter_map(|p| frame_to_image(g, at.image, *p))
                .map(|p| [p[0] as f32, p[1] as f32])
                .collect();
            simplify(&mut path, 0.25);
            let placed_patch = if s.mode == SpotMode::Remove && !s.model.is_empty() {
                let key = patch::key(&at.photo, s);
                let found = patch::store()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&key);
                found.and_then(|p| {
                    let corner = |f: [f32; 2]| frame_to_image(g, at.image, f);
                    patch::affine(&p, &corner).map(|affine| PlacedPatch {
                        key,
                        w: p.w,
                        h: p.h,
                        affine,
                    })
                })
            } else {
                None
            };
            spots.push(PlacedSpot {
                patch: placed_patch,
                mode: if s.mode == SpotMode::Remove {
                    SpotMode::Heal
                } else {
                    s.mode
                },
                layer: s.layer,
                centre: [c[0] as f32, c[1] as f32],
                offset: off,
                radius: ((f64::from(s.size) * k) as f32).max(0.5),
                feather: s.feather,
                opacity: s.opacity,
                blur: ((f64::from(s.blur) * k) as f32).max(0.3),
                path,
            });
        }
        // Picture pixels per sensor pixel, for the split radius (a binned frame's pixel is
        // `sensor_bin` sensor pixels wide).
        let per_source = match g {
            Geometry::Frame { w, .. } => f64::from(at.image.0.max(1)) / w.max(1.0),
            Geometry::Warp(plan) => f64::from(at.image.0.max(1)) / plan.crop_rect()[2].max(1e-9),
        } / at.sensor_bin.max(1.0);
        Placed {
            image: at.image,
            split: ((f64::from(r.split_radius) * per_source) as f32).max(0.3),
            spots,
            draft: at.draft,
        }
    }

    pub fn to_fields(&self) -> Fields {
        let mut v = vec![
            self.image.0 as f32,
            self.image.1 as f32,
            self.split,
            self.spots.len() as f32,
        ];
        for s in &self.spots {
            v.extend_from_slice(&[
                s.mode.index() as f32,
                s.layer.index() as f32,
                s.centre[0],
                s.centre[1],
                s.offset[0] as f32,
                s.offset[1] as f32,
                s.radius,
                s.feather,
                s.opacity,
                s.blur,
                s.path.len() as f32,
            ]);
            match &s.patch {
                Some(p) => {
                    v.push(1.0);
                    v.extend(
                        p.key
                            .chunks_exact(2)
                            .map(|c| f32::from(u16::from_le_bytes([c[0], c[1]]))),
                    );
                    v.extend_from_slice(&[p.w as f32, p.h as f32]);
                    v.extend_from_slice(&p.affine);
                }
                None => v.extend(std::iter::repeat_n(0.0, SPOT_FLOATS - 11)),
            }
        }
        for s in &self.spots {
            v.extend(s.path.iter().flatten());
        }
        Fields::new()
            .with(PLACED, Value::Floats(v))
            .with(DRAFT, Value::Bool(self.draft))
    }

    /// The placed form, if `f` holds one.
    pub fn from_fields(f: &Fields) -> Option<Placed> {
        let Some(Value::Floats(v)) = f.get(PLACED) else {
            return None;
        };
        let head = v.get(..4)?;
        let n = head[3] as usize;
        let body = v.get(4..4 + n * SPOT_FLOATS)?;
        let mut path_at = 4 + n * SPOT_FLOATS;
        let mut spots = Vec::with_capacity(n);
        for c in body.chunks_exact(SPOT_FLOATS) {
            let np = c[10] as usize;
            let pts = v.get(path_at..path_at + 2 * np)?;
            path_at += 2 * np;
            spots.push(PlacedSpot {
                mode: SpotMode::ALL.get(c[0] as usize)?.0,
                layer: Layer::ALL.get(c[1] as usize)?.0,
                centre: [c[2], c[3]],
                offset: [c[4] as i32, c[5] as i32],
                radius: c[6],
                feather: c[7],
                opacity: c[8],
                blur: c[9],
                path: pts.chunks_exact(2).map(|p| [p[0], p[1]]).collect(),
                patch: (c[11] == 1.0).then(|| {
                    let mut key = [0u8; 32];
                    for (k, h) in c[12..28].iter().enumerate() {
                        key[2 * k..2 * k + 2].copy_from_slice(&(*h as u16).to_le_bytes());
                    }
                    PlacedPatch {
                        key,
                        w: c[28] as u32,
                        h: c[29] as u32,
                        affine: std::array::from_fn(|i| c[30 + i]),
                    }
                }),
            });
        }
        Some(Placed {
            image: (head[0] as u32, head[1] as u32),
            split: head[2],
            spots,
            draft: f.bool_or(DRAFT, false),
        })
    }
}

/// Drops path points within `tol` pixels of the line through their neighbours
/// (Ramer–Douglas–Peucker), so long strokes stay cheap per pixel.
fn simplify(path: &mut Vec<[f32; 2]>, tol: f32) {
    if path.len() < 3 {
        return;
    }
    let mut keep = vec![false; path.len()];
    keep[0] = true;
    *keep.last_mut().expect("non-empty") = true;
    let mut stack = vec![(0usize, path.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (path[a], path[b]);
        let ab = [pb[0] - pa[0], pb[1] - pa[1]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1];
        let mut far = (0.0f32, a);
        for (i, p) in path.iter().enumerate().take(b).skip(a + 1) {
            let ap = [p[0] - pa[0], p[1] - pa[1]];
            let t = if l2 > 0.0 {
                ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = ((ap[0] - ab[0] * t).powi(2) + (ap[1] - ab[1] * t).powi(2)).sqrt();
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((a, far.1));
            stack.push((far.1, b));
        }
    }
    let mut i = 0;
    path.retain(|_| {
        let k = keep[i];
        i += 1;
        k
    });
}

/// The placed form of `p`; params never placed (a graph built without a picture) are
/// placed on a 1024² frame.
pub fn placed(p: &Params) -> Placed {
    let f = p.to_fields();
    Placed::from_fields(&f).unwrap_or_else(|| {
        Placed::new(
            &Retouch::from_fields(&f),
            &Placement {
                geometry: Geometry::Frame { w: 1.0, h: 1.0 },
                image: (1024, 1024),
                draft: false,
                sensor_bin: 1.0,
                photo: [0; 32],
            },
        )
    })
}

/// Retouch, param version 1.
pub struct RetouchOp;

impl Op for RetouchOp {
    fn name(&self) -> &'static str {
        "retouch_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn heavy(&self) -> bool {
        true
    }
    fn halo(&self, p: &Params, _scale: f32) -> u32 {
        layout::Layout::new(&placed(p)).halo
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _p: &Params, _scale: f32, _out: &mut [u32]) {}
    fn identity(&self, p: &Params) -> bool {
        match Placed::from_fields(&p.to_fields()) {
            Some(pl) => pl.spots.is_empty(),
            None => Retouch::of(p).is_identity(),
        }
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        let pl = Placed::new(&Retouch::of(p), at);
        Some(Params::Other(ModuleKind::RETOUCH, pl.to_fields()))
    }
    fn local(&self, p: &Params, input: Roi, _scale: f32) -> Option<Program> {
        Some(layout::Layout::new(&placed(p)).program(input))
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        let pl = placed(p);
        Some(Arc::new(OpData::new(layout::Layout::new(&pl).patched(&pl))))
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[shader::LC_WGSL, shader::COMMON_WGSL, shader::COMPOSITE_WGSL]
    }
    fn wgsl(&self) -> &'static str {
        "    return rt_composite(x, y);"
    }
}
