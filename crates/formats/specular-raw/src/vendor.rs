//! Dispatch for the vendor codecs that read one whole compressed range (Hasselblad, Kodak,
//! Samsung, Phase One, Canon CRW, Sigma X3F, ...).

use std::ops::Range;

use crate::DecodeOptions;
use crate::error::Result;

/// A vendor codec and its parameters.
#[derive(Clone, Debug)]
pub(crate) enum VendorCodec {
    /// Hasselblad lossless (`base` is added to the row-start prediction).
    Hasselblad { base: i32 },
    /// Kodak compression 65000.
    Kodak65000(Box<crate::kodak::K65000>),
    /// Kodak DC120 (rotated 8-bit rows).
    KodakDc120,
    /// Kodak DC120 with JPEG compression.
    KodakDc120Jpeg,
    /// Samsung SRW version 1; `rows` holds the LE 32-bit offset of each row from the data start.
    SamsungV1 { rows: Range<usize> },
    /// Samsung SRW version 2.
    SamsungV2,
    /// Samsung SRW version 3.
    SamsungV3,
    /// Panasonic RW2 raw format 8 (strips at absolute offsets).
    Pana8(Box<crate::pana8::Params>),
    /// Canon CRW: the RawData record, decoder table set and whether low-order bits precede
    /// the coded data.
    Crw { table: u32, low_bits: bool },
    /// Phase One IIQ L / S: `rows` is the table of row offsets into the data range.
    PhaseOne {
        kind: crate::phase_one::Kind,
        rows: Range<usize>,
        /// Black tables and the calibration block, applied after decoding.
        cal: Box<crate::phase_one_cal::Calibration>,
    },
    /// Phase One uncompressed 16-bit samples with an optional bit-exchange key
    /// (`a`, `b`, `mask`), and the calibration applied after decoding (as for IIQ L / S).
    PhaseOnePlain {
        key: Option<(u16, u16, u16)>,
        cal: Box<crate::phase_one_cal::Calibration>,
    },
    /// Sigma X3F TRUE / Merrill / Quattro planes (`quattro`: plane sizes precede the seeds),
    /// with the Quattro AF-pixel grid to correct.
    X3fTrue {
        quattro: bool,
        af: Option<crate::x3f::AfGrid>,
        gain: Option<Box<crate::x3f::SpatialGain>>,
    },
    /// Sigma X3F Huffman 10-bit (`stride` 0) or mapped 32-bit words per pixel (`stride` bytes
    /// per row); `white`: saturation before the decoder's offset is added.
    X3fHuffman {
        stride: usize,
        white: Option<u16>,
        gain: Option<Box<crate::x3f::SpatialGain>>,
    },
    /// ARRI ARI: 12-bit samples over little-endian words, column pairs swapped.
    Ari,
    /// Kodak RADC (DC40, DC50); `cbpp`: numerator of the CompressedBitsPerPixel tag.
    KodakRadc { cbpp: u32 },
    /// VC-5 (GoPro GPR, DNG compression 9).
    Vc5(crate::vc5::Params),
    /// VC-5 split into DNG tiles (or strips) of `tw x th`, each its own bitstream, at these
    /// absolute byte ranges (row-major).
    Vc5Tiles {
        p: crate::vc5::Params,
        tiles: Vec<Range<usize>>,
        tw: usize,
        th: usize,
    },
    /// Sony SRF: big-endian 16-bit samples under an XOR stream from `key`.
    SonySrf { key: u32 },
    /// Minolta RD-175: three CCDs into one GRBG mosaic.
    Rd175,
    /// Nikon High Efficiency (HE, HE*): an intoPIX TicoRAW codestream.
    NikonHe,
}

/// What a vendor decode reports besides the samples.
#[derive(Default)]
pub(crate) struct Decoded {
    pub truncated: bool,
    /// A white level only known after decoding.
    pub white: Option<f32>,
}

/// VC-5 tiles of `tw x th` (each a bitstream of that size) into the `w x h` sensor; missing
/// tiles stay zero and count as truncation.
fn vc5_tiles(
    d: &[u8],
    tiles: &[Range<usize>],
    p: crate::vc5::Params,
    (tw, th): (usize, usize),
    (w, h): (usize, usize),
    out: &mut [u16],
    parallel: bool,
) -> Result<bool> {
    if tw == 0 || th == 0 {
        return Err(crate::error::RawError::Malformed("VC-5 tile size"));
    }
    let across = w.div_ceil(tw);
    let need = across * h.div_ceil(th);
    let mut short = tiles.len() < need;
    let mut buf = vec![0u16; tw * th];
    for (i, r) in tiles.iter().take(need).enumerate() {
        let src = d
            .get(r.start.min(d.len())..r.end.min(d.len()))
            .unwrap_or(&[]);
        buf.fill(0);
        short |= r.end > d.len()
            || crate::vc5::decode(src, tw, th, p, &mut buf, parallel)
                .map_err(crate::error::RawError::Malformed)?;
        let (x0, y0) = ((i % across) * tw, (i / across) * th);
        let cw = tw.min(w - x0);
        for y in 0..th.min(h - y0) {
            out[(y0 + y) * w + x0..][..cw].copy_from_slice(&buf[y * tw..y * tw + cw]);
        }
    }
    Ok(short)
}

/// Decodes the whole sensor (`w x h x cpp` samples) of `codec` from `d[range]`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode(
    d: &[u8],
    range: &Range<usize>,
    codec: &VendorCodec,
    w: usize,
    h: usize,
    cpp: usize,
    out: &mut [u16],
    opts: &DecodeOptions,
) -> Result<Decoded> {
    let src = d
        .get(range.start.min(d.len())..range.end.min(d.len()))
        .unwrap_or(&[]);
    let short = range.end > d.len();
    let n = w.saturating_mul(h).saturating_mul(cpp);
    let out = out
        .get_mut(..n)
        .ok_or(crate::error::RawError::Malformed("sensor buffer too small"))?;
    let mut r = match codec {
        VendorCodec::Hasselblad { base } => Decoded {
            truncated: crate::hasselblad::decode(src, w, h, *base, out)?,
            ..Default::default()
        },
        VendorCodec::KodakDc120 => Decoded {
            truncated: crate::kodak::decode_dc120(src, out),
            ..Default::default()
        },
        VendorCodec::KodakDc120Jpeg => Decoded {
            truncated: crate::kodak::decode_dc120_jpeg(src, out)?,
            ..Default::default()
        },
        VendorCodec::SamsungV1 { rows } => Decoded {
            truncated: crate::samsung::decode_v1(
                src,
                d.get(rows.start.min(d.len())..rows.end.min(d.len()))
                    .unwrap_or(&[]),
                w,
                h,
                out,
            ),
            ..Default::default()
        },
        VendorCodec::SamsungV2 => Decoded {
            truncated: crate::samsung::decode_v2(src, w, h, out),
            ..Default::default()
        },
        VendorCodec::SamsungV3 => Decoded {
            truncated: crate::samsung::decode_v3(src, w, h, out),
            ..Default::default()
        },
        VendorCodec::Pana8(p) => Decoded {
            truncated: crate::pana8::decode(d, p, w, h, out, opts.parallel),
            ..Default::default()
        },
        VendorCodec::Crw { table, low_bits } => Decoded {
            truncated: crate::crw::decode(src, *table, *low_bits, w, h, out),
            ..Default::default()
        },
        VendorCodec::PhaseOne { kind, rows, cal } => {
            let truncated = crate::phase_one::decode(
                src,
                d.get(rows.start.min(d.len())..rows.end.min(d.len()))
                    .unwrap_or(&[]),
                *kind,
                w,
                h,
                out,
                opts.parallel,
            );
            cal.apply(d, out, w, h, opts.parallel);
            Decoded {
                truncated,
                ..Default::default()
            }
        }
        VendorCodec::PhaseOnePlain { key, cal } => {
            let truncated = crate::phase_one::decode_plain(src, *key, out);
            cal.apply(d, out, w, h, opts.parallel);
            Decoded {
                truncated,
                ..Default::default()
            }
        }
        VendorCodec::X3fTrue { quattro, af, gain } => {
            let truncated = crate::x3f::decode_true(src, *quattro, *af, w, h, out, opts.parallel);
            if let Some(g) = gain {
                g.apply(out, w, h, opts.parallel);
            }
            Decoded {
                truncated,
                ..Default::default()
            }
        }
        VendorCodec::X3fHuffman {
            stride,
            white,
            gain,
        } => {
            let (truncated, offset) =
                crate::x3f::decode_huffman10(src, *stride, w, h, out, opts.parallel);
            if let Some(g) = gain {
                g.apply(out, w, h, opts.parallel);
            }
            Decoded {
                truncated,
                white: white.map(|v| v as f32 + offset as f32),
            }
        }
        VendorCodec::Ari => Decoded {
            truncated: crate::ari::decode(src, w, h, out, opts.parallel),
            ..Default::default()
        },
        VendorCodec::KodakRadc { cbpp } => Decoded {
            truncated: crate::radc::decode(src, *cbpp, w, h, out),
            ..Default::default()
        },
        VendorCodec::Vc5(p) => Decoded {
            truncated: crate::vc5::decode(src, w, h, *p, out, opts.parallel)
                .map_err(crate::error::RawError::Malformed)?,
            ..Default::default()
        },
        VendorCodec::Vc5Tiles { p, tiles, tw, th } => Decoded {
            truncated: vc5_tiles(d, tiles, *p, (*tw, *th), (w, h), out, opts.parallel)?,
            ..Default::default()
        },
        VendorCodec::Rd175 => Decoded {
            truncated: crate::rd175::decode(src, out),
            ..Default::default()
        },
        VendorCodec::SonySrf { key } => Decoded {
            truncated: crate::srf::decode(src, *key, out),
            ..Default::default()
        },
        VendorCodec::NikonHe => Decoded {
            truncated: crate::nikon_he::decode(src, w, h, out, opts.parallel)?,
            ..Default::default()
        },
        VendorCodec::Kodak65000(k) => Decoded {
            truncated: crate::kodak::decode_65000(src, w, h, k, out, opts.parallel)?,
            ..Default::default()
        },
    };
    r.truncated |= short;
    Ok(r)
}
