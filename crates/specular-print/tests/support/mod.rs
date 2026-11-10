//! Test-only helpers: a strict reader for the PDFs the writer makes (the oracle of the
//! structure tests), and a synthetic image source.

#![allow(dead_code)]

use std::collections::HashMap;

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_print::{ImageSource, Loaded};

/// One page as read back.
#[derive(Debug)]
pub struct ParsedPage {
    pub media_box: [f64; 4],
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub content: String,
}

#[derive(Debug)]
pub struct ParsedPdf {
    pub version: String,
    pub pages: Vec<ParsedPage>,
    pub icc: Vec<u8>,
    /// Device channels of the colour space (3 sRGB or RGB printer, 4 CMYK).
    pub channels: usize,
    pub intent: String,
    pub info: String,
}

struct Obj {
    dict: String,
    stream: Option<Vec<u8>>,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The value of `/key` in a dictionary's text: an array, a dictionary, a string, a
/// reference (`n 0 R`) or a single token.
pub fn get<'a>(dict: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("/{key}");
    let mut from = 0;
    let at = loop {
        let i = dict[from..].find(&pat)? + from;
        let end = i + pat.len();
        let next = dict[end..].chars().next().unwrap_or(' ');
        if !next.is_alphanumeric() {
            break end;
        }
        from = end;
    };
    let rest = dict[at..].trim_start();
    let start = dict.len() - rest.len();
    let len = if rest.starts_with("<<") || rest.starts_with('[') {
        let (open, close) = if rest.starts_with('[') {
            ("[", "]")
        } else {
            ("<<", ">>")
        };
        let mut depth = 0;
        let mut i = 0;
        loop {
            if rest[i..].starts_with(open) {
                depth += 1;
                i += open.len();
            } else if rest[i..].starts_with(close) {
                depth -= 1;
                i += close.len();
                if depth == 0 {
                    break i;
                }
            } else {
                i += rest[i..].chars().next()?.len_utf8();
            }
        }
    } else if rest.starts_with('(') {
        rest.find(')')? + 1
    } else {
        let toks: Vec<&str> = rest.split_whitespace().take(3).collect();
        // A token ends at the next delimiter (a name starts with its own '/').
        let tok = |s: &str| s[1..].find(['/', '>', ']', '[']).map_or(s.len(), |e| e + 1);
        if toks.len() == 3 && toks[2].starts_with('R') && toks[1] == "0" {
            rest.find(" R").unwrap() + 2
        } else {
            tok(toks.first()?)
        }
    };
    Some(&dict[start..start + len])
}

fn reference(v: &str) -> u32 {
    let mut it = v.split_whitespace();
    let n = it.next().unwrap().parse().unwrap();
    assert_eq!(it.next(), Some("0"), "generation in {v}");
    assert_eq!(it.next(), Some("R"), "reference {v}");
    n
}

/// Reads `bytes` as a PDF, checking the structure on the way: the header, the
/// cross-reference table against the real object offsets, every stream's length against
/// its `endstream`, the page tree, and the decompressed size of every image.
pub fn parse(bytes: &[u8]) -> ParsedPdf {
    assert!(bytes.starts_with(b"%PDF-1."), "header");
    let version = String::from_utf8_lossy(&bytes[5..8]).into_owned();
    assert!(bytes.ends_with(b"%%EOF\n"), "trailer end");
    let sx = bytes
        .windows(9)
        .rposition(|w| w == b"startxref")
        .expect("startxref");
    let tail = std::str::from_utf8(&bytes[sx..]).unwrap();
    let xref: usize = tail.lines().nth(1).unwrap().trim().parse().unwrap();
    let table = std::str::from_utf8(&bytes[xref..sx]).expect("ASCII xref");
    let mut lines = table.lines();
    assert_eq!(lines.next(), Some("xref"));
    let sub: Vec<usize> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(sub[0], 0, "one subsection from object 0");
    let n = sub[1];
    let mut offsets = Vec::new();
    for i in 0..n {
        let l = lines.next().unwrap();
        assert_eq!(l.len(), 19, "xref entries are 20 bytes with the EOL: {l:?}");
        let off: usize = l[..10].parse().unwrap();
        if i == 0 {
            assert!(l.ends_with(" 65535 f "));
            continue;
        }
        assert!(l.ends_with(" 00000 n "), "{l}");
        offsets.push(off);
    }
    assert_eq!(lines.next(), Some("trailer"));
    let trailer: String = lines.collect::<Vec<_>>().join(" ");
    assert_eq!(get(&trailer, "Size").unwrap().parse::<usize>().unwrap(), n);
    let root = reference(get(&trailer, "Root").expect("Root"));
    let info_ref = reference(get(&trailer, "Info").expect("Info"));

    // Objects, with stream lengths resolved in a second step (indirect lengths come later).
    let mut raw: HashMap<u32, (String, Option<usize>)> = HashMap::new();
    for (i, &off) in offsets.iter().enumerate() {
        let id = i as u32 + 1;
        let head = format!("{id} 0 obj\n");
        assert_eq!(
            &bytes[off..off + head.len()],
            head.as_bytes(),
            "object {id} at its xref offset"
        );
        let body = off + head.len();
        let s = find(bytes, b"stream\n", body);
        let e = find(bytes, b"endobj", body).unwrap();
        match s {
            Some(s) if s < e => {
                let dict = std::str::from_utf8(&bytes[body..s])
                    .unwrap()
                    .trim()
                    .to_owned();
                raw.insert(id, (dict, Some(s + 7)));
            }
            _ => {
                let dict = std::str::from_utf8(&bytes[body..e])
                    .unwrap()
                    .trim()
                    .to_owned();
                raw.insert(id, (dict, None));
            }
        }
    }
    let mut objs: HashMap<u32, Obj> = HashMap::new();
    for (&id, (dict, start)) in &raw {
        let stream = start.map(|s| {
            let lv = get(dict, "Length").expect("stream Length");
            let len: usize = if lv.ends_with('R') {
                raw[&reference(lv)].0.parse().unwrap()
            } else {
                lv.parse().unwrap()
            };
            let end = s + len;
            assert!(
                bytes[end..].starts_with(b"\nendstream") || bytes[end..].starts_with(b"endstream"),
                "object {id}: endstream right after /Length bytes"
            );
            let data = &bytes[s..end];
            if get(dict, "Filter") == Some("/FlateDecode") {
                miniz_oxide::inflate::decompress_to_vec_zlib(data).expect("valid zlib")
            } else {
                data.to_vec()
            }
        });
        objs.insert(
            id,
            Obj {
                dict: dict.clone(),
                stream,
            },
        );
    }

    let catalog = &objs[&root].dict;
    assert_eq!(get(catalog, "Type"), Some("/Catalog"));
    let intents = get(catalog, "OutputIntents").expect("output intent");
    let intent_ref = reference(intents.trim_matches(|c| c == '[' || c == ']'));
    let intent = objs[&intent_ref].dict.clone();
    assert_eq!(get(&intent, "S"), Some("/GTS_PDFA1"));
    let icc_ref = reference(get(&intent, "DestOutputProfile").unwrap());
    let icc_obj = &objs[&icc_ref];
    let channels: usize = get(&icc_obj.dict, "N").unwrap().parse().unwrap();
    assert!(matches!(channels, 1 | 3 | 4), "N {channels}");
    let icc = icc_obj.stream.clone().unwrap();

    let pages_ref = reference(get(catalog, "Pages").unwrap());
    let tree = &objs[&pages_ref].dict;
    assert_eq!(get(tree, "Type"), Some("/Pages"));
    let kids = get(tree, "Kids").unwrap();
    let kids: Vec<&str> = kids
        .trim_matches(|c| c == '[' || c == ']')
        .split(" R")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        get(tree, "Count").unwrap().parse::<usize>().unwrap(),
        kids.len()
    );
    let mut pages = Vec::new();
    for k in kids {
        let pid = reference(&format!("{k} R"));
        let page = &objs[&pid].dict;
        assert_eq!(get(page, "Type"), Some("/Page"));
        assert_eq!(reference(get(page, "Parent").unwrap()), pages_ref);
        let mb: Vec<f64> = get(page, "MediaBox")
            .unwrap()
            .trim_matches(|c| c == '[' || c == ']')
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let res = get(page, "Resources").unwrap();
        let xo = get(res, "XObject").unwrap();
        let im = reference(get(xo, "Im0").unwrap());
        let img = &objs[&im];
        assert_eq!(get(&img.dict, "Subtype"), Some("/Image"));
        assert_eq!(get(&img.dict, "BitsPerComponent"), Some("8"));
        let cs = get(&img.dict, "ColorSpace").unwrap();
        assert!(cs.starts_with("[/ICCBased"), "{cs}");
        assert_eq!(
            reference(
                cs.trim_matches(|c| c == '[' || c == ']')
                    .trim_start_matches("/ICCBased")
            ),
            icc_ref
        );
        let w: u32 = get(&img.dict, "Width").unwrap().parse().unwrap();
        let h: u32 = get(&img.dict, "Height").unwrap().parse().unwrap();
        let rgb = img.stream.clone().unwrap();
        assert_eq!(rgb.len(), w as usize * h as usize * channels, "image size");
        let content = objs[&reference(get(page, "Contents").unwrap())]
            .stream
            .clone()
            .unwrap();
        pages.push(ParsedPage {
            media_box: [mb[0], mb[1], mb[2], mb[3]],
            width: w,
            height: h,
            rgb,
            content: String::from_utf8(content).unwrap(),
        });
    }
    ParsedPdf {
        version,
        pages,
        icc,
        channels,
        intent,
        info: objs[&info_ref].dict.clone(),
    }
}

/// `qpdf --check` as a second opinion when qpdf is installed (true when it is not).
pub fn qpdf_ok(path: &std::path::Path) -> bool {
    match std::process::Command::new("qpdf")
        .arg("--check")
        .arg(path)
        .output()
    {
        Ok(o) => {
            if !o.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&o.stdout));
                eprintln!("{}", String::from_utf8_lossy(&o.stderr));
            }
            o.status.success()
        }
        Err(_) => {
            eprintln!("qpdf not installed; skipping its check");
            true
        }
    }
}

/// A test image: quadrants of four colours (top-left red, top-right green, bottom-left
/// blue, bottom-right yellow, in stored orientation) with a soft diagonal gradient.
#[derive(Clone, Debug)]
pub struct Synthetic {
    pub size: (u32, u32),
    pub orientation: Orientation,
    /// Alpha in the right half (to show that transparency prints over white).
    pub alpha: bool,
}

impl Synthetic {
    pub fn raster(&self) -> Raster {
        let (w, h) = self.size;
        let mut data = Vec::with_capacity(w as usize * h as usize * 4);
        for y in 0..h {
            for x in 0..w {
                let right = x * 2 >= w;
                let bottom = y * 2 >= h;
                let shade = ((x + y) * 60 / (w + h)) as u8;
                let c: [u8; 3] = match (right, bottom) {
                    (false, false) => [220, 40 + shade, 40],
                    (true, false) => [40, 190 + shade / 2, 60],
                    (false, true) => [40, 70, 200 + shade / 2],
                    (true, true) => [230, 200, 30 + shade],
                };
                let a = if self.alpha && right { 96 } else { 255 };
                data.extend_from_slice(&[c[0], c[1], c[2], a]);
            }
        }
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        }
    }

    pub fn display_size(&self) -> (u32, u32) {
        self.orientation.display_size(self.size.0, self.size.1)
    }
}

/// Synthetic images; `None` items fail to load.
#[derive(Debug, Default)]
pub struct SyntheticSource {
    pub images: Vec<Option<Synthetic>>,
}

impl ImageSource for SyntheticSource {
    fn key(&self, index: usize) -> u64 {
        index as u64 + 1
    }

    fn load(&self, index: usize, _long_px: u32) -> Result<Loaded, String> {
        let s = self
            .images
            .get(index)
            .cloned()
            .flatten()
            .ok_or("unreadable test image")?;
        Ok(Loaded {
            raster: s.raster(),
            stored: s.size,
            orientation: s.orientation,
            lut: None,
        })
    }
}

/// A mixed set of `n` images: landscape, portrait (by EXIF orientation), square, wide, and
/// one in nine with transparency.
pub fn mixed(n: usize) -> Vec<Option<Synthetic>> {
    (0..n)
        .map(|i| {
            let (size, o) = match i % 4 {
                0 => ((300, 200), 1),
                1 => ((300, 200), 6),
                2 => ((160, 160), 1),
                _ => ((360, 120), 3),
            };
            Some(Synthetic {
                size,
                orientation: Orientation(o),
                alpha: i % 9 == 4,
            })
        })
        .collect()
}
