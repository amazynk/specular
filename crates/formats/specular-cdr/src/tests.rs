use super::*;

fn chunk(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut c = id.to_vec();
    c.extend_from_slice(&(body.len() as u32).to_le_bytes());
    c.extend_from_slice(body);
    if body.len() % 2 == 1 {
        c.push(0);
    }
    c
}

fn list(kind: &[u8], items: &[Vec<u8>]) -> Vec<u8> {
    let mut b = kind.to_vec();
    for i in items {
        b.extend_from_slice(i);
    }
    chunk(b"LIST", &b)
}

fn riff(kind: &[u8], items: &[Vec<u8>]) -> Vec<u8> {
    let mut b = kind.to_vec();
    for i in items {
        b.extend_from_slice(i);
    }
    chunk(b"RIFF", &b)
}

/// A 2×2 4-bit DIB (BITMAPINFOHEADER, 16 colours).
fn dib() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&4u16.to_le_bytes());
    d.extend_from_slice(&[0; 24]);
    for i in 0..16u8 {
        d.extend_from_slice(&[i * 16, i, 255 - i, 0]);
    }
    d.extend_from_slice(&[0x12, 0, 0, 0, 0x34, 0, 0, 0]);
    d
}

#[test]
fn riff_versions_and_previews() {
    let mut disp = 8u32.to_le_bytes().to_vec();
    disp.extend_from_slice(&dib());
    let f = riff(
        b"CDRD",
        &[
            chunk(b"vrsn", &1300u16.to_le_bytes()),
            chunk(b"DISP", &disp),
            list(b"doc ", &[chunk(b"mcfg", &[0; 10])]),
        ],
    );
    assert!(probe(&f));
    let c = parse(&f).unwrap();
    assert_eq!(c.container, Container::Riff);
    assert_eq!(c.version, Some(1300));
    assert_eq!(version_name(1300), "CorelDRAW X3");
    let p = &c.previews[0];
    assert_eq!(p.image, ImageKind::Bmp);
    let bmp = p.data(&f);
    assert!(bmp.starts_with(b"BM"));
    // Pixel data offset: file header, info header, 16 colours.
    assert_eq!(le32(bmp, 10), Some(14 + 40 + 64));

    // Version 3: a BMP file in the page list's imhd chunk.
    let bmpfile = b"BM0123456789abcdef".to_vec();
    let v3 = riff(
        b"CDR ",
        &[
            chunk(b"vrsn", &300u16.to_le_bytes()),
            list(b"page", &[chunk(b"imhd", &bmpfile)]),
        ],
    );
    let c = parse(&v3).unwrap();
    assert_eq!(c.version, Some(300));
    assert_eq!(c.previews[0].data(&v3), &bmpfile[..]);

    // Lower-case type, no preview: refused with the reason.
    let none = riff(b"cdr8", &[chunk(b"vrsn", &800u16.to_le_bytes())]);
    assert!(probe(&none));
    assert!(matches!(parse(&none), Err(Error::Unsupported(s)) if s.contains("no preview")));
    assert!(
        matches!(parse(b"WLe\0\x1c\0\0\0"), Err(Error::Unsupported(s)) if s.contains("1 or 2"))
    );
    assert_eq!(parse(b"RIFF\0\0\0\0WAVE"), Err(Error::NotCdr));
}

#[test]
fn version_names() {
    assert_eq!(version_name(300), "CorelDRAW 3");
    assert_eq!(version_name(1200), "CorelDRAW 12");
    assert_eq!(version_name(1400), "CorelDRAW X4");
    assert_eq!(version_name(1800), "CorelDRAW X8");
    assert_eq!(version_name(1900), "CorelDRAW 2017");
    assert_eq!(version_name(2500), "CorelDRAW 2023");
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A stored-only ZIP of `members`.
fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in members {
        let off = out.len() as u32;
        let crc = crc32(body);
        let fixed = |sig: &[u8], central: bool| {
            let mut h = sig.to_vec();
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&[0; 8]);
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(body.len() as u32).to_le_bytes());
            h.extend_from_slice(&(body.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h
        };
        out.extend_from_slice(&fixed(b"PK\x03\x04", false));
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        central.extend_from_slice(&fixed(b"PK\x01\x02", true));
        central.extend_from_slice(&[0; 10]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    let n = (members.len() as u16).to_le_bytes();
    out.extend_from_slice(&n);
    out.extend_from_slice(&n);
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

#[test]
fn zip_packages() {
    let root = riff(b"CDRH", &[chunk(b"vrsn", &1700u16.to_le_bytes())]);
    let xmp = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"></x:xmpmeta>";
    let f = zip(&[
        ("mimetype", b"application/x-vnd.corel.zcf.draw.document+zip"),
        ("content/root.dat", &root),
        ("previews/thumbnail.png", b"T"),
        ("previews/page2.png", b"P2"),
        ("previews/page1.png", b"P1"),
        ("META-INF/metadata.xml", xmp),
    ]);
    assert!(!probe(&f) && probe_file(&f));
    let c = parse(&f).unwrap();
    assert_eq!(c.container, Container::Zip);
    assert_eq!(c.version, Some(1700));
    let kinds: Vec<_> = c
        .previews
        .iter()
        .map(|p| (p.kind, p.data(&f).to_vec()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (PreviewKind::Page(1), b"P1".to_vec()),
            (PreviewKind::Page(2), b"P2".to_vec()),
            (PreviewKind::Thumbnail, b"T".to_vec()),
        ]
    );
    assert_eq!(c.xmp.as_deref(), Some(&xmp[..]));

    // X4: BMP thumbnails beside riffData.cdr.
    let x4 = zip(&[
        ("content/riffData.cdr", &riff(b"CDRE", &[])),
        ("metadata/thumbnails/thumbnail.bmp", b"BMx"),
    ]);
    let c = parse(&x4).unwrap();
    assert_eq!(c.version, Some(1400));
    assert_eq!(c.previews[0].image, ImageKind::Bmp);
    // A package without previews is refused with the reason.
    let bare = zip(&[("content/riffData.cdr", &riff(b"CDRF", &[]))]);
    assert!(matches!(parse(&bare), Err(Error::Unsupported(_))));
    // Not CorelDRAW.
    assert!(!probe_file(&zip(&[("a.txt", b"x")])));
}

#[test]
fn hostile_files_fail_cleanly() {
    let mut disp = 8u32.to_le_bytes().to_vec();
    disp.extend_from_slice(&dib());
    let good = riff(
        b"CDR9",
        &[chunk(b"vrsn", &900u16.to_le_bytes()), chunk(b"DISP", &disp)],
    );
    for n in 0..good.len() {
        let _ = parse(&good[..n]);
    }
    let mut seed = 5u32;
    for _ in 0..3000 {
        let mut b = good.clone();
        for _ in 0..3 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let at = seed as usize % b.len();
            b[at] = (seed >> 8) as u8;
        }
        if let Ok(c) = parse(&b) {
            for p in &c.previews {
                let _ = p.data(&b);
            }
        }
    }
    // Many nested lists: bounded.
    let mut deep = chunk(b"vrsn", &[0, 0]);
    for _ in 0..50 {
        deep = list(b"page", &[deep]);
    }
    let f = riff(b"CDR7", &[deep]);
    assert!(parse(&f).is_err());
}

#[test]
fn corpus_files() {
    let dir = std::env::var_os("SPECULAR_EDITORS_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/editors")
        })
        .join("cdr");
    if !dir.is_dir() {
        eprintln!("CDR corpus absent: skipped");
        return;
    }
    let mut files = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cdr")) {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut shown, mut refused) = (0, std::collections::BTreeMap::<String, u32>::new());
    for f in &files {
        let d = std::fs::read(f).unwrap();
        assert!(probe_file(&d), "{}", f.display());
        match parse(&d) {
            Ok(c) => {
                assert!(!c.previews.is_empty());
                shown += 1;
            }
            Err(e) => *refused.entry(e.to_string()).or_default() += 1,
        }
    }
    eprintln!(
        "CDR corpus: {} files, {shown} with a preview; refused: {refused:?}",
        files.len()
    );
}

/// A fixed-Huffman DEFLATE stream of `1 + 258 * matches` zero bytes (13 bits per 258).
fn zeros_deflated(matches: usize) -> Vec<u8> {
    let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
    let mut bits = |v: u64, len: u32, out: &mut Vec<u8>| {
        acc |= v << n;
        n += len;
        while n >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            n -= 8;
        }
    };
    // Huffman codes are sent most significant bit first.
    let code = |c: u64, len: u32| (0..len).fold(0, |r, i| r | ((c >> i) & 1) << (len - 1 - i));
    bits(0b011, 3, &mut out); // Final block, fixed codes.
    bits(code(0x30, 8), 8, &mut out); // Literal 0.
    for _ in 0..matches {
        bits(code(0xC5, 8), 8, &mut out); // Length 258.
        bits(0, 5, &mut out); // Distance 1.
    }
    bits(0, 7, &mut out); // End of block.
    bits(0, 7, &mut out);
    out
}

/// Five deflated 60 MiB page previews (about 2 MB of file): only `MAX_PREVIEWS_HELD` bytes
/// of them are inflated and kept.
#[test]
fn inflated_previews_are_bounded() {
    let matches = (60 << 20) / 258;
    let size = (1 + 258 * matches) as u32;
    let body = zeros_deflated(matches);
    let root = riff(b"CDRH", &[chunk(b"vrsn", &1700u16.to_le_bytes())]);
    let names: Vec<String> = (1..=5).map(|i| format!("previews/page{i}.png")).collect();
    let mut members: Vec<(&str, &[u8])> = vec![("content/root.dat", &root)];
    members.extend(names.iter().map(|n| (n.as_str(), &body[..])));
    let mut f = zip(&members);
    // Mark the previews deflated, with their inflated size (local and central headers).
    for sig in [&b"PK\x03\x04"[..], &b"PK\x01\x02"[..]] {
        let mut at = 0;
        while let Some(p) = f[at..].windows(4).position(|w| w == sig) {
            let h = at + p + if sig[2] == 1 { 2 } else { 0 };
            let name_len = u16::from_le_bytes([f[h + 26], f[h + 27]]) as usize;
            let name_at = h + 30 + if sig[2] == 1 { 14 } else { 0 };
            if f[name_at..name_at + name_len].starts_with(b"previews/") {
                f[h + 8..h + 10].copy_from_slice(&8u16.to_le_bytes());
                f[h + 22..h + 26].copy_from_slice(&size.to_le_bytes());
            }
            at += p + 4;
        }
    }
    let c = parse(&f).unwrap();
    let held: usize = c.previews.iter().map(|p| p.data(&f).len()).sum();
    assert!(!c.previews.is_empty());
    assert!(held as u64 <= crate::MAX_PREVIEWS_HELD, "{held}");
    assert!(held as u64 > 3 * (60 << 20), "{held}");
}
