//! A bracket merge's ghost mask over the picture in Develop.
//!
//! - **Kept:** when a merge took moving parts from one photo, its ghost mask (on the merge's
//!   block grid) is saved in the cache beside the merge's other files, keyed by the merged
//!   DNG (path, size and modification time, as AI masks key a photo). Nothing goes in the
//!   sidecar; a changed or moved DNG simply has no mask.
//! - **Shown:** a merged DNG with a kept mask gets "Show ghosts" in the Clipping card and
//!   the picture's menu. On, the left-out areas are tinted through the crop, straightening,
//!   turns and lens corrections: each overlay pixel is mapped back to the photo through the
//!   picture's warp ([`Geometry::frame_point`]), drawn on the CPU at up to [`LONG`] px.
//! - The merge's toast "Show ghosts" opens the merge in Develop with it on.

use std::path::{Path, PathBuf};

use specular_develop::mask::Geometry;
use specular_develop::merge::GhostMask;
use specular_develop::ops::geometry::Plan;
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};

/// The longest side of the overlay (the mask is on blocks of several pixels anyway).
pub(super) const LONG: u32 = 640;
/// File format tag and version.
const MAGIC: &[u8; 8] = b"SPGHOST1";
/// The tint (sRGB) and its strength at a fully left-out block.
const TINT: [f32; 3] = [1.0, 0.18, 0.12];
const ALPHA: f32 = 0.55;

/// A kept ghost mask and its overlay.
pub(in crate::app) struct Ghosts {
    pub mask: GhostMask,
    /// The merged photo's size (sensor pixels; the mask's grid covers it).
    pub frame: (u32, u32),
    pub on: bool,
    /// The overlay: what it was drawn for, texture, id.
    tex: Option<(u64, wgpu::Texture, TexId)>,
}

impl std::fmt::Debug for Ghosts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ghosts")
            .field("frame", &self.frame)
            .field("on", &self.on)
            .finish_non_exhaustive()
    }
}

impl Ghosts {
    pub fn new(mask: GhostMask, frame: (u32, u32)) -> Ghosts {
        Ghosts {
            mask,
            frame,
            on: false,
            tex: None,
        }
    }

    /// The overlay texture for the picture drawn through `plan` at `size` physical px
    /// (made again when either changed); `retired` takes replaced textures.
    pub fn texture(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        plan: &Plan,
        size: (u32, u32),
        retired: &mut Vec<TexId>,
    ) -> Option<TexId> {
        let (w, h) = fit(size);
        let key = key(plan, (w, h));
        if let Some((k, _, id)) = &self.tex
            && *k == key
        {
            return Some(*id);
        }
        let rgba = overlay(&self.mask, self.frame, plan, w, h);
        let (tex, id) = super::preview::upload(gpu, ui, w, h, &rgba);
        if let Some(old) = self.tex.replace((key, tex, id)) {
            retired.push(old.2);
        }
        Some(id)
    }

    /// Gives up the texture (to be unregistered).
    pub fn take_texture(&mut self) -> Option<TexId> {
        self.tex.take().map(|t| t.2)
    }
}

/// `size` scaled down to at most [`LONG`] on the long side (at least 1×1).
fn fit(size: (u32, u32)) -> (u32, u32) {
    let (w, h) = (size.0.max(1), size.1.max(1));
    let l = w.max(h);
    if l <= LONG {
        return (w, h);
    }
    let s = f64::from(LONG) / f64::from(l);
    (
        ((f64::from(w) * s).round() as u32).max(1),
        ((f64::from(h) * s).round() as u32).max(1),
    )
}

/// What an overlay depends on: the warp's geometry and the size.
fn key(plan: &Plan, size: (u32, u32)) -> u64 {
    let r = &plan.radial;
    let text = format!(
        "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}{}{}{}{size:?}",
        plan.persp,
        plan.frame,
        plan.oriented,
        plan.orient,
        plan.angle,
        plan.crop,
        plan.dng,
        r.zoom,
        r.manual,
        r.maker.is_some(),
        r.use_distortion,
        r.use_ca,
        r.use_vignetting,
    );
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// The mask's value (0..=255) at frame fractions `f` (outside the frame: 0).
fn mask_at(m: &GhostMask, frame: (u32, u32), f: (f64, f64)) -> u8 {
    if !(0.0..1.0).contains(&f.0) || !(0.0..1.0).contains(&f.1) {
        return 0;
    }
    let b = f64::from(m.block.max(1));
    let gx = (f.0 * f64::from(frame.0) / b) as usize;
    let gy = (f.1 * f64::from(frame.1) / b) as usize;
    if gx >= m.width as usize || gy >= m.height as usize {
        return 0;
    }
    m.mask.get(gy * m.width as usize + gx).copied().unwrap_or(0)
}

/// The overlay of `m` (on a `frame`-sized photo) over the picture `plan` draws, `w`×`h`
/// premultiplied sRGB RGBA: each pixel centre mapped back to the photo.
pub(super) fn overlay(m: &GhostMask, frame: (u32, u32), plan: &Plan, w: u32, h: u32) -> Vec<u8> {
    let g = Geometry::Warp(plan);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let v = (f64::from(y) + 0.5) / f64::from(h);
        for x in 0..w {
            let u = (f64::from(x) + 0.5) / f64::from(w);
            let a = f32::from(mask_at(m, frame, g.frame_point(u, v))) / 255.0 * ALPHA;
            if a <= 0.0 {
                continue;
            }
            let i = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (TINT[c] * a * 255.0).round() as u8;
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// Where ghost masks are kept: the merge cache folder, named by the merged DNG's identity.
pub(super) fn cache_file(dng: &Path) -> PathBuf {
    let id = super::ai_masks::photo_id(dng);
    let hex: String = id[..16].iter().map(|b| format!("{b:02x}")).collect();
    super::merge::cache_dir().join(format!("{hex}.ghosts"))
}

/// `m` (on a `frame`-sized merge) as stored.
pub(super) fn encode(m: &GhostMask, frame: (u32, u32)) -> Vec<u8> {
    let mut b = Vec::with_capacity(28 + m.mask.len());
    b.extend_from_slice(MAGIC);
    for v in [m.width, m.height, m.block, frame.0, frame.1] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&m.mask);
    b
}

/// A stored mask, `None` when it is not one (or does not fit its sizes).
pub(super) fn decode(b: &[u8]) -> Option<(GhostMask, (u32, u32))> {
    let rest = b.strip_prefix(MAGIC.as_slice())?;
    let word = |i: usize| -> Option<u32> {
        let s = rest.get(4 * i..4 * i + 4)?;
        Some(u32::from_le_bytes(s.try_into().ok()?))
    };
    let (w, h, block, fw, fh) = (word(0)?, word(1)?, word(2)?, word(3)?, word(4)?);
    let mask = rest.get(20..)?;
    let n = (w as usize).checked_mul(h as usize)?;
    if mask.len() != n || block == 0 || fw == 0 || fh == 0 {
        return None;
    }
    if w != fw.div_ceil(block) || h != fh.div_ceil(block) {
        return None;
    }
    Some((
        GhostMask {
            width: w,
            height: h,
            block,
            mask: mask.to_vec(),
        },
        (fw, fh),
    ))
}

/// Keeps `m` for the merge saved at `dng`.
pub(super) fn save(dng: &Path, m: &GhostMask, frame: (u32, u32)) -> std::io::Result<()> {
    let p = cache_file(dng);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, encode(m, frame))
}

/// The kept mask of the merge at `dng`, if any.
pub(super) fn load(dng: &Path) -> Option<Ghosts> {
    if !specular_develop::merge::dng::looks_merged(dng) {
        return None;
    }
    let b = std::fs::read(cache_file(dng)).ok()?;
    let (m, frame) = decode(&b)?;
    Some(Ghosts::new(m, frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::params::CropOrientation;
    use specular_develop::model::{Edit, ModuleKind, Params, Registry};
    use specular_develop::ops::geometry::Context;
    use specular_develop::preview::Orient;

    fn mask() -> (GhostMask, (u32, u32)) {
        // 600×400 photo, 20 px blocks: a left-out square at blocks (10..15, 5..10).
        let (w, h) = (30u32, 20u32);
        let mut v = vec![0u8; (w * h) as usize];
        for y in 5..10 {
            for x in 10..15 {
                v[(y * w + x) as usize] = 255;
            }
        }
        (
            GhostMask {
                width: w,
                height: h,
                block: 20,
                mask: v,
            },
            (600, 400),
        )
    }

    #[test]
    fn kept_masks_round_trip_and_refuse_bad_files() {
        let (m, frame) = mask();
        let b = encode(&m, frame);
        let (back, f) = decode(&b).expect("reads");
        assert_eq!(f, frame);
        assert_eq!(back.mask, m.mask);
        assert_eq!((back.width, back.height, back.block), (30, 20, 20));
        assert!(decode(&b[..b.len() - 1]).is_none());
        assert!(decode(b"SPGHOST2").is_none());
        let mut wrong = b.clone();
        wrong[8..12].copy_from_slice(&31u32.to_le_bytes());
        assert!(decode(&wrong).is_none());
    }

    #[test]
    fn kept_masks_follow_the_dng_and_only_merges() {
        let dir = std::env::temp_dir().join(format!("specular-ghosts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dng = dir.join("IMG_0002-HDR.dng");
        std::fs::write(&dng, b"merged").unwrap();
        let (m, frame) = mask();
        save(&dng, &m, frame).unwrap();
        let kept = cache_file(&dng);
        let g = load(&dng).expect("kept");
        assert_eq!(g.frame, frame);
        assert!(!g.on);
        // A different file at the same path (size changed) has no mask.
        std::fs::write(&dng, b"merged again").unwrap();
        assert!(load(&dng).is_none());
        // Not a merge's name: never looked up.
        let other = dir.join("IMG_0002.dng");
        std::fs::write(&other, b"x").unwrap();
        save(&other, &m, frame).unwrap();
        assert!(load(&other).is_none());
        let _ = std::fs::remove_file(kept);
        let _ = std::fs::remove_file(cache_file(&other));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn plan(crop: CropOrientation) -> Plan {
        let mut e = Edit::default_for(Registry::builtin(), Default::default(), true);
        if let Some(m) = e.module_mut(ModuleKind::CROP_ORIENTATION, 0) {
            m.params = Params::CropOrientation(crop);
        }
        Plan::new(&e, &Context::image(600, 400, Orient::default()), true)
    }

    /// The tint where the mask is, through no change, a crop and a straighten.
    #[test]
    fn the_overlay_follows_crop_and_rotation() {
        let (m, frame) = mask();
        let alpha = |px: &[u8], w: u32, x: f64, y: f64| {
            let (i, j) = (x as u32, y as u32);
            px[((j * w + i) * 4 + 3) as usize]
        };
        // Uncropped: the square covers photo pixels 200..300 × 100..200.
        let p = plan(CropOrientation::default());
        let px = overlay(&m, frame, &p, 600, 400);
        assert!(alpha(&px, 600, 250.0, 150.0) > 100);
        assert_eq!(alpha(&px, 600, 150.0, 150.0), 0);
        assert_eq!(alpha(&px, 600, 350.0, 250.0), 0);
        // Cropped to the right half (photo x 300..600): the square (x 200..300) is out.
        let p = plan(CropOrientation {
            crop: [0.5, 0.0, 1.0, 1.0],
            ..CropOrientation::default()
        });
        let px = overlay(&m, frame, &p, 300, 400);
        assert!(px.chunks_exact(4).all(|q| q[3] == 0));
        // Cropped to the left half: same place as uncropped.
        let p = plan(CropOrientation {
            crop: [0.0, 0.0, 0.5, 1.0],
            ..CropOrientation::default()
        });
        let px = overlay(&m, frame, &p, 300, 400);
        assert!(alpha(&px, 300, 250.0, 150.0) > 100);
        // Straightened: every tinted overlay pixel maps back into the square.
        let p = plan(CropOrientation {
            angle_deg: 10.0,
            ..CropOrientation::default()
        });
        let (w, h) = (600, 400);
        let px = overlay(&m, frame, &p, w, h);
        let g = Geometry::Warp(&p);
        let mut tinted = 0;
        for y in 0..h {
            for x in 0..w {
                if alpha(&px, w, f64::from(x), f64::from(y)) == 0 {
                    continue;
                }
                tinted += 1;
                let f = g.frame_point(
                    (f64::from(x) + 0.5) / f64::from(w),
                    (f64::from(y) + 0.5) / f64::from(h),
                );
                let (sx, sy) = (f.0 * 600.0, f.1 * 400.0);
                assert!((200.0..300.0).contains(&sx) && (100.0..200.0).contains(&sy));
            }
        }
        assert!(tinted > 5000, "{tinted}");
    }

    #[test]
    fn overlays_are_capped_and_keyed_by_size_and_geometry() {
        assert_eq!(fit((1280, 800)), (640, 400));
        assert_eq!(fit((300, 200)), (300, 200));
        let a = plan(CropOrientation::default());
        let b = plan(CropOrientation {
            angle_deg: 1.0,
            ..CropOrientation::default()
        });
        assert_eq!(key(&a, (10, 10)), key(&a, (10, 10)));
        assert_ne!(key(&a, (10, 10)), key(&a, (11, 10)));
        assert_ne!(key(&a, (10, 10)), key(&b, (10, 10)));
    }
}
