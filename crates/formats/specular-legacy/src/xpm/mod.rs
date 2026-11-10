//! X PixMap 3: a C array of strings — `"w h ncolors cpp …"`, `ncolors` colour lines
//! (`<chars> c <colour>`, with `m`/`g`/`g4`/`s` fallbacks), then `h` pixel rows.
//! Colours: `#rgb`, `#rrggbb`, `#rrrgggbbb`, `#rrrrggggbbbb`, `None`, X11 names.

mod x11;

use std::collections::HashMap;

use crate::*;

pub fn probe(head: &[u8]) -> bool {
    let s = &head[..head.len().min(512)];
    s.windows(9).any(|w| w == b"/* XPM */")
}

/// Iterate over the C string literals of the file (comments skipped, escapes kept raw).
struct Strings<'a> {
    d: &'a [u8],
    i: usize,
}

impl<'a> Iterator for Strings<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        let d = self.d;
        while self.i < d.len() {
            match d[self.i] {
                b'/' if d.get(self.i + 1) == Some(&b'*') => {
                    let end = d[self.i + 2..].windows(2).position(|w| w == b"*/")?;
                    self.i += end + 4;
                }
                b'"' => {
                    let start = self.i + 1;
                    let mut j = start;
                    while j < d.len() && d[j] != b'"' {
                        j += if d[j] == b'\\' { 2 } else { 1 };
                    }
                    let end = j.min(d.len());
                    self.i = end + 1;
                    return Some(&d[start..end]);
                }
                _ => self.i += 1,
            }
        }
        None
    }
}

struct Header {
    w: u32,
    h: u32,
    ncolors: usize,
    cpp: usize,
}

fn header(data: &[u8]) -> Result<(Header, Strings<'_>)> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let mut it = Strings { d: data, i: 0 };
    let first = it
        .next()
        .ok_or_else(|| Error::Truncated("no values line".into()))?;
    let v: Vec<u32> = String::from_utf8_lossy(first)
        .split_whitespace()
        .take(4)
        .map(|s| s.parse().unwrap_or(0))
        .collect();
    if v.len() < 4 {
        return Err(Error::Malformed("values line".into()));
    }
    check_dims(v[0], v[1])?;
    let (ncolors, cpp) = (v[2] as usize, v[3] as usize);
    if ncolors == 0 || !(1..=8).contains(&cpp) {
        return Err(Error::Malformed(format!(
            "{ncolors} colours, {cpp} chars/pixel"
        )));
    }
    // Each colour line takes at least cpp + 4 bytes.
    if ncolors > data.len() / (cpp + 4) {
        return Err(Error::Truncated("colour table".into()));
    }
    Ok((
        Header {
            w: v[0],
            h: v[1],
            ncolors,
            cpp,
        },
        it,
    ))
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (h, mut it) = header(data)?;
    let has_alpha = uses_none(&h, &mut it, data.len());
    Ok(Info {
        format: Format::Xpm,
        width: h.w,
        height: h.h,
        pixel_format: PixelFormat::Rgba8,
        has_alpha,
        model: crate::Model::Palette,
        bit_depth: 8,
        coding: format!("{} colours, {} chars/pixel", h.ncolors, h.cpp),
    })
}

/// Whether some pixel is transparent: the colour table has a `None` entry and a pixel row
/// uses it. Rows of files over [`ALPHA_SCAN_MAX`] are not read: the entry is enough.
fn uses_none(h: &Header, it: &mut Strings, len: usize) -> bool {
    let mut none: Vec<&[u8]> = Vec::new();
    for _ in 0..h.ncolors {
        // A short table fails to decode anyway.
        let Some(line) = it.next().filter(|l| l.len() >= h.cpp) else {
            return !none.is_empty();
        };
        let (k, rest) = line.split_at(h.cpp);
        if line_color(&String::from_utf8_lossy(rest))[3] == 0 {
            none.push(k);
        }
    }
    if none.is_empty() || len > ALPHA_SCAN_MAX {
        return !none.is_empty();
    }
    // Missing rows and pixels decode opaque grey.
    (0..h.h).map_while(|_| it.next()).any(|row| {
        row.chunks_exact(h.cpp)
            .take(h.w as usize)
            .any(|k| none.contains(&k))
    })
}

fn hex(s: &str) -> Option<[u8; 4]> {
    let s = s.strip_prefix('#')?;
    // Non-ASCII text would make the byte slicing below split a character.
    if s.is_empty() || !s.is_ascii() || !s.len().is_multiple_of(3) || s.len() > 12 {
        return None;
    }
    let n = s.len() / 3;
    let mut out = [0, 0, 0, 255];
    for (c, o) in out.iter_mut().take(3).enumerate() {
        let v = u32::from_str_radix(&s[c * n..(c + 1) * n], 16).ok()?;
        // Scale n hex digits to 8 bits.
        *o = match n {
            1 => (v * 17) as u8,
            2 => v as u8,
            _ => (v >> (4 * (n - 2))) as u8,
        };
    }
    Some(out)
}

/// Look up an X11 colour name (case and spaces ignored, `grey` = `gray`, `grayN`).
pub fn named_color(name: &str) -> Option<[u8; 3]> {
    let key: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    let key = key.replace("grey", "gray");
    if let Some(n) = key.strip_prefix("gray")
        && !n.is_empty()
        && let Ok(n) = n.parse::<u32>()
    {
        if n > 100 {
            return None;
        }
        let v = (n as f64 * 2.55 + 0.5) as u8;
        return Some([v, v, v]);
    }
    x11::X11_COLORS
        .binary_search_by(|(k, _)| (*k).cmp(key.as_str()))
        .ok()
        .map(|i| x11::X11_COLORS[i].1)
}

fn parse_color(spec: &str) -> Option<[u8; 4]> {
    let s = spec.trim();
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return Some([0, 0, 0, 0]);
    }
    if s.starts_with('#') {
        return hex(s);
    }
    named_color(s).map(|[r, g, b]| [r, g, b, 255])
}

/// Colour of one colour-line remainder: prefer the `c` visual, then `g`, `g4`, `m`.
fn line_color(rest: &str) -> [u8; 4] {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    let keys = ["c", "g", "g4", "m", "s"];
    let mut found: HashMap<&str, String> = HashMap::new();
    let mut i = 0;
    while i < toks.len() {
        if keys.contains(&toks[i]) && i + 1 < toks.len() {
            let key = toks[i];
            let mut j = i + 1;
            let mut val = toks[j].to_string();
            j += 1;
            while j < toks.len() && !keys.contains(&toks[j]) {
                val.push(' ');
                val.push_str(toks[j]);
                j += 1;
            }
            found.entry(key).or_insert(val);
            i = j;
        } else {
            i += 1;
        }
    }
    for k in ["c", "g", "g4", "m"] {
        if let Some(c) = found.get(k).and_then(|v| parse_color(v)) {
            return c;
        }
    }
    [0, 0, 0, 255]
}

pub fn decode(data: &[u8]) -> Result<Image> {
    let (hd, mut it) = header(data)?;
    let cpp = hd.cpp;
    let mut small: Vec<[u8; 4]> = Vec::new();
    let mut small_set: Vec<bool> = Vec::new();
    let mut big: HashMap<&[u8], [u8; 4]> = HashMap::new();
    if cpp <= 2 {
        small = vec![[0, 0, 0, 255]; 1 << (8 * cpp)];
        small_set = vec![false; 1 << (8 * cpp)];
    }
    let key_of = |k: &[u8]| k.iter().fold(0usize, |a, &b| a << 8 | b as usize);
    for _ in 0..hd.ncolors {
        let line = it
            .next()
            .ok_or_else(|| Error::Truncated("colour table".into()))?;
        if line.len() < cpp {
            return Err(Error::Malformed("colour line".into()));
        }
        let (k, rest) = line.split_at(cpp);
        let c = line_color(&String::from_utf8_lossy(rest));
        if cpp <= 2 {
            small[key_of(k)] = c;
            small_set[key_of(k)] = true;
        } else {
            big.insert(k, c);
        }
    }
    let (w, h) = (hd.w as usize, hd.h as usize);
    check_payload(data.len(), (w * h * cpp) as u64, 1)?;
    let mut img = Image::new(hd.w, hd.h, PixelFormat::Rgba8);
    for y in 0..h {
        let out = &mut img.data[y * w * 4..(y + 1) * w * 4];
        let row = it.next().unwrap_or(&[]);
        for (x, o) in out.chunks_exact_mut(4).enumerate() {
            let Some(k) = row.get(x * cpp..(x + 1) * cpp) else {
                o.copy_from_slice(&[GREY, GREY, GREY, 255]);
                img.truncated = true;
                continue;
            };
            let c = if cpp <= 2 {
                let i = key_of(k);
                if !small_set[i] {
                    img.truncated = true;
                }
                small[i]
            } else {
                big.get(k).copied().unwrap_or_else(|| {
                    img.truncated = true;
                    [0, 0, 0, 255]
                })
            };
            o.copy_from_slice(&c);
        }
    }
    Ok(img)
}
