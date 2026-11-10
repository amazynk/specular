//! Specular's own reader of the codes printed in pictures: QR codes (ISO/IEC 18004,
//! versions 1–40, every level and mask, ECI character sets, kanji, structured append and
//! GS1), Data Matrix ECC 200 (ISO/IEC 16022, every square and rectangular size and
//! encodation, see [`dm`]) and Aztec codes (ISO/IEC 24778, compact, full-range and runes,
//! see [`aztec`]), found at any rotation and under perspective, mirrored or light on dark.
//!
//! The pipeline: grey → local-threshold binarisation (`image`), which also gathers each
//! row's dark runs → QR: finder patterns from those runs and their triples → bottom-right
//! alignment pattern → perspective grid → format and version words (BCH, nearest code
//! word) → unmasking and codeword order → Reed–Solomon (`rs`) → segments. Aztec and Data
//! Matrix are looked for after QR in the same binary picture and runs, leaving out what
//! codes already read cover. Large pictures are also searched at half size, where blur and
//! noise matter less.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

pub mod aztec;
pub mod dm;
pub mod image;
pub mod qr;
mod rs;

pub use image::Gray;

/// A code found in a picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Code {
    /// "QR", "Data Matrix", "Aztec" or "Aztec rune".
    pub kind: &'static str,
    /// QR version (1..=40); Aztec data layers; 0 for Data Matrix and runes.
    pub version: usize,
    /// QR error correction level ("L", "M", "Q", "H"); "compact" for compact Aztec
    /// symbols; empty otherwise.
    pub level: &'static str,
    /// Data Matrix and Aztec rows and columns.
    pub size: Option<(usize, usize)>,
    pub text: String,
    pub bytes: Vec<u8>,
    /// The outline in image pixels: the symbol's top-left, top-right, bottom-right and
    /// bottom-left corners as printed.
    pub corners: [(f32, f32); 4],
    /// Seen from behind (mirror image).
    pub mirrored: bool,
    /// Light modules on a dark ground.
    pub inverted: bool,
    /// Bytes put right by error correction.
    pub corrected: usize,
    /// Structured append: this part's position and the number of parts.
    pub part: Option<(u8, u8)>,
    pub gs1: bool,
}

impl Code {
    /// The link the text is, if it is one (http or https, no spaces or controls).
    pub fn url(&self) -> Option<&str> {
        let t = self.text.trim();
        let lower = t.to_ascii_lowercase();
        let ok = (lower.starts_with("https://") || lower.starts_with("http://"))
            && !t.chars().any(|c| c.is_whitespace() || c.is_control());
        ok.then_some(t)
    }

    /// The host of the link, for the confirmation before opening it.
    pub fn host(&self) -> Option<&str> {
        let u = self.url()?;
        let rest = &u[u.find("://")? + 3..];
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let auth = &rest[..end];
        // drop user info; keep the port
        let host = auth.rsplit('@').next().unwrap_or(auth);
        (!host.is_empty()).then_some(host)
    }
}

/// Every code in one binarised picture (`scale` pixels of the picture a pixel of `b`) not
/// already in `out`. QR codes first: their finder patterns are cheap to find, and the
/// costlier outline search for Data Matrix then leaves out what they cover.
fn scan_binary(
    b: &image::Binary,
    runs: &image::Runs,
    grey: &Gray,
    scale: f32,
    inverted: bool,
    dm: bool,
    out: &mut Vec<Code>,
) -> bool {
    for s in qr::find(b, runs, Some((grey, inverted))) {
        let code = Code {
            kind: "QR",
            version: s.version,
            level: s.level.name(),
            size: None,
            text: s.content.text.clone(),
            bytes: s.content.bytes.clone(),
            corners: s.corners.map(|(x, y)| (x * scale, y * scale)),
            mirrored: s.mirrored,
            inverted,
            corrected: s.corrected,
            part: s.content.structured.map(|(p, t, _)| (p, t)),
            gs1: s.content.gs1,
        };
        push(out, code, scale);
    }
    scan_aztec(b, runs, scale, inverted, out);
    dm && scan_data_matrix(b, runs, grey, scale, inverted, out)
}

/// The quadrilaterals of the codes in `out`, in the pixels of a binary picture at `scale`.
fn taken(out: &[Code], scale: f32) -> Vec<[(f64, f64); 4]> {
    out.iter()
        .map(|c| {
            c.corners
                .map(|(x, y)| (f64::from(x / scale), f64::from(y / scale)))
        })
        .collect()
}

/// Aztec symbols, as `scan_binary`.
fn scan_aztec(
    b: &image::Binary,
    runs: &image::Runs,
    scale: f32,
    inverted: bool,
    out: &mut Vec<Code>,
) {
    for s in aztec::find(b, runs, &taken(out, scale)) {
        let code = Code {
            kind: if s.rune.is_some() {
                "Aztec rune"
            } else {
                "Aztec"
            },
            version: s.layers,
            level: if s.compact { "compact" } else { "" },
            size: Some((s.size, s.size)),
            text: s.content.text.clone(),
            bytes: s.content.bytes.clone(),
            corners: s.corners.map(|(x, y)| (x * scale, y * scale)),
            mirrored: s.mirrored,
            inverted,
            corrected: s.corrected,
            part: s.content.structured,
            gs1: s.content.gs1,
        };
        push(out, code, scale);
    }
}

fn push(out: &mut Vec<Code>, code: Code, scale: f32) {
    let dup = out.iter().any(|c| {
        c.text == code.text && {
            let (a, b) = (centre(&c.corners), centre(&code.corners));
            (a.0 - b.0).hypot(a.1 - b.1) < 16.0 * scale
        }
    });
    if !dup {
        out.push(code);
    }
}

/// Data Matrix symbols, as `scan_binary`; whether anything looked like one.
fn scan_data_matrix(
    b: &image::Binary,
    runs: &image::Runs,
    grey: &Gray,
    scale: f32,
    inverted: bool,
    out: &mut Vec<Code>,
) -> bool {
    let (found, seen) = dm::find(b, runs, grey, &taken(out, scale));
    for s in found {
        let code = Code {
            kind: "Data Matrix",
            version: 0,
            level: "",
            size: Some((s.size.rows, s.size.cols)),
            text: s.content.text.clone(),
            bytes: s.content.bytes.clone(),
            corners: s.corners.map(|(x, y)| (x * scale, y * scale)),
            mirrored: s.mirrored,
            inverted,
            corrected: s.corrected,
            part: s.content.structured,
            gs1: s.content.gs1,
        };
        push(out, code, scale);
    }
    seen
}

fn centre(c: &[(f32, f32); 4]) -> (f32, f32) {
    let x = c.iter().map(|p| p.0).sum::<f32>() / 4.0;
    let y = c.iter().map(|p| p.1).sum::<f32>() / 4.0;
    (x, y)
}

/// Every code found in a grey picture, in reading order (top to bottom, then left to right).
pub fn scan(g: &Gray) -> Vec<Code> {
    let mut out = Vec::new();
    // Each binarised copy is searched as it is and, if nothing turned up, inverted.
    // `grey`: the picture `b` was binarised from (Data Matrix reads noisy modules from it).
    let search =
        |b: &image::Binary, runs: &image::Runs, grey: &Gray, scale, dm, out: &mut Vec<Code>| {
            let mut seen = scan_binary(b, runs, grey, scale, false, dm, out);
            if out.is_empty() {
                let inv = (&b.inverted(), &runs.inverted(b.w));
                seen |= scan_binary(inv.0, inv.1, grey, scale, true, dm, out);
            }
            seen
        };
    let (b, runs) = image::binarize_runs(g);
    // Whether anything looked like a Data Matrix symbol at full size: if not, the smaller
    // copies are not searched for one either.
    let mut dm = search(&b, &runs, g, 1.0, true, &mut out);
    if out.is_empty() {
        // noisy: smoothed first
        let s = g.smoothed();
        let (sb, sruns) = image::binarize_runs(&s);
        dm |= search(&sb, &sruns, &s, 1.0, true, &mut out);
    }
    // half size: blurred, noisy or very large pictures
    let mut half = g.clone();
    let mut scale = 1.0;
    while half.w >= 160 && half.h >= 160 && scale < 8.0 {
        half = half.half();
        scale *= 2.0;
        let (hb, hruns) = image::binarize_runs(&half);
        let before = out.len();
        search(&hb, &hruns, &half, scale, dm, &mut out);
        if out.len() == before && (!out.is_empty() || half.w < 400 || half.h < 400) {
            break;
        }
    }
    out.sort_by(|a, b| {
        let (ca, cb) = (centre(&a.corners), centre(&b.corners));
        (ca.1 as i32 / 32, ca.0 as i32).cmp(&(cb.1 as i32 / 32, cb.0 as i32))
    });
    out
}

/// Every code in an RGBA8 picture (transparent pixels count as white).
pub fn scan_rgba(w: usize, h: usize, rgba: &[u8]) -> Vec<Code> {
    scan(&Gray::from_rgba(w, h, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(text: &str) -> Code {
        Code {
            kind: "QR",
            version: 1,
            level: "M",
            size: None,
            text: text.into(),
            bytes: text.as_bytes().to_vec(),
            corners: [(0.0, 0.0); 4],
            mirrored: false,
            inverted: false,
            corrected: 0,
            part: None,
            gs1: false,
        }
    }

    /// A picture of a symbol: `module` px a module, turned by `angle` radians, on white.
    fn picture(g: &qr::matrix::Grid, module: f32, angle: f32) -> Gray {
        let n = g.size as f32;
        let side = ((n + 8.0) * module * 1.5) as usize;
        let c = side as f32 / 2.0;
        let (s, co) = angle.sin_cos();
        let mut px = vec![255u8; side * side];
        for y in 0..side {
            for x in 0..side {
                // back into symbol coordinates (modules), centred
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                let u = (co * dx + s * dy) / module + n / 2.0;
                let v = (-s * dx + co * dy) / module + n / 2.0;
                if u >= 0.0 && v >= 0.0 && u < n && v < n && g.get(u as usize, v as usize) {
                    px[y * side + x] = 20;
                }
            }
        }
        Gray {
            w: side,
            h: side,
            px,
        }
    }

    fn symbol(text: &str, version: usize, level: qr::Level, mask: u8) -> qr::matrix::Grid {
        let mut w = qr::content::Writer::new();
        w.put(4, 4);
        w.put(text.len() as u32, qr::tables::count_bits(version)[2]);
        for b in text.bytes() {
            w.put(u32::from(b), 8);
        }
        let cap: usize = qr::tables::blocks(version, level)
            .iter()
            .map(|b| b.data)
            .sum();
        qr::matrix::draw(version, level, mask, &w.finish(cap))
    }

    /// A symbol on a bent label: columns squeezed towards both sides (as round a bottle),
    /// rows even. Read with the columns where the timing pattern puts them, the down timing
    /// pattern read in its moved column, and the finders' columns on the bend's curve.
    #[test]
    fn reads_bent_symbols() {
        // One homography reads none (the timing patterns fail its check).
        for (version, bend) in [
            (7usize, 2.5f32),
            (7, 3.5),
            (7, 4.5),
            (4, 3.0),
            (4, 4.0),
            (2, 4.5),
            (5, 5.0),
        ] {
            let g = symbol("https://ex.org/bent", version, qr::Level::M, 3);
            let found = scan(&bent(&g, 4.0, bend));
            assert_eq!(found.len(), 1, "version {version}, bent {bend}");
            assert_eq!(found[0].text, "https://ex.org/bent");
        }
    }

    /// A symbol bent as round a bottle: the outer columns pushed out by `bend` modules
    /// (image x from symbol u, both centred: u + a·u³, inverted by a few Newton steps).
    fn bent(g: &qr::matrix::Grid, module: f32, bend: f32) -> Gray {
        let n = g.size as f32;
        let side = ((n + 8.0) * module) as usize;
        let c = side as f32 / 2.0;
        let a = bend / (n / 2.0).powi(3);
        let mut px = vec![255u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let xs = (x as f32 + 0.5 - c) / module;
                let mut u = xs;
                for _ in 0..8 {
                    u -= (u + a * u * u * u - xs) / (1.0 + 3.0 * a * u * u);
                }
                let u = u + n / 2.0;
                let v = (y as f32 + 0.5 - c) / module + n / 2.0;
                if u >= 0.0 && v >= 0.0 && u < n && v < n && g.get(u as usize, v as usize) {
                    px[y * side + x] = 20;
                }
            }
        }
        Gray {
            w: side,
            h: side,
            px,
        }
    }

    #[test]
    #[ignore = "a study aid"]
    fn bend_sweep() {
        for version in [2usize, 3, 4, 5, 7] {
            for bend in [2.0f32, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0] {
                let g = symbol("https://ex.org/bent", version, qr::Level::M, 3);
                let p = bent(&g, 4.0, bend);
                let (b, runs) = image::binarize_runs(&p);
                let f = qr::detect::finders(&b, &runs);
                let ok = scan(&p).iter().any(|c| c.text == "https://ex.org/bent");
                if !ok && std::env::var_os("SPECULAR_QR_DEBUG").is_some() {
                    let big: Vec<_> = f.iter().filter(|f| f.count >= 12).copied().collect();
                    if let Some((fs, dim)) = qr::detect::arrange(&b, &big[0], &big[1], &big[2]) {
                        for d in [dim, dim + 4, dim.saturating_sub(4)] {
                            for align in [true, false] {
                                let loc = qr::detect::place(&b, &fs[0], &fs[1], &fs[2], d, align);
                                let dbg = loc.as_ref().map(|l| qr::detect::debug_timed(&b, l));
                                eprintln!("  dim {d} (est {dim}) align {align}: {dbg:?}");
                            }
                        }
                    }
                }
                eprintln!(
                    "version {version} bend {bend}: {} finders {:?}, read {ok}",
                    f.len(),
                    f.iter()
                        .map(|f| (f.x as i32, f.y as i32, f.module, f.count))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    #[ignore = "a study aid: SPECULAR_QR_ONE=<file.pgm>"]
    fn one_photo() {
        let Some(path) = std::env::var_os("SPECULAR_QR_ONE") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        // binary PGM: P5 w h 255
        let mut parts = data.splitn(4, |c| c.is_ascii_whitespace());
        let _ = parts.next();
        let w: usize = std::str::from_utf8(parts.next().unwrap())
            .unwrap()
            .parse()
            .unwrap();
        let h: usize = std::str::from_utf8(parts.next().unwrap())
            .unwrap()
            .parse()
            .unwrap();
        let rest = parts.next().unwrap();
        let px = rest[rest.len() - w * h..].to_vec();
        let g = Gray { w, h, px };
        for (name, g) in [("full", g.clone()), ("smoothed", g.smoothed())] {
            let (b, runs) = image::binarize_runs(&g);
            let f = qr::detect::finders(&b, &runs);
            eprintln!(
                "{name}: {} finders {:?}",
                f.len(),
                f.iter()
                    .map(|f| (f.x as i32, f.y as i32, f.module, f.count))
                    .collect::<Vec<_>>()
            );
            let mut big = f.clone();
            big.sort_by_key(|f| std::cmp::Reverse(f.count));
            if big.len() >= 3
                && let Some((fs, dim)) = qr::detect::arrange(&b, &big[0], &big[1], &big[2])
            {
                for d in [dim, dim + 4, dim.saturating_sub(4)] {
                    for align in [true, false] {
                        let loc = qr::detect::place(&b, &fs[0], &fs[1], &fs[2], d, align);
                        let dbg = loc.as_ref().map(|l| qr::detect::debug_timed(&b, l));
                        let plain =
                            qr::detect::locate(&b, &fs[0], &fs[1], &fs[2], d, align).map(|l| {
                                qr::matrix::read(&l.grid)
                                    .map(|_| ())
                                    .map_err(|e| format!("{e:?}"))
                            });
                        eprintln!(
                            "  dim {d} (est {dim}) align {align}: timed {dbg:?}, plain {plain:?}"
                        );
                    }
                }
            }
        }
        eprintln!(
            "read: {:?}",
            scan(&g).iter().map(|c| c.text.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn finds_turned_symbols() {
        for (version, level, angle) in [
            (1, qr::Level::M, 0.0f32),
            (2, qr::Level::H, 0.4),
            (7, qr::Level::Q, 2.0),
            (10, qr::Level::L, 4.0),
        ] {
            let g = symbol("https://ex.org", version, level, version as u8 % 8);
            let p = picture(&g, 4.0, angle);
            let found = scan(&p);
            assert_eq!(found.len(), 1, "version {version} at {angle}");
            assert_eq!(found[0].text, "https://ex.org");
            assert_eq!(found[0].version, version);
            assert_eq!(found[0].host(), Some("ex.org"));
        }
    }

    /// A small symbol in strong perspective and slightly bent, as a close photo of a label:
    /// read through its outline (the alignment pattern found from the outer edges).
    #[test]
    fn finds_symbols_in_strong_perspective() {
        let g = symbol("https://example.org/a/long/enough/path", 6, qr::Level::M, 3);
        let n = g.size as f64;
        let to_symbol = image::Homography::quad_to_quad(
            [(56.0, 78.0), (177.0, 31.0), (220.0, 191.0), (93.0, 214.0)],
            [(0.0, 0.0), (n, 0.0), (n, n), (0.0, n)],
        );
        let side = 240;
        let mut px = vec![235u8; side * side];
        for y in 0..side {
            for x in 0..side {
                // four samples a pixel, for soft edges
                let mut dark = 0;
                for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (u, v) = to_symbol.map(x as f64 + ox, y as f64 + oy);
                    // a slight bend, as of a label on a curved surface
                    let bend = 0.4;
                    let (u, v) = (
                        u + bend * 4.0 * (v / n - 0.5).powi(2),
                        v + bend * 4.0 * (u / n - 0.5).powi(2),
                    );
                    if u >= 0.0 && v >= 0.0 && u < n && v < n && g.get(u as usize, v as usize) {
                        dark += 1;
                    }
                }
                px[y * side + x] = 235 - dark * 50;
            }
        }
        let p = Gray {
            w: side,
            h: side,
            px,
        }
        .smoothed()
        .smoothed();
        let found = scan(&p);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "https://example.org/a/long/enough/path");
    }

    #[test]
    fn finds_mirrored_and_inverted_symbols() {
        let g = symbol("mirror", 3, qr::Level::M, 2);
        let found = scan(&picture(&g.transposed(), 3.0, 0.3));
        assert_eq!(
            found.first().map(|c| (c.text.as_str(), c.mirrored)),
            Some(("mirror", true))
        );
        let mut p = picture(&g, 3.0, 0.0);
        for v in &mut p.px {
            *v = 255 - *v;
        }
        let found = scan(&p);
        assert_eq!(
            found.first().map(|c| (c.text.as_str(), c.inverted)),
            Some(("mirror", true))
        );
    }

    /// A picture of a Data Matrix grid: `module` px a module, turned by `angle`, on white.
    fn dm_picture(g: &dm::Grid, module: f32, angle: f32, mirror: bool) -> Gray {
        let (nr, nc) = (g.rows as f32, g.cols as f32);
        let side = ((nr.max(nc) + 8.0) * module * 1.5) as usize;
        let c = side as f32 / 2.0;
        let (s, co) = angle.sin_cos();
        let mut px = vec![255u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                let mut u = (co * dx + s * dy) / module + nc / 2.0;
                let v = (-s * dx + co * dy) / module + nr / 2.0;
                if mirror {
                    u = nc - u;
                }
                if u >= 0.0 && v >= 0.0 && u < nc && v < nr && g.get(v as usize, u as usize) {
                    px[y * side + x] = 20;
                }
            }
        }
        Gray {
            w: side,
            h: side,
            px,
        }
    }

    #[test]
    fn finds_data_matrix_symbols() {
        let cases: Vec<(String, bool, f32, f32, bool)> = vec![
            ("123456".into(), false, 4.0, 0.0, false),
            ("Specular reads Data Matrix".into(), false, 3.0, 0.7, false),
            ("rect".into(), true, 4.0, 2.5, false),
            (
                "A longer rectangular one, 16x48".into(),
                true,
                3.0,
                4.0,
                true,
            ),
            ("x".repeat(250), false, 2.5, 1.2, false),
        ];
        for (text, rect, module, angle, mirror) in cases {
            let cw = dm::ascii(text.as_bytes());
            let size = dm::smallest(cw.len(), rect).unwrap();
            let g = dm::draw(&size, &cw);
            let found = scan(&dm_picture(&g, module, angle, mirror));
            assert_eq!(found.len(), 1, "{} at {angle}", size.name());
            assert_eq!(found[0].text, text);
            assert_eq!(found[0].size, Some((size.rows, size.cols)));
            assert_eq!(found[0].mirrored, mirror);
        }
    }

    /// Small modules under heavy noise: read from each module's mean grey, with the outline
    /// moved onto the timing pattern.
    #[test]
    fn reads_noisy_data_matrix() {
        let text = "Noise of sigma 50 on 2 px modules, 0123456789".repeat(2);
        let cw = dm::ascii(text.as_bytes());
        let size = dm::smallest(cw.len(), false).unwrap();
        let mut pic = dm_picture(&dm::draw(&size, &cw), 2.0, 0.35, false);
        // Gaussian-like noise (a sum of four uniforms), σ ≈ 50, the same each run; turned.
        let mut s = 16022u32;
        let mut uni = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f32::from((s >> 16) as u16) / 65535.0 - 0.5
        };
        for p in &mut pic.px {
            let n = (uni() + uni() + uni() + uni()) * 50.0 / (4.0f32 / 12.0).sqrt();
            *p = (f32::from(*p) + n).clamp(0.0, 255.0) as u8;
        }
        let found = scan(&pic);
        assert_eq!(found.len(), 1, "{}", size.name());
        assert_eq!(found[0].text, text);
    }

    /// A picture of an Aztec grid: `module` px a module, turned by `angle`, on white.
    fn az_picture(g: &aztec::Grid, module: f32, angle: f32, mirror: bool) -> Gray {
        let n = g.size as f32;
        let side = ((n + 8.0) * module * 1.5) as usize;
        let c = side as f32 / 2.0;
        let (s, co) = angle.sin_cos();
        let mut px = vec![255u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                let mut u = (co * dx + s * dy) / module + n / 2.0;
                let v = (-s * dx + co * dy) / module + n / 2.0;
                if mirror {
                    u = n - u;
                }
                if u >= 0.0 && v >= 0.0 && u < n && v < n && g.get(u as usize, v as usize) {
                    px[y * side + x] = 20;
                }
            }
        }
        Gray {
            w: side,
            h: side,
            px,
        }
    }

    #[test]
    fn finds_aztec_symbols() {
        let long: String = (0..400)
            .map(|i| (b'A' + (i * 7 % 26) as u8) as char)
            .collect();
        let cases: Vec<(String, bool, f32, f32, bool)> = vec![
            ("HELLO AZTEC".into(), true, 4.0, 0.0, false),
            ("Turned, with bytes: é ß".into(), true, 3.0, 0.7, false),
            ("MIRRORED".into(), true, 4.0, 2.5, true),
            (long, false, 3.0, 4.1, false),
        ];
        for (text, compact_ok, module, angle, mirror) in cases {
            let bits = aztec::draw::simple_bits(text.as_bytes());
            let (compact, layers, words) = aztec::draw::fit(&bits, compact_ok);
            let g = aztec::draw::grid(compact, layers, &words, None);
            let found = scan(&az_picture(&g, module, angle, mirror));
            assert_eq!(found.len(), 1, "{text} at {angle}");
            assert_eq!(found[0].kind, "Aztec");
            assert_eq!(found[0].text, text);
            assert_eq!(found[0].size, Some((g.size, g.size)));
            assert_eq!(found[0].version, layers);
            assert_eq!(found[0].mirrored, mirror);
        }
        // a full-range symbol big enough for the reference grid's crossings
        assert!(aztec::size_of(false, 8) > 37);
    }

    #[test]
    fn finds_aztec_runes_and_parts() {
        let g = aztec::draw::grid(true, 0, &[], Some(42));
        let found = scan(&az_picture(&g, 5.0, 1.0, false));
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].kind, found[0].text.as_str()),
            ("Aztec rune", "42")
        );
        assert_eq!(found[0].size, Some((11, 11)));
        // structured append: M/L U/L, then "BC" (part 2 of 3)
        let mut bits: Vec<bool> = [true, true, true, false, true].repeat(2);
        bits.extend(aztec::draw::simple_bits(b"BCPART TWO"));
        let (compact, layers, words) = aztec::draw::fit(&bits, true);
        let g = aztec::draw::grid(compact, layers, &words, None);
        let found = scan(&az_picture(&g, 4.0, 0.2, false));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "PART TWO");
        assert_eq!(found[0].part, Some((2, 3)));
    }

    #[test]
    fn nothing_in_noise() {
        let mut s = 12345u32;
        let px: Vec<u8> = (0..200 * 200)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 24) as u8
            })
            .collect();
        assert!(scan(&Gray { w: 200, h: 200, px }).is_empty());
    }

    #[test]
    fn links_and_hosts() {
        assert_eq!(code("https://example.org/a?b").host(), Some("example.org"));
        assert_eq!(code("HTTP://user@x.test:8080").host(), Some("x.test:8080"));
        assert_eq!(
            code("https://evil.test@good.test/").host(),
            Some("good.test")
        );
        assert_eq!(code("javascript:alert(1)").url(), None);
        assert_eq!(code("https://a b").url(), None);
        assert_eq!(code("WIFI:S:x;;").url(), None);
    }
}
