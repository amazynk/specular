//! HTJ2K256 / HTJ2K32 chunks (OpenEXR 3.4+) and LJ2K chunks (OpenEXR 3.5): a small header,
//! then one HTJ2K (JPEG 2000 Part 15) codestream holding the chunk's channels as components,
//! decoded by `specular-jp2`.
//!
//! Chunk layout: magic `HT` (big-endian u16), the payload length (u32), then the payload: the
//! number of components (u16) and, for each codestream component, the index of the part
//! channel it holds (u16), possibly followed by bytes we skip. The codestream follows the
//! payload. Every component is coded losslessly (5/3 wavelet, RCT on an R/G/B triple of one
//! type): a HALF channel as a signed 16-bit component, a FLOAT channel as a signed 32-bit one
//! (both with their negative bit patterns put in order by the binary-complement
//! non-linearity, NLT type 3, undone by `specular-jp2`), a UINT channel as an unsigned 32-bit
//! one. The decoded sample is the channel's bit pattern again. 32-bit components have up to
//! 34 magnitude bit-planes and go through `specular-jp2`'s 64-bit path.
//!
//! LJ2K (lossy) codes the first three channels with the irreversible 9/7 wavelet and the ICT,
//! the others as above. Its lossy HALF / FLOAT components carry a lookup-table non-linearity
//! (NLT type 4: a transfer function like DWA's, then binary complement) that `specular-jp2`
//! undoes, giving the bit patterns again; the header magic is then `HL`. Files of OpenEXR
//! 3.5.0 / 3.5.1 have the magic `HT` and the transfer function applied before coding, undone
//! here ([`legacy_half`], [`legacy_float`]). A lossy chunk may be coded padded (its last row
//! and column repeated up to a size the wavelet folds well on); the padding is dropped.

use specular_jp2::Decoder;

use crate::codec::{BlockGeom, Layout};
use crate::half::f2h;
use crate::header::{Channel, PixelType};

const MAGIC: u16 = u16::from_be_bytes(*b"HT");
const MAGIC_LJ2K: u16 = u16::from_be_bytes(*b"HL");

/// The transfer function of lossy LJ2K samples back to linear light (sign kept): a 2.2 power
/// up to 1, exponential above. The constants are `f32` widened, as OpenEXR computes them.
fn tf_to_linear(x: f64) -> f64 {
    let g = 2.2f32 as f64;
    let v = x.abs();
    let l = if v <= 1.0 {
        v.powf(g)
    } else {
        (g * (v - 1.0)).exp()
    };
    if x < 0.0 { -l } else { l }
}

/// A legacy (OpenEXR 3.5.0 / 3.5.1) lossy 16-bit sample to half bits.
pub(crate) fn legacy_half(f: i32) -> u16 {
    // 32767 / (ln(65504) / 2.2 + 1).
    const FACTOR: f64 = 5424.23808866629;
    f2h(tf_to_linear(f.clamp(-32767, 32767) as f64 / FACTOR) as f32)
}

/// A legacy lossy 32-bit sample to float bits.
pub(crate) fn legacy_float(f: i32) -> u32 {
    // 2147483647 / (ln(FLT_MAX) / 2.2 + 1).
    const FACTOR: f64 = 51961246.180338;
    let v = tf_to_linear(f as f64 / FACTOR).clamp(-(f32::MAX as f64), f32::MAX as f64);
    (v as f32).to_bits()
}

/// Decode one chunk into `out` (`g.total` bytes, zeroed), channel after channel
/// ([`Layout::Planar`]); `lj2k` for compression LJ2K. `None` on malformed data.
pub(crate) fn decompress(
    input: &[u8],
    g: &BlockGeom,
    chans: &[Channel],
    out: &mut [u8],
    lj2k: bool,
) -> Option<Layout> {
    let rd16 = |p: usize| {
        input
            .get(p..p + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    let magic = rd16(0)?;
    if magic != MAGIC && !(lj2k && magic == MAGIC_LJ2K) {
        return None;
    }
    let legacy = lj2k && magic == MAGIC;
    let plen = u32::from_be_bytes(input.get(2..6)?.try_into().ok()?) as usize;
    let cs = input.get(6usize.checked_add(plen)?..)?;
    let nch = rd16(6)? as usize;
    if nch != chans.len() || nch != g.chans.len() || plen < 2 + 2 * nch {
        return None;
    }
    // Codestream component -> part channel.
    let mut file_of = Vec::with_capacity(nch);
    let mut seen = vec![false; nch];
    for i in 0..nch {
        let f = rd16(8 + 2 * i)? as usize;
        if f >= nch || std::mem::replace(&mut seen[f], true) {
            return None;
        }
        file_of.push(f);
    }
    let dec = Decoder::new(cs).ok()?;
    let info = dec.info();
    if info.components.len() != nch {
        return None;
    }
    for (i, &f) in file_of.iter().enumerate() {
        let c = &info.components[i];
        let ok = match g.chans[f].ty {
            PixelType::Half => c.precision == 16 && c.signed,
            PixelType::Float => c.precision == 32 && c.signed,
            PixelType::Uint => c.precision == 32 && !c.signed,
        };
        if !ok {
            return None;
        }
    }
    let comps = dec.decode_components_u32().ok()?;
    // Lossy LJ2K chunks of unsubsampled channels may be coded padded (every component).
    let padded = lj2k
        && !info.components[0].reversible
        && g.chans
            .iter()
            .all(|c| c.nx == g.chans[0].nx && c.ny == g.chans[0].ny);
    // Byte offset of each channel's plane in `out`.
    let mut start = Vec::with_capacity(nch);
    let mut pos = 0usize;
    for cg in &g.chans {
        start.push(pos);
        pos += cg.nx * cg.ny * cg.bytes;
    }
    for (i, (w, h, samples)) in comps.iter().enumerate() {
        let f = file_of[i];
        let cg = &g.chans[f];
        let (w, h) = (*w as usize, *h as usize);
        let lossy = !info.components[i].reversible;
        if (w, h) != (cg.nx, cg.ny) && !(padded && w >= cg.nx && h >= cg.ny) {
            return None;
        }
        if w == 0 {
            continue;
        }
        let tf = legacy && lossy;
        let dst = out.get_mut(start[f]..start[f] + cg.nx * cg.ny * cg.bytes)?;
        // Native samples of signed components are offset by 2^(prec - 1); the signed value
        // is the channel's bit pattern (or, for legacy lossy LJ2K, the coded value).
        let rows = samples.chunks_exact(w).map(|r| &r[..cg.nx]);
        match cg.ty {
            PixelType::Half => {
                let px = rows.flatten().map(|&s| (s as u16) ^ 0x8000);
                for (d, s) in dst.chunks_exact_mut(2).zip(px) {
                    let v = if tf { legacy_half(s as i16 as i32) } else { s };
                    d.copy_from_slice(&v.to_le_bytes());
                }
            }
            PixelType::Float => {
                let px = rows.flatten().map(|&s| s ^ 0x8000_0000);
                for (d, s) in dst.chunks_exact_mut(4).zip(px) {
                    let v = if tf { legacy_float(s as i32) } else { s };
                    d.copy_from_slice(&v.to_le_bytes());
                }
            }
            PixelType::Uint => {
                for (d, &s) in dst.chunks_exact_mut(4).zip(rows.flatten()) {
                    d.copy_from_slice(&s.to_le_bytes());
                }
            }
        }
    }
    Some(Layout::Planar)
}
