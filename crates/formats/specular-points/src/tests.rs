//! Readers against tiny files written here, and robustness: truncated, corrupted and
//! over-declaring files are errors (never panics, never huge allocations).

use crate::{Error, Limits, PointCloud, PointFormat, read};

thread_local! {
    /// Makes [`crate::Job::run`] panic on this thread (a decoder bug, for its guard).
    static PANIC_IN_JOB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test hook at the start of [`crate::Job::run`].
pub(crate) fn trip(_bytes: &[u8]) {
    if PANIC_IN_JOB.with(std::cell::Cell::get) {
        panic!("test panic in the points job");
    }
}

/// A point as written: coordinates (in file units), intensity, class, return/returns, rgb8.
#[derive(Clone, Copy, Debug)]
struct P {
    xyz: [f64; 3],
    intensity: u16,
    class: u8,
    ret: (u8, u8),
    rgb: [u8; 3],
}

fn points(n: usize) -> Vec<P> {
    (0..n)
        .map(|i| {
            let f = i as f64;
            P {
                xyz: [
                    1000.0 + (f * 0.37).sin() * 50.0,
                    2000.0 + (f * 0.11).cos() * 30.0,
                    5.0 + (i % 17) as f64 * 0.25,
                ],
                intensity: (i * 37 % 4000) as u16,
                class: [2u8, 5, 6, 9][i % 4],
                ret: (1 + (i % 3) as u8, 3),
                rgb: [(i * 7) as u8, (i * 13) as u8, (i * 29) as u8],
            }
        })
        .collect()
}

/// The cloud's points in file coordinates with their first attribute word, sorted.
fn world(c: &PointCloud) -> Vec<([i64; 3], u32, u32)> {
    let mut v: Vec<_> = c
        .positions
        .iter()
        .zip(&c.attrs)
        .map(|(p, a)| {
            (
                [0, 1, 2].map(|k| ((f64::from(p[k]) + c.origin[k]) * 100.0).round() as i64),
                a[0],
                a[1],
            )
        })
        .collect();
    v.sort_unstable();
    v
}

fn expected(
    pts: &[P],
    rgb: bool,
    class: bool,
    intensity: bool,
    returns: bool,
) -> Vec<([i64; 3], u32, u32)> {
    let mut v: Vec<_> = pts
        .iter()
        .map(|p| {
            let c = if rgb {
                u32::from(p.rgb[0]) | (u32::from(p.rgb[1]) << 8) | (u32::from(p.rgb[2]) << 16)
            } else {
                0
            };
            let cl = if class { u32::from(p.class) << 24 } else { 0 };
            let i = if intensity { u32::from(p.intensity) } else { 0 };
            let r = if returns {
                (u32::from(p.ret.0) | (u32::from(p.ret.1) << 4)) << 16
            } else {
                0
            };
            (
                [0, 1, 2].map(|k| (p.xyz[k] * 100.0).round() as i64),
                c | cl,
                i | r,
            )
        })
        .collect();
    v.sort_unstable();
    v
}

// ---------------------------------------------------------------------------------------
// LAS.

const RECORD_LEN: [usize; 11] = [20, 28, 26, 34, 57, 63, 30, 36, 38, 59, 67];

struct LasOpts {
    minor: u8,
    format: u8,
    extra: usize,
    rgb16: bool,
    vlrs: Vec<(&'static str, u16, Vec<u8>)>,
}

fn las(pts: &[P], o: &LasOpts) -> Vec<u8> {
    let header_size: usize = match o.minor {
        0..=2 => 227,
        3 => 235,
        _ => 375,
    };
    let vlr_bytes: usize = o.vlrs.iter().map(|v| 54 + v.2.len()).sum();
    let data_offset = header_size + vlr_bytes;
    let rec = RECORD_LEN[usize::from(o.format)] + o.extra;
    let mut b = vec![0u8; data_offset];
    b[..4].copy_from_slice(b"LASF");
    b[24] = 1;
    b[25] = o.minor;
    b[94..96].copy_from_slice(&(header_size as u16).to_le_bytes());
    b[96..100].copy_from_slice(&(data_offset as u32).to_le_bytes());
    b[100..104].copy_from_slice(&(o.vlrs.len() as u32).to_le_bytes());
    b[104] = o.format;
    b[105..107].copy_from_slice(&(rec as u16).to_le_bytes());
    let legacy = if o.format >= 6 { 0 } else { pts.len() as u32 };
    b[107..111].copy_from_slice(&legacy.to_le_bytes());
    let scale = [0.01f64; 3];
    let offset = [1000.0f64, 2000.0, 0.0];
    for k in 0..3 {
        b[131 + 8 * k..139 + 8 * k].copy_from_slice(&scale[k].to_le_bytes());
        b[155 + 8 * k..163 + 8 * k].copy_from_slice(&offset[k].to_le_bytes());
        let max = pts.iter().map(|p| p.xyz[k]).fold(f64::MIN, f64::max);
        let min = pts.iter().map(|p| p.xyz[k]).fold(f64::MAX, f64::min);
        b[179 + 16 * k..187 + 16 * k].copy_from_slice(&max.to_le_bytes());
        b[187 + 16 * k..195 + 16 * k].copy_from_slice(&min.to_le_bytes());
    }
    if o.minor >= 4 {
        b[247..255].copy_from_slice(&(pts.len() as u64).to_le_bytes());
    }
    let mut at = header_size;
    for (user, id, data) in &o.vlrs {
        b[at + 2..at + 2 + user.len()].copy_from_slice(user.as_bytes());
        b[at + 18..at + 20].copy_from_slice(&id.to_le_bytes());
        b[at + 20..at + 22].copy_from_slice(&(data.len() as u16).to_le_bytes());
        b[at + 54..at + 54 + data.len()].copy_from_slice(data);
        at += 54 + data.len();
    }
    for p in pts {
        let mut r = vec![0xA5u8; rec];
        for k in 0..3 {
            let v = ((p.xyz[k] - offset[k]) / scale[k]).round() as i32;
            r[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        r[12..14].copy_from_slice(&p.intensity.to_le_bytes());
        let c = |v: u8| {
            if o.rgb16 {
                u16::from(v) * 257
            } else {
                u16::from(v)
            }
        };
        let rgb_at = |r: &mut [u8], at: usize| {
            for k in 0..3 {
                r[at + 2 * k..at + 2 * k + 2].copy_from_slice(&c(p.rgb[k]).to_le_bytes());
            }
        };
        if o.format <= 5 {
            r[14] = p.ret.0 | (p.ret.1 << 3);
            r[15] = p.class;
            match o.format {
                2 => rgb_at(&mut r, 20),
                3 | 5 => rgb_at(&mut r, 28),
                _ => {}
            }
        } else {
            r[14] = p.ret.0 | (p.ret.1 << 4);
            r[15] = 0;
            r[16] = p.class;
            if matches!(o.format, 7 | 8 | 10) {
                rgb_at(&mut r, 30);
            }
        }
        b.extend_from_slice(&r);
    }
    b
}

fn opts(minor: u8, format: u8) -> LasOpts {
    LasOpts {
        minor,
        format,
        extra: 0,
        rgb16: true,
        vlrs: Vec::new(),
    }
}

#[test]
fn las_every_point_format() {
    let pts = points(3000);
    for format in 0..=10u8 {
        let minor = if format >= 6 { 4 } else { 2 };
        let b = las(&pts, &opts(minor, format));
        assert_eq!(PointFormat::sniff(&b), Some(PointFormat::Las));
        let c = read(PointFormat::Las, &b, &Limits::default()).unwrap();
        let rgb = matches!(format, 2 | 3 | 5 | 7 | 8 | 10);
        assert_eq!(
            world(&c),
            expected(&pts, rgb, true, true, true),
            "format {format}"
        );
        assert_eq!(c.info.point_format, Some(format));
        assert_eq!(c.info.total, 3000);
        assert_eq!(c.info.loaded, 3000);
        assert_eq!(
            c.info.version.as_deref(),
            Some(if format >= 6 { "1.4" } else { "1.2" })
        );
        assert_eq!(
            c.info.classes,
            vec![(2, 750), (5, 750), (6, 750), (9, 750)],
            "format {format}"
        );
    }
}

#[test]
fn las_versions_extra_bytes_and_8bit_colour() {
    let pts = points(500);
    for minor in 0..=4u8 {
        let mut o = opts(minor, 3);
        o.extra = 7;
        o.rgb16 = minor % 2 == 0;
        let c = read(PointFormat::Las, &las(&pts, &o), &Limits::default()).unwrap();
        assert_eq!(
            world(&c),
            expected(&pts, true, true, true, true),
            "1.{minor}"
        );
    }
}

#[test]
fn las_crs_name_and_extent() {
    let wkt = br#"PROJCS["NAD83 / UTM zone 15N",GEOGCS["NAD83"]]"#.to_vec();
    let mut o = opts(4, 6);
    o.vlrs.push(("LASF_Projection", 2112, wkt));
    o.vlrs.push(("someone", 7, vec![1, 2, 3]));
    let pts = points(100);
    let c = read(PointFormat::Las, &las(&pts, &o), &Limits::default()).unwrap();
    assert_eq!(c.info.crs.as_deref(), Some("NAD83 / UTM zone 15N"));
    let e = c.info.extent.unwrap();
    assert!(e[0] <= e[3] && e[2] <= e[5]);
}

#[test]
fn las_subsamples_to_the_cap() {
    let pts = points(200_000);
    let b = las(&pts, &opts(2, 1));
    let limits = Limits {
        max_points: 10_000,
        ..Limits::default()
    };
    let c = read(PointFormat::Las, &b, &limits).unwrap();
    assert_eq!(c.info.total, 200_000);
    assert_eq!(c.len(), 10_000);
    assert_eq!(c.info.loaded, 10_000);
}

#[test]
fn las_loading_publishes_batches() {
    let pts = points(300_000);
    let b = las(&pts, &opts(2, 0));
    let (loading, job) = crate::open(PointFormat::Las, &b, &Limits::default()).unwrap();
    assert_eq!(loading.expected, 300_000);
    assert!(!loading.bounds.is_empty());
    assert!(loading.result().is_none());
    job.unwrap().run(&b, &loading);
    let c = loading.wait().unwrap();
    assert_eq!(c.len(), 300_000);
    assert_eq!(loading.loaded(), 300_000);
    // Leaves cover the points.
    assert_eq!(
        c.leaves.iter().map(|l| l.count as usize).sum::<usize>(),
        c.len()
    );
}

/// A job that panics still ends the loading, as malformed, so whoever waits on it (the
/// viewer, a thumbnail) gets an error instead of hanging.
#[test]
fn a_panicking_job_ends_the_loading() {
    let b = las(&points(1000), &opts(2, 0));
    let (loading, job) = crate::open(PointFormat::Las, &b, &Limits::default()).unwrap();
    PANIC_IN_JOB.with(|p| p.set(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        job.unwrap().run(&b, &loading);
    }));
    PANIC_IN_JOB.with(|p| p.set(false));
    assert!(r.is_err());
    match loading.wait() {
        Err(Error::Malformed(m)) => assert!(m.contains("internal error"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn las_over_declaring_is_refused_or_clipped() {
    let pts = points(10);
    let mut b = las(&pts, &opts(4, 6));
    b[247..255].copy_from_slice(&(1u64 << 50).to_le_bytes());
    assert!(matches!(
        read(PointFormat::Las, &b, &Limits::default()),
        Err(Error::TooLarge(_))
    ));
    // A count larger than the data reads what is there.
    b[247..255].copy_from_slice(&1000u64.to_le_bytes());
    let c = read(PointFormat::Las, &b, &Limits::default()).unwrap();
    assert_eq!(c.len(), 10);
    // A header size past the file.
    let mut b = las(&pts, &opts(2, 0));
    b[94..96].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(read(PointFormat::Las, &b, &Limits::default()).is_err());
}

// ---------------------------------------------------------------------------------------
// LAZ (files written by laspy with lazrs, see tests/data/README).

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// LAZ fixtures against their LAS twins (the same points, uncompressed).
#[test]
fn laz_matches_its_las() {
    for (laz, las_name) in [
        ("p1.laz", "p1.las"),
        ("p3.laz", "p3.las"),
        ("p6.laz", "p6.las"),
        ("p7.laz", "p7.las"),
        ("p8.laz", "p8.las"),
    ] {
        let (z, l) = (fixture(laz), fixture(las_name));
        assert_eq!(PointFormat::sniff(&z), Some(PointFormat::Laz));
        let cz = read(PointFormat::Laz, &z, &Limits::default()).unwrap();
        let cl = read(PointFormat::Las, &l, &Limits::default()).unwrap();
        assert_eq!(cz.len(), cl.len(), "{laz}");
        assert_eq!(cz.origin, cl.origin, "{laz}");
        assert_eq!(world(&cz), world(&cl), "{laz}");
        assert!(
            cz.info.coding.as_deref().unwrap().starts_with("LASzip"),
            "{laz}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// PCD.

fn pcd_header(n: usize, data: &str) -> String {
    format!(
        "# .PCD v0.7 - Point Cloud Data file format\nVERSION 0.7\nFIELDS x y z rgb intensity label\n\
         SIZE 4 4 4 4 4 4\nTYPE F F F U F U\nCOUNT 1 1 1 1 1 1\nWIDTH {n}\nHEIGHT 1\n\
         VIEWPOINT 0 0 0 1 0 0 0\nPOINTS {n}\nDATA {data}\n"
    )
}

fn rgb_word(p: &P) -> u32 {
    (u32::from(p.rgb[0]) << 16) | (u32::from(p.rgb[1]) << 8) | u32::from(p.rgb[2])
}

/// LZF made of literal runs only.
fn lzf_literal(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in raw.chunks(32) {
        out.push((c.len() - 1) as u8);
        out.extend_from_slice(c);
    }
    out
}

fn pcd_points() -> Vec<P> {
    // Coordinates exact in f32 at centimetres.
    points(1000)
        .into_iter()
        .map(|mut p| {
            p.xyz = p.xyz.map(|v| (v * 4.0).round() / 4.0);
            p
        })
        .collect()
}

#[test]
fn pcd_all_encodings() {
    let pts = pcd_points();
    let n = pts.len();
    // ascii
    let mut a = pcd_header(n, "ascii");
    for p in &pts {
        a += &format!(
            "{} {} {} {} {} {}\n",
            p.xyz[0],
            p.xyz[1],
            p.xyz[2],
            rgb_word(p),
            p.intensity,
            p.class
        );
    }
    // binary
    let mut b = pcd_header(n, "binary").into_bytes();
    for p in &pts {
        for v in p.xyz {
            b.extend_from_slice(&(v as f32).to_le_bytes());
        }
        b.extend_from_slice(&rgb_word(p).to_le_bytes());
        b.extend_from_slice(&f32::from(p.intensity).to_le_bytes());
        b.extend_from_slice(&u32::from(p.class).to_le_bytes());
    }
    // binary_compressed: columns
    let mut raw = Vec::new();
    for k in 0..3 {
        for p in &pts {
            raw.extend_from_slice(&(p.xyz[k] as f32).to_le_bytes());
        }
    }
    for p in &pts {
        raw.extend_from_slice(&rgb_word(p).to_le_bytes());
    }
    for p in &pts {
        raw.extend_from_slice(&f32::from(p.intensity).to_le_bytes());
    }
    for p in &pts {
        raw.extend_from_slice(&u32::from(p.class).to_le_bytes());
    }
    let packed = lzf_literal(&raw);
    let mut z = pcd_header(n, "binary_compressed").into_bytes();
    z.extend_from_slice(&(packed.len() as u32).to_le_bytes());
    z.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    z.extend_from_slice(&packed);

    for (name, file) in [
        ("ascii", a.into_bytes()),
        ("binary", b),
        ("binary_compressed", z),
    ] {
        assert_eq!(PointFormat::sniff(&file), Some(PointFormat::Pcd), "{name}");
        let c = read(PointFormat::Pcd, &file, &Limits::default()).unwrap();
        assert_eq!(c.info.coding.as_deref(), Some(name));
        assert_eq!(c.len(), n, "{name}");
        // Intensity as a float of 0‥4000 maps to itself in 16 bits.
        let got: Vec<_> = world(&c).into_iter().map(|w| (w.0, w.1)).collect();
        let want: Vec<_> = expected(&pts, true, true, false, false)
            .into_iter()
            .map(|w| (w.0, w.1))
            .collect();
        assert_eq!(got, want, "{name}");
        assert!(c.info.channels.rgb && c.info.channels.intensity && c.info.channels.classification);
    }
}

#[test]
fn lzf_back_references() {
    // "abc" then a back-reference of 6 from 3 back: "abcabcabc".
    let src = [2u8, b'a', b'b', b'c', (4 << 5) as u8, 2];
    assert_eq!(crate::pcd::lzf(&src, 9).unwrap(), b"abcabcabc");
    // Long match (7 + extra).
    let src = [0u8, b'x', (7 << 5) as u8, 3, 0];
    assert_eq!(crate::pcd::lzf(&src, 13).unwrap(), vec![b'x'; 13]);
    // Reference before the start, and output overflow.
    assert!(crate::pcd::lzf(&[(1 << 5) as u8, 5], 3).is_err());
    assert!(crate::pcd::lzf(&src, 5).is_err());
}

#[test]
fn pcd_over_declaring() {
    let mut z = pcd_header(1 << 30, "binary_compressed").into_bytes();
    z.extend_from_slice(&16u32.to_le_bytes());
    z.extend_from_slice(&u32::MAX.to_le_bytes());
    z.extend_from_slice(&[0u8; 16]);
    assert!(read(PointFormat::Pcd, &z, &Limits::default()).is_err());
    let b = pcd_header(1 << 30, "binary").into_bytes();
    assert!(read(PointFormat::Pcd, &b, &Limits::default()).is_err());
    // 1 MB packed claiming 100× that (more than LZF can expand): refused before the
    // 100 MB output is reserved (the old bound was 300×).
    let n = 4_166_666;
    let mut z = pcd_header(n, "binary_compressed").into_bytes();
    z.extend_from_slice(&1_000_000u32.to_le_bytes());
    z.extend_from_slice(&((n * 24) as u32).to_le_bytes());
    z.extend_from_slice(&vec![0u8; 1_000_000]);
    let e = read(PointFormat::Pcd, &z, &Limits::default())
        .err()
        .unwrap();
    assert!(e.to_string().contains("implausible"), "{e}");
}

// ---------------------------------------------------------------------------------------
// Text columns.

#[test]
fn xyz_and_pts() {
    let pts = pcd_points();
    let mut x = String::from("// x y z r g b\n");
    for p in &pts {
        x += &format!(
            "{:.2},{:.2},{:.2},{},{},{}\n",
            p.xyz[0], p.xyz[1], p.xyz[2], p.rgb[0], p.rgb[1], p.rgb[2]
        );
    }
    let c = read(PointFormat::Xyz, x.as_bytes(), &Limits::default()).unwrap();
    let got: Vec<_> = world(&c).into_iter().map(|w| (w.0, w.1)).collect();
    let want: Vec<_> = expected(&pts, true, false, false, false)
        .into_iter()
        .map(|w| (w.0, w.1))
        .collect();
    assert_eq!(got, want);

    let mut s = format!("{}\n", pts.len());
    for p in &pts {
        s += &format!(
            "{:.3} {:.3} {:.3} {} {} {} {}\r\n",
            p.xyz[0],
            p.xyz[1],
            p.xyz[2],
            i32::from(p.intensity % 4000) - 2048,
            p.rgb[0],
            p.rgb[1],
            p.rgb[2]
        );
    }
    let c = read(PointFormat::Pts, s.as_bytes(), &Limits::default()).unwrap();
    assert_eq!(c.len(), pts.len());
    assert!(c.info.channels.rgb && c.info.channels.intensity);
    let got: Vec<_> = world(&c)
        .into_iter()
        .map(|w| (w.0, w.1 & 0xFF_FFFF))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn text_without_points() {
    assert!(read(PointFormat::Xyz, b"hello\nworld\n", &Limits::default()).is_err());
    assert!(read(PointFormat::Xyz, b"", &Limits::default()).is_err());
}

// ---------------------------------------------------------------------------------------
// PLY.

#[test]
fn ply_points() {
    let pts = pcd_points();
    let mut a = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\n\
         property float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\n\
         property list uchar int extra\nend_header\n",
        pts.len()
    );
    for p in &pts {
        a += &format!(
            "{} {} {} {} {} {} 2 7 8\n",
            p.xyz[0], p.xyz[1], p.xyz[2], p.rgb[0], p.rgb[1], p.rgb[2]
        );
    }
    let mut b = format!(
        "ply\nformat binary_big_endian 1.0\nelement vertex {}\nproperty double x\n\
         property double y\nproperty double z\nproperty uchar red\nproperty uchar green\n\
         property uchar blue\nend_header\n",
        pts.len()
    )
    .into_bytes();
    for p in &pts {
        for v in p.xyz {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&p.rgb);
    }
    let want: Vec<_> = expected(&pts, true, false, false, false)
        .into_iter()
        .map(|w| (w.0, w.1))
        .collect();
    for file in [a.into_bytes(), b] {
        assert!(crate::ply_is_points(&file));
        let c = read(PointFormat::Ply, &file, &Limits::default()).unwrap();
        let got: Vec<_> = world(&c).into_iter().map(|w| (w.0, w.1)).collect();
        assert_eq!(got, want);
    }
    let mesh = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\n\
                 property float z\nelement face 1\nproperty list uchar int vertex_indices\n\
                 end_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";
    assert!(!crate::ply_is_points(mesh));
    let huge = b"ply\nformat binary_little_endian 1.0\nelement vertex 999999999999\n\
                 property float x\nproperty float y\nproperty float z\nend_header\n\0\0\0\0";
    assert!(read(PointFormat::Ply, huge, &Limits::default()).is_err());
}

// ---------------------------------------------------------------------------------------
// E57.

struct Bits {
    out: Vec<u8>,
    acc: u128,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }

    fn put(&mut self, v: u64, bits: u32) {
        if bits == 0 {
            return;
        }
        let v = if bits == 64 { v } else { v & ((1 << bits) - 1) };
        self.acc |= u128::from(v) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn phys(l: usize) -> usize {
    l / 1020 * 1024 + l % 1020
}

struct E57Scan {
    name: &'static str,
    /// w, x, y, z; tx, ty, tz.
    pose: Option<([f64; 4], [f64; 3])>,
    spherical: bool,
    /// Cartesian (or range, azimuth, elevation), intensity 0‥2, rgb, invalid state.
    pts: Vec<([f64; 3], f32, [u8; 3], u8)>,
}

/// Appends a binary section of the scan's fields at logical offset `logical.len()`;
/// returns its physical offset and the prototype XML.
fn e57_section(logical: &mut Vec<u8>, s: &E57Scan) -> (usize, String) {
    let n = s.pts.len();
    let mut streams: Vec<Vec<u8>> = Vec::new();
    let proto = if s.spherical {
        let mut f = [Bits::new(), Bits::new(), Bits::new()];
        for p in &s.pts {
            f[0].put(p.0[0].to_bits(), 64);
            f[1].put(u64::from((p.0[1] as f32).to_bits()), 32);
            f[2].put(u64::from((p.0[2] as f32).to_bits()), 32);
        }
        streams.extend(f.map(Bits::finish));
        "<sphericalRange type=\"Float\"/>\
                 <sphericalAzimuth type=\"Float\" precision=\"single\"/>\
                 <sphericalElevation type=\"Float\" precision=\"single\"/>"
            .to_string()
    } else {
        let mut f = [Bits::new(), Bits::new(), Bits::new()];
        for p in &s.pts {
            for (fk, v) in f.iter_mut().zip(p.0) {
                let raw = (v / 0.001).round() as i64 + 100_000_000;
                fk.put(raw as u64, 28);
            }
        }
        streams.extend(f.map(Bits::finish));
        ["X", "Y", "Z"]
            .map(|a| {
                format!(
                    "<cartesian{a} type=\"ScaledInteger\" minimum=\"-100000000\" \
                     maximum=\"100000000\" scale=\"0.001\" offset=\"0\"/>"
                )
            })
            .concat()
    };
    let mut inten = Bits::new();
    let mut col = [Bits::new(), Bits::new(), Bits::new()];
    let mut inv = Bits::new();
    let mut skip = Bits::new();
    for p in &s.pts {
        inten.put(u64::from(p.1.to_bits()), 32);
        for (ck, v) in col.iter_mut().zip(p.2) {
            ck.put(u64::from(v), 8);
        }
        inv.put(u64::from(p.3), 2);
        skip.put(5, 3);
    }
    streams.push(inten.finish());
    streams.extend(col.map(Bits::finish));
    streams.push(inv.finish());
    streams.push(skip.finish());
    let proto = format!(
        "{proto}<intensity type=\"Float\" precision=\"single\"/>\
         <colorRed type=\"Integer\" minimum=\"0\" maximum=\"255\"/>\
         <colorGreen type=\"Integer\" minimum=\"0\" maximum=\"255\"/>\
         <colorBlue type=\"Integer\" minimum=\"0\" maximum=\"255\"/>\
         <{}InvalidState type=\"Integer\" minimum=\"0\" maximum=\"2\"/>\
         <rowIndex type=\"Integer\" minimum=\"0\" maximum=\"7\"/>",
        if s.spherical {
            "spherical"
        } else {
            "cartesian"
        }
    );
    // Packets carry each stream in pieces of up to 97 bytes (so values straddle packets).
    let start = logical.len();
    let mut sec = vec![0u8; 32];
    sec[0] = 1;
    let mut offs = vec![0usize; streams.len()];
    // An index packet first, to be skipped.
    sec.extend_from_slice(&[0, 0, 15, 0]);
    sec.extend_from_slice(&[0u8; 12]);
    while offs.iter().zip(&streams).any(|(o, s)| o < &s.len()) {
        let mut p = vec![1u8, 0, 0, 0];
        p.extend_from_slice(&(streams.len() as u16).to_le_bytes());
        let lens: Vec<usize> = offs
            .iter()
            .zip(&streams)
            .map(|(o, s)| (s.len() - o).min(97))
            .collect();
        for l in &lens {
            p.extend_from_slice(&(*l as u16).to_le_bytes());
        }
        for (i, l) in lens.iter().enumerate() {
            p.extend_from_slice(&streams[i][offs[i]..offs[i] + l]);
            offs[i] += l;
        }
        while p.len() % 4 != 0 {
            p.push(0);
        }
        let len = (p.len() - 1) as u16;
        p[2..4].copy_from_slice(&len.to_le_bytes());
        sec.extend_from_slice(&p);
    }
    let sec_len = sec.len() as u64;
    sec[8..16].copy_from_slice(&sec_len.to_le_bytes());
    sec[16..24].copy_from_slice(&(phys(start + 32) as u64).to_le_bytes());
    logical.extend_from_slice(&sec);
    let _ = n;
    (phys(start), proto)
}

fn e57(scans: &[E57Scan]) -> Vec<u8> {
    e57_edited(scans, |x| x)
}

fn e57_edited(scans: &[E57Scan], edit: impl Fn(String) -> String) -> Vec<u8> {
    let mut logical = vec![0u8; 48];
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<e57Root type=\"Structure\" \
         xmlns=\"http://www.astm.org/COMMIT/E57/2010-e57-v1.0\">\
         <formatName type=\"String\"><![CDATA[ASTM E57 3D Imaging Data File]]></formatName>\
         <data3D type=\"Vector\" allowHeterogeneousChildren=\"1\">",
    );
    for s in scans {
        let (at, proto) = e57_section(&mut logical, s);
        let pose = s.pose.map_or(String::new(), |(q, t)| {
            format!(
                "<pose type=\"Structure\"><rotation type=\"Structure\"><w type=\"Float\">{}</w>\
                 <x type=\"Float\">{}</x><y type=\"Float\">{}</y><z type=\"Float\">{}</z>\
                 </rotation><translation type=\"Structure\"><x type=\"Float\">{}</x>\
                 <y type=\"Float\">{}</y><z type=\"Float\"/></translation></pose>",
                q[0], q[1], q[2], q[3], t[0], t[1]
            )
        });
        xml += &format!(
            "<vectorChild type=\"Structure\"><name type=\"String\">{}</name>{pose}\
             <intensityLimits type=\"Structure\"><intensityMinimum type=\"Float\">0</intensityMinimum>\
             <intensityMaximum type=\"Float\">2</intensityMaximum></intensityLimits>\
             <points type=\"CompressedVector\" fileOffset=\"{at}\" recordCount=\"{}\">\
             <prototype type=\"Structure\">{proto}</prototype>\
             <codecs type=\"Vector\" allowHeterogeneousChildren=\"1\"/></points></vectorChild>",
            s.name,
            s.pts.len()
        );
    }
    xml += "</data3D></e57Root>";
    let xml = edit(xml);
    let xml_at = logical.len();
    logical.extend_from_slice(xml.as_bytes());
    let pages = logical.len().div_ceil(1020);
    let h = &mut logical[..48];
    h[..8].copy_from_slice(b"ASTM-E57");
    h[8..12].copy_from_slice(&1u32.to_le_bytes());
    h[16..24].copy_from_slice(&((pages * 1024) as u64).to_le_bytes());
    h[24..32].copy_from_slice(&(phys(xml_at) as u64).to_le_bytes());
    h[32..40].copy_from_slice(&(xml.len() as u64).to_le_bytes());
    h[40..48].copy_from_slice(&1024u64.to_le_bytes());
    crate::e57::paginate(&logical, 1024)
}

fn e57_scans() -> Vec<E57Scan> {
    let grid = |n: usize| -> Vec<([f64; 3], f32, [u8; 3], u8)> {
        (0..n)
            .map(|i| {
                let f = i as f64;
                (
                    [f * 0.01, (f * 0.3).sin() * 2.0, (i % 11) as f64 * 0.1],
                    (i % 21) as f32 * 0.1,
                    [i as u8, (i * 3) as u8, 200],
                    u8::from(i % 50 == 0) * 2,
                )
            })
            .collect()
    };
    let h = std::f64::consts::FRAC_1_SQRT_2;
    vec![
        E57Scan {
            name: "first &amp; plain",
            pose: None,
            spherical: false,
            pts: grid(700),
        },
        E57Scan {
            name: "turned",
            pose: Some(([h, 0.0, 0.0, h], [100.0, -5.0, 0.0])),
            spherical: false,
            pts: grid(300),
        },
        E57Scan {
            name: "sphere",
            pose: None,
            spherical: true,
            pts: (0..400)
                .map(|i| {
                    let a = i as f64 * 0.05;
                    ([10.0, a, 0.25], 1.0f32, [9, 9, 9], 0)
                })
                .collect(),
        },
    ]
}

#[test]
fn e57_scans_poses_and_spherical() {
    let scans = e57_scans();
    let b = e57(&scans);
    assert_eq!(PointFormat::sniff(&b), Some(PointFormat::E57));
    let c = read(PointFormat::E57, &b, &Limits::default()).unwrap();
    let info = &c.info;
    assert_eq!(info.total, 1400);
    assert_eq!(info.scans.len(), 3);
    assert_eq!(info.scans[0].name.as_deref(), Some("first & plain"));
    assert!(!info.scans[0].posed && info.scans[1].posed && info.scans[2].spherical);
    assert!(info.channels.rgb && info.channels.intensity);
    // Invalid points (every 50th of the cartesian scans) are dropped.
    assert_eq!(c.len(), 700 - 14 + 300 - 6 + 400);
    // Expected points in file coordinates.
    let mut want: Vec<([i64; 3], u32)> = Vec::new();
    for s in &scans {
        for p in &s.pts {
            if p.3 != 0 {
                continue;
            }
            let mut q = if s.spherical {
                let (r, az, el) = (p.0[0], f64::from(p.0[1] as f32), f64::from(p.0[2] as f32));
                [
                    r * el.cos() * az.cos(),
                    r * el.cos() * az.sin(),
                    r * el.sin(),
                ]
            } else {
                p.0
            };
            if let Some((_, t)) = s.pose {
                // 90° about z.
                q = [-q[1] + t[0], q[0] + t[1], q[2]];
            }
            let i = (f64::from(p.1) / 2.0 * 65_535.0).round() as u32;
            want.push(([0, 1, 2].map(|k| (q[k] * 100.0).round() as i64), i));
        }
    }
    let got: Vec<_> = world(&c).into_iter().map(|w| (w.0, w.2)).collect();
    assert_eq!(got.len(), want.len());
    // Each expected point has its own decoded point within a centimetre (f32 positions and
    // rounding at ties make exact matching brittle).
    let mut used = vec![false; got.len()];
    for w in &want {
        let hit = got.iter().enumerate().position(|(i, g)| {
            !used[i] && (0..3).all(|k| (g.0[k] - w.0[k]).abs() <= 1) && g.1.abs_diff(w.1) <= 1
        });
        let Some(i) = hit else {
            panic!("no decoded point near {w:?}");
        };
        used[i] = true;
    }
}

#[test]
fn e57_checksums_and_bad_counts() {
    let mut b = e57(&e57_scans());
    b[300] ^= 1;
    assert!(matches!(
        read(PointFormat::E57, &b, &Limits::default()),
        Err(Error::Malformed(m)) if m.contains("checksum")
    ));
    // More records than the section holds, and more than may be declared.
    let mut scans = e57_scans();
    scans.truncate(1);
    let b = e57_edited(&scans, |x| {
        x.replace("recordCount=\"700\"", "recordCount=\"900\"")
    });
    assert!(read(PointFormat::E57, &b, &Limits::default()).is_err());
    let b = e57_edited(&scans, |x| {
        x.replace("recordCount=\"700\"", "recordCount=\"99999999999999\"")
    });
    assert!(matches!(
        read(
            PointFormat::E57,
            &b,
            &Limits {
                max_declared: 1 << 30,
                ..Limits::default()
            }
        ),
        Err(Error::TooLarge(_))
    ));
}

mod hostile {
    use super::*;

    #[test]
    fn e57_constant_fields_with_a_huge_record_count() {
        let mut scans = e57_scans();
        scans.truncate(1);
        let b = e57_edited(&scans, |x| {
            x.replace(
                "minimum=\"-100000000\" maximum=\"100000000\"",
                "minimum=\"0\" maximum=\"0\"",
            )
            .replace(
                "<intensity type=\"Float\" precision=\"single\"/>",
                "<intensity type=\"Integer\" minimum=\"0\" maximum=\"0\"/>",
            )
            .replace(
                "minimum=\"0\" maximum=\"255\"",
                "minimum=\"0\" maximum=\"0\"",
            )
            .replace("minimum=\"0\" maximum=\"2\"", "minimum=\"0\" maximum=\"0\"")
            .replace("recordCount=\"700\"", "recordCount=\"1099511627776\"")
        });
        let c = read(PointFormat::E57, &b, &Limits::default()).unwrap();
        assert!(c.len() < b.len());
    }

    #[test]
    fn ply_list_of_the_largest_length() {
        let mut b = b"ply\nformat binary_little_endian 1.0\nelement vertex 2\n\
                      property list double int extra\nproperty float x\nproperty float y\n\
                      property float z\nend_header\n"
            .to_vec();
        b.extend_from_slice(&f64::MAX.to_le_bytes());
        b.extend_from_slice(&[0; 32]);
        let _ = read(PointFormat::Ply, &b, &Limits::default());
    }

    #[test]
    fn ply_element_without_properties() {
        let mut b = b"ply\nformat binary_little_endian 1.0\nelement empty 18446744073709551615\n\
                      element vertex 1\nproperty float x\nproperty float y\nproperty float z\n\
                      end_header\n"
            .to_vec();
        b.extend_from_slice(&[0; 12]);
        assert_eq!(
            read(PointFormat::Ply, &b, &Limits::default())
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn pcd_fields_of_no_values() {
        let mut b = pcd_header(4, "binary")
            .replace("COUNT 1 1 1 1 1 1", "COUNT 0 0 0 0 0 0")
            .into_bytes();
        b.extend([7; 16]);
        assert!(read(PointFormat::Pcd, &b, &Limits::default()).is_err());
    }

    #[test]
    fn pcd_width_times_height_overflowing() {
        let b = pcd_header(4, "binary")
            .replace("WIDTH 4\nHEIGHT 1", "WIDTH 18446744073709551615\nHEIGHT 3")
            .replace("POINTS 4\n", "");
        assert!(read(PointFormat::Pcd, b.as_bytes(), &Limits::default()).is_err());
    }
}

// ---------------------------------------------------------------------------------------
// Robustness.

/// Truncations and corruptions of `file` never panic.
fn survives(format: PointFormat, file: &[u8]) {
    let limits = Limits {
        max_points: 100_000,
        ..Limits::default()
    };
    let step = (file.len() / 97).max(1);
    for cut in (0..file.len()).step_by(step) {
        let _ = read(format, &file[..cut], &limits);
    }
    let mut s = 0x1234_5678u32;
    for _ in 0..200 {
        let mut f = file.to_vec();
        for _ in 0..4 {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let at = s as usize % f.len();
            f[at] = (s >> 24) as u8;
        }
        let _ = read(format, &f, &limits);
    }
}

#[test]
fn robust_las_pcd_text_ply_e57() {
    let pts = points(300);
    for format in [0u8, 3, 6, 8] {
        survives(PointFormat::Las, &las(&pts, &opts(4, format)));
    }
    let mut b = pcd_header(100, "binary").into_bytes();
    b.extend(std::iter::repeat_n(7u8, 2400));
    survives(PointFormat::Pcd, &b);
    let mut s = String::from("10\n");
    for p in &pts {
        s += &format!("{} {} {} 100 1 2 3\n", p.xyz[0], p.xyz[1], p.xyz[2]);
    }
    survives(PointFormat::Pts, s.as_bytes());
    let mut ply = format!(
        "ply\nformat binary_little_endian 1.0\nelement vertex {}\nproperty float x\n\
         property float y\nproperty float z\nproperty list uchar int idx\nend_header\n",
        pts.len()
    )
    .into_bytes();
    for p in &pts {
        for v in p.xyz {
            ply.extend_from_slice(&(v as f32).to_le_bytes());
        }
        ply.push(1);
        ply.extend_from_slice(&5i32.to_le_bytes());
    }
    survives(PointFormat::Ply, &ply);
    survives(PointFormat::E57, &e57(&e57_scans()));
}

#[test]
fn robust_laz() {
    for name in ["p1.laz", "p3.laz", "p6.laz", "p8.laz"] {
        survives(PointFormat::Laz, &fixture(name));
    }
}

#[test]
fn preview_draws_something() {
    let pts = points(5000);
    let c = read(
        PointFormat::Las,
        &las(&pts, &opts(2, 3)),
        &Limits::default(),
    )
    .unwrap();
    let img = crate::preview::render(&c, 64, 48);
    assert_eq!(img.len(), 64 * 48 * 4);
    let opaque = img.chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(opaque > 200, "{opaque}");
}

/// A sparse cloud's splats grow with the rendering: at 1024 pixels each of 50 points covers
/// well over the 3×3 pixels it had, so the points survive fitting and thumbnails (the visual
/// review found a 50-point XYZ cloud nearly empty).
#[test]
fn sparse_splats_scale_with_the_picture() {
    let pts = points(50);
    let c = read(
        PointFormat::Las,
        &las(&pts, &opts(2, 3)),
        &Limits::default(),
    )
    .unwrap();
    let img = crate::preview::render(&c, 1024, 1024);
    let opaque = img.chunks_exact(4).filter(|p| p[3] > 0).count();
    assert!(opaque > 50 * 25, "{opaque}");
}
