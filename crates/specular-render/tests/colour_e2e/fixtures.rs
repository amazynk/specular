//! Synthetic test images for the end-to-end colour test: one layout of flat patches and
//! ramps, filled from a per-case palette, plus the small encoders (PNG, ICC profiles, raw
//! Y'CbCr planes) that turn it into files. Nothing here comes from a real photo.

/// Image size. Patches are 16×16 and MCU-aligned, so 4:2:0 JPEG chroma is flat inside them.
pub const W: u32 = 128;
pub const H: u32 = 112;

/// What a pixel of the layout shows.
#[derive(Clone, Copy, Debug)]
pub enum Cell {
    /// Flat patch `i` (8 per row, 3 rows: y 0‥48).
    Patch(usize),
    /// Horizontal ramp band `i` (16 rows each, y 48‥80) at `t` ∈ [0, 1] along x.
    HRamp(usize, f64),
    /// Vertical ramp strip `i` (16 columns each, y 80‥112) at `t` along y.
    VRamp(usize, f64),
}

pub fn cell(x: u32, y: u32) -> Cell {
    if y < 48 {
        Cell::Patch((y / 16 * 8 + x / 16) as usize)
    } else if y < 80 {
        Cell::HRamp(((y - 48) / 16) as usize, f64::from(x) / f64::from(W - 1))
    } else {
        Cell::VRamp((x / 16) as usize, f64::from(y - 80) / 31.0)
    }
}

/// Colours of the layout, `N` channels each (in whatever space the case encodes).
pub struct Palette<const N: usize> {
    /// 24 patches.
    pub patches: Vec<[f64; N]>,
    /// Two horizontal ramps, each a list of evenly spaced stops.
    pub hramps: [Vec<[f64; N]>; 2],
    /// Eight vertical ramps (two stops each).
    pub vramps: [([f64; N], [f64; N]); 8],
}

fn lerp<const N: usize>(a: [f64; N], b: [f64; N], t: f64) -> [f64; N] {
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t)
}

impl<const N: usize> Palette<N> {
    pub fn at(&self, x: u32, y: u32) -> [f64; N] {
        match cell(x, y) {
            Cell::Patch(i) => self.patches[i],
            Cell::HRamp(i, t) => {
                let s = &self.hramps[i];
                let f = t * (s.len() - 1) as f64;
                let k = (f.floor() as usize).min(s.len() - 2);
                lerp(s[k], s[k + 1], f - k as f64)
            }
            Cell::VRamp(i, t) => lerp(self.vramps[i].0, self.vramps[i].1, t),
        }
    }

    /// The whole image, row-major.
    pub fn image(&self) -> Vec<[f64; N]> {
        (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .map(|(x, y)| self.at(x, y))
            .collect()
    }
}

fn rgb8(v: [u8; 3]) -> [f64; 3] {
    v.map(|c| f64::from(c) / 255.0)
}

/// The SDR palette, sRGB-encoded 0‥1: greys from black to white, primaries and secondaries,
/// a few natural colours; a saturated hue ramp and a fine grey ramp (banding); vertical ramps
/// across strong chroma edges (chroma siting).
pub fn sdr_palette() -> Palette<3> {
    let patches = [
        [0, 0, 0],
        [255, 255, 255],
        [128, 128, 128],
        [64, 64, 64],
        [192, 192, 192],
        [16, 16, 16],
        [240, 240, 240],
        [100, 100, 100],
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 255, 255],
        [255, 0, 255],
        [180, 40, 40],
        [40, 160, 60],
        [50, 70, 190],
        [224, 172, 140],
        [120, 170, 230],
        [90, 60, 30],
        [200, 120, 40],
        [30, 90, 110],
        [150, 50, 160],
        [230, 220, 90],
    ]
    .map(rgb8)
    .to_vec();
    let hue = [
        [230, 20, 20],
        [230, 230, 20],
        [20, 230, 20],
        [20, 230, 230],
        [20, 20, 230],
        [230, 20, 230],
    ]
    .map(rgb8)
    .to_vec();
    let grey = vec![[0.0; 3], [1.0; 3]];
    let v = |a, b| (rgb8(a), rgb8(b));
    Palette {
        patches,
        hramps: [hue, grey],
        vramps: [
            v([255, 0, 0], [0, 255, 255]),
            v([0, 0, 255], [255, 255, 0]),
            v([0, 255, 0], [255, 0, 255]),
            v([0, 0, 0], [255, 255, 255]),
            v([224, 172, 140], [120, 170, 230]),
            v([200, 30, 60], [30, 60, 200]),
            v([20, 120, 20], [240, 200, 220]),
            v([128, 128, 128], [200, 40, 40]),
        ],
    }
}

/// CMYK ink amounts 0‥1 (not inverted).
pub fn cmyk_palette() -> Palette<4> {
    let p = |c: u8, m: u8, y: u8, k: u8| [c, m, y, k].map(|v| f64::from(v) / 100.0);
    Palette {
        patches: vec![
            p(0, 0, 0, 0),
            p(0, 0, 0, 100),
            p(100, 100, 100, 100),
            p(0, 0, 0, 50),
            p(100, 0, 0, 0),
            p(0, 100, 0, 0),
            p(0, 0, 100, 0),
            p(100, 100, 0, 0),
            p(0, 100, 100, 0),
            p(100, 0, 100, 0),
            p(50, 0, 0, 0),
            p(0, 50, 0, 0),
            p(0, 0, 50, 0),
            p(25, 25, 25, 0),
            p(60, 40, 40, 20),
            p(10, 30, 40, 0),
            p(70, 20, 0, 0),
            p(0, 60, 80, 10),
            p(40, 0, 70, 30),
            p(80, 80, 0, 20),
            p(5, 5, 5, 5),
            p(90, 90, 90, 0),
            p(30, 60, 0, 50),
            p(0, 25, 75, 75),
        ],
        hramps: [
            vec![p(0, 0, 0, 0), p(100, 0, 0, 0), p(100, 100, 0, 0)],
            vec![p(0, 0, 0, 0), p(0, 0, 0, 100)],
        ],
        vramps: [
            (p(100, 0, 0, 0), p(0, 100, 100, 0)),
            (p(0, 0, 100, 0), p(100, 100, 0, 0)),
            (p(0, 100, 0, 0), p(100, 0, 100, 0)),
            (p(0, 0, 0, 0), p(100, 100, 100, 100)),
            (p(10, 30, 40, 0), p(50, 20, 0, 0)),
            (p(0, 0, 0, 0), p(0, 0, 0, 100)),
            (p(40, 0, 70, 30), p(0, 60, 80, 10)),
            (p(20, 20, 20, 20), p(0, 100, 100, 0)),
        ],
    }
}

/// HDR palette: linear light in units of SDR reference white (203 cd/m²), BT.709 primaries,
/// up to ~4.9 (1000 cd/m²) so the tone curve's shoulder is exercised.
pub fn hdr_palette() -> Palette<3> {
    let g = |v: f64| [v; 3];
    Palette {
        patches: vec![
            g(0.0),
            g(0.01),
            g(0.05),
            g(0.18),
            g(0.5),
            g(0.79),
            g(1.0),
            g(1.5),
            g(2.0),
            g(3.0),
            g(4.0),
            g(4.9),
            [0.8, 0.05, 0.05],
            [0.05, 0.6, 0.05],
            [0.05, 0.05, 0.7],
            [2.5, 0.3, 0.1],
            [0.2, 2.0, 0.4],
            [0.3, 0.4, 3.0],
            [0.6, 0.45, 0.35],
            [0.25, 0.4, 0.6],
            [4.0, 3.5, 1.0],
            [0.02, 0.03, 0.05],
            [1.2, 1.0, 0.8],
            [0.7, 0.7, 2.2],
        ],
        hramps: [vec![g(0.0), g(1.0)], vec![g(0.0), g(4.9)]],
        vramps: [
            ([0.6, 0.05, 0.05], [0.05, 0.5, 0.5]),
            ([0.05, 0.05, 0.6], [0.6, 0.6, 0.05]),
            (g(0.0), g(2.0)),
            (g(0.5), g(4.9)),
            ([3.0, 0.2, 0.2], [0.2, 0.2, 3.0]),
            ([0.6, 0.45, 0.35], [0.25, 0.4, 0.6]),
            ([0.1, 1.5, 0.1], [1.5, 0.1, 1.5]),
            (g(0.18), [2.0, 1.0, 0.5]),
        ],
    }
}

/// Alpha of the straight-alpha case: opaque, ~63 % and 25 % patch rows, an alpha ramp over
/// the grey ramp band, opaque elsewhere.
pub fn alpha_at(x: u32, y: u32) -> u8 {
    match cell(x, y) {
        Cell::Patch(i) => [255, 160, 64][i / 8],
        Cell::HRamp(1, t) => (t * 255.0).round() as u8,
        _ => 255,
    }
}

// ---- PNG ---------------------------------------------------------------------------------

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for d in chunks {
        for &b in *d {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
    }
    !c
}

/// Extra PNG chunks before the image data.
#[derive(Default)]
pub struct PngTags {
    pub srgb: bool,
    pub icc: Option<Vec<u8>>,
    /// gAMA × 100 000.
    pub gamma: Option<u32>,
    /// cHRM: white x, y, then red, green, blue x, y.
    pub chrm: Option<[f64; 8]>,
}

/// A PNG of `channels` samples per pixel (1 grey, 3 RGB, 4 RGBA) at `depth` 8 or 16 bits,
/// samples given as integers.
pub fn png(channels: u8, depth: u8, samples: &[u16], tags: &PngTags) -> Vec<u8> {
    let colour_type = match channels {
        1 => 0,
        2 => 4,
        3 => 2,
        4 => 6,
        _ => unreachable!(),
    };
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |ty: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(ty);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&[ty, data]).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&W.to_be_bytes());
    ihdr.extend_from_slice(&H.to_be_bytes());
    ihdr.extend_from_slice(&[depth, colour_type, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    if let Some(g) = tags.gamma {
        chunk(b"gAMA", &g.to_be_bytes());
    }
    if let Some(c) = tags.chrm {
        let d: Vec<u8> = c
            .iter()
            .flat_map(|v| ((v * 100_000.0).round() as u32).to_be_bytes())
            .collect();
        chunk(b"cHRM", &d);
    }
    if tags.srgb {
        chunk(b"sRGB", &[1]);
    }
    if let Some(icc) = &tags.icc {
        let mut d = b"test\0\0".to_vec();
        d.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(icc, 6));
        chunk(b"iCCP", &d);
    }
    let row = W as usize * channels as usize;
    let mut raw = Vec::new();
    for line in samples.chunks_exact(row) {
        raw.push(0u8);
        for &s in line {
            if depth == 16 {
                raw.extend_from_slice(&s.to_be_bytes());
            } else {
                raw.push(s as u8);
            }
        }
    }
    chunk(
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
    );
    chunk(b"IEND", &[]);
    out
}

// ---- A CMYK printer profile (ICC v2, lut16 A2B0 to Lab) ------------------------------------

/// The synthetic press: ink amounts → linear RGB reflectance (each ink absorbs one channel,
/// black absorbs all, with a 3 % floor so the darkest black is not zero and black point
/// compensation matters) → XYZ (D50, the Bradford-adapted sRGB primaries) → CIELAB.
pub fn press_lab(cmyk: [f64; 4]) -> [f64; 3] {
    let [c, m, y, k] = cmyk;
    let refl = |ink: f64| 0.03 + 0.97 * ((1.0 - ink) * (1.0 - k)).powf(1.8);
    let rgb = [refl(c), refl(m), refl(y)];
    const M: [[f64; 3]; 3] = [
        [0.436_074_7, 0.385_064_9, 0.143_080_4],
        [0.222_504_5, 0.716_878_6, 0.060_616_9],
        [0.013_932_2, 0.097_104_5, 0.714_173_3],
    ];
    let xyz: [f64; 3] = std::array::from_fn(|r| (0..3).map(|i| M[r][i] * rgb[i]).sum());
    let white = [0.9642, 1.0, 0.8249];
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let [fx, fy, fz] = std::array::from_fn(|i| f(xyz[i] / white[i]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn s15(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

/// The press as an ICC v2 output profile: CMYK → Lab `mft2` A2B0 on a 9⁴ grid.
pub fn cmyk_icc() -> Vec<u8> {
    const G: usize = 9;
    let mut a2b = b"mft2\0\0\0\0".to_vec();
    a2b.extend_from_slice(&[4, 3, G as u8, 0]);
    for r in 0..3 {
        for c in 0..3 {
            a2b.extend_from_slice(&s15(if r == c { 1.0 } else { 0.0 }));
        }
    }
    a2b.extend_from_slice(&2u16.to_be_bytes());
    a2b.extend_from_slice(&2u16.to_be_bytes());
    for _ in 0..4 {
        a2b.extend_from_slice(&[0, 0, 0xff, 0xff]);
    }
    let n = |i: usize| i as f64 / (G - 1) as f64;
    for c in 0..G {
        for m in 0..G {
            for y in 0..G {
                for k in 0..G {
                    let [l, a, b] = press_lab([n(c), n(m), n(y), n(k)]);
                    // ICC v2 16-bit Lab: L 0‥100 → 0‥0xff00, a/b −128‥127.996 → 0‥0xffff.
                    let enc = [
                        l / 100.0 * 65280.0,
                        (a + 128.0) * 256.0,
                        (b + 128.0) * 256.0,
                    ];
                    for v in enc {
                        a2b.extend_from_slice(
                            &(v.round().clamp(0.0, 65535.0) as u16).to_be_bytes(),
                        );
                    }
                }
            }
        }
    }
    for _ in 0..3 {
        a2b.extend_from_slice(&[0, 0, 0xff, 0xff]);
    }
    // Output profiles must have B2A0; Little CMS finds a CMYK printer's black point by
    // round-tripping L* = 0 through it (without one it skips black point compensation). A
    // grey-axis table is enough: L* 0 → an ink-limited rich black, L* 100 → paper.
    let mut b2a = b"mft2\0\0\0\0".to_vec();
    b2a.extend_from_slice(&[3, 4, 2, 0]);
    for r in 0..3 {
        for c in 0..3 {
            b2a.extend_from_slice(&s15(if r == c { 1.0 } else { 0.0 }));
        }
    }
    b2a.extend_from_slice(&2u16.to_be_bytes());
    b2a.extend_from_slice(&2u16.to_be_bytes());
    for _ in 0..3 {
        b2a.extend_from_slice(&[0, 0, 0xff, 0xff]);
    }
    for l in 0..2 {
        for _ab in 0..4 {
            let ink: [f64; 4] = if l == 0 {
                [0.75, 0.65, 0.6, 1.0]
            } else {
                [0.0; 4]
            };
            for v in ink {
                b2a.extend_from_slice(&((v * 65535.0).round() as u16).to_be_bytes());
            }
        }
    }
    for _ in 0..4 {
        b2a.extend_from_slice(&[0, 0, 0xff, 0xff]);
    }
    icc(
        b"prtr",
        b"CMYK",
        b"Lab ",
        &[(*b"A2B0", a2b), (*b"B2A0", b2a)],
    )
}

/// An ICC v2.1 profile with `desc`, `cprt`, `wtpt` (D50) and the given tags.
fn icc(class: &[u8; 4], space: &[u8; 4], pcs: &[u8; 4], tags: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut desc = b"desc\0\0\0\0".to_vec();
    let name = b"Specular colour test\0";
    desc.extend_from_slice(&(name.len() as u32).to_be_bytes());
    desc.extend_from_slice(name);
    desc.extend_from_slice(&[0; 4 + 4 + 2 + 1 + 67]);
    let mut cprt = b"text\0\0\0\0".to_vec();
    cprt.extend_from_slice(b"No copyright, synthetic\0");
    let mut wtpt = b"XYZ \0\0\0\0".to_vec();
    for v in [0.9642, 1.0, 0.8249] {
        wtpt.extend_from_slice(&s15(v));
    }
    let mut all = vec![(*b"desc", desc), (*b"cprt", cprt), (*b"wtpt", wtpt)];
    all.extend(tags.iter().cloned());
    let table = 4 + 12 * all.len();
    let mut body = Vec::new();
    let mut entries = Vec::new();
    for (sig, data) in &all {
        let off = 128 + table + body.len();
        entries.push((*sig, off as u32, data.len() as u32));
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }
    let size = 128 + table + body.len();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&(size as u32).to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&0x0210_0000u32.to_be_bytes());
    out.extend_from_slice(class);
    out.extend_from_slice(space);
    out.extend_from_slice(pcs);
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(b"acsp");
    out.extend_from_slice(&[0; 24]);
    out.extend_from_slice(&0u32.to_be_bytes()); // intent
    for v in [0.9642, 1.0, 0.8249] {
        out.extend_from_slice(&s15(v));
    }
    out.resize(128, 0);
    out.extend_from_slice(&(all.len() as u32).to_be_bytes());
    for (sig, off, len) in entries {
        out.extend_from_slice(&sig);
        out.extend_from_slice(&off.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
    }
    out.extend_from_slice(&body);
    out
}

// ---- Y'CbCr for the AVIF fixtures -------------------------------------------------------

/// Luma coefficients (Kr, Kb).
pub const BT709: (f64, f64) = (0.2126, 0.0722);
pub const BT2020: (f64, f64) = (0.2627, 0.0593);

/// Limited-range Y'CbCr codes at `bits` of a non-linear R'G'B' triple (0‥1).
pub fn ycbcr_limited(rgb: [f64; 3], (kr, kb): (f64, f64), bits: u32) -> [u16; 3] {
    let kg = 1.0 - kr - kb;
    let y = kr * rgb[0] + kg * rgb[1] + kb * rgb[2];
    let cb = (rgb[2] - y) / (2.0 * (1.0 - kb));
    let cr = (rgb[0] - y) / (2.0 * (1.0 - kr));
    let s = f64::from(1u32 << (bits - 8));
    let max = f64::from((1u32 << bits) - 1);
    let q = |v: f64| v.round().clamp(0.0, max) as u16;
    [
        q((16.0 + 219.0 * y) * s),
        q((128.0 + 224.0 * cb) * s),
        q((128.0 + 224.0 * cr) * s),
    ]
}

/// The inverse of [`ycbcr_limited`], unclipped R'G'B'.
pub fn rgb_from_limited(c: [u16; 3], (kr, kb): (f64, f64), bits: u32) -> [f64; 3] {
    let s = f64::from(1u32 << (bits - 8));
    let y = (f64::from(c[0]) / s - 16.0) / 219.0;
    let cb = (f64::from(c[1]) / s - 128.0) / 224.0;
    let cr = (f64::from(c[2]) / s - 128.0) / 224.0;
    let r = y + 2.0 * (1.0 - kr) * cr;
    let b = y + 2.0 * (1.0 - kb) * cb;
    let g = (y - kr * r - kb * b) / (1.0 - kr - kb);
    [r, g, b]
}
