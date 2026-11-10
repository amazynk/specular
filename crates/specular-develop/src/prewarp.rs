//! Denoise before the warp (checkpoint C2): denoise works on the
//! unwarped picture, so lens, perspective and straighten resampling never stretch or
//! correlate the noise it removes.
//!
//! - [`split`] cuts an edit in two: the part before the warp (denoise and what precedes
//!   it) and the rest (denoise left out). The preview and export render the first part on
//!   the unwarped source, warp its result and render the rest on that.
//! - Only the source area the warp reads is denoised ([`source_box`]): a crop still saves
//!   the work outside it. The engine renders a region exactly as the same pixels of the
//!   whole, so the result does not depend on the region.
//! - Masks on denoise are evaluated in frame geometry (they live in source coordinates);
//!   denoise's AI result is sampled through the identity grid.
//! - Without a warp, or with only quarter turns and flips, denoise stays in the graph (the
//!   same pixels either way, and no extra pass).

use crate::graph::Roi;
use crate::model::{Edit, ModuleKind, Registry, Stage};
use crate::ops::geometry::Plan;

/// Pixels around the warp's footprint that are denoised as well (the warp's 4×4
/// Catmull-Rom taps plus rounding).
pub const MARGIN: f64 = 4.0;

/// Below this fraction of the source, only the box is denoised; above, all of it (one
/// region is cheaper than a copy per row).
pub const BOX_FRACTION: f64 = 0.85;

/// Whether `edit` has denoise enabled (the module this moves).
pub fn has_denoise(edit: &Edit) -> bool {
    edit.modules
        .iter()
        .any(|m| m.kind == ModuleKind::DENOISE && m.enabled && m.version == 1)
}

/// `edit` split at the warp: (denoise and every module before it, the warp left out; the
/// edit with denoise left out). `None` when denoise is off.
pub fn split(edit: &Edit) -> Option<(Edit, Edit)> {
    if !has_denoise(edit) {
        return None;
    }
    let reg = Registry::builtin();
    let cut = reg.rank(ModuleKind::DENOISE);
    let mut before = edit.clone();
    for m in &mut before.modules {
        let warp = reg.get(m.kind).is_some_and(|s| s.stage == Stage::Warp);
        if reg.rank(m.kind) > cut || warp {
            m.enabled = false;
        }
    }
    let mut after = edit.clone();
    for m in &mut after.modules {
        if m.kind == ModuleKind::DENOISE {
            m.enabled = false;
        }
    }
    Some((before, after))
}

/// Whether denoise runs before `plan`'s warp: the warp resamples (not identity, not only
/// quarter turns and flips).
pub fn wanted(plan: &Plan) -> bool {
    !(plan.is_identity() || plan.orientation_only())
}

/// The region of a `w`×`h` source (covering `plan.frame`) the warp reads, grown by
/// [`MARGIN`]; the whole source when the region is most of it.
pub fn source_box(plan: &Plan, w: u32, h: u32) -> Roi {
    let full = Roi::full(w, h);
    let [fx, fy, fw, _] = plan.frame;
    let k = f64::from(w) / fw.max(1e-9);
    let [x0, y0, cw, ch] = plan.crop_rect();
    let planes: &[usize] = if plan.per_plane() { &[0, 1, 2] } else { &[1] };
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    // The crop's edge densely, its inside on a coarse grid (radial models are monotone,
    // but a strong moustache can bulge inwards).
    const N: usize = 64;
    let mut at = |u: f64, v: f64| {
        for &p in planes {
            let (s, _) = plan.frame_to_source(p, (x0 + u * cw, y0 + v * ch));
            let (bx, by) = ((s.0 - fx) * k, (s.1 - fy) * k);
            if !(bx.is_finite() && by.is_finite()) {
                lo = (f64::MIN, f64::MIN);
                hi = (f64::MAX, f64::MAX);
                return;
            }
            lo = (lo.0.min(bx), lo.1.min(by));
            hi = (hi.0.max(bx), hi.1.max(by));
        }
    };
    for i in 0..=N {
        let t = i as f64 / N as f64;
        at(t, 0.0);
        at(t, 1.0);
        at(0.0, t);
        at(1.0, t);
    }
    for j in 1..16 {
        for i in 1..16 {
            at(i as f64 / 16.0, j as f64 / 16.0);
        }
    }
    let x0 = (lo.0 - MARGIN).floor().clamp(0.0, f64::from(w)) as u32;
    let y0 = (lo.1 - MARGIN).floor().clamp(0.0, f64::from(h)) as u32;
    let x1 = (hi.0 + MARGIN).ceil().clamp(0.0, f64::from(w)) as u32;
    let y1 = (hi.1 + MARGIN).ceil().clamp(0.0, f64::from(h)) as u32;
    if x1 <= x0 || y1 <= y0 {
        return full;
    }
    let b = Roi::new(x0, y0, x1 - x0, y1 - y0);
    if b.pixels() as f64 >= BOX_FRACTION * full.pixels() as f64 {
        full
    } else {
        b
    }
}

/// Records copies of `src` (`roi.w`×`roi.h` packed pixels of `bpp` bytes) into `dst` (a
/// `w`-wide image) at `roi`.
pub fn copy_into(
    enc: &mut wgpu::CommandEncoder,
    src: &wgpu::Buffer,
    roi: Roi,
    dst: &wgpu::Buffer,
    w: u32,
    bpp: u64,
) {
    let row = u64::from(roi.w) * bpp;
    if roi.x == 0 && roi.w == w {
        let off = u64::from(roi.y) * row;
        enc.copy_buffer_to_buffer(src, 0, dst, off, row * u64::from(roi.h));
        return;
    }
    for j in 0..roi.h {
        let to = (u64::from(roi.y + j) * u64::from(w) + u64::from(roi.x)) * bpp;
        enc.copy_buffer_to_buffer(src, u64::from(j) * row, dst, to, row);
    }
}
