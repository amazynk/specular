//! Quake 2 textures (`.wal`): a 100-byte little-endian header (32-byte name, width, height,
//! four mip offsets, the next animation frame's name, flags, contents, value), then 8-bit
//! palette indices for each mip level. The palette is the game's (`pics/colormap.pcx`), not
//! in the file: the caller hands it over when it finds one, else the indices show as grey.

use crate::*;

const HEADER: usize = 100;

fn header(data: &[u8]) -> Result<(u32, u32, usize, String)> {
    let w = u32le(data, 32).ok_or_else(|| truncated("header"))?;
    let h = u32le(data, 36).ok_or_else(|| truncated("header"))?;
    check_dims(w, h)?;
    let off = u32le(data, 40).ok_or_else(|| truncated("header"))? as usize;
    let name: String = data[..32]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    Ok((w, h, off, name))
}

/// Whether the mip offsets follow from the size, as every WAL's do.
pub fn probe_file(data: &[u8]) -> bool {
    let Ok((w, h, off, name)) = header(data) else {
        return false;
    };
    let n = w as usize * h as usize;
    let o = |k: usize| u32le(data, 40 + 4 * k).map(|v| v as usize);
    off == HEADER
        && o(1) == Some(HEADER + n)
        && o(2) == Some(HEADER + n + n / 4)
        && name.bytes().all(|b| b.is_ascii_graphic())
}

/// The palette of a PCX file (its last 768 bytes after a 0x0C marker), such as Quake 2's
/// `colormap.pcx`.
pub fn pcx_palette(pcx: &[u8]) -> Option<Vec<[u8; 4]>> {
    if pcx.first() != Some(&0x0a) || pcx.len() < 769 + 128 {
        return None;
    }
    let p = &pcx[pcx.len() - 769..];
    if p[0] != 0x0c {
        return None;
    }
    Some(
        p[1..]
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2], 255])
            .collect(),
    )
}

pub fn info(data: &[u8], palette: bool) -> Result<Info> {
    let (w, h, _, name) = header(data)?;
    let what = if palette {
        "game palette"
    } else {
        "no palette found: grey"
    };
    Ok(if palette {
        Info::new(
            w,
            h,
            PixelFormat::Rgba8,
            Model::Palette,
            8,
            format!("8-bit indexed \"{name}\", {what}"),
        )
    } else {
        Info::new(
            w,
            h,
            PixelFormat::Gray8,
            Model::Grey,
            8,
            format!("8-bit indexed \"{name}\", {what}"),
        )
    })
}

pub fn decode(data: &[u8], palette: Option<&[[u8; 4]]>) -> Result<Image> {
    let (w, h, off, _) = header(data)?;
    let n = w as usize * h as usize;
    let src = data.get(off..).unwrap_or(&[]);
    check_payload(src.len(), n as u64, 1)?;
    let mut idx = vec![0u8; n];
    let k = src.len().min(n);
    idx[..k].copy_from_slice(&src[..k]);
    let mut img = match palette {
        Some(p) => Image::from_indices(w, h, &idx, p),
        None => Image {
            width: w,
            height: h,
            format: PixelFormat::Gray8,
            data: idx,
            truncated: false,
        },
    };
    img.truncated = k < n;
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn build(w: u32, h: u32) -> Vec<u8> {
        let mut f = vec![0u8; HEADER];
        f[..9].copy_from_slice(b"e1u1/wall");
        f[32..36].copy_from_slice(&w.to_le_bytes());
        f[36..40].copy_from_slice(&h.to_le_bytes());
        let n = (w * h) as usize;
        let mut off = HEADER;
        for k in 0..4 {
            f[40 + 4 * k..44 + 4 * k].copy_from_slice(&(off as u32).to_le_bytes());
            off += n >> (2 * k);
        }
        for k in 0..4 {
            f.extend((0..n >> (2 * k)).map(|i| i as u8));
        }
        f
    }

    #[test]
    fn grey_and_palette() {
        let f = build(8, 4);
        assert!(probe_file(&f));
        let img = decode(&f, None).unwrap();
        assert_eq!(&img.data[..3], &[0, 1, 2]);
        let pal: Vec<[u8; 4]> = (0..=255u8).map(|i| [i, 0, 255 - i, 255]).collect();
        let img = decode(&f, Some(&pal)).unwrap();
        assert_eq!(&img.data[8..12], &[2, 0, 253, 255]);
        let mut pcx = vec![0x0a];
        pcx.resize(200, 0);
        pcx.push(0x0c);
        pcx.extend((0..768).map(|i| i as u8));
        assert_eq!(pcx_palette(&pcx).unwrap()[1], [3, 4, 5, 255]);
        assert!(info(&f, false).unwrap().coding.contains("e1u1/wall"));
    }
}
