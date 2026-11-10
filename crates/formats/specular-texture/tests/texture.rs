//! DDS / KTX2 / KTX / PVR / VTF decode tests on fixtures written here (no files on disk).

use std::sync::atomic::AtomicBool;

use specular_texture::{Container, DecodeOptions, Decoded, Error, Region, Texture};

// ---------------------------------------------------------------------------------------------
// Fixture writers
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

enum DdsPf {
    /// DX9 RGB(A) masks.
    Masks {
        bits: u32,
        masks: [u32; 4],
    },
    FourCc(&'static [u8; 4]),
    Dx10(u32),
}

fn dds(w: u32, h: u32, pf: DdsPf, levels: &[Vec<u8>]) -> Vec<u8> {
    let mut d = vec![0u8; 128];
    let put = |d: &mut Vec<u8>, p: usize, v: u32| d[p..p + 4].copy_from_slice(&v.to_le_bytes());
    d[..4].copy_from_slice(b"DDS ");
    put(&mut d, 4, 124);
    put(&mut d, 8, 0x1007 | 0x2_0000);
    put(&mut d, 12, h);
    put(&mut d, 16, w);
    put(&mut d, 28, levels.len() as u32);
    put(&mut d, 76, 32);
    match &pf {
        DdsPf::Masks { bits, masks } => {
            put(&mut d, 80, 0x40 | if masks[3] != 0 { 1 } else { 0 });
            put(&mut d, 88, *bits);
            for (k, m) in masks.iter().enumerate() {
                put(&mut d, 92 + 4 * k, *m);
            }
        }
        DdsPf::FourCc(cc) => {
            put(&mut d, 80, 4);
            d[84..88].copy_from_slice(*cc);
        }
        DdsPf::Dx10(code) => {
            put(&mut d, 80, 4);
            d[84..88].copy_from_slice(b"DX10");
            let mut x = [0u8; 20];
            x[..4].copy_from_slice(&code.to_le_bytes());
            x[4..8].copy_from_slice(&3u32.to_le_bytes());
            x[12..16].copy_from_slice(&1u32.to_le_bytes());
            d.extend_from_slice(&x);
        }
    }
    put(&mut d, 108, 0x1000);
    for l in levels {
        d.extend_from_slice(l);
    }
    d
}

#[derive(Clone, Copy)]
enum Sc {
    None,
    Zstd,
    Zlib,
}

fn ktx2(vk: u32, w: u32, h: u32, sc: Sc, flip: bool, levels: &[Vec<u8>]) -> Vec<u8> {
    let mut d = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let p32 = |d: &mut Vec<u8>, v: u32| d.extend_from_slice(&v.to_le_bytes());
    let n = levels.len();
    p32(&mut d, vk);
    p32(&mut d, 1);
    p32(&mut d, w);
    p32(&mut d, h);
    p32(&mut d, 0);
    p32(&mut d, 0);
    p32(&mut d, 1);
    p32(&mut d, n as u32);
    p32(
        &mut d,
        match sc {
            Sc::None => 0,
            Sc::Zstd => 2,
            Sc::Zlib => 3,
        },
    );
    // DFD (empty), KVD, SGD.
    let mut kvd = Vec::new();
    if flip {
        let kv = b"KTXorientation\0ru\0";
        kvd.extend_from_slice(&(kv.len() as u32).to_le_bytes());
        kvd.extend_from_slice(kv);
        while kvd.len() % 4 != 0 {
            kvd.push(0);
        }
    }
    let kvd_off = 80 + 24 * n;
    p32(&mut d, 0);
    p32(&mut d, 0);
    p32(&mut d, kvd_off as u32);
    p32(&mut d, kvd.len() as u32);
    d.extend_from_slice(&[0u8; 16]);
    let packed: Vec<Vec<u8>> = levels
        .iter()
        .map(|l| match sc {
            Sc::None => l.clone(),
            Sc::Zstd => ruzstd::encoding::compress_to_vec(
                &l[..],
                ruzstd::encoding::CompressionLevel::Fastest,
            ),
            Sc::Zlib => {
                let mut out = vec![0u8; zlib_rs::compress_bound(l.len())];
                let (c, _) =
                    zlib_rs::compress_slice(&mut out, l, zlib_rs::DeflateConfig::default());
                c.to_vec()
            }
        })
        .collect();
    let mut off = (kvd_off + kvd.len()).next_multiple_of(16);
    let mut offs = Vec::new();
    for (p, l) in packed.iter().zip(levels) {
        offs.push(off);
        d.extend_from_slice(&(off as u64).to_le_bytes());
        d.extend_from_slice(&(p.len() as u64).to_le_bytes());
        d.extend_from_slice(&(l.len() as u64).to_le_bytes());
        off = (off + p.len()).next_multiple_of(16);
    }
    d.extend_from_slice(&kvd);
    for (p, o) in packed.iter().zip(offs) {
        d.resize(o, 0);
        d.extend_from_slice(p);
    }
    d
}

fn pattern(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            v.extend_from_slice(&[
                (x * 7 + y) as u8,
                (y * 5) as u8,
                (x ^ y) as u8,
                (200 + x % 50) as u8,
            ]);
        }
    }
    v
}

fn decode(t: &Texture, data: &[u8], scale: u32, region: Option<Region>, parallel: bool) -> Decoded {
    t.decode(
        data,
        &DecodeOptions {
            scale,
            region,
            cancel: None,
            parallel,
        },
    )
    .unwrap()
}

fn crop(full: &Decoded, r: Region) -> Vec<u8> {
    let mut v = Vec::new();
    for y in r.y..r.y + r.h {
        let s = y as usize * full.stride + r.x as usize * 4;
        v.extend_from_slice(&full.data[s..s + r.w as usize * 4]);
    }
    v
}

/// Reference box filter (linear light, alpha weighted) in f64 over level-0 RGBA.
fn reference_box(px: &[u8], w: u32, h: u32, f: u32) -> Vec<u8> {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let enc = |l: f64| {
        let s = if l <= 0.003_130_8 {
            l * 12.92
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    let (ow, oh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Vec::new();
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut s, mut sw, mut sa, mut n) = ([0f64; 3], 0f64, 0f64, 0f64);
            for y in oy * f..((oy + 1) * f).min(h) {
                for x in ox * f..((ox + 1) * f).min(w) {
                    let p = &px[(y * w + x) as usize * 4..][..4];
                    let wt = f64::from(p[3]) * 256.0 + 1.0;
                    for c in 0..3 {
                        s[c] += wt * lin(p[c]);
                    }
                    sw += wt;
                    sa += f64::from(p[3]);
                    n += 1.0;
                }
            }
            out.extend_from_slice(&[
                enc(s[0] / sw),
                enc(s[1] / sw),
                enc(s[2] / sw),
                (sa / n).round() as u8,
            ]);
        }
    }
    out
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

const RGBA_MASKS: [u32; 4] = [0xff, 0xff00, 0xff_0000, 0xff00_0000];

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn rgba8_mips_serve_scales() {
    let (w, h) = (66, 50);
    let l0 = pattern(w, h);
    let l1: Vec<u8> = [10, 20, 30, 255].repeat(33 * 25);
    let l2: Vec<u8> = [40, 50, 60, 255].repeat(16 * 12);
    let file = dds(
        w,
        h,
        DdsPf::Masks {
            bits: 32,
            masks: RGBA_MASKS,
        },
        &[l0.clone(), l1, l2],
    );
    let t = Texture::open(&file).unwrap();
    assert_eq!(t.container, Container::Dds);
    assert_eq!(t.level_sizes(), vec![(66, 50), (33, 25), (16, 12)]);
    assert!(t.format.has_alpha());

    let full = decode(&t, &file, 1, None, true);
    assert_eq!((full.width, full.height), (66, 50));
    assert_eq!(full.data, l0);
    assert!(!full.truncated);

    let s2 = decode(&t, &file, 2, None, true);
    assert_eq!((s2.width, s2.height), (33, 25));
    assert!(s2.data.chunks(4).all(|p| p == [10, 20, 30, 255]));

    // Level 2 is 16×12 for a virtual 17×13: the edge is clamped.
    let s4 = decode(&t, &file, 4, None, true);
    assert_eq!((s4.width, s4.height), (17, 13));
    assert!(s4.data.chunks(4).all(|p| p == [40, 50, 60, 255]));
    let s8 = decode(&t, &file, 8, None, false);
    assert_eq!((s8.width, s8.height), (9, 7));
    assert!(s8.data.chunks(4).all(|p| p == [40, 50, 60, 255]));

    // No native level for 3: box filtered from level 0.
    let s3 = decode(&t, &file, 3, None, true);
    assert_eq!((s3.width, s3.height), (22, 17));
    assert!(max_diff(&s3.data, &reference_box(&l0, w, h, 3)) <= 1);
}

#[test]
fn regions_match_full_decode() {
    let mut rng = Rng(7);
    let (w, h) = (90u32, 70u32);
    let blocks = (w.div_ceil(4) * h.div_ceil(4)) as usize;
    let files = [
        dds(w, h, DdsPf::Dx10(98), &[rng.bytes(blocks * 16)]),
        dds(w, h, DdsPf::FourCc(b"DXT1"), &[rng.bytes(blocks * 8)]),
        dds(w, h, DdsPf::FourCc(b"DXT5"), &[rng.bytes(blocks * 16)]),
        dds(w, h, DdsPf::FourCc(b"BC5S"), &[rng.bytes(blocks * 16)]),
        dds(w, h, DdsPf::Dx10(95), &[rng.bytes(blocks * 16)]),
        dds(
            w,
            h,
            DdsPf::Masks {
                bits: 16,
                masks: [0xf800, 0x7e0, 0x1f, 0],
            },
            &[rng.bytes((w * h * 2) as usize)],
        ),
        dds(
            w,
            h,
            DdsPf::Masks {
                bits: 24,
                masks: [0xff_0000, 0xff00, 0xff, 0],
            },
            &[rng.bytes((w * h * 3) as usize)],
        ),
        ktx2(
            97,
            w,
            h,
            Sc::None,
            false,
            &[rng.bytes((w * h * 8) as usize)],
        ),
    ];
    let regions = [
        Region {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        },
        Region {
            x: 3,
            y: 5,
            w: 17,
            h: 9,
        },
        Region {
            x: 13,
            y: 0,
            w: 17,
            h: 20,
        },
    ];
    for file in &files {
        let t = Texture::open(file).unwrap();
        for scale in [1, 3] {
            let full = decode(&t, file, scale, None, true);
            assert_eq!(
                full.data,
                decode(&t, file, scale, None, false).data,
                "{}",
                t.description()
            );
            let (fw, fh) = (full.width, full.height);
            let last = Region {
                x: fw - 7,
                y: fh - 5,
                w: 7,
                h: 5,
            };
            for r in regions.iter().chain([&last]) {
                let d = decode(&t, file, scale, Some(*r), true);
                assert_eq!((d.width, d.height, d.origin), (r.w, r.h, (r.x, r.y)));
                assert_eq!(
                    d.data,
                    crop(&full, *r),
                    "{} scale {scale} {r:?}",
                    t.description()
                );
            }
        }
    }
}

#[test]
fn block_placement_matches_bcdec_rs() {
    let mut rng = Rng(99);
    let (w, h) = (10, 6);
    let data = rng.bytes(3 * 2 * 16);
    let file = dds(w, h, DdsPf::Dx10(98), std::slice::from_ref(&data));
    let t = Texture::open(&file).unwrap();
    let got = decode(&t, &file, 1, None, true);
    let mut want = vec![0u8; 12 * 8 * 4];
    for by in 0..2 {
        for bx in 0..3 {
            let k = (by * 3 + bx) * 16;
            bcdec_rs::bc7(&data[k..k + 16], &mut want[by * 4 * 48 + bx * 16..], 48);
        }
    }
    for y in 0..h as usize {
        assert_eq!(&got.data[y * 40..y * 40 + 40], &want[y * 48..y * 48 + 40]);
    }
}

#[test]
fn ktx2_supercompression_and_flip() {
    let mut rng = Rng(3);
    let (w, h) = (40, 24);
    let l0 = rng.bytes(10 * 6 * 16);
    let l1 = rng.bytes(5 * 3 * 16);
    let plain = ktx2(145, w, h, Sc::None, false, &[l0.clone(), l1.clone()]);
    let t = Texture::open(&plain).unwrap();
    assert_eq!(t.container, Container::Ktx2);
    assert!(!t.flip_y);
    let want0 = decode(&t, &plain, 1, None, true);
    let want1 = decode(&t, &plain, 2, None, true);
    for sc in [Sc::Zstd, Sc::Zlib] {
        let file = ktx2(145, w, h, sc, true, &[l0.clone(), l1.clone()]);
        let t = Texture::open(&file).unwrap();
        assert!(t.flip_y);
        assert!(t.description().contains("BC7"), "{}", t.description());
        let d = decode(&t, &file, 1, None, true);
        assert!(!d.truncated);
        assert_eq!(d.data, want0.data);
        assert_eq!(decode(&t, &file, 2, None, true).data, want1.data);
        let r = Region {
            x: 5,
            y: 3,
            w: 9,
            h: 4,
        };
        assert_eq!(decode(&t, &file, 1, Some(r), true).data, crop(&want0, r));
        assert!(t.memory_estimate(&DecodeOptions::default()) >= 40 * 24 * 4);
    }
}

#[test]
fn wide_formats_convert_to_eight_bits() {
    // RGBA16 UNORM.
    let px: Vec<u8> = [0u16, 32768, 65535, 257]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let f = ktx2(91, 1, 1, Sc::None, false, &[px]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(t.format.bit_depth(), 16);
    assert_eq!(decode(&t, &f, 1, None, true).data, [0, 128, 255, 1]);

    // RGBA16F: 1.0, 0.5 (linear → sRGB 188), -1 (clamped), alpha 0.5 (kept linear).
    let px: Vec<u8> = [0x3c00u16, 0x3800, 0xbc00, 0x3800]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let f = dds(1, 1, DdsPf::Dx10(10), &[px]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [255, 188, 0, 128]);

    // RGB32F via DX10 code 6 and R32F (grey).
    let px: Vec<u8> = [0.5f32, 2.0, f32::NAN]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let f = dds(1, 1, DdsPf::Dx10(6), &[px]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [188, 255, 0, 255]);
    let f = dds(1, 1, DdsPf::Dx10(41), &[0.5f32.to_le_bytes().to_vec()]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [188, 188, 188, 255]);

    // RGB9E5: 1.0 in red = mantissa 256, exponent 16.
    let v: u32 = 256 | (16 << 27);
    let f = dds(1, 1, DdsPf::Dx10(67), &[v.to_le_bytes().to_vec()]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [255, 0, 0, 255]);

    // R11G11B10F: green 1.0 (exponent 15, mantissa 0).
    let v: u32 = (15 << 6) << 11;
    let f = ktx2(122, 1, 1, Sc::None, false, &[v.to_le_bytes().to_vec()]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [0, 255, 0, 255]);

    // R8G8 SNORM: -1 → 0, 0 → 128.
    let f = ktx2(17, 1, 1, Sc::None, false, &[vec![0x81, 0]]);
    let t = Texture::open(&f).unwrap();
    assert_eq!(decode(&t, &f, 1, None, true).data, [0, 128, 0, 255]);
}

#[test]
fn truncated_and_malformed_never_panic() {
    let mut rng = Rng(11);
    let (w, h) = (36, 20);
    let blocks = 9 * 5;
    let files = vec![
        dds(
            w,
            h,
            DdsPf::Dx10(98),
            &[rng.bytes(blocks * 16), rng.bytes(5 * 3 * 16)],
        ),
        dds(
            w,
            h,
            DdsPf::Masks {
                bits: 32,
                masks: RGBA_MASKS,
            },
            &[pattern(w, h)],
        ),
        ktx2(131, w, h, Sc::Zstd, false, &[rng.bytes(blocks * 8)]),
        ktx2(37, w, h, Sc::Zlib, true, &[pattern(w, h)]),
        ktx2(
            97,
            w,
            h,
            Sc::None,
            false,
            &[rng.bytes((w * h * 8) as usize)],
        ),
    ];
    for file in &files {
        // Every truncation point.
        for n in 0..file.len() {
            if let Ok(t) = Texture::open(&file[..n]) {
                for scale in [1, 2, 5] {
                    if let Ok(d) = t.decode(
                        &file[..n],
                        &DecodeOptions {
                            scale,
                            ..Default::default()
                        },
                    ) {
                        assert_eq!(d.data.len(), d.stride * d.height as usize);
                    }
                }
            }
        }
        // Half the data: the decode succeeds and is flagged truncated.
        let full = Texture::open(file).unwrap();
        let cut = &file[..file.len() * 2 / 3];
        if let Ok(t) = Texture::open(cut)
            && t.level_sizes()[0] == full.level_sizes()[0]
        {
            let d = t.decode(cut, &DecodeOptions::default()).unwrap();
            assert!(d.truncated, "{}", t.description());
        }
        // Random corruption.
        for _ in 0..300 {
            let mut f = file.clone();
            for _ in 0..1 + rng.next() % 4 {
                let p = (rng.next() % f.len().min(256) as u64) as usize;
                f[p] = rng.next() as u8;
            }
            if let Ok(t) = Texture::open(&f) {
                let opts = DecodeOptions {
                    scale: 1 + (rng.next() % 4) as u32,
                    ..Default::default()
                };
                if t.memory_estimate(&opts) < 64 << 20 {
                    let _ = t.decode(&f, &opts);
                }
            }
        }
    }
}

#[test]
fn absurd_dimensions_and_bad_regions() {
    let f = dds(1 << 20, 4, DdsPf::Dx10(98), &[vec![0; 64]]);
    assert!(matches!(Texture::open(&f), Err(Error::TooLarge)));
    let f = ktx2(37, 0, 4, Sc::None, false, &[vec![0; 64]]);
    assert!(Texture::open(&f).is_err());
    let f = ktx2(1_000_054_002, 8, 8, Sc::None, false, &[vec![0; 64]]);
    assert!(matches!(Texture::open(&f), Err(Error::Unsupported(s)) if s.contains("PVRTC2")));
    // ASTC HDR is read.
    let f = ktx2(1_000_066_000, 8, 8, Sc::None, false, &[vec![0; 64]]);
    assert!(Texture::open(&f).unwrap().format.is_float());
    // 100000² RGBA8 header with almost no data: refuses or decodes a capped amount, never OOM.
    let f = dds(
        100_000,
        100_000,
        DdsPf::Masks {
            bits: 32,
            masks: RGBA_MASKS,
        },
        &[vec![0; 64]],
    );
    let t = Texture::open(&f).unwrap();
    assert!(matches!(
        t.decode(&f, &DecodeOptions::default()),
        Err(Error::TooLarge)
    ));
    let r = t.decode(
        &f,
        &DecodeOptions {
            region: Some(Region {
                x: 0,
                y: 0,
                w: 8,
                h: 8,
            }),
            ..Default::default()
        },
    );
    assert!(r.unwrap().truncated);

    let f = dds(
        8,
        8,
        DdsPf::Masks {
            bits: 32,
            masks: RGBA_MASKS,
        },
        &[pattern(8, 8)],
    );
    let t = Texture::open(&f).unwrap();
    for r in [
        Region {
            x: 0,
            y: 0,
            w: 0,
            h: 1,
        },
        Region {
            x: 4,
            y: 0,
            w: 5,
            h: 1,
        },
        Region {
            x: 0,
            y: 8,
            w: 1,
            h: 1,
        },
    ] {
        assert!(matches!(
            t.decode(
                &f,
                &DecodeOptions {
                    region: Some(r),
                    ..Default::default()
                }
            ),
            Err(Error::BadRegion)
        ));
    }
}

#[test]
fn cancel_stops_decode() {
    let mut rng = Rng(5);
    let f = dds(64, 64, DdsPf::Dx10(98), &[rng.bytes(16 * 16 * 16)]);
    let t = Texture::open(&f).unwrap();
    let cancel = AtomicBool::new(true);
    let r = t.decode(
        &f,
        &DecodeOptions {
            cancel: Some(&cancel),
            ..Default::default()
        },
    );
    assert!(matches!(r, Err(Error::Cancelled)));
}

#[test]
fn dxt2_and_dxt4_decode_to_straight_alpha() {
    // One block: alpha 136 everywhere (BC2 nibbles 8; BC3 endpoints 136), colour red 132
    // stored premultiplied, so straight red is 132·255/136 = 247.5 → 248.
    let colour = [0x00, 0x80, 0x00, 0x80, 0, 0, 0, 0];
    let bc2: Vec<u8> = [[0x88; 8].as_slice(), &colour].concat();
    let bc3: Vec<u8> = [[136, 136, 0, 0, 0, 0, 0, 0].as_slice(), &colour].concat();
    for (plain, premul, block) in [(b"DXT3", b"DXT2", &bc2), (b"DXT5", b"DXT4", &bc3)] {
        let f = dds(4, 4, DdsPf::FourCc(plain), std::slice::from_ref(block));
        let t = Texture::open(&f).unwrap();
        assert!(!t.premultiplied);
        assert_eq!(decode(&t, &f, 1, None, false).data[..4], [132, 0, 0, 136]);
        let f = dds(4, 4, DdsPf::FourCc(premul), std::slice::from_ref(block));
        let t = Texture::open(&f).unwrap();
        assert!(t.premultiplied && t.description().contains("premultiplied"));
        let d = decode(&t, &f, 1, None, false);
        assert!(d.data.chunks(4).all(|p| p == [248, 0, 0, 136]));
        // Reduced decodes average the straight colour.
        assert_eq!(decode(&t, &f, 2, None, false).data[..4], [248, 0, 0, 136]);
    }
}

#[test]
fn bc1_has_alpha_only_with_a_transparent_texel() {
    // Four-colour mode (c0 > c1): index 3 is a colour.
    let opaque = [0x00, 0xf8, 0x1f, 0x00, 0xff, 0xff, 0xff, 0xff];
    // Three-colour mode (c0 <= c1) without index 3, then with one texel at index 3.
    let three = [0x1f, 0x00, 0x00, 0xf8, 0x55, 0xaa, 0x00, 0x00];
    let hole = [0x1f, 0x00, 0x00, 0xf8, 0x00, 0x00, 0x00, 0xc0];
    for (block, alpha) in [(opaque, false), (three, false), (hole, true)] {
        let mut level = opaque.repeat(3);
        level.extend_from_slice(&block);
        let f = dds(8, 8, DdsPf::FourCc(b"DXT1"), &[level]);
        let t = Texture::open(&f).unwrap();
        assert_eq!(t.has_alpha(&f), alpha, "{block:?}");
        let d = decode(&t, &f, 1, None, false);
        assert_eq!(d.data.chunks(4).any(|p| p[3] != 255), alpha);
    }
    // Truncated: missing blocks decode transparent.
    let f = dds(8, 8, DdsPf::FourCc(b"DXT1"), &[opaque.repeat(3)]);
    assert!(Texture::open(&f).unwrap().has_alpha(&f));
}

/// PVR v3: `layers` surfaces, `levels[k]` holding all of them.
fn pvr(id: u32, w: u32, h: u32, layers: u32, levels: &[Vec<u8>]) -> Vec<u8> {
    let mut d = Vec::new();
    for v in [
        0x0352_5650u32,
        0,
        id,
        0,
        0,
        0,
        h,
        w,
        1,
        layers,
        1,
        levels.len() as u32,
        0,
    ] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    levels.iter().for_each(|l| d.extend_from_slice(l));
    d
}

/// KTX (version 1), compressed `internal` format, `layers` array elements.
fn ktx1(internal: u32, w: u32, h: u32, layers: u32, levels: &[Vec<u8>]) -> Vec<u8> {
    let mut d = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let n = levels.len() as u32;
    for v in [
        0x0403_0201u32,
        0,
        1,
        0,
        internal,
        0x1908,
        w,
        h,
        0,
        layers,
        1,
        n,
        0,
    ] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    for l in levels {
        d.extend_from_slice(&(l.len() as u32).to_le_bytes());
        d.extend_from_slice(l);
    }
    d
}

/// VTF 7.2 without a thumbnail: `frames` frames, levels stored smallest first.
fn vtf(code: u32, w: u32, h: u32, frames: u32, levels: &[Vec<u8>]) -> Vec<u8> {
    let mut d = vec![0u8; 80];
    d[..4].copy_from_slice(b"VTF\0");
    d[4..8].copy_from_slice(&7u32.to_le_bytes());
    d[8..12].copy_from_slice(&2u32.to_le_bytes());
    d[12..16].copy_from_slice(&80u32.to_le_bytes());
    d[16..18].copy_from_slice(&(w as u16).to_le_bytes());
    d[18..20].copy_from_slice(&(h as u16).to_le_bytes());
    d[24..26].copy_from_slice(&(frames as u16).to_le_bytes());
    d[52..56].copy_from_slice(&code.to_le_bytes());
    d[56] = levels.len() as u8;
    d[57..61].copy_from_slice(&u32::MAX.to_le_bytes());
    d[63..65].copy_from_slice(&1u16.to_le_bytes());
    levels.iter().rev().for_each(|l| d.extend_from_slice(l));
    d
}

/// The same blocks in PVR, KTX and VTF decode like a one-layer KTX2 per layer, at full size
/// and from mip 1; every layer is its own image.
#[test]
fn pvr_ktx1_vtf_layers_match_ktx2() {
    let mut rng = Rng(11);
    let (w, h, layers) = (16u32, 8u32, 3u32);
    // (KTX2 vkFormat, PVR id, KTX internal format, VTF format, bytes per block).
    let cases = [
        (133, Some(7), 0x83F1, Some(20), 8),   // BC1 with alpha
        (137, Some(11), 0x83F3, Some(15), 16), // BC3 (DXT5)
        (147, Some(22), 0x9274, None, 8),      // ETC2 RGB
        (151, Some(23), 0x9278, None, 16),     // ETC2 RGBA
        (153, Some(25), 0x9270, None, 8),      // EAC R11
        (157, Some(27), 0x93B0, None, 16),     // ASTC 4×4
        (171, Some(34), 0x93B7, None, 16),     // ASTC 8×8
    ];
    for (vk, pvr_id, gl, vtf_code, bb) in cases {
        let (bw, bh) = if vk == 171 { (8, 8) } else { (4, 4) };
        let blocks = |lw: u32, lh: u32| (lw.div_ceil(bw) * lh.div_ceil(bh)) as usize * bb;
        let per_layer: Vec<[Vec<u8>; 2]> = (0..layers)
            .map(|_| [rng.bytes(blocks(w, h)), rng.bytes(blocks(w / 2, h / 2))])
            .collect();
        let levels: Vec<Vec<u8>> = (0..2)
            .map(|k| per_layer.iter().flat_map(|l| l[k].clone()).collect())
            .collect();
        let mut files = vec![("KTX", ktx1(gl, w, h, layers, &levels))];
        if let Some(id) = pvr_id {
            files.push(("PVR", pvr(id, w, h, layers, &levels)));
        }
        if let Some(code) = vtf_code {
            files.push(("VTF", vtf(code, w, h, layers, &levels)));
        }
        for (name, f) in files {
            let mut t = Texture::open(&f).unwrap_or_else(|e| panic!("{name} {vk}: {e}"));
            assert_eq!(t.image_count(), layers, "{name} {vk}");
            for (i, l) in per_layer.iter().enumerate() {
                let r = ktx2(vk, w, h, Sc::None, false, l);
                let rt = Texture::open(&r).unwrap();
                assert_eq!(t.format, rt.format, "{name} {vk}");
                assert!(t.select_image(i as u32));
                for s in [1, 2] {
                    let (a, b) = (
                        decode(&t, &f, s, None, true),
                        decode(&rt, &r, s, None, true),
                    );
                    assert!(a.data == b.data, "{name} {vk} layer {i} scale {s}");
                }
            }
        }
    }
}
