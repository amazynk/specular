//! The temperature grid as a float32 TIFF or CSV.

/// A little-endian, uncompressed, single-strip TIFF of 32-bit float samples (SampleFormat
/// 3), one per pixel, row-major; `description` goes in ImageDescription.
pub fn tiff_f32(width: usize, height: usize, values: &[f32], description: &str) -> Vec<u8> {
    assert_eq!(values.len(), width * height, "one value per pixel");
    let mut desc = description.as_bytes().to_vec();
    desc.push(0);
    const ENTRIES: u16 = 13;
    let ifd = 8usize;
    let ifd_len = 2 + 12 * usize::from(ENTRIES) + 4;
    let desc_at = ifd + ifd_len;
    let data_at = (desc_at + desc.len()).next_multiple_of(4);
    let data_len = values.len() * 4;
    let mut out = Vec::with_capacity(data_at + data_len);
    out.extend_from_slice(b"II*\0");
    out.extend_from_slice(&(ifd as u32).to_le_bytes());
    out.extend_from_slice(&ENTRIES.to_le_bytes());
    let mut entry = |tag: u16, ty: u16, count: u32, value: u32| {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if ty == 3 && count == 1 {
            out.extend_from_slice(&(value as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
        } else {
            out.extend_from_slice(&value.to_le_bytes());
        }
    };
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    const ASCII: u16 = 2;
    entry(256, LONG, 1, width as u32);
    entry(257, LONG, 1, height as u32);
    entry(258, SHORT, 1, 32);
    entry(259, SHORT, 1, 1);
    entry(262, SHORT, 1, 1);
    entry(270, ASCII, desc.len() as u32, desc_at as u32);
    entry(273, LONG, 1, data_at as u32);
    entry(277, SHORT, 1, 1);
    entry(278, LONG, 1, height as u32);
    entry(279, LONG, 1, data_len as u32);
    entry(284, SHORT, 1, 1);
    entry(296, SHORT, 1, 1);
    entry(339, SHORT, 1, 3);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&desc);
    out.resize(data_at, 0);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// One line per row, comma-separated values with `decimals` decimals; NaN is an empty
/// field.
pub fn csv(width: usize, values: &[f32], decimals: usize) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(values.len() * (decimals + 5));
    for row in values.chunks(width.max(1)) {
        for (i, v) in row.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            if v.is_finite() {
                let _ = write!(s, "{v:.decimals$}");
            }
        }
        s.push('\n');
    }
    s
}
