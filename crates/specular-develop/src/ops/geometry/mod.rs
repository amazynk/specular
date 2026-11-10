//! Geometry: orientation, straighten, crop and
//! lens correction as **one fused warp** that samples the source once.
//!
//! # Coordinates
//!
//! - **Source**: the developed source buffer (`sw × sh` pixels) covers the *frame*, a
//!   rectangle in sensor pixels ([`Context::frame`]: the raw's crop, or the image itself).
//! - **Corrected frame**: the same rectangle after lens correction (same size and centre).
//! - **Oriented frame** (`Wo × Ho`): the corrected frame turned by the base orientation (the
//!   file's, as the viewer shows it) and then the edit's quarter turns and flips, in square
//!   pixels: a sensor with non-square pixels (DNG `DefaultScale`, [`Context::stretch`]) is
//!   stretched here as the viewer stretches it, so crops, angles and aspect ratios are the
//!   shown ones.
//! - **Straightened frame**: the oriented frame rotated by `angle_deg` (positive: clockwise
//!   on screen) about its centre, keeping its size; corners the rotation uncovers are empty.
//! - **Crop**: `[left, top, right, bottom]` fractions of the straightened frame. The output
//!   is that rectangle at any scale.
//!
//! [`Plan::to_source`] maps an output point back to the source, plane by plane, with the
//! vignetting gain there; the GPU kernel ([`warp`]) does the same in f32 and samples with a
//! Catmull-Rom filter. [`Plan::to_output`] inverts it. Masks are stored in source
//! coordinates (fractions of the frame) and evaluated through these two maps, so they follow
//! crop, rotation and lens correction.
//!
//! # Order of the back-map
//!
//! output pixel → crop → straighten → perspective ([`persp`]: keystone and upright, on the
//! oriented frame in square pixels, so on lens-corrected positions) → orientation → fill zoom → manual distortion and
//! colour → the camera's model (maker tables) → DNG `OpcodeList3` warps and vignettes
//! (exactly as the DNG specification and the CPU develop, from the first warp of the list
//! on) → source pixel, with the vignetting gains multiplied on.
//!
//! Colour planes are camera planes: with the camera matrix given ([`Context::camera`]) the
//! kernel samples each plane's position, takes that plane in camera RGB and converts back,
//! so per-plane warps (TCA, DNG three-plane warps) are exact for matrix profiles.

pub mod crop;
pub mod homography;
pub mod lens;
pub mod lensfun;
pub mod lsd;
pub mod persp;
pub mod upright;
pub mod warp;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_persp;

use std::sync::Arc;

use specular_raw::sensor::{LensCorrection, OpcodeInfo, PointKind};

use crate::model::params::{CropOrientation, ModuleParams};
use crate::model::{Edit, Fields, ModuleKind, Params, Value};
use crate::preview::Orient;
use lens::{MakerModel, Manual, Radial};

/// Lens correction params (fields of [`ModuleKind::LENS`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct LensParams {
    /// Use the file's own data (DNG opcodes, Sony and Fujifilm maker tables).
    pub embedded: bool,
    /// Which parts of the file's data apply.
    pub distortion_on: bool,
    pub vignetting_on: bool,
    pub ca_on: bool,
    /// Manual distortion, −100..100 (positive straightens barrel distortion).
    pub distortion: f32,
    /// Manual vignetting: stops of brightening at the corner, −3..3.
    pub vignette: f32,
    /// Manual red and blue scales, percent of the radius, −0.5..0.5.
    pub tca_red: f32,
    pub tca_blue: f32,
    /// Zoom so no empty edges show.
    pub auto_scale: bool,
    /// The Lensfun profile for a raw without corrections of its own ([`lensfun`]): empty
    /// for the automatic pick, [`lensfun::NONE`], or a lens's key. Written only when set.
    pub lensfun: String,
}

impl Default for LensParams {
    fn default() -> Self {
        Self {
            embedded: true,
            distortion_on: true,
            vignetting_on: true,
            ca_on: true,
            distortion: 0.0,
            vignette: 0.0,
            tca_red: 0.0,
            tca_blue: 0.0,
            auto_scale: true,
            lensfun: String::new(),
        }
    }
}

impl ModuleParams for LensParams {
    const KIND: ModuleKind = ModuleKind::LENS;
    const VERSION: u16 = 1;

    fn to_fields(&self) -> Fields {
        let f = Fields::new()
            .with("embedded", Value::Bool(self.embedded))
            .with("distortion_on", Value::Bool(self.distortion_on))
            .with("vignetting_on", Value::Bool(self.vignetting_on))
            .with("ca_on", Value::Bool(self.ca_on))
            .with("distortion", Value::Float(self.distortion))
            .with("vignette", Value::Float(self.vignette))
            .with("tca_red", Value::Float(self.tca_red))
            .with("tca_blue", Value::Float(self.tca_blue))
            .with("auto_scale", Value::Bool(self.auto_scale));
        // Older edits stay as they were: the field only once a profile was picked.
        if self.lensfun.is_empty() {
            f
        } else {
            f.with("lensfun", Value::Text(self.lensfun.clone()))
        }
    }

    fn from_fields(x: &Fields) -> Self {
        let d = Self::default();
        let c = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { 0.0 };
        Self {
            embedded: x.bool_or("embedded", d.embedded),
            distortion_on: x.bool_or("distortion_on", d.distortion_on),
            vignetting_on: x.bool_or("vignetting_on", d.vignetting_on),
            ca_on: x.bool_or("ca_on", d.ca_on),
            distortion: c(x.f32_or("distortion", 0.0), -100.0, 100.0),
            vignette: c(x.f32_or("vignette", 0.0), -3.0, 3.0),
            tca_red: c(x.f32_or("tca_red", 0.0), -0.5, 0.5),
            tca_blue: c(x.f32_or("tca_blue", 0.0), -0.5, 0.5),
            auto_scale: x.bool_or("auto_scale", d.auto_scale),
            lensfun: x.text("lensfun").unwrap_or("").to_string(),
        }
    }
}

/// The edit's lens params: the defaults (the file's own corrections on) when the edit has no
/// lens module, `None` when it is switched off.
pub fn lens_params(edit: &Edit) -> Option<LensParams> {
    match edit.modules.iter().find(|m| m.kind == ModuleKind::LENS) {
        None => Some(LensParams::default()),
        Some(m) if !m.enabled => None,
        Some(m) => Some(LensParams::from_fields(&m.params.to_fields())),
    }
}

/// The edit's crop and orientation params (defaults when missing or off).
pub fn crop_params(edit: &Edit) -> CropOrientation {
    edit.modules
        .iter()
        .find(|m| m.kind == ModuleKind::CROP_ORIENTATION && m.enabled)
        .map_or_else(CropOrientation::default, |m| match &m.params {
            Params::CropOrientation(p) => p.clone(),
            p => CropOrientation::from_fields(&p.to_fields()),
        })
}

/// One DNG `OpcodeList3` step the warp runs (from the first warp of the list on).
#[derive(Clone, Debug, PartialEq)]
pub enum DngStep {
    /// `WarpRectilinear` / `WarpFisheye`: per plane `kr0..kr3, kt0, kt1`; centre and `m` in
    /// sensor pixels.
    Warp {
        fisheye: bool,
        k: [[f64; 6]; 3],
        centre: (f64, f64),
        m: f64,
    },
    /// `FixVignetteRadial`: gain `1 + k0 r² + … + k4 r¹⁰`, `r² = d² · inv_m2`.
    Vignette {
        k: [f64; 5],
        centre: (f64, f64),
        inv_m2: f64,
    },
}

/// Centre (absolute) and `1/m²` of a DNG radial opcode (the DNG specification: `m` is the
/// distance from the centre to the farthest corner of the bounds).
fn radial(c: (f64, f64), b: specular_raw::Rect) -> ((f64, f64), f64) {
    let (bx, by, bw, bh) = (
        f64::from(b.x),
        f64::from(b.y),
        f64::from(b.w),
        f64::from(b.h),
    );
    let cx = bx + c.0 * bw;
    let cy = by + c.1 * bh;
    let m2 = [(bx, by), (bx + bw, by), (bx, by + bh), (bx + bw, by + bh)]
        .iter()
        .map(|&(x, y)| (x - cx) * (x - cx) + (y - cy) * (y - cy))
        .fold(0.0, f64::max);
    ((cx, cy), if m2 > 0.0 { 1.0 / m2 } else { 0.0 })
}

/// The steps of list 3 the warp takes over: every op from the first warp on that is a warp
/// or `FixVignetteRadial` (other point ops stay in the front end, in their order there).
pub fn dng_steps(list3: &[OpcodeInfo]) -> Vec<DngStep> {
    let Some(first) = list3
        .iter()
        .position(|o| matches!(o, OpcodeInfo::Warp { .. }))
    else {
        return Vec::new();
    };
    list3[first..]
        .iter()
        .filter_map(|o| match o {
            OpcodeInfo::Warp {
                fisheye,
                k,
                centre,
                bounds,
            } => {
                let (c, inv_m2) = radial(*centre, *bounds);
                let m = if inv_m2 > 0.0 {
                    1.0 / inv_m2.sqrt()
                } else {
                    1.0
                };
                Some(DngStep::Warp {
                    fisheye: *fisheye,
                    k: *k,
                    centre: c,
                    m,
                })
            }
            OpcodeInfo::Point {
                kind: PointKind::Vignette { k, centre, inv_m2 },
                ..
            } => Some(DngStep::Vignette {
                k: *k,
                centre: *centre,
                inv_m2: *inv_m2,
            }),
            _ => None,
        })
        .collect()
}

/// Whether the front end must leave list 3's op `i` to the warp (see [`dng_steps`]).
pub fn warp_takes(list3: &[OpcodeInfo], i: usize) -> bool {
    let first = list3
        .iter()
        .position(|o| matches!(o, OpcodeInfo::Warp { .. }));
    first.is_some_and(|f| {
        i >= f
            && matches!(
                list3[i],
                OpcodeInfo::Warp { .. }
                    | OpcodeInfo::Point {
                        kind: PointKind::Vignette { .. },
                        ..
                    }
            )
    })
}

impl DngStep {
    /// Back-maps `p` (sensor pixels) for `plane`; multiplies `gain` for a vignette.
    fn apply(&self, plane: usize, p: (f64, f64), gain: &mut f64) -> (f64, f64) {
        match self {
            DngStep::Warp {
                fisheye,
                k,
                centre,
                m,
            } => {
                let k = &k[plane];
                let dx = (p.0 - centre.0) / m;
                let dy = (p.1 - centre.1) / m;
                let r2 = dx * dx + dy * dy;
                let (fx, fy) = if *fisheye {
                    let r = r2.sqrt();
                    if r < 1e-12 {
                        (k[0] * dx, k[0] * dy)
                    } else {
                        let t = r.atan();
                        let t2 = t * t;
                        let rd = t * (k[0] + t2 * (k[1] + t2 * (k[2] + t2 * k[3])));
                        (rd / r * dx, rd / r * dy)
                    }
                } else {
                    let f = k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
                    let dxy = 2.0 * dx * dy;
                    (
                        f * dx + k[4] * dxy + k[5] * (r2 + 2.0 * dx * dx),
                        f * dy + k[5] * dxy + k[4] * (r2 + 2.0 * dy * dy),
                    )
                };
                (centre.0 + m * fx, centre.1 + m * fy)
            }
            DngStep::Vignette { k, centre, inv_m2 } => {
                let dx = p.0 - centre.0;
                let dy = p.1 - centre.1;
                let r2 = (dx * dx + dy * dy) * inv_m2;
                let mut acc = 0.0;
                let mut rp = r2;
                for kk in k {
                    acc += kk * rp;
                    rp *= r2;
                }
                *gain *= 1.0 + acc;
                p
            }
        }
    }
}

/// What the warp needs besides the edit.
#[derive(Clone, Debug)]
pub struct Context {
    /// The rectangle the source covers, in sensor pixels: x, y, width, height (a raw's crop;
    /// `0, 0, w, h` for other images).
    pub frame: [f64; 4],
    /// The orientation the image is shown in before the edit's own turns (the file's).
    pub base: Orient,
    /// The camera's lens tables (maker notes).
    pub lens: Option<Arc<LensCorrection>>,
    /// DNG `OpcodeList3` as the file has it.
    pub list3: Vec<OpcodeInfo>,
    /// Camera RGB → the source's RGB (rows), when the source came through a matrix: per-plane
    /// warps then act on camera planes.
    pub camera: Option<[[f32; 3]; 3]>,
    /// Red and blue radial scales (percent of the radius, as the lens card's fringe
    /// sliders) of the chromatic aberration module for sources that are not raw (raws correct it on the CFA).
    pub rgb_ca: Option<[f32; 2]>,
    /// The Lensfun lookup of a raw without lens data of its own ([`lensfun::shot_of_raw`]).
    pub lensfun: Option<Arc<lensfun::Shot>>,
    /// Non-square pixels (DNG `DefaultScale`): the horizontal and vertical stretch (each at
    /// least 1) that makes a frame pixel square, as the viewer shows it; `[1, 1]` otherwise.
    pub stretch: [f64; 2],
    /// The file's 35 mm equivalent focal length (Exif `FocalLengthIn35mmFilm`), for the
    /// perspective model ([`persp::focal_px`]).
    pub focal35: Option<f64>,
}

impl Context {
    /// A plain image of `w × h` pixels shown with `base`.
    pub fn image(w: u32, h: u32, base: Orient) -> Context {
        Context {
            frame: [0.0, 0.0, f64::from(w), f64::from(h)],
            base,
            lens: None,
            list3: Vec::new(),
            camera: None,
            rgb_ca: None,
            lensfun: None,
            stretch: [1.0, 1.0],
            focal35: None,
        }
    }

    /// A raw's sensor data shown with `base`.
    pub fn raw(s: &specular_raw::SensorData, base: Orient) -> Context {
        let c = s.crop;
        Context {
            frame: [
                f64::from(c.x),
                f64::from(c.y),
                f64::from(c.w),
                f64::from(c.h),
            ],
            base,
            lens: s.meta.lens.clone(),
            list3: s.meta.opcodes[2].clone(),
            camera: Some(crate::ops::raw::camera_matrix(&s.meta)),
            rgb_ca: None,
            lensfun: None,
            stretch: s
                .meta
                .default_scale
                .map_or([1.0, 1.0], |k| k.map(f64::from)),
            focal35: None,
        }
    }
}

/// The base orientation then the edit's quarter turns (clockwise) and flips, as one
/// [`Orient`] (oriented frame → corrected frame).
pub fn total_orient(base: Orient, p: &CropOrientation) -> Orient {
    let mut o = base;
    let turn = Orient {
        transpose: true,
        flip_x: true,
        flip_y: false,
    };
    for _ in 0..p.quarter_turns.rem_euclid(4) {
        o = compose(o, turn);
    }
    if p.flip_horizontal {
        o = compose(
            o,
            Orient {
                flip_x: true,
                ..Orient::default()
            },
        );
    }
    if p.flip_vertical {
        o = compose(
            o,
            Orient {
                flip_y: true,
                ..Orient::default()
            },
        );
    }
    o
}

/// `a` then `b` (b turns what a shows).
fn compose(a: Orient, b: Orient) -> Orient {
    // Track where the unit axes go: compose the 2×2 sign/permutation maps exactly.
    let ma = orient_matrix(a);
    let mb = orient_matrix(b);
    // source_of: displayed → stored. For a then b: displayed_b → displayed_a → stored.
    let m = [
        [
            ma[0][0] * mb[0][0] + ma[0][1] * mb[1][0],
            ma[0][0] * mb[0][1] + ma[0][1] * mb[1][1],
        ],
        [
            ma[1][0] * mb[0][0] + ma[1][1] * mb[1][0],
            ma[1][0] * mb[0][1] + ma[1][1] * mb[1][1],
        ],
    ];
    if m[0][0] == 0 {
        Orient {
            transpose: true,
            flip_x: m[1][0] < 0,
            flip_y: m[0][1] < 0,
        }
    } else {
        Orient {
            transpose: false,
            flip_x: m[0][0] < 0,
            flip_y: m[1][1] < 0,
        }
    }
}

/// The linear part of [`Orient`]'s displayed → stored map (centred coordinates).
fn orient_matrix(o: Orient) -> [[i32; 2]; 2] {
    let sx = if o.flip_x { -1 } else { 1 };
    let sy = if o.flip_y { -1 } else { 1 };
    if o.transpose {
        [[0, sy], [sx, 0]]
    } else {
        [[sx, 0], [0, sy]]
    }
}

/// The parsed `aspect` field: `None` for free, else width over height (`original`: the
/// oriented frame's).
pub fn aspect_ratio(aspect: &str, frame_w: f64, frame_h: f64) -> Option<f64> {
    match aspect {
        "free" => None,
        "original" => Some(frame_w / frame_h.max(1e-9)),
        s => {
            let (a, b) = s.split_once(':')?;
            let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
            (a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite()).then(|| a / b)
        }
    }
}

/// The fused warp of one edit on one source (see the module documentation).
#[derive(Clone, Debug)]
pub struct Plan {
    /// Source frame in sensor pixels (x, y, w, h).
    pub frame: [f64; 4],
    /// [`Context::stretch`]: sensor pixels → square pixels along the frame's axes.
    pub stretch: [f64; 2],
    /// Oriented frame size (square pixels: sensor pixels stretched by [`Plan::stretch`]).
    pub oriented: (f64, f64),
    pub orient: Orient,
    /// Straighten angle (radians, clockwise).
    pub angle: f64,
    /// Crop fractions of the straightened frame.
    pub crop: [f64; 4],
    pub radial: Radial,
    pub dng: Vec<DngStep>,
    pub camera: Option<[[f32; 3]; 3]>,
    /// Perspective ([`persp`]): the map straightened frame → oriented frame (square pixels,
    /// both centred), straighten included; `None` without one (the angle alone turns).
    pub persp: Option<[[f64; 3]; 3]>,
}

impl Plan {
    /// The warp of `edit` on a source described by `ctx`. `crop_on`: false shows the whole
    /// straightened frame (while the crop is being edited).
    pub fn new(edit: &Edit, ctx: &Context, crop_on: bool) -> Plan {
        let cp = crop_params(edit);
        let lp = lens_params(edit);
        let orient = total_orient(ctx.base, &cp);
        let [_, _, fw, fh] = ctx.frame;
        let stretch = if ctx.stretch.iter().all(|k| k.is_finite() && *k > 0.0) {
            ctx.stretch
        } else {
            [1.0, 1.0]
        };
        let (sw, sh) = (fw * stretch[0], fh * stretch[1]);
        let oriented = if orient.transpose { (sh, sw) } else { (sw, sh) };
        let mut crop = cp.crop.map(f64::from);
        if !crop_on || !valid_crop(&crop) {
            crop = [0.0, 0.0, 1.0, 1.0];
        }
        let embedded = lp.as_ref().is_some_and(|l| l.embedded);
        let own = ctx.lens.as_deref().filter(|l| !l.is_empty());
        let maker = match own {
            Some(l) => embedded.then(|| MakerModel::new(l, fw, fh)),
            // A raw without data of its own: the Lensfun profile chosen ([`lensfun`]).
            None if embedded && dng_steps(&ctx.list3).is_empty() => {
                let setting = lp.as_ref().map_or("", |l| l.lensfun.as_str());
                ctx.lensfun
                    .as_deref()
                    .and_then(|s| s.chosen(setting))
                    .map(|c| MakerModel::lensfun(c.profile.clone()))
            }
            None => None,
        };
        let mut manual = lp.as_ref().map_or_else(Manual::default, |l| Manual {
            distortion: l.distortion,
            vignette_ev: l.vignette,
            tca_red: l.tca_red,
            tca_blue: l.tca_blue,
        });
        // RGB chromatic aberration joins the fringe scales (one sampling).
        if let Some([r, b]) = ctx.rgb_ca {
            manual.tca_red += r;
            manual.tca_blue += b;
        }
        let mut radial = Radial {
            maker,
            use_distortion: lp.as_ref().is_some_and(|l| l.distortion_on),
            use_ca: lp.as_ref().is_some_and(|l| l.ca_on),
            use_vignetting: lp.as_ref().is_some_and(|l| l.vignetting_on),
            manual,
            zoom: 1.0,
        };
        if lp.as_ref().is_some_and(|l| l.auto_scale) && !radial.is_identity() {
            radial.zoom = radial.fill_zoom(fw, fh);
        }
        let dng = if embedded {
            dng_steps(&ctx.list3)
        } else {
            Vec::new()
        };
        let dng = if lp.as_ref().is_some_and(|l| !l.distortion_on) {
            dng.into_iter()
                .filter(|s| matches!(s, DngStep::Vignette { .. }))
                .collect()
        } else {
            dng
        };
        let dng = if lp.as_ref().is_some_and(|l| !l.vignetting_on) {
            dng.into_iter()
                .filter(|s| matches!(s, DngStep::Warp { .. }))
                .collect()
        } else {
            dng
        };
        let mut plan = Plan {
            frame: ctx.frame,
            stretch,
            oriented,
            orient,
            angle: f64::from(cp.angle_deg).to_radians(),
            crop,
            radial,
            dng,
            camera: ctx.camera,
            persp: None,
        };
        if let Some(pp) = persp::persp_params(edit).filter(|p| !p.is_neutral()) {
            plan.persp = plan.perspective(&pp, ctx.focal35);
        }
        plan
    }

    /// The straightened → oriented map of `pp` on this plan (see [`Plan::persp`]).
    fn perspective(&self, pp: &persp::PerspParams, file_mm35: Option<f64>) -> Option<persp::M3> {
        let (w, h) = self.oriented;
        let f = persp::focal_px(pp, file_mm35, w, h);
        let base = match pp.mode {
            persp::Mode::Off => [0.0; 3],
            persp::Mode::Guided => self.guided(pp, f).unwrap_or([0.0; 3]),
            _ => pp
                .solved
                .map_or([0.0; 3], |s| s.map(|v| f64::from(v).to_radians())),
        };
        let s = pp.slider_angles();
        let angles = [base[0] + s[0], base[1] + s[1], base[2] + s[2]];
        let fwd = persp::forward(pp, angles, f, w, h);
        let t = persp::mul(&persp::turn(self.angle), &fwd);
        persp::inverse(&t).filter(|g| g.iter().flatten().all(|v| v.is_finite()))
    }

    /// The angles that make `pp`'s guides plumb (nearer vertical) and level (nearer
    /// horizontal) after the straighten turn.
    fn guided(&self, pp: &persp::PerspParams, f: f64) -> Option<[f64; 3]> {
        let lines = self.guide_lines(&pp.guides);
        let (mut v, mut hz) = (Vec::new(), Vec::new());
        for l in lines {
            let (dx, dy) = (l.b.0 - l.a.0, l.b.1 - l.a.1);
            // On screen after the straighten turn.
            let (s, c) = self.angle.sin_cos();
            let (sx, sy) = (dx * c - dy * s, dx * s + dy * c);
            if sy.abs() >= sx.abs() {
                v.push(l);
            } else {
                hz.push(l);
            }
        }
        let (w, h) = self.oriented;
        let p = upright::Problem {
            w,
            h,
            f,
            straighten: self.angle,
            vertical: v,
            horizontal: hz,
        };
        let s = upright::solve(upright::Goal::Guided, &p).ok()?;
        Some([s.pitch, s.yaw, s.roll])
    }

    /// Guides (`[x0, y0, x1, y1]`, fractions of the source frame) as lines in the oriented
    /// frame (square pixels, centred), through the lens correction; ones that do not map are
    /// left out.
    pub fn guide_lines(&self, guides: &[[f32; 4]]) -> Vec<upright::Line> {
        let base = Plan {
            angle: 0.0,
            crop: [0.0, 0.0, 1.0, 1.0],
            persp: None,
            ..self.clone()
        };
        let [fx, fy, fw, fh] = self.frame;
        let (wo, ho) = self.oriented;
        let at = |u: f32, v: f32| {
            let s = (fx + f64::from(u) * fw, fy + f64::from(v) * fh);
            base.to_output(s, 1.0)
                .map(|o| (o.0 - wo / 2.0, o.1 - ho / 2.0))
        };
        guides
            .iter()
            .filter_map(|g| {
                let a = at(g[0], g[1])?;
                let b = at(g[2], g[3])?;
                ((b.0 - a.0).hypot(b.1 - a.1) > 1e-6).then(|| upright::Line::new(a, b, 1.0))
            })
            .collect()
    }

    /// The region of the straightened frame that shows the image, for the crop helpers.
    pub fn crop_frame(&self) -> crop::Frame {
        let (w, h) = self.oriented;
        match self.persp {
            Some(g) => crop::Frame::warped(w, h, g),
            None => crop::Frame::turned(w, h, self.angle),
        }
    }

    /// Nothing moves and no gain applies: the source is the output.
    pub fn is_identity(&self) -> bool {
        self.orient == Orient::default()
            && self.stretch == [1.0, 1.0]
            && self.persp.is_none()
            && self.angle == 0.0
            && self.crop == [0.0, 0.0, 1.0, 1.0]
            && self.radial.is_identity()
            && self.dng.is_empty()
    }

    /// Only quarter turns and flips: every output pixel is a source pixel.
    pub fn orientation_only(&self) -> bool {
        self.angle == 0.0
            && self.persp.is_none()
            && self.stretch == [1.0, 1.0]
            && self.crop == [0.0, 0.0, 1.0, 1.0]
            && self.radial.is_identity()
            && self.dng.is_empty()
    }

    /// Whether colour planes move apart (three samples per pixel).
    pub fn per_plane(&self) -> bool {
        self.radial.per_plane()
            || self.dng.iter().any(|s| match s {
                DngStep::Warp { k, .. } => k[0] != k[1] || k[2] != k[1],
                DngStep::Vignette { .. } => false,
            })
    }

    /// The crop in (square) pixels of the straightened frame: x, y, width, height.
    pub fn crop_rect(&self) -> [f64; 4] {
        let (w, h) = self.oriented;
        let c = self.crop;
        [c[0] * w, c[1] * h, (c[2] - c[0]) * w, (c[3] - c[1]) * h]
    }

    /// The output size at `scale` output pixels per sensor pixel (at least 1×1).
    pub fn output_size(&self, scale: f64) -> (u32, u32) {
        let [_, _, w, h] = self.crop_rect();
        (
            ((w * scale).round() as u32).max(1),
            ((h * scale).round() as u32).max(1),
        )
    }

    /// The scale (output pixels per sensor pixel) whose output fits a long side of `long`.
    pub fn scale_for(&self, long: u32) -> f64 {
        let [_, _, w, h] = self.crop_rect();
        f64::from(long.max(1)) / w.max(h).max(1e-9)
    }

    /// The source point (sensor pixels) and gain of `plane` for straightened-frame point `f`
    /// (square pixels of the oriented frame). `None` where it leaves no source.
    pub fn frame_to_source(&self, plane: usize, f: (f64, f64)) -> ((f64, f64), f64) {
        let (wo, ho) = self.oriented;
        let (cx, cy) = (wo / 2.0, ho / 2.0);
        let (dx, dy) = (f.0 - cx, f.1 - cy);
        let p = match &self.persp {
            Some(g) => match persp::apply(g, (dx, dy)) {
                Some(q) => (cx + q.0, cy + q.1),
                // Beyond the horizon: nowhere in the source.
                None => return ((-1e30, -1e30), 0.0),
            },
            None => {
                let (s, c) = self.angle.sin_cos();
                // Rotated back by the angle (clockwise on screen is positive in y-down
                // coordinates).
                (cx + dx * c + dy * s, cy - dx * s + dy * c)
            }
        };
        let rel = self.oriented_to_corrected(p);
        self.corrected_to_source(plane, rel)
    }

    /// Oriented-frame point → corrected-frame point (sensor pixels) relative to the frame
    /// centre.
    pub fn oriented_to_corrected(&self, p: (f64, f64)) -> (f64, f64) {
        let m = self.corrected_matrix();
        let (wo, ho) = self.oriented;
        let (x, y) = (p.0 - wo / 2.0, p.1 - ho / 2.0);
        (m[0][0] * x + m[0][1] * y, m[1][0] * x + m[1][1] * y)
    }

    /// The linear map oriented frame (square pixels) → corrected frame (sensor pixels),
    /// centred: the orientation, then the stretch undone.
    pub fn corrected_matrix(&self) -> [[f64; 2]; 2] {
        let m = orient_matrix(self.orient);
        let [kx, ky] = self.stretch;
        [
            [f64::from(m[0][0]) / kx, f64::from(m[0][1]) / kx],
            [f64::from(m[1][0]) / ky, f64::from(m[1][1]) / ky],
        ]
    }

    /// Corrected point relative to the frame centre → source point (sensor pixels) and gain.
    pub fn corrected_to_source(&self, plane: usize, rel: (f64, f64)) -> ((f64, f64), f64) {
        let [fx, fy, fw, fh] = self.frame;
        let r = (fw * fw + fh * fh).sqrt() / 2.0;
        let rho = (rel.0 * rel.0 + rel.1 * rel.1).sqrt() / r;
        let k = self.radial.factor(plane, rho);
        let mut p = (fx + fw / 2.0 + rel.0 * k, fy + fh / 2.0 + rel.1 * k);
        let mut gain = 1.0;
        for st in self.dng.iter().rev() {
            p = st.apply(plane, p, &mut gain);
        }
        let (sx, sy) = (p.0 - fx - fw / 2.0, p.1 - fy - fh / 2.0);
        gain *= self.radial.gain((sx * sx + sy * sy).sqrt() / r);
        (p, gain)
    }

    /// The source point (sensor pixels) and gain of `plane` for output point `o` (output
    /// pixels at `scale`, continuous: pixel centres at `i + 0.5`).
    pub fn to_source(&self, plane: usize, o: (f64, f64), scale: f64) -> ((f64, f64), f64) {
        let [x0, y0, _, _] = self.crop_rect();
        self.frame_to_source(plane, (x0 + o.0 / scale, y0 + o.1 / scale))
    }

    /// The output point (output pixels at `scale`) showing source point `s` (sensor pixels),
    /// green plane, by Newton's method on [`Plan::to_source`]; `None` if it does not settle.
    pub fn to_output(&self, s: (f64, f64), scale: f64) -> Option<(f64, f64)> {
        // Start from the map without lens: undo the frame offset, orientation and rotation.
        let mut o = self.to_output_rigid(s, scale);
        for _ in 0..30 {
            let f = |o: (f64, f64)| self.to_source(1, o, scale).0;
            let p = f(o);
            let e = (p.0 - s.0, p.1 - s.1);
            if e.0.abs() < 1e-6 && e.1.abs() < 1e-6 {
                return Some(o);
            }
            let h = 0.5;
            let px = f((o.0 + h, o.1));
            let py = f((o.0, o.1 + h));
            let j = [
                [(px.0 - p.0) / h, (py.0 - p.0) / h],
                [(px.1 - p.1) / h, (py.1 - p.1) / h],
            ];
            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
            if det.abs() < 1e-12 {
                return None;
            }
            o.0 -= (j[1][1] * e.0 - j[0][1] * e.1) / det;
            o.1 -= (-j[1][0] * e.0 + j[0][0] * e.1) / det;
        }
        let p = self.to_source(1, o, scale).0;
        ((p.0 - s.0).abs() < 1e-3 && (p.1 - s.1).abs() < 1e-3).then_some(o)
    }

    /// [`Plan::to_output`] ignoring the lens.
    fn to_output_rigid(&self, s: (f64, f64), scale: f64) -> (f64, f64) {
        let [fx, fy, fw, fh] = self.frame;
        let [kx, ky] = self.stretch;
        let rel = ((s.0 - fx - fw / 2.0) * kx, (s.1 - fy - fh / 2.0) * ky);
        // The orientation matrix is orthogonal: its inverse is its transpose.
        let m = orient_matrix(self.orient);
        let (wo, ho) = self.oriented;
        let p = (
            wo / 2.0 + f64::from(m[0][0]) * rel.0 + f64::from(m[1][0]) * rel.1,
            ho / 2.0 + f64::from(m[0][1]) * rel.0 + f64::from(m[1][1]) * rel.1,
        );
        let (sn, c) = self.angle.sin_cos();
        let (dx, dy) = (p.0 - wo / 2.0, p.1 - ho / 2.0);
        let fwd = self.persp.as_ref().and_then(persp::inverse);
        let f = match fwd.and_then(|m| persp::apply(&m, (dx, dy))) {
            Some(q) => (wo / 2.0 + q.0, ho / 2.0 + q.1),
            None => (wo / 2.0 + dx * c - dy * sn, ho / 2.0 + dx * sn + dy * c),
        };
        let [x0, y0, _, _] = self.crop_rect();
        ((f.0 - x0) * scale, (f.1 - y0) * scale)
    }
}

/// A crop that is a non-empty rectangle inside 0..1.
pub fn valid_crop(c: &[f64; 4]) -> bool {
    c.iter().all(|v| v.is_finite())
        && c[0] >= -1e-6
        && c[1] >= -1e-6
        && c[2] <= 1.0 + 1e-6
        && c[3] <= 1.0 + 1e-6
        && c[2] - c[0] > 1e-4
        && c[3] - c[1] > 1e-4
}
