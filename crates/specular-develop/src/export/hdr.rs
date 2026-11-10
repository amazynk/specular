//! HDR export.
//!
//! One edit gives both renditions ([`crate::ops::view::for_headroom`]): the SDR one is the
//! edit as it is (peak 1), the HDR one the same edit with the view transform's white raised
//! to the HDR limit (the export's headroom, `2^hdr_limit_ev` units of SDR white, 1.0 =
//! 203 cd/m² as BT.2408 has it).
//!
//! - **PQ and HLG** (AVIF, JPEG XL, PNG): the HDR rendition in BT.2100 (Rec. 2020
//!   primaries), linear float from the GPU, then the ST 2084 inverse EOTF or the BT.2100 HLG
//!   inverse (display light → signal through the inverse OOTF for a 1000 cd/m² display),
//!   16-bit samples labelled by `cICP` / `nclx` / the codestream's colour encoding. HLG
//!   cannot go above 1000 cd/m², so its headroom is capped at 1000 / 203.
//! - **Gain maps** (JPEG, AVIF, JPEG XL; ISO 21496-1): the SDR rendition is the base image,
//!   encoded as usual in the chosen space; the map holds per-channel `log2((hdr + k) /
//!   (sdr + k))` (k = 1/64) against the 8-bit base as stored, quantised to 8 bits between
//!   the image's own smallest and largest gain. Readers apply it with the same formula
//!   (`specular_core::GainMapParams::apply`).

use super::preset::{Depth, FileFormat, Preset};
use super::render::{Exported, Exporter, Rendition, Samples};
use super::{Space, Transfer};
use crate::gpu::Error;
use crate::model::Edit;
use crate::ops::effects::frame::{Vars, add_alpha, add_to};
use crate::ops::view;
use crate::preview::{Input, Orient};
use specular_convert::gain_map::GainMapImage;
use specular_core::GainMapParams;

/// SDR reference white in cd/m² (BT.2408).
pub const REFERENCE_WHITE_NITS: f64 = 203.0;
/// The brightest HLG signal on its 1000 cd/m² reference display, in units of SDR white.
pub const HLG_PEAK: f32 = (1000.0 / REFERENCE_WHITE_NITS) as f32;
/// Offsets added to both renditions before the ratio (ISO 21496-1 `k`).
pub const GAIN_OFFSET: f32 = 1.0 / 64.0;

/// How an export carries HDR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HdrOutput {
    /// SDR only.
    #[default]
    Off,
    /// BT.2100 PQ (SMPTE ST 2084).
    Pq,
    /// BT.2100 hybrid log-gamma.
    Hlg,
    /// An SDR image with an ISO 21496-1 gain map to the HDR rendition.
    GainMap,
}

impl HdrOutput {
    pub const ALL: [HdrOutput; 4] = [
        HdrOutput::Off,
        HdrOutput::Pq,
        HdrOutput::Hlg,
        HdrOutput::GainMap,
    ];

    pub fn id(self) -> &'static str {
        match self {
            HdrOutput::Off => "off",
            HdrOutput::Pq => "pq",
            HdrOutput::Hlg => "hlg",
            HdrOutput::GainMap => "gain-map",
        }
    }

    pub fn from_id(s: &str) -> Option<HdrOutput> {
        let s = s.trim().to_ascii_lowercase();
        let s = if s == "gainmap" { "gain-map" } else { &s };
        HdrOutput::ALL.into_iter().find(|h| h.id() == s)
    }

    /// Whether `format` can hold it.
    pub fn fits(self, format: FileFormat) -> bool {
        use FileFormat as F;
        match self {
            HdrOutput::Off => true,
            HdrOutput::Pq | HdrOutput::Hlg => matches!(format, F::Avif | F::Jxl | F::Png),
            HdrOutput::GainMap => matches!(format, F::Jpeg | F::Avif | F::Jxl),
        }
    }

    /// The `cICP` code points (primaries, transfer, matrix, full range) of PQ and HLG.
    pub fn cicp(self) -> Option<[u8; 4]> {
        match self {
            HdrOutput::Pq => Some([9, 16, 0, 1]),
            HdrOutput::Hlg => Some([9, 18, 0, 1]),
            _ => None,
        }
    }
}

/// The CICP colour primaries code of a space (for the gain map's alternate rendition).
pub fn cicp_primaries(s: Space) -> u16 {
    match s {
        Space::Srgb => 1,
        Space::DisplayP3 => 12,
        Space::Rec2020 => 9,
        // Not a CICP space: readers keep the base's (the gains are in it).
        Space::AdobeRgb => 2,
    }
}

/// ST 2084 (PQ) inverse EOTF: linear light (1.0 = SDR white, 203 cd/m²) → signal 0..1.
pub fn pq_encode(l: f64) -> f64 {
    const M1: f64 = 2610.0 / 16384.0;
    const M2: f64 = 2523.0 / 4096.0 * 128.0;
    const C1: f64 = 3424.0 / 4096.0;
    const C2: f64 = 2413.0 / 4096.0 * 32.0;
    const C3: f64 = 2392.0 / 4096.0 * 32.0;
    let y = (l * REFERENCE_WHITE_NITS / 10_000.0).clamp(0.0, 1.0);
    let p = y.powf(M1);
    ((C1 + C2 * p) / (1.0 + C3 * p)).powf(M2)
}

/// BT.2100 HLG: display light (BT.2020, 1.0 = SDR white) → signal, through the inverse
/// OOTF of a 1000 cd/m² display (system gamma 1.2) and the HLG OETF. The inverse of
/// `specular_core::hdr::hlg_to_linear`.
pub fn hlg_encode(rgb: [f64; 3]) -> [f64; 3] {
    const A: f64 = 0.178_832_77;
    const B: f64 = 0.284_668_92;
    const C: f64 = 0.559_910_7;
    let fd = rgb.map(|v| (v * REFERENCE_WHITE_NITS / 1000.0).clamp(0.0, 1.0));
    let yd = 0.2627 * fd[0] + 0.6780 * fd[1] + 0.0593 * fd[2];
    if yd <= 0.0 {
        return [0.0; 3];
    }
    // Fd = Ys^0.2 · E with Ys = Yd^(1/1.2).
    let k = yd.powf(-0.2 / 1.2);
    fd.map(|f| {
        let e = (f * k).clamp(0.0, 1.0);
        if e <= 1.0 / 12.0 {
            (3.0 * e).sqrt()
        } else {
            A * (12.0 * e - B).ln() + C
        }
    })
}

/// Linear BT.2020 floats (3 per pixel) → 16-bit PQ or HLG samples.
pub fn encode_signal(v: &[f32], hdr: HdrOutput) -> Vec<u16> {
    use rayon::prelude::*;
    let q = |e: f64| (e * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16;
    let mut out = vec![0u16; v.len()];
    out.par_chunks_mut(3 * 1024)
        .zip(v.par_chunks(3 * 1024))
        .for_each(|(o, s)| {
            for (o, s) in o.chunks_exact_mut(3).zip(s.chunks_exact(3)) {
                let l = [s[0], s[1], s[2]].map(f64::from);
                let e = match hdr {
                    HdrOutput::Hlg => hlg_encode(l),
                    _ => l.map(pq_encode),
                };
                for c in 0..3 {
                    o[c] = q(e[c]);
                }
            }
        });
    out
}

/// The per-channel gain map from the stored 8-bit SDR base (`transfer`-coded) to the
/// linear HDR rendition (same primaries, same size), for a headroom of `headroom`.
pub fn gain_map(
    sdr: &[u8],
    hdr: &[f32],
    transfer: Transfer,
    headroom: f32,
    w: u32,
    h: u32,
) -> GainMapImage {
    use rayon::prelude::*;
    let k = GAIN_OFFSET;
    let lut: Vec<f32> = (0..256)
        .map(|i| transfer.decode(f64::from(i) / 255.0) as f32)
        .collect();
    let gains: Vec<f32> = sdr
        .par_iter()
        .zip(hdr.par_iter())
        .map(|(&s, &h)| ((h.max(0.0) + k) / (lut[usize::from(s)] + k)).log2())
        .collect();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for px in gains.chunks_exact(3) {
        for c in 0..3 {
            lo[c] = lo[c].min(px[c]);
            hi[c] = hi[c].max(px[c]);
        }
    }
    let cap = headroom.max(1.0).log2() + 6.0;
    for c in 0..3 {
        if !lo[c].is_finite() || !hi[c].is_finite() {
            (lo[c], hi[c]) = (0.0, 0.0);
        }
        lo[c] = lo[c].clamp(-cap, cap);
        hi[c] = hi[c].clamp(lo[c], cap);
        if hi[c] - lo[c] < 1e-3 {
            hi[c] = lo[c] + 1e-3;
        }
    }
    let samples = gains
        .par_chunks(3)
        .flat_map_iter(|px| {
            (0..3).map(move |c| {
                let t = (px[c] - lo[c]) / (hi[c] - lo[c]);
                (t.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            })
        })
        .collect();
    GainMapImage {
        width: w,
        height: h,
        channels: 3,
        samples,
        params: GainMapParams {
            gain_min: lo,
            gain_max: hi,
            gamma: [1.0; 3],
            base_offset: [k; 3],
            alt_offset: [k; 3],
            base_headroom: 0.0,
            alt_headroom: headroom.max(1.0).log2(),
            base_is_hdr: false,
            space: specular_core::Primaries::Bt709,
        },
    }
}

/// The export's headroom: the edit's HDR limit (HLG at most its 1000 cd/m² peak).
pub fn export_headroom(edit: &Edit, hdr: HdrOutput) -> f32 {
    let h = view::limit(edit).max(1.0);
    if hdr == HdrOutput::Hlg {
        h.min(HLG_PEAK)
    } else {
        h
    }
}

/// A float render of `preset`'s size in its space (the HDR rendition), whatever its format.
fn float_preset(preset: &Preset) -> Preset {
    Preset {
        format: FileFormat::Tiff,
        depth: Depth::Float,
        hdr: HdrOutput::Off,
        ..preset.clone()
    }
}

/// [`Exporter::render`] with `preset`'s HDR choice: PQ/HLG samples, or the SDR base with
/// its gain map. `progress` sees 0..1 over the whole job.
pub fn render(
    ex: &mut Exporter,
    input: Input,
    edit: &Edit,
    orient: Orient,
    preset: &Preset,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Exported, Error> {
    render_framed(ex, input, edit, orient, preset, None, progress)
}

/// [`render`] with the edit's borders and watermark when `vars` are given (a
/// watermark that cannot be drawn is left out), and the source's transparency kept
/// ([`Exported::alpha`]).
///
/// - SDR: added to the samples as they are.
/// - PQ and HLG: added to the linear HDR rendition before it is coded.
/// - Gain maps: added to both renditions (rendered together, [`Exporter::render_renditions`])
///   before the map is made, so the map covers the frame too: the border and the watermark
///   have the same light in both (gain 0 there) and the photo inside keeps its gains.
pub fn render_framed(
    ex: &mut Exporter,
    input: Input,
    edit: &Edit,
    orient: Orient,
    preset: &Preset,
    vars: Option<&Vars>,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Exported, Error> {
    let preset = preset.clone().normalised();
    let alpha_src = alpha_of(&input);
    let frame = |img: &mut Exported, float: bool| {
        if let Some(v) = vars {
            let _ = add_to(img, edit, v, preset.space, float);
        }
    };
    // The alpha pass runs after the main one: a share of the progress.
    let main = if alpha_src.is_some() { 0.8 } else { 1.0 };
    let mut part = |f: f32| progress(f * main);
    let mut img = match preset.hdr {
        HdrOutput::Off => {
            let mut img = ex.render(input, edit, orient, &preset, &mut part)?;
            frame(&mut img, preset.depth.float());
            img
        }
        HdrOutput::Pq | HdrOutput::Hlg => {
            let h = export_headroom(edit, preset.hdr);
            let e = view::for_headroom(edit, h);
            let mut img = ex.render_at(input, &e, orient, &float_preset(&preset), h, &mut part)?;
            frame(&mut img, true);
            let Samples::Float(v) = &img.samples else {
                return Err(Error::BadRequest("the HDR render is not float"));
            };
            img.samples = Samples::Sixteen(encode_signal(v, preset.hdr));
            img
        }
        HdrOutput::GainMap => {
            let h = export_headroom(edit, preset.hdr);
            let e = view::for_headroom(edit, h);
            let fp = float_preset(&preset);
            let list = [
                Rendition {
                    edit,
                    preset: &preset,
                    peak: 1.0,
                },
                Rendition {
                    edit: &e,
                    preset: &fp,
                    peak: h,
                },
            ];
            let mut v = ex.render_renditions(input, orient, &list, &mut part)?;
            let mut alt = v.pop().ok_or(Error::BadRequest("no HDR rendition"))?;
            let mut base = v.pop().ok_or(Error::BadRequest("no SDR rendition"))?;
            frame(&mut base, false);
            frame(&mut alt, true);
            let (Samples::Eight(sdr), Samples::Float(hdr)) = (&base.samples, &alt.samples) else {
                return Err(Error::BadRequest(
                    "the gain-map renders are not 8-bit and float",
                ));
            };
            if (alt.width, alt.height) != (base.width, base.height) {
                return Err(Error::BadRequest("the two renditions differ in size"));
            }
            base.gain_map = Some(gain_map(
                sdr,
                hdr,
                preset.space.transfer(),
                h,
                base.width,
                base.height,
            ));
            base
        }
    };
    if let Some((w, h, a)) = alpha_src {
        let mut rest = |f: f32| progress(main + f * (1.0 - main));
        let alpha = render_alpha(ex, w, h, a, edit, orient, &preset, &mut rest)?;
        let Samples::Float(v) = alpha.samples else {
            return Err(Error::BadRequest("the alpha render is not float"));
        };
        let plane: Vec<f32> = v.chunks_exact(3).map(|p| p[1].clamp(0.0, 1.0)).collect();
        // The border is opaque; the watermark adds its own coverage.
        let (aw, ah, plane) =
            match vars.and_then(|vars| add_alpha(&plane, alpha.width, alpha.height, edit, vars)) {
                Some(framed) => framed,
                None => (alpha.width, alpha.height, plane),
            };
        if (aw, ah) == (img.width, img.height) {
            img.alpha = Some(plane);
        }
    }
    Ok(img)
}

/// The source's coverage plane (`width`, `height`, values), when some pixel is not opaque.
fn alpha_of(input: &Input) -> Option<(u32, u32, Vec<f32>)> {
    let Input::Rgb {
        width,
        height,
        rgba,
    } = input
    else {
        return None;
    };
    rgba.chunks_exact(4).any(|p| p[3] < 1.0).then(|| {
        (
            *width,
            *height,
            rgba.chunks_exact(4).map(|p| p[3]).collect(),
        )
    })
}

/// The source's coverage `a` (`w × h`) through the edit's geometry, resize and turns, as
/// the photo goes: a grey image whose value is the coverage, rendered with only the crop
/// and the lens geometry (no vignetting), in linear float.
#[allow(clippy::too_many_arguments)]
fn render_alpha(
    ex: &mut Exporter,
    w: u32,
    h: u32,
    a: Vec<f32>,
    edit: &Edit,
    orient: Orient,
    preset: &Preset,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Exported, Error> {
    let mut e = edit.clone();
    e.masks.clear();
    e.modules.retain(|m| {
        matches!(
            m.kind,
            crate::model::ModuleKind::CROP_ORIENTATION
                | crate::model::ModuleKind::LENS
                | crate::model::ModuleKind::PERSPECTIVE
        )
    });
    if let Some(m) = e.module_mut(crate::model::ModuleKind::LENS, 0) {
        let f = m
            .params
            .to_fields()
            .with("vignette", crate::model::Value::Float(0.0));
        m.params.set_fields(f);
    }
    let rgba = a.iter().flat_map(|&v| [v, v, v, 1.0]).collect();
    let input = Input::Rgb {
        width: w,
        height: h,
        rgba,
    };
    let p = Preset {
        format: FileFormat::Tiff,
        depth: Depth::Float,
        hdr: HdrOutput::Off,
        space: Space::Rec2020,
        ..preset.clone()
    };
    ex.render(input, &e, orient, &p, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Our PQ and HLG encoders invert the decoders' curves (`specular_core::hdr`).
    #[test]
    fn signals_invert_the_decoders() {
        for i in 0..=400 {
            let l = f64::from(i) / 400.0 * 40.0;
            let e = pq_encode(l) as f32;
            let back = f64::from(specular_core::hdr::pq_to_linear(e));
            assert!((back - l).abs() <= 1e-3 * l.max(0.01), "{l} → {e} → {back}");
        }
        for rgb in [
            [1.0, 1.0, 1.0],
            [0.18, 0.18, 0.18],
            [3.0, 1.0, 0.2],
            [0.01, 0.4, 2.5],
            [4.9, 4.9, 4.9],
        ] {
            let e = hlg_encode(rgb).map(|v| v as f32);
            let back = specular_core::hdr::hlg_to_linear(e);
            for c in 0..3 {
                let d = (f64::from(back[c]) - rgb[c]).abs();
                assert!(d <= 2e-4 * rgb[c].max(0.05), "{rgb:?} → {e:?} → {back:?}");
            }
        }
        // BT.2408: SDR white is PQ 0.58 and HLG 0.75.
        assert!((pq_encode(1.0) - 0.5807).abs() < 1e-3);
        assert!((hlg_encode([1.0; 3])[0] - 0.75).abs() < 0.01);
    }

    /// The map rebuilds the HDR rendition from the stored base with the decoder's maths.
    #[test]
    fn gain_maps_rebuild_the_hdr_rendition() {
        let (w, h) = (64u32, 16u32);
        let t = Transfer::Srgb;
        let mut sdr = Vec::new();
        let mut hdr = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let s = [x as f64 / 63.0, y as f64 / 15.0, 0.5];
                let lin = s.map(|v| t.decode(v));
                // Highlights lifted more than shadows, a little colour shift.
                let boost = 1.0 + 3.0 * lin[0] * lin[0];
                sdr.extend(s.map(|v| (t.encode(t.decode(v)) * 255.0).round() as u8));
                hdr.extend(
                    [lin[0] * boost, lin[1] * boost * 0.95, lin[2] * boost].map(|v| v as f32),
                );
            }
        }
        let m = gain_map(&sdr, &hdr, t, 4.0, w, h);
        assert_eq!(m.samples.len(), (w * h * 3) as usize);
        let weight = m.params.weight(4.0);
        assert_eq!(weight, 1.0);
        let mut worst = 0f32;
        for (i, px) in m.samples.chunks(3).enumerate() {
            for c in 0..3 {
                let base = t.decode(f64::from(sdr[i * 3 + c]) / 255.0) as f32;
                let g = f32::from(px[c]) / 255.0;
                let got = m.params.apply(c, base, g, weight);
                let want = hdr[i * 3 + c];
                worst = worst.max((got - want).abs() / want.max(0.05));
            }
        }
        // 8-bit gains over the image's own range: within half a code of log2 steps.
        assert!(worst < 0.01, "worst relative error {worst}");
        assert!(m.params.weight(1.0) == 0.0);
    }

    #[test]
    fn hdr_choices_fit_their_formats() {
        for h in HdrOutput::ALL {
            assert_eq!(HdrOutput::from_id(h.id()), Some(h));
        }
        assert!(HdrOutput::Pq.fits(FileFormat::Png));
        assert!(!HdrOutput::Pq.fits(FileFormat::Jpeg));
        assert!(HdrOutput::GainMap.fits(FileFormat::Jpeg));
        assert!(!HdrOutput::GainMap.fits(FileFormat::Png));
        let p = Preset {
            format: FileFormat::Png,
            hdr: HdrOutput::Hlg,
            ..Preset::default()
        }
        .normalised();
        assert_eq!((p.space, p.depth), (Space::Rec2020, Depth::Sixteen));
        let p = Preset {
            format: FileFormat::Tiff,
            hdr: HdrOutput::Pq,
            ..Preset::default()
        }
        .normalised();
        assert_eq!(p.hdr, HdrOutput::Off);
    }
}
