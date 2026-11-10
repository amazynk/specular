//! Leica PTX: one or more scans, each a header of ten lines (columns, rows, the scanner's
//! position, its three axes, a 4×4 transformation whose last row is the translation) and
//! then columns × rows point lines `x y z intensity [r g b]` in the scanner's frame. Points
//! at 0 0 0 are missing returns and dropped; the rest are put in place by their scan's
//! transformation. Intensity is 0‥1 (−2048‥2047 from some writers), colour 0‥255. Each
//! scan's body is parsed in parallel pieces starting at line breaks.

use rayon::prelude::*;

use crate::build;
use crate::{Channels, CloudInfo, Error, Limits, PointCloud, PointFormat, Result, Scan};

/// One scan: its transformation and the byte range of its point lines.
struct Body {
    /// Row-major 4×4, applied as `p' = [x y z 1] · M`.
    m: [[f64; 4]; 4],
    start: usize,
    end: usize,
    lines: u64,
}

/// The next line from `at`: its bytes without the line break, and where the next starts.
fn line(b: &[u8], at: usize) -> Option<(&[u8], usize)> {
    if at >= b.len() {
        return None;
    }
    let e = b[at..]
        .iter()
        .position(|&c| c == b'\n')
        .map_or(b.len(), |i| at + i);
    let l = &b[at..e];
    Some((l.strip_suffix(b"\r").unwrap_or(l), (e + 1).min(b.len())))
}

fn nums<const N: usize>(l: &[u8]) -> Option<[f64; N]> {
    let mut out = [0f64; N];
    let mut it = l
        .split(|c| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty());
    for v in &mut out {
        *v = std::str::from_utf8(it.next()?).ok()?.parse::<f64>().ok()?;
    }
    out.iter().all(|v| v.is_finite()).then_some(out)
}

/// Where `n` lines from `at` end (after the last one's break), or the end of the file.
fn skip_lines(b: &[u8], mut at: usize, n: u64) -> (usize, u64) {
    let mut done = 0u64;
    while done < n && at < b.len() {
        match b[at..].iter().position(|&c| c == b'\n') {
            Some(i) => at += i + 1,
            None => at = b.len(),
        }
        done += 1;
    }
    (at, done)
}

/// The scans' headers and bodies; an error when the first header is not a PTX header.
fn scans(b: &[u8]) -> Result<Vec<Body>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    loop {
        // Blank lines between scans.
        while let Some((l, next)) = line(b, at) {
            if !l.trim_ascii().is_empty() {
                break;
            }
            at = next;
        }
        let mut head: Vec<&[u8]> = Vec::with_capacity(10);
        let mut p = at;
        while head.len() < 10 {
            let Some((l, next)) = line(b, p) else { break };
            head.push(l);
            p = next;
        }
        let parsed = (head.len() == 10).then(|| {
            let cols = nums::<1>(head[0])?[0];
            let rows = nums::<1>(head[1])?[0];
            nums::<3>(head[2])?;
            for h in &head[3..6] {
                nums::<3>(h)?;
            }
            let mut m = [[0f64; 4]; 4];
            for (r, h) in head[6..10].iter().enumerate() {
                m[r] = nums::<4>(h)?;
            }
            let ok = |v: f64| v >= 0.0 && v.fract() == 0.0 && v < 1e9;
            (ok(cols) && ok(rows)).then_some((cols as u64 * rows as u64, m))
        });
        match parsed.flatten() {
            Some((n, m)) => {
                let (end, lines) = skip_lines(b, p, n);
                out.push(Body {
                    m,
                    start: p,
                    end,
                    lines,
                });
                at = end;
                if at >= b.len() {
                    break;
                }
            }
            None if out.is_empty() => {
                return Err(Error::Malformed(
                    "not a PTX file: no scan header (columns, rows, position, axes, matrix)".into(),
                ));
            }
            // Trailing text after the last scan.
            None => break,
        }
    }
    Ok(out)
}

/// One piece's points: positions relative to the origin, packed attributes, lines seen,
/// whether any line had colour.
type Part = (Vec<[f32; 3]>, Vec<[u32; 2]>, u64, bool);

/// Whether `b` is NVIDIA PTX GPU assembly (a `.version` and a `.target` directive among the
/// first lines, after `//` comments), which shares the `.ptx` extension.
fn nvidia_ptx(b: &[u8]) -> bool {
    let head = &b[..b.len().min(16 << 10)];
    let (mut version, mut target) = (false, false);
    let mut at = 0;
    for _ in 0..200 {
        let Some((l, next)) = line(head, at) else {
            break;
        };
        at = next;
        let l = l.trim_ascii();
        version |= l.starts_with(b".version ");
        target |= l.starts_with(b".target ");
        if version && target {
            return true;
        }
    }
    false
}

pub(crate) fn read(b: &[u8], limits: &Limits) -> Result<PointCloud> {
    if nvidia_ptx(b) {
        return Err(Error::Unsupported(
            "this is NVIDIA PTX GPU program code, not a Leica PTX point cloud".into(),
        ));
    }
    let bodies = scans(b)?;
    let expected: u64 = bodies.iter().map(|s| s.lines).sum();
    let stride = expected.div_ceil(limits.max_points.max(1)).max(1);
    let place = |m: &[[f64; 4]; 4], p: [f64; 3]| -> [f64; 3] {
        [0, 1, 2].map(|c| p[0] * m[0][c] + p[1] * m[1][c] + p[2] * m[2][c] + m[3][c])
    };
    // The origin: the first return, placed.
    let origin = bodies
        .iter()
        .find_map(|s| {
            let mut at = s.start;
            while let Some((l, next)) = line(b, at) {
                if next > s.end || at >= s.end {
                    break;
                }
                at = next;
                if let Some(p) = nums::<3>(l)
                    && p != [0.0; 3]
                {
                    return Some(place(&s.m, p).map(f64::round));
                }
            }
            None
        })
        .ok_or(Error::Empty)?;
    // Intensity is 0‥1 by the format; some writers put the scanner's −2048‥2047 instead.
    let wide = bodies.iter().take(1).any(|s| {
        b[s.start..s.end]
            .split(|&c| c == b'\n')
            .take(1000)
            .filter_map(nums::<4>)
            .any(|v| v[3].abs() > 1.0)
    });
    let piece = 4 << 20;
    let mut tasks: Vec<(usize, usize, usize)> = Vec::new();
    for (k, s) in bodies.iter().enumerate() {
        let mut from = s.start;
        while from < s.end {
            let mut to = (from + piece).min(s.end);
            if to < s.end {
                to = b[to..s.end]
                    .iter()
                    .position(|&c| c == b'\n')
                    .map_or(s.end, |i| to + i + 1);
            }
            tasks.push((k, from, to));
            from = to;
        }
    }
    let parts: Vec<(usize, Part)> = tasks
        .par_iter()
        .map(|&(k, from, to)| {
            let m = &bodies[k].m;
            let (mut pos, mut attrs) = (Vec::new(), Vec::new());
            let (mut seen, mut rgb_any) = (0u64, false);
            let mut vals = [0f64; 7];
            for l in b[from..to].split(|&c| c == b'\n') {
                let mut n = 0usize;
                let mut bad = false;
                for t in l
                    .split(|c| c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty())
                {
                    if n == 7 {
                        break;
                    }
                    match std::str::from_utf8(t)
                        .ok()
                        .and_then(|s| s.parse::<f64>().ok())
                    {
                        Some(v) => vals[n] = v,
                        None => {
                            bad = true;
                            break;
                        }
                    }
                    n += 1;
                }
                if bad || n < 3 {
                    continue;
                }
                seen += 1;
                let p = [vals[0], vals[1], vals[2]];
                if p == [0.0; 3] || !(seen - 1).is_multiple_of(stride) {
                    continue;
                }
                let q = place(m, p);
                if !q.iter().all(|v| v.is_finite()) {
                    continue;
                }
                pos.push([0, 1, 2].map(|i| (q[i] - origin[i]) as f32));
                let i = if n >= 4 && wide {
                    ((vals[3] + 2048.0) * 16.0).round().clamp(0.0, 65_535.0) as u32
                } else if n >= 4 {
                    (vals[3].clamp(0.0, 1.0) * 65_535.0).round() as u32
                } else {
                    0
                };
                let rgb = if n >= 7 {
                    rgb_any = true;
                    let c = |v: f64| v.round().clamp(0.0, 255.0) as u32;
                    c(vals[4]) | (c(vals[5]) << 8) | (c(vals[6]) << 16)
                } else {
                    0
                };
                attrs.push([rgb, i]);
            }
            (k, (pos, attrs, seen, rgb_any))
        })
        .collect();
    let mut positions = Vec::new();
    let mut attrs = Vec::new();
    let mut per_scan = vec![0u64; bodies.len()];
    let (mut total, mut rgb) = (0u64, false);
    for (k, (p, a, seen, c)) in parts {
        per_scan[k] += seen;
        total += seen;
        rgb |= c;
        positions.extend(p);
        attrs.extend(a);
    }
    while positions.len() as u64 > limits.max_points.saturating_mul(2) {
        let mut i = 0;
        positions.retain(|_| {
            i += 1;
            i % 2 == 1
        });
        let mut i = 0;
        attrs.retain(|_| {
            i += 1;
            i % 2 == 1
        });
    }
    let identity = |m: &[[f64; 4]; 4]| {
        (0..4).all(|r| (0..4).all(|c| (m[r][c] - f64::from(u8::from(r == c))).abs() < 1e-12))
    };
    let info = CloudInfo {
        format: Some(PointFormat::Ptx),
        total,
        channels: Channels {
            rgb,
            intensity: true,
            classification: false,
            returns: false,
        },
        scans: if bodies.len() > 1 || bodies.iter().any(|s| !identity(&s.m)) {
            bodies
                .iter()
                .zip(&per_scan)
                .map(|(s, &n)| Scan {
                    name: None,
                    points: n,
                    spherical: false,
                    posed: !identity(&s.m),
                })
                .collect()
        } else {
            Vec::new()
        },
        ..CloudInfo::default()
    };
    build::finish(positions, attrs, info, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(cols: u32, rows: u32, m: [[f64; 4]; 4]) -> String {
        let mut s = format!("{cols}\n{rows}\n0 0 0\n1 0 0\n0 1 0\n0 0 1\n");
        for r in m {
            s.push_str(&format!("{} {} {} {}\n", r[0], r[1], r[2], r[3]));
        }
        s
    }

    const ID: [[f64; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    #[test]
    fn two_scans_with_poses_colour_and_missing_returns() {
        let mut s = header(2, 2, ID);
        s.push_str("1 2 3 0.5 255 0 0\n0 0 0 0.5 0 0 0\n4 5 6 1 0 255 0\n7 8 9 0 0 0 255\n");
        // The second scan turned 90° about z and moved 100 along x.
        let m = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [100.0, 0.0, 0.0, 1.0],
        ];
        s.push_str(&header(1, 2, m));
        s.push_str("1 0 0 0.25 10 20 30\r\n0 0 0 0.5 0 0 0\r\n");
        let c = read(s.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(c.info.total, 6);
        assert_eq!(c.positions.len(), 4);
        assert!(c.info.channels.rgb && c.info.channels.intensity);
        assert_eq!(c.info.scans.len(), 2);
        assert_eq!(c.info.scans[1].points, 2);
        assert!(c.info.scans[1].posed && !c.info.scans[0].posed);
        let world: Vec<[f64; 3]> = c
            .positions
            .iter()
            .map(|p| [0, 1, 2].map(|i| f64::from(p[i]) + c.origin[i]))
            .collect();
        // (1, 0, 0) turned onto +y, then moved: (100, 1, 0).
        assert!(
            world
                .iter()
                .any(|p| (p[0] - 100.0).abs() < 1e-4 && (p[1] - 1.0).abs() < 1e-4),
            "{world:?}"
        );
        assert!(
            world
                .iter()
                .any(|p| (p[0] - 7.0).abs() < 1e-4 && (p[2] - 9.0).abs() < 1e-4)
        );
    }

    #[test]
    fn intensity_only_and_subsampling() {
        let mut s = header(10, 100, ID);
        for i in 0..1000 {
            s.push_str(&format!("{} 1 1 0.5\n", i + 1));
        }
        let c = read(
            s.as_bytes(),
            &Limits {
                max_points: 100,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(c.info.total, 1000);
        assert!(c.positions.len() <= 200 && c.positions.len() >= 50);
        assert!(!c.info.channels.rgb);
    }

    /// PDAL's PTX test files, against the counts and coordinates PDAL's own tests expect
    /// of them (test/unit/io/PtxReaderTest.cpp): taken as data.
    #[test]
    fn corpus_against_pdal_expectations() {
        let dir = std::env::var_os("SPECULAR_MODELS_EXTRA_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../tools/corpus/models-extra"
                ))
            })
            .join("ptx");
        if !dir.join("no-color.ptx").is_file() {
            eprintln!("skipped: no PTX corpus at {}", dir.display());
            return;
        }
        type Case = (&'static str, usize, bool, &'static [[f64; 3]]);
        let cases: [Case; 4] = [
            (
                "1.2-with-color.ptx",
                15 * 71,
                true,
                &[
                    [637012.24, 849028.31, 431.66],
                    [635770.47, 851464.67, 422.28],
                ],
            ),
            (
                "complex-transform.ptx",
                4,
                true,
                &[[-3.034408, -3.173781, -1.823750]],
            ),
            ("multiple-and-transform.ptx", 2 * 2 + 4, false, &[]),
            ("no-color.ptx", 15, false, &[[635795.24, 849310.43, 426.61]]),
        ];
        let mut mismatches = Vec::new();
        for (name, n, rgb, points) in cases {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let c = read(&bytes, &Limits::default()).unwrap();
            if c.positions.len() != n || c.info.channels.rgb != rgb {
                mismatches.push(format!(
                    "{name}: {} points (rgb {}), expected {n} (rgb {rgb})",
                    c.positions.len(),
                    c.info.channels.rgb
                ));
            }
            for p in points {
                let found = c
                    .positions
                    .iter()
                    .any(|q| (0..3).all(|i| (f64::from(q[i]) + c.origin[i] - p[i]).abs() < 2e-3));
                if !found {
                    mismatches.push(format!("{name}: no point at {p:?}"));
                }
            }
        }
        eprintln!("ptx corpus: 4 files, {} mismatches", mismatches.len());
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    /// `cargo test -p specular-points ptx_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn ptx_timing() {
        let (cols, rows) = (2000u32, 2000u32);
        let mut s = header(cols, rows, ID);
        for i in 0..cols * rows {
            let f = f64::from(i);
            s.push_str(&format!(
                "{:.6} {:.6} {:.6} 0.500000 {} {} {}\n",
                f * 1e-3,
                (f * 0.37).sin(),
                (f * 0.11).cos(),
                i % 256,
                (i / 7) % 256,
                (i / 13) % 256
            ));
        }
        let t0 = std::time::Instant::now();
        let c = read(s.as_bytes(), &Limits::default()).unwrap();
        eprintln!(
            "ptx: {} points, {} MB in {:.0} ms",
            c.positions.len(),
            s.len() >> 20,
            t0.elapsed().as_secs_f64() * 1e3
        );
    }

    #[test]
    fn not_ptx_is_an_error() {
        assert!(read(b"hello\nworld\n", &Limits::default()).is_err());
        assert!(read(b"", &Limits::default()).is_err());
        let s = header(1, 1, ID) + "0 0 0 0.5\n";
        assert!(read(s.as_bytes(), &Limits::default()).is_err());
        let s = header(4_000_000_000, 4_000_000_000, ID);
        assert!(read(s.as_bytes(), &Limits::default()).is_err());
    }

    /// NVIDIA's GPU assembly shares the extension: refused by name.
    #[test]
    fn nvidia_ptx_is_named() {
        let s = "//\n// Generated by NVIDIA NVVM Compiler\n//\n\n.version 8.4\n\
                 .target sm_52\n.address_size 64\n\n.visible .entry k()\n{\n\tret;\n}\n";
        match read(s.as_bytes(), &Limits::default()) {
            Err(Error::Unsupported(m)) => assert!(m.contains("NVIDIA"), "{m}"),
            r => panic!("{:?}", r.map(|_| ())),
        }
    }
}
