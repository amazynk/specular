//! Firaxis textures (`.ftc` compressed, `.ftu` uncompressed), from Civilization games:
//! little-endian `FTEX`, a version, width, height, the mip count and the number of formats,
//! then per format its code (0 DXT1, 1 uncompressed RGB) and where its data starts. There
//! each mip level is a 32-bit byte count and the level, the largest first.
//!
//! Files with more than one format are not known to exist; the first is shown.

use crate::{Container, Error, Format, Layout, Level, Result, Supercompression, Texture};

pub(crate) const MAGIC: &[u8; 4] = b"FTEX";

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    let g = |o| u32le(d, o).ok_or(Error::Truncated("FTEX header"));
    let (width, height, mips, formats) = (g(8)?, g(12)?, g(16)?, g(20)?);
    if formats == 0 {
        return Err(Error::Malformed("FTEX without a format"));
    }
    let (code, at) = (g(24)?, g(28)? as usize);
    let format = match code {
        0 => Format::Bc1 { alpha: true },
        1 => Format::Masked {
            bytes: 3,
            masks: [0xFF, 0xFF00, 0xFF_0000, 0],
            grey: false,
        },
        c => {
            return Err(Error::Unsupported(format!(
                "Firaxis texture format {c} is not supported (DXT1 and RGB are)"
            )));
        }
    };
    let (bw, bh, bb) = format.block();
    let mut levels = Vec::new();
    let mut p = at;
    let (mut w, mut h) = (width, height);
    for _ in 0..mips.clamp(1, 16) {
        let Some(len) = u32le(d, p) else { break };
        // The size is the file's word: a forged one must not overflow.
        let need = (w.div_ceil(bw) as usize)
            .checked_mul(h.div_ceil(bh) as usize)
            .and_then(|n| n.checked_mul(bb))
            .ok_or(Error::TooLarge)?;
        let start = p + 4;
        if len as usize != need && !levels.is_empty() {
            break;
        }
        let end = start.saturating_add(need).min(d.len());
        levels.push(Level {
            width: w,
            height: h,
            range: start.min(end)..end,
            uncompressed_len: need,
        });
        p = start.saturating_add(len as usize);
        if w == 1 && h == 1 {
            break;
        }
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    if levels.is_empty() {
        return Err(Error::Truncated("FTEX data"));
    }
    Ok(Texture {
        container: Container::Ftex,
        format,
        width,
        height,
        depth: 1,
        layers: 1,
        faces: 1,
        srgb: false,
        flip_y: false,
        premultiplied: false,
        levels,
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::{DecodeOptions, Texture};

    #[test]
    fn uncompressed_with_mips() {
        let mut d = b"FTEX".to_vec();
        for v in [1u32, 2, 2, 2, 1, 1, 32] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&12u32.to_le_bytes());
        d.extend((0..12).map(|i| i as u8));
        d.extend_from_slice(&3u32.to_le_bytes());
        d.extend_from_slice(&[9, 9, 9]);
        let t = Texture::open(&d).unwrap();
        assert_eq!((t.width, t.height, t.levels.len()), (2, 2, 2));
        let out = t.decode(&d, &DecodeOptions::default()).unwrap();
        assert_eq!(&out.data[..8], &[0, 1, 2, 255, 3, 4, 5, 255]);
    }
}
