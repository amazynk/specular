//! Compact proxies of raws for local copies (the Library's "Keep a local copy", proxies):
//! the visible area binned to about [`specular_library`'s 2560 px] on the long edge as
//! linear camera RGB, written as a demosaiced (LinearRaw) half-float DNG through the
//! merge's DNG writer, so the viewer, the gallery and Develop read it as any raw.
//!
//! - **Values:** each output pixel is the mean of each colour's samples in an `s × s` block
//!   of the visible area (`s` a multiple of the colour pattern's period), black subtracted
//!   and scaled by the white level: scene-linear, unclipped, no white balance applied.
//!   Binning whole pattern periods needs no demosaic and keeps the noise low.
//! - **Kept:** the camera's colour (DNG colour tags, else its matrix), the as-shot white
//!   balance, orientation, Exif and GPS, and list-3 lens warps (in normalised coordinates
//!   they hold at any size) with the maker-note lens corrections.
//! - **Left out:** opcode lists 1 and 2 and list-3 gain maps (in sensor pixels), bad-pixel
//!   lists. An XMP packet names the original (`sppx:original`, namespace [`NS`]).

use rayon::prelude::*;
use specular_raw::sensor::{Layout, OpcodeInfo, SensorData};

use crate::merge::dng::{Bits, DngOptions};
use crate::merge::{GhostMask, Merged, Mosaic, Shot, Transform};

/// The XMP namespace of a proxy's facts.
pub const NS: &str = "http://ns.specular.app/proxy/1/";

/// Why a proxy was not made (the file is then copied whole).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a raw whose sensor data the decoder reads.
    NotRaw,
    /// Decoding failed.
    Decode(String),
    /// Already as small as a proxy would be.
    Small,
}

/// The block side for a visible area of long side `long` and colour period `period`.
fn block(long: u32, edge: u32, period: u32) -> u32 {
    let s = long.div_ceil(edge.max(1)).max(1);
    s.div_ceil(period) * period
}

/// The colour period of a layout (the side of its repeating tile).
fn period(l: &Layout) -> u32 {
    match l {
        Layout::Bayer(_) => 2,
        Layout::Pattern(c) => c.width.max(c.height).max(1),
        Layout::Linear | Layout::Mono => 1,
    }
}

/// Bins the visible area of `s` into blocks of `side`: (width, height, samples per pixel,
/// linear values, 0 black … 1 white).
pub fn bin(s: &SensorData, side: u32) -> (u32, u32, u32, Vec<f32>) {
    let m = &s.meta;
    let c = s.crop;
    let (ow, oh) = ((c.w / side).max(1), (c.h / side).max(1));
    let mono = matches!(m.layout, Layout::Mono);
    let ocpp = if mono { 1 } else { 3 };
    let (w, cpp) = (s.width as usize, s.cpp as usize);
    let white = m.white;
    let value = |x: u32, y: u32, p: u32| -> f32 {
        let i = (y as usize * w + x as usize) * cpp + p as usize;
        let v = match &s.float {
            Some(f) => f.get(i).copied().unwrap_or(0.0),
            None => f32::from(s.samples.get(i).copied().unwrap_or(0)),
        };
        let b = m.black.at(x, y, p);
        (v - b) / (white - b).max(1.0)
    };
    let mut out = vec![0f32; ow as usize * oh as usize * ocpp];
    out.par_chunks_mut(ow as usize * ocpp)
        .enumerate()
        .for_each(|(oy, row)| {
            for ox in 0..ow {
                let mut sum = [0f64; 3];
                let mut n = [0u32; 3];
                let (x0, y0) = (c.x + ox * side, c.y + oy as u32 * side);
                for y in y0..(y0 + side).min(c.y + c.h) {
                    for x in x0..(x0 + side).min(c.x + c.w) {
                        if cpp == 3 {
                            for p in 0..3 {
                                sum[p as usize] += f64::from(value(x, y, p));
                                n[p as usize] += 1;
                            }
                        } else {
                            let k = if mono {
                                0
                            } else {
                                (m.layout.color(x, y) as usize).min(2)
                            };
                            sum[k] += f64::from(value(x, y, 0));
                            n[k] += 1;
                        }
                    }
                }
                let px = &mut row[ox as usize * ocpp..(ox as usize + 1) * ocpp];
                for (k, v) in px.iter_mut().enumerate() {
                    *v = if n[k] > 0 {
                        (sum[k] / f64::from(n[k])) as f32
                    } else {
                        0.0
                    };
                }
            }
        });
    (ow, oh, ocpp as u32, out)
}

/// The proxy of the raw at `path` (its bytes `data`), its long edge about `edge` pixels.
pub fn make(path: &std::path::Path, data: &[u8], edge: u32) -> Result<Vec<u8>, Error> {
    let f = specular_raw::parse(data).map_err(|e| Error::Decode(e.to_string()))?;
    if !f.can_decode_sensor() {
        return Err(Error::NotRaw);
    }
    let s = f
        .decode_sensor(data, &specular_raw::DecodeOptions::default())
        .map_err(|e| Error::Decode(e.to_string()))?;
    let per = period(&s.meta.layout);
    let long = s.crop.w.max(s.crop.h);
    if per == 1 && long <= edge {
        return Err(Error::Small);
    }
    let side = block(long, edge, per);
    let (w, h, cpp, radiance) = bin(&s, side);
    let mut meta = s.meta.clone();
    meta.layout = if cpp == 1 {
        Layout::Mono
    } else {
        Layout::Linear
    };
    meta.black = specular_raw::sensor::BlackLevels::simple([0.0; 4], cpp);
    meta.white = 1.0;
    meta.clip_white = 1.0;
    meta.linearization = None;
    // Lens warps are in normalised coordinates: they hold for the whole proxy.
    let full = specular_raw::Rect { x: 0, y: 0, w, h };
    let warps: Vec<OpcodeInfo> = meta.opcodes[2]
        .iter()
        .filter_map(|o| match o {
            OpcodeInfo::Warp {
                fisheye, k, centre, ..
            } => Some(OpcodeInfo::Warp {
                fisheye: *fisheye,
                k: *k,
                centre: *centre,
                bounds: full,
            }),
            _ => None,
        })
        .collect();
    meta.opcodes = [Vec::new(), Vec::new(), warps];
    meta.opcodes_unknown = Vec::new();
    let shot = Shot::of_raw(path.to_path_buf(), data).unwrap_or_else(|| Shot {
        path: path.to_path_buf(),
        ..Shot::default()
    });
    let merged = Merged {
        width: w,
        height: h,
        cpp,
        mosaic: if cpp == 1 {
            Mosaic::Mono
        } else {
            Mosaic::Linear
        },
        radiance,
        clip: 1.0,
        reference: 0,
        exposures: vec![1.0],
        transforms: vec![Transform {
            angle: 0.0,
            tx: 0.0,
            ty: 0.0,
            cx: 0.0,
            cy: 0.0,
        }],
        warps: vec![crate::merge::Homography::IDENTITY],
        ghosts: GhostMask::default(),
        meta,
        crop: full,
        shots: vec![shot],
    };
    let o = DngOptions {
        bits: Bits::F16,
        cfa: false,
    };
    Ok(crate::merge::dng::encode_with_xmp(
        &merged,
        data,
        &o,
        xmp(path, s.crop.w, s.crop.h),
    ))
}

/// Whether a file's first bytes are a proxy's (its XMP names [`NS`]).
pub fn is_proxy(head: &[u8]) -> bool {
    let head = &head[..head.len().min(256 * 1024)];
    head.windows(NS.len()).any(|w| w == NS.as_bytes())
}

fn xmp(path: &std::path::Path, w: u32, h: u32) -> Vec<u8> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let esc = name
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;");
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:sppx=\"{NS}\" sppx:original=\"{esc}\" sppx:width=\"{w}\" sppx:height=\"{h}\"/>\
</rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>"
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_hold_whole_colour_periods() {
        assert_eq!(block(6000, 2560, 2), 4);
        assert_eq!(block(5120, 2560, 2), 2);
        assert_eq!(block(1000, 2560, 2), 2);
        assert_eq!(block(6000, 2560, 6), 6);
        assert_eq!(block(9000, 2560, 6), 6);
        assert_eq!(block(16000, 2560, 6), 12);
        assert_eq!(block(6000, 2560, 1), 3);
    }
}
