//! Profile-to-profile transforms through the PCS (XYZ D50), with rendering intents and
//! black point compensation.

use crate::curve::Curve;
use crate::icc::{ColorSpace, Error, Lut, Profile};
use crate::lut::{MAX_CH, Stage};
use crate::math::{D50, Vec3, lab_to_xyz, mul3, xyz_to_lab};
use crate::profiles::Model;

/// ICC rendering intent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Perceptual (`A2B0` / `B2A0`).
    Perceptual,
    /// Media-relative colorimetric (`A2B1` / `B2A1`).
    #[default]
    Relative,
    /// Saturation (`A2B2` / `B2A2`).
    Saturation,
    /// ICC-absolute colorimetric (`A2B1` / `B2A1` scaled by the media white points).
    Absolute,
}

impl Intent {
    pub(crate) fn tag(self) -> usize {
        match self {
            Intent::Perceptual => 0,
            Intent::Relative | Intent::Absolute => 1,
            Intent::Saturation => 2,
        }
    }
}

#[derive(Clone, Debug)]
enum ToPcs {
    Matrix {
        trc: [Curve; 3],
        m: [f32; 9],
    },
    Gray {
        trc: Curve,
        lab: bool,
    },
    /// `v2_lab`: an L*a*b* device on a v2 (`mft1`/`mft2`) table, whose device side uses the
    /// v2 encoding (L* 100 at 0xFF00) like its PCS side.
    Lut {
        lut: Lut,
        v2_lab: bool,
    },
    Model(Model),
}

/// v4 → v2 L*a*b* normalization factor.
const V2_LAB: f32 = 65280.0 / 65535.0;

fn v2_lab_device(p: &Profile, l: &Lut) -> bool {
    p.color_space == ColorSpace::Lab && l.pcs == crate::icc::PcsEncoding::LabV2
}

#[derive(Clone, Debug)]
enum FromPcs {
    Matrix { m: [f32; 9], inv: [Curve; 3] },
    Gray { inv: Curve, lab: bool },
    Lut { lut: Lut, v2_lab: bool },
}

impl ToPcs {
    fn new(p: &Profile, intent: Intent) -> Result<ToPcs, Error> {
        if let Some(m) = p.model {
            return Ok(ToPcs::Model(m));
        }
        if let Some(l) = p.a2b_for(intent.tag()) {
            return Ok(ToPcs::Lut {
                lut: l.clone(),
                v2_lab: v2_lab_device(p, l),
            });
        }
        if let (Some(m), Some(t)) = (&p.matrix, &p.trc) {
            return Ok(ToPcs::Matrix {
                trc: t.clone(),
                m: m.to_f32(),
            });
        }
        if let Some(t) = &p.gray_trc {
            return Ok(ToPcs::Gray {
                trc: t.clone(),
                lab: p.pcs_lab,
            });
        }
        Err(Error::Unsupported("no device → PCS transform"))
    }

    #[inline]
    fn eval(&self, input: &[f32]) -> Vec3 {
        match self {
            ToPcs::Matrix { trc, m } => mul3(
                m,
                [
                    trc[0].eval(input[0]),
                    trc[1].eval(input[1]),
                    trc[2].eval(input[2]),
                ],
            ),
            ToPcs::Gray { trc, lab } => {
                let v = trc.eval(input[0]);
                if *lab {
                    lab_to_xyz([v * 100.0, 0.0, 0.0])
                } else {
                    [D50[0] * v, D50[1] * v, D50[2] * v]
                }
            }
            ToPcs::Lut { lut: l, v2_lab } => {
                let mut buf = [0f32; MAX_CH];
                buf[..l.pipe.inputs].copy_from_slice(&input[..l.pipe.inputs]);
                if *v2_lab {
                    buf[..3].iter_mut().for_each(|v| *v *= V2_LAB);
                }
                l.pipe.eval(&mut buf);
                l.pcs.decode([buf[0], buf[1], buf[2]])
            }
            ToPcs::Model(m) => m.eval(input),
        }
    }
}

impl FromPcs {
    fn new(p: &Profile, intent: Intent) -> Result<FromPcs, Error> {
        if let Some(l) = p.b2a_for(intent.tag()) {
            let mut lut = l.clone();
            if p.pcs_lab {
                // Lab-indexed tables interpolate trilinearly, as in Little CMS.
                for s in &mut lut.pipe.stages {
                    if let Stage::Clut(t) = s {
                        t.set_trilinear(true);
                    }
                }
            }
            return Ok(FromPcs::Lut {
                lut,
                v2_lab: v2_lab_device(p, l),
            });
        }
        if let (Some(m), Some(t)) = (&p.matrix, &p.trc) {
            let inv = m
                .inverse()
                .ok_or(Error::Malformed("singular colorant matrix"))?;
            return Ok(FromPcs::Matrix {
                m: inv.to_f32(),
                inv: [t[0].inverse(), t[1].inverse(), t[2].inverse()],
            });
        }
        if let Some(t) = &p.gray_trc {
            return Ok(FromPcs::Gray {
                inv: t.inverse(),
                lab: p.pcs_lab,
            });
        }
        Err(Error::Unsupported("no PCS → device transform"))
    }

    #[inline]
    fn eval(&self, xyz: Vec3, out: &mut [f32]) {
        match self {
            FromPcs::Matrix { m, inv } => {
                let l = mul3(m, xyz);
                out[0] = inv[0].eval(l[0]);
                out[1] = inv[1].eval(l[1]);
                out[2] = inv[2].eval(l[2]);
            }
            FromPcs::Gray { inv, lab } => {
                let y = if *lab {
                    xyz_to_lab(xyz)[0] / 100.0
                } else {
                    xyz[1] / D50[1]
                };
                out[0] = inv.eval(y);
            }
            FromPcs::Lut { lut: l, v2_lab } => {
                let mut buf = [0f32; MAX_CH];
                let e = l.pcs.encode(xyz);
                buf[..3].copy_from_slice(&e);
                l.pipe.eval(&mut buf);
                if *v2_lab {
                    buf[..3].iter_mut().for_each(|v| *v /= V2_LAB);
                }
                out[..l.pipe.outputs].copy_from_slice(&buf[..l.pipe.outputs]);
            }
        }
    }
}

/// A device-to-device transform between two profiles.
#[derive(Clone, Debug)]
pub struct Transform {
    src: ToPcs,
    dst: FromPcs,
    inputs: usize,
    outputs: usize,
    /// PCS adjustment (black point compensation, absolute intent): `xyz · scale + offset`.
    scale: Vec3,
    offset: Vec3,
    identity_pcs: bool,
}

fn channels(cs: ColorSpace) -> Result<usize, Error> {
    match cs {
        ColorSpace::Other(_) => Err(Error::Unsupported("colour space")),
        c => Ok(c.channels()),
    }
}

fn subtractive(cs: ColorSpace) -> bool {
    matches!(cs, ColorSpace::Cmyk | ColorSpace::Cmy)
}

/// The fixed perceptual-intent black of ICC v4 (XYZ).
const PERCEPTUAL_BLACK: Vec3 = [0.003_36, 0.003_473_1, 0.002_87];

/// Neutralises a black point (a* = b* = 0); zero when implausible (L* > 50) or invalid. For
/// the perceptual round trip of CMYK output profiles, which Little CMS still neutralises.
fn neutral_black(xyz: Vec3) -> Vec3 {
    let l = xyz_to_lab(xyz)[0];
    if !l.is_finite() || l <= 0.0 || l > 50.0 {
        return [0.0; 3];
    }
    lab_to_xyz([l, 0.0, 0.0])
}

/// The darkest colorant as a black point, as Little CMS 2.19 takes it: L* clipped (to 0 above
/// 95, which only a synthetic negative profile reaches, or below 0; to 50 above 50), a* and
/// b* kept. Earlier versions neutralised it (a* = b* = 0); a tinted black is now compensated
/// with its tint. Zero when invalid.
fn colorant_black(xyz: Vec3) -> Vec3 {
    let [l, a, b] = xyz_to_lab(xyz);
    if !(l.is_finite() && a.is_finite() && b.is_finite()) {
        return [0.0; 3];
    }
    let l = if !(0.0..=95.0).contains(&l) {
        0.0
    } else {
        l.min(50.0)
    };
    lab_to_xyz([l, a, b])
}

/// Matrix/shaper in Little CMS's sense: RGB colorants and curves, or a gray curve.
fn shaper(p: &Profile) -> bool {
    p.is_matrix_shaper() || (p.color_space == ColorSpace::Gray && p.gray_trc.is_some())
}

/// Whether the profile has its own device → PCS table for an intent (not the `A2B0`
/// fallback); Little CMS detects no black point for an intent a profile lacks.
fn supports(p: &Profile, intent: Intent) -> bool {
    p.model.is_some() || p.a2b[intent.tag().min(2)].is_some() || shaper(p)
}

/// The colour of the device's darkest colorant combination through the intent's table: with
/// its tint ([`colorant_black`]), or `neutral` ([`neutral_black`]).
fn darker_colorant(p: &Profile, intent: Intent, neutral: bool) -> Vec3 {
    let n = p.color_space.channels();
    if !supports(p, intent) {
        return [0.0; 3];
    }
    let Ok(to) = ToPcs::new(p, intent) else {
        return [0.0; 3];
    };
    let mut black = if subtractive(p.color_space) {
        [1f32; MAX_CH]
    } else {
        [0f32; MAX_CH]
    };
    if p.color_space == ColorSpace::Lab {
        // Lab's darkest colour is L* = 0 with a* = b* = 0 (Little CMS's 0, 0x8080, 0x8080),
        // not the device zero (a* = b* = −128): since the black keeps its tint, that one
        // compensated every perceptual Lab → Lab transform by a saturated blue (ΔE00 up to 95
        // on ITULab.icc and LCMSLABI.ICM).
        black[1] = 0x8080 as f32 / 65535.0;
        black[2] = 0x8080 as f32 / 65535.0;
    }
    let xyz = to.eval(&black[..n]);
    if neutral {
        neutral_black(xyz)
    } else {
        colorant_black(xyz)
    }
}

/// Whether Little CMS treats `p` as a destination with Adobe's black point algorithm: a
/// gray, RGB or ink profile with its own PCS → device table for the intent.
fn clut_destination(p: &Profile, intent: Intent) -> bool {
    matches!(
        p.color_space,
        ColorSpace::Gray | ColorSpace::Rgb | ColorSpace::Cmyk | ColorSpace::Cmy
    ) && p.b2a[intent.tag().min(2)].is_some()
}

/// Black point of a profile for an intent, as Little CMS (2.19) detects it, as the source or
/// the destination (`dest`): v4 perceptual and saturation tables use the fixed perceptual
/// black; CMYK output profiles round-trip L* = 0 through the perceptual table (which discounts
/// ink limiting); everything else uses the darkest colorant, with its tint, except that a
/// table-based destination is neutral for perceptual and saturation (Adobe's algorithm, which
/// Little CMS uses there, starts from a neutral black and keeps its tint). Abstract profiles
/// report none; gray TRC profiles count as matrix/shaper.
fn black_point(p: &Profile, intent: Intent, dest: bool) -> Vec3 {
    // Abstract profiles (the L*a*b* identity among them) have no black of their own.
    if &p.class == b"abst" {
        return [0.0; 3];
    }
    if p.version >= 4 && matches!(intent, Intent::Perceptual | Intent::Saturation) {
        return if shaper(p) {
            darker_colorant(p, Intent::Relative, false)
        } else {
            PERCEPTUAL_BLACK
        };
    }
    if intent == Intent::Relative
        && &p.class == b"prtr"
        && p.color_space == ColorSpace::Cmyk
        && let (Ok(from), Ok(to)) = (
            FromPcs::new(p, Intent::Perceptual),
            ToPcs::new(p, Intent::Relative),
        )
    {
        let mut dev = [0f32; MAX_CH];
        from.eval([0.0; 3], &mut dev);
        return neutral_black(to.eval(&dev[..4]));
    }
    let neutral = dest
        && matches!(intent, Intent::Perceptual | Intent::Saturation)
        && clut_destination(p, intent);
    darker_colorant(p, intent, neutral)
}

/// The media white for the absolute intent. v2 display profiles count as D50 (their `wtpt`
/// is the display's white while the colorants are already adapted), as in Little CMS.
fn media_white(p: &Profile) -> Vec3 {
    if p.version < 4 && &p.class == b"mntr" {
        D50
    } else {
        p.white
    }
}

impl Transform {
    /// Builds a transform from `src` device values to `dst` device values. `bpc` asks for
    /// black point compensation (always on for perceptual/saturation into v4 profiles).
    pub fn new(
        src: &Profile,
        dst: &Profile,
        intent: Intent,
        bpc: bool,
    ) -> Result<Transform, Error> {
        let inputs = channels(src.color_space)?;
        let outputs = channels(dst.color_space)?;
        let to = ToPcs::new(src, intent)?;
        let from = FromPcs::new(dst, intent)?;
        let mut scale = [1f32; 3];
        let mut offset = [0f32; 3];
        if intent == Intent::Absolute {
            let (ws, wd) = (media_white(src), media_white(dst));
            for (i, s) in scale.iter_mut().enumerate() {
                *s = ws[i] / wd[i].max(1e-6);
            }
        } else if bpc
            || (matches!(intent, Intent::Perceptual | Intent::Saturation) && dst.version >= 4)
        {
            // Like Little CMS, v4 destinations force BPC for perceptual and saturation.
            let bs = black_point(src, intent, false);
            let bd = black_point(dst, intent, true);
            if bs.iter().zip(&bd).any(|(a, b)| (a - b).abs() > 1e-5) {
                for i in 0..3 {
                    let s = (D50[i] - bd[i]) / (D50[i] - bs[i]);
                    scale[i] = s;
                    offset[i] = bd[i] - bs[i] * s;
                }
            }
        }
        // Little CMS drops a PCS stage this close to identity (`IsEmptyLayer`: the summed
        // deviation of the matrix and of the offset, in its XYZ encoding, under 0.002); a
        // near-neutral tinted black then compensates nothing, and neither does this.
        const MAX_ENCODEABLE_XYZ: f32 = 1.0 + 32767.0 / 32768.0;
        let deviation: f32 = scale.iter().map(|s| (s - 1.0).abs()).sum::<f32>()
            + offset.iter().map(|o| o.abs()).sum::<f32>() / MAX_ENCODEABLE_XYZ;
        if intent != Intent::Absolute && deviation < 0.002 {
            scale = [1.0; 3];
            offset = [0.0; 3];
        }
        let identity_pcs = scale == [1.0; 3] && offset == [0.0; 3];
        Ok(Transform {
            src: to,
            dst: from,
            inputs,
            outputs,
            scale,
            offset,
            identity_pcs,
        })
    }

    /// Input channel count.
    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// Output channel count.
    pub fn outputs(&self) -> usize {
        self.outputs
    }

    /// The PCS adjustment applied between source and destination (scale, offset).
    pub fn pcs_adjust(&self) -> (Vec3, Vec3) {
        (self.scale, self.offset)
    }

    /// Source device values → PCS XYZ (D50), after the intent's PCS adjustment.
    #[inline]
    pub fn to_pcs(&self, input: &[f32]) -> Vec3 {
        let x = self.src.eval(input);
        if self.identity_pcs {
            return x;
        }
        [
            x[0] * self.scale[0] + self.offset[0],
            x[1] * self.scale[1] + self.offset[1],
            x[2] * self.scale[2] + self.offset[2],
        ]
    }

    /// Converts one colour: `input[..inputs]` → `out[..outputs]`, all normalized.
    #[inline]
    pub fn eval(&self, input: &[f32], out: &mut [f32]) {
        self.dst.eval(self.to_pcs(input), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;

    #[test]
    fn srgb_to_linear_matches_formula() {
        let t = Transform::new(
            &profiles::srgb(),
            &profiles::linear_bt709(),
            Intent::Relative,
            true,
        )
        .unwrap();
        let mut o = [0f32; 3];
        for i in 0..=20 {
            let v = i as f32 / 20.0;
            t.eval(&[v, v * 0.5, 1.0 - v], &mut o);
            let want = [v, v * 0.5, 1.0 - v].map(crate::math::srgb_to_linear);
            for c in 0..3 {
                assert!((o[c] - want[c]).abs() < 1e-4, "{v}: {o:?} vs {want:?}");
            }
        }
    }

    #[test]
    fn p3_round_trip() {
        let a = Transform::new(
            &profiles::display_p3(),
            &profiles::srgb(),
            Intent::Relative,
            false,
        )
        .unwrap();
        let b = Transform::new(
            &profiles::srgb(),
            &profiles::display_p3(),
            Intent::Relative,
            false,
        )
        .unwrap();
        let (mut m, mut r) = ([0f32; 3], [0f32; 3]);
        for v in [[0.2, 0.4, 0.6], [0.5, 0.5, 0.5], [0.9, 0.7, 0.8]] {
            b.eval(&v, &mut m);
            a.eval(&m, &mut r);
            for c in 0..3 {
                assert!((r[c] - v[c]).abs() < 1e-4, "{v:?} → {m:?} → {r:?}");
            }
        }
    }

    #[test]
    fn lab_profile_is_identity_to_pcs() {
        let t = Transform::new(
            &profiles::lab(),
            &profiles::linear_bt709(),
            Intent::Relative,
            true,
        )
        .unwrap();
        let mut o = [0f32; 3];
        // L = 100, a = b = 0 → D50 white → linear white.
        t.eval(&[1.0, 128.0 / 255.0, 128.0 / 255.0], &mut o);
        for v in o {
            assert!((v - 1.0).abs() < 2e-3, "{o:?}");
        }
    }

    #[test]
    fn generic_cmyk_is_plausible() {
        let t = Transform::new(
            &profiles::generic_cmyk(),
            &profiles::srgb(),
            Intent::Relative,
            true,
        )
        .unwrap();
        let mut o = [0f32; 3];
        t.eval(&[0.0; 4], &mut o);
        assert!(o.iter().all(|v| *v > 0.99), "paper {o:?}");
        t.eval(&[1.0; 4], &mut o);
        assert!(o.iter().all(|v| *v < 0.01), "rich black with BPC {o:?}");
        t.eval(&[1.0, 0.0, 0.0, 0.0], &mut o);
        assert!(o[0] < 0.2 && o[2] > 0.8, "cyan {o:?}");
        t.eval(&[0.0, 0.0, 1.0, 0.0], &mut o);
        assert!(o[0] > 0.9 && o[2] < 0.2, "yellow {o:?}");
    }
}
