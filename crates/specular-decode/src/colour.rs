//! Colour-managed conversion of CMYK and L*a*b* sources to sRGB while decoding
//! (specular-color, under the process-wide policy). RGB and grey sources are not touched here:
//! their embedded profile goes to the GPU source transform through [`crate::ImageInfo`].
//!
//! * JPEG and TIFF CMYK: row converters registered as the decoders' CMYK hooks.
//! * TIFF L*a*b* (CIELab, ICCLab, ITULab): the strided [`LabConverter`] registered as the TIFF
//!   Lab hook, fed straight from the decoded rows or planes.
//! * PSD CMYK and Lab: the composite planes converted here ([`psd_rgba`]).

use std::borrow::Cow;
use std::sync::{Arc, Once};

use rayon::prelude::*;

use specular_color::{CmykConverter, LabConverter};
use specular_psd as sp;

/// Registers the decoders' CMYK hooks (once per process).
pub(crate) fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        specular_jpeg::set_cmyk_hook(Some(row_converter));
        specular_tiff::set_cmyk_hook(Some(row_converter));
        specular_jxl::set_cmyk_hook(Some(row_converter));
        specular_tiff::set_lab_hook(Some(lab_converter));
    });
}

/// The TIFF L*a*b* converter, unless colour management is off (the decoder's scalar formula is
/// used then).
fn lab_converter() -> Option<specular_tiff::LabRowConverter> {
    if !specular_color::policy().enabled {
        return None;
    }
    let lab = LabConverter::default();
    Some(Arc::new(
        move |s: &specular_tiff::LabSamples, rgba: &mut [u8]| {
            let c = &s.coding;
            let coding = specular_color::LabCoding {
                wide: c.wide,
                big_endian: c.big_endian,
                signed: c.signed,
                scale: c.scale,
                offset: c.offset,
            };
            lab.convert_samples(s.channels, s.step, &coding, rgba)
        },
    ))
}

/// The CMYK converter for a profile, unless colour management is off (the decoders' own naive
/// formula is used then).
fn converter(icc: Option<&[u8]>) -> Option<Arc<CmykConverter>> {
    let t0 = std::time::Instant::now();
    let c = specular_color::policy::cmyk_converter(icc);
    specular_log::debug!(target: specular_log::COLOUR, "CMYK converter ({}) in {:.1} ms", if icc.is_some() { "embedded profile" } else { "generic" }, t0.elapsed().as_secs_f64() * 1e3);
    (!c.is_naive()).then_some(c)
}

#[allow(clippy::type_complexity)]
fn row_converter(icc: Option<&[u8]>) -> Option<Arc<dyn Fn(&[u8], &mut [u8]) + Send + Sync>> {
    let c = converter(icc)?;
    Some(Arc::new(move |ink: &[u8], rgba: &mut [u8]| {
        c.convert_row(ink, rgba)
    }))
}

/// CMYK inks (4 bytes per pixel, 0 = no ink) to 8-bit sRGB RGBA through the embedded
/// profile (the generic CMYK profile without one; the naive formula with colour management
/// off), with straight `alpha` when given. For JPEG XL's CMYK (a black extra channel).
pub(crate) fn cmyk_rgba(
    ink: &[u8],
    width: usize,
    icc: Option<&[u8]>,
    alpha: Option<&[u8]>,
) -> Vec<u8> {
    let conv = specular_color::policy::cmyk_converter(icc);
    let w = width.max(1);
    let mut out = vec![0u8; ink.len()];
    // Bands of about 16 K pixels in parallel.
    let rows = (16384 / w).max(1);
    out.par_chunks_mut(w * 4 * rows)
        .zip(ink.par_chunks(w * 4 * rows))
        .for_each(|(o, i)| conv.convert_row(i, o));
    if let Some(a) = alpha {
        for (o, &a) in out.chunks_exact_mut(4).zip(a) {
            o[3] = a;
        }
    }
    out
}

/// A PSD composite in CMYK or Lab mode, colour-managed to 8-bit RGBA. `None` for other modes,
/// float samples, or when colour management is off (the reader's naive conversion is used).
pub(crate) fn psd_rgba(p: &sp::Planes, icc: Option<&[u8]>) -> Option<Vec<u8>> {
    if !specular_color::policy().enabled {
        return None;
    }
    let (w, h) = (p.width as usize, p.height as usize);
    let n = w * h;
    // 8-bit view of plane `c` (borrowed when the planes are 8-bit).
    let plane = |c: usize| -> Option<Cow<[u8]>> {
        let r = c * n..(c + 1) * n;
        match &p.samples {
            sp::Samples::U8(s) => s.get(r).map(Cow::Borrowed),
            sp::Samples::U16(s) => s.get(r).map(|s| {
                Cow::Owned(
                    s.par_iter()
                        .with_min_len(1 << 16)
                        .map(|&v| ((v as u32 * 255 + 32767) / 65535) as u8)
                        .collect(),
                )
            }),
            sp::Samples::F32(_) => None,
        }
    };
    let alpha = if p.alpha {
        plane(p.color_channels)
    } else {
        None
    };
    let mut out = vec![0u8; n * 4];
    match p.mode {
        sp::ColorMode::Cmyk if p.color_channels >= 4 => {
            let conv = converter(icc)?;
            let planes: Vec<Cow<[u8]>> = (0..4).map(plane).collect::<Option<_>>()?;
            // Bands of about 16 K pixels in parallel.
            let rows = (16384 / w.max(1)).max(1);
            out.par_chunks_mut((w * 4 * rows).max(1))
                .enumerate()
                .for_each(|(b, band)| {
                    let mut ink = vec![0u8; w * 4];
                    for (r, row) in band.chunks_exact_mut(w * 4).enumerate() {
                        let y = b * rows + r;
                        // Photoshop stores CMYK inverted (255 = no ink).
                        for (x, i) in ink.chunks_exact_mut(4).enumerate() {
                            let at = y * w + x;
                            for (c, pl) in planes.iter().enumerate() {
                                i[c] = 255 - pl[at];
                            }
                        }
                        conv.convert_row(&ink, row);
                    }
                });
        }
        sp::ColorMode::Lab if p.color_channels >= 3 => {
            let conv = LabConverter::default();
            // Bands of about 16 K pixels in parallel, straight from the planes.
            let mut band = |f: &(dyn Fn(usize, &mut [u8]) + Sync)| {
                let rows = (16384 / w.max(1)).max(1);
                out.par_chunks_mut((w * 4 * rows).max(1))
                    .enumerate()
                    .for_each(|(b, o)| f(b * rows * w, o));
            };
            match &p.samples {
                sp::Samples::U8(s) if s.len() >= 3 * n => band(&|at, o| {
                    let r = |c: usize| &s[c * n + at..c * n + at + o.len() / 4];
                    conv.convert_planes8(r(0), r(1), r(2), o);
                }),
                sp::Samples::U16(s) if s.len() >= 3 * n => band(&|at, o| {
                    let r = |c: usize| &s[c * n + at..c * n + at + o.len() / 4];
                    conv.convert_planes16(r(0), r(1), r(2), o);
                }),
                _ => return None,
            }
        }
        _ => return None,
    }
    if let Some(a) = alpha {
        const BAND: usize = 1 << 16;
        out.par_chunks_mut(BAND * 4)
            .zip(a.par_chunks(BAND))
            .for_each(|(out, a)| {
                for (o, &a) in out.chunks_exact_mut(4).zip(a) {
                    o[3] = a;
                    if p.matted_on_white && a != 0 && a != 255 {
                        // Undo Photoshop's flattening onto white: c = (c' − (1 − a)) / a.
                        let a = a as i32;
                        for c in &mut o[..3] {
                            *c = (((*c as i32 - (255 - a)) * 255 + a / 2) / a).clamp(0, 255) as u8;
                        }
                    }
                }
            });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the hooks in place, CMYK goes through the generic CMYK profile: paper white stays
    /// white, full black is dark, cyan ink is not the naive (0, 255, 255).
    #[test]
    fn managed_cmyk_row() {
        let conv = row_converter(None).expect("colour management is on by default");
        let ink = [0u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let mut rgba = [0u8; 12];
        conv(&ink, &mut rgba);
        assert!(rgba[..3].iter().all(|&v| v >= 250), "{rgba:?}");
        assert!(
            rgba[4] < 100 && rgba[5] > 120 && rgba[6] > 200,
            "cyan {rgba:?}"
        );
        assert!(rgba[8..11].iter().all(|&v| v < 60), "black {rgba:?}");
    }

    /// JPEG XL CMYK: inks to sRGB over several bands, alpha passed
    /// through straight.
    #[test]
    fn jxl_cmyk_rows() {
        let (w, h) = (301usize, 120usize);
        let mut ink = vec![0u8; w * h * 4];
        // The last pixel solid black.
        ink[w * h * 4 - 4..].copy_from_slice(&[255, 255, 255, 255]);
        let alpha: Vec<u8> = (0..w * h).map(|i| (i % 256) as u8).collect();
        let out = cmyk_rgba(&ink, w, None, Some(&alpha));
        assert!(out[..3].iter().all(|&v| v >= 250), "paper {:?}", &out[..4]);
        assert!(out[w * h * 4 - 4..w * h * 4 - 1].iter().all(|&v| v < 60));
        for (o, a) in out.chunks_exact(4).zip(&alpha) {
            assert_eq!(o[3], *a);
        }
    }

    /// PSD Lab composites (8- and 16-bit, with alpha) convert band by band to the same
    /// colours as one pixel at a time; partial alpha is set, not matted (not Photoshop's).
    #[test]
    fn psd_lab_in_bands() {
        // Several bands, the last one partial.
        let (w, h) = (301usize, 157usize);
        let n = w * h;
        let mut seed = 3u32;
        let v16: Vec<u16> = (0..4 * n)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 16) as u16
            })
            .collect();
        let v8: Vec<u8> = v16.iter().map(|&v| (v >> 8) as u8).collect();
        let conv = LabConverter::default();
        for wide in [false, true] {
            let p = sp::Planes {
                width: w as u32,
                height: h as u32,
                mode: sp::ColorMode::Lab,
                color_channels: 3,
                alpha: true,
                samples: if wide {
                    sp::Samples::U16(v16.clone())
                } else {
                    sp::Samples::U8(v8.clone())
                },
                truncated: false,
                palette: None,
                transparent_index: None,
                matted_on_white: false,
            };
            let out = psd_rgba(&p, None).expect("colour management is on by default");
            for (i, o) in out.chunks_exact(4).enumerate() {
                let lab = |c: usize| {
                    if wide {
                        v16[c * n + i] as f32 / 257.0
                    } else {
                        v8[c * n + i] as f32
                    }
                };
                let want = conv.convert([lab(0) * (100.0 / 255.0), lab(1) - 128.0, lab(2) - 128.0]);
                for c in 0..3 {
                    assert!(o[c].abs_diff(want[c]) <= 1, "{wide} {i}: {o:?} {want:?}");
                }
                let a = if wide {
                    ((v16[3 * n + i] as u32 * 255 + 32767) / 65535) as u8
                } else {
                    v8[3 * n + i]
                };
                assert_eq!(o[3], a);
            }
        }
    }
}
