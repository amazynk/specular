//! X BitMap: C source with `#define <name>_width`/`_height` and a `char` (X11) or `short` (X10)
//! array; bits LSB-first, a set bit is black (foreground).

use crate::*;

pub fn probe(head: &[u8]) -> bool {
    let s = &head[..head.len().min(512)];
    let t = String::from_utf8_lossy(s);
    let t = t.trim_start();
    (t.starts_with("#define") || t.starts_with("/*")) && t.contains("_width")
}

struct Header {
    w: u32,
    h: u32,
    x10: bool,
    body: usize,
}

fn define(text: &str, suffix: &str) -> Option<u32> {
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("#define")
            && let Some(name) = it.next()
            && name.ends_with(suffix)
        {
            return it.next()?.parse().ok();
        }
    }
    None
}

fn header(data: &[u8]) -> Result<Header> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let brace = data
        .iter()
        .position(|&b| b == b'{')
        .ok_or_else(|| Error::Truncated("no bitmap array".into()))?;
    let head = String::from_utf8_lossy(&data[..brace]);
    let w = define(&head, "_width").ok_or_else(|| Error::Malformed("no width".into()))?;
    let h = define(&head, "_height").ok_or_else(|| Error::Malformed("no height".into()))?;
    check_dims(w, h)?;
    let decl = head.rsplit("#define").next().unwrap_or("");
    Ok(Header {
        w,
        h,
        x10: decl.contains("short"),
        body: brace + 1,
    })
}

pub fn info(data: &[u8]) -> Result<Info> {
    let h = header(data)?;
    Ok(Info {
        format: Format::Xbm,
        width: h.w,
        height: h.h,
        pixel_format: PixelFormat::Gray8,
        has_alpha: false,
        model: crate::Model::Grey,
        bit_depth: 1,
        coding: if h.x10 { "X10 bitmap" } else { "X11 bitmap" }.into(),
    })
}

/// Next C integer literal (hex or decimal) in `d` from `*i`; stops at `}`.
fn next_int(d: &[u8], i: &mut usize) -> Option<u32> {
    while *i < d.len() && !d[*i].is_ascii_digit() {
        if d[*i] == b'}' {
            return None;
        }
        *i += 1;
    }
    let start = *i;
    let (radix, digits_at) =
        if d.get(*i) == Some(&b'0') && matches!(d.get(*i + 1), Some(b'x' | b'X')) {
            (16, *i + 2)
        } else {
            (10, *i)
        };
    *i = digits_at;
    let mut v: u32 = 0;
    while let Some(dig) = d.get(*i).and_then(|&c| (c as char).to_digit(radix)) {
        v = v.wrapping_mul(radix).wrapping_add(dig);
        *i += 1;
    }
    if *i == start {
        return None;
    }
    Some(v)
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let hd = header(data)?;
    let (w, h) = (hd.w as usize, hd.h as usize);
    // Each value is at least two source bytes ("0,").
    let unit_bits = if hd.x10 { 16 } else { 8 };
    let per_row = w.div_ceil(unit_bits);
    check_payload(data.len(), (per_row * h) as u64, 4)?;
    let mut img = Image::new(hd.w, hd.h, PixelFormat::Gray8);
    let mut i = hd.body;
    let mut units = vec![0u32; per_row];
    for y in 0..h {
        for u in units.iter_mut() {
            match next_int(data, &mut i) {
                Some(v) => *u = v,
                None => {
                    img.truncated = true;
                    img.data[y * w..].fill(GREY);
                    return Ok(img);
                }
            }
        }
        let row = &mut img.data[y * w..(y + 1) * w];
        for (x, o) in row.iter_mut().enumerate() {
            let bit = units[x / unit_bits] >> (x % unit_bits) & 1;
            *o = if bit != 0 { 0 } else { 255 };
        }
    }
    Ok(img)
}
