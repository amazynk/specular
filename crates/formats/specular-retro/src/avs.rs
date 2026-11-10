//! Stardent AVS X image (`.avs`, `.x`): width and height as big-endian u32s, then
//! alpha, red, green and blue bytes per pixel, top row first. No signature: the header must
//! describe the file's size exactly (with a little slack for trailing bytes).

use crate::*;

fn header(data: &[u8]) -> Result<(u32, u32)> {
    let w = u32be(data, 0).ok_or_else(|| truncated("header"))?;
    let h = u32be(data, 4).ok_or_else(|| truncated("header"))?;
    check_dims(w, h)?;
    Ok((w, h))
}

/// Whether the whole file is an AVS image: its size matches the header's.
pub fn probe_file(data: &[u8]) -> bool {
    header(data).is_ok_and(|(w, h)| {
        let need = 8 + u64::from(w) * u64::from(h) * 4;
        (need..=need + 512).contains(&(data.len() as u64))
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (w, h) = header(data)?;
    let alpha = data[8..].chunks_exact(4).any(|p| p[0] != 255);
    Ok(Info::new(w, h, PixelFormat::Rgba8, Model::Rgb, 8, "8-bit ARGB").alpha(alpha))
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let (w, h) = header(data)?;
    let src = &data[8..];
    check_payload(src.len(), u64::from(w) * u64::from(h) * 4, 1)?;
    let mut img = Image::filled(w, h, PixelFormat::Rgba8, GREY);
    img.truncated = src.len() < img.data.len();
    for (o, p) in img.data.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        o.copy_from_slice(&[p[1], p[2], p[3], p[0]]);
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small() {
        let mut f = vec![0, 0, 0, 2, 0, 0, 0, 1];
        f.extend_from_slice(&[255, 1, 2, 3, 0, 4, 5, 6]);
        assert!(probe_file(&f));
        assert!(!probe_file(&f[..12]));
        let i = info(&f).unwrap();
        assert!(i.has_alpha);
        assert_eq!(decode(&f).unwrap().data, [1, 2, 3, 255, 4, 5, 6, 0]);
    }
}
