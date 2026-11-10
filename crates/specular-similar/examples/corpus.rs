//! Synthetic corpus, quality and timing check for duplicates and similar shots.
//!
//! ```text
//! cargo run --release -p specular-similar --example corpus -- <scratch dir> [real dir]
//! cargo run --release -p specular-similar --example corpus -- --bench
//! ```
//!
//! With a scratch dir: writes a corpus of generated scenes there (re-encodes, PNG exports,
//! resizes, byte copies, crops, bursts with blur and motion variations, near-misses that must
//! not group), runs the real scan (thumbnail engine, disk cache, store) and prints pairwise
//! precision and recall, best-pick accuracy, protection and timings (cold, then warm). With a
//! second dir: also scans those files read-only (timings and group counts; nothing is
//! written next to them, the cache and store go in the scratch dir).
//!
//! `--bench`: grouping time at 10k and 100k items with synthetic signatures.
//!
//! `--folder <scratch> <tag> [runs]`: scan timings over a generated 2,000-file folder (cold,
//! the cache a scan left, a cache the gallery filled, store hits), with dumps of the
//! signatures and groups that `--compare <dump> <dump>` sets side by side.
//!
//! `--bestpick <scratch> [runs]`: best-pick on bursts of 12 MP frames, each with one frame
//! defocused by 3–5 px (under the signature thumbnail's resolving limit) and some with a
//! noisy sharp frame, through the real scan: accuracy and scan time.
//!
//! `--focus <runs> <file>...`: one file's best-pick focus measure on 1, 2, 4 and 8 threads:
//! median wall and CPU time.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_similar::group::{self, Item};
use specular_similar::hints::Hints;
use specular_similar::index::HashIndex;
use specular_similar::scan::{self, ScanSetup};
use specular_similar::{Category, Signature, Store, analyse};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const W: usize = 1200;
const H: usize = 900;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = self.0;
        (x ^ (x >> 31)).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (x >> 17)
    }
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

#[derive(Clone)]
struct Shape {
    ellipse: bool,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    col: [f32; 3],
}

/// A scene: background gradient, shapes, a striped "subject" near the centre, fine texture.
#[derive(Clone)]
struct Scene {
    bg0: [f32; 3],
    bg1: [f32; 3],
    shapes: Vec<Shape>,
    subject: (f32, f32),
    stripe: f32,
    seed: u64,
}

fn scene(seed: u64) -> Scene {
    let mut r = Rng(seed * 7919 + 13);
    let col = |r: &mut Rng| {
        [
            r.range(20.0, 235.0),
            r.range(20.0, 235.0),
            r.range(20.0, 235.0),
        ]
    };
    let n = 8 + (r.next() % 12) as usize;
    let shapes = (0..n)
        .map(|_| Shape {
            ellipse: r.next().is_multiple_of(2),
            cx: r.range(0.0, W as f32),
            cy: r.range(0.0, H as f32),
            rx: r.range(30.0, 260.0),
            ry: r.range(30.0, 220.0),
            col: col(&mut r),
        })
        .collect();
    Scene {
        bg0: col(&mut r),
        bg1: col(&mut r),
        shapes,
        subject: (r.range(0.3, 0.7) * W as f32, r.range(0.3, 0.7) * H as f32),
        stripe: r.range(5.0, 11.0),
        seed,
    }
}

/// A near-miss: the same background and palette, other shapes and subject place.
fn near_miss(s: &Scene, seed: u64) -> Scene {
    let mut o = scene(seed);
    o.bg0 = s.bg0;
    o.bg1 = s.bg1;
    for (a, b) in o.shapes.iter_mut().zip(&s.shapes) {
        a.col = b.col;
    }
    o
}

/// The scene's colour at (`fx`, `fy`) (scene px) with the subject moved by `subj`, before
/// texture.
fn colour_at(s: &Scene, fx: f32, fy: f32, subj: (f32, f32)) -> [f32; 3] {
    let t = (fy / H as f32).clamp(0.0, 1.0);
    let mut c: [f32; 3] = std::array::from_fn(|k| s.bg0[k] * (1.0 - t) + s.bg1[k] * t);
    for sh in &s.shapes {
        let dx = (fx - sh.cx) / sh.rx;
        let dy = (fy - sh.cy) / sh.ry;
        let inside = if sh.ellipse {
            dx * dx + dy * dy <= 1.0
        } else {
            dx.abs() <= 1.0 && dy.abs() <= 1.0
        };
        if inside {
            c = sh.col;
        }
    }
    // The subject: a striped disc with detail.
    let (sx, sy) = (s.subject.0 + subj.0, s.subject.1 + subj.1);
    let d = ((fx - sx).powi(2) + (fy - sy).powi(2)).sqrt();
    if d < 120.0 {
        let v = (((fx + fy * 0.5) / s.stripe).sin() * 0.5 + 0.5) * 200.0 + 25.0;
        c = [v, 255.0 - v, (v * 0.6 + 40.0)];
    }
    c
}

/// Renders at `W`×`H` with the camera moved by `cam` and the subject by `subj` (px).
fn render(s: &Scene, cam: (f32, f32), subj: (f32, f32)) -> Vec<u8> {
    let mut out = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32 + cam.0, y as f32 + cam.1);
            let c = colour_at(s, fx, fy, subj);
            // Texture (fixed to the scene, so it moves with the camera).
            let h = ((fx as i64 as u64).wrapping_mul(0x9E37_79B9)
                ^ (fy as i64 as u64).wrapping_mul(0x85EB_CA6B)
                ^ s.seed)
                .wrapping_mul(0xC2B2_AE35);
            let n = ((h >> 24) & 15) as f32 - 7.5;
            let o = (y * W + x) * 4;
            for k in 0..3 {
                out[o + k] = (c[k] + n).clamp(0.0, 255.0) as u8;
            }
            out[o + 3] = 255;
        }
    }
    out
}

/// Separable box blur (`rx`, `ry` radii; 0 = none).
fn blur(px: &[u8], w: usize, h: usize, rx: usize, ry: usize) -> Vec<u8> {
    let mut a: Vec<f32> = px.iter().map(|&v| f32::from(v)).collect();
    let mut tmp = a.clone();
    if rx > 0 {
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    let mut s = 0.0;
                    let mut n = 0.0;
                    for xx in x.saturating_sub(rx)..(x + rx + 1).min(w) {
                        s += a[(y * w + xx) * 4 + c];
                        n += 1.0;
                    }
                    tmp[(y * w + x) * 4 + c] = s / n;
                }
            }
        }
        a.copy_from_slice(&tmp);
    }
    if ry > 0 {
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    let mut s = 0.0;
                    let mut n = 0.0;
                    for yy in y.saturating_sub(ry)..(y + ry + 1).min(h) {
                        s += a[(yy * w + x) * 4 + c];
                        n += 1.0;
                    }
                    tmp[(y * w + x) * 4 + c] = s / n;
                }
            }
        }
        a.copy_from_slice(&tmp);
    }
    a.iter().map(|&v| v.round() as u8).collect()
}

fn resize(px: &[u8], w: usize, h: usize, dw: usize, dh: usize) -> Vec<u8> {
    let planes: Vec<Vec<f32>> = (0..4)
        .map(|c| {
            let p: Vec<f32> = px.chunks_exact(4).map(|q| f32::from(q[c])).collect();
            specular_similar::sig::resize_plane(&p, w, h, dw, dh)
        })
        .collect();
    let mut out = vec![0u8; dw * dh * 4];
    for i in 0..dw * dh {
        for c in 0..4 {
            out[i * 4 + c] = planes[c][i].round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn crop(px: &[u8], w: usize, h: usize, x0: usize, y0: usize, cw: usize, ch: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(cw * ch * 4);
    for y in y0..y0 + ch {
        out.extend_from_slice(&px[(y * w + x0) * 4..(y * w + x0 + cw) * 4]);
    }
    let _ = h;
    out
}

/// A minimal little-endian EXIF block: Make, Model, DateTimeOriginal, SubSecTimeOriginal,
/// BodySerialNumber.
fn exif(time: &str, subsec: &str, serial: &str) -> Vec<u8> {
    fn ascii(tag: u16, s: &str) -> (u16, Vec<u8>) {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        (tag, v)
    }
    fn ifd(entries: &[(u16, Vec<u8>)], at: u32, extra_long: Option<(u16, u32)>) -> Vec<u8> {
        // Layout: count, entries, next=0, then data.
        let n = entries.len() + usize::from(extra_long.is_some());
        let head = 2 + 12 * n + 4;
        let mut data = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(&(n as u16).to_le_bytes());
        let mut all: Vec<(u16, Option<&Vec<u8>>, u32)> =
            entries.iter().map(|(t, v)| (*t, Some(v), 0)).collect();
        if let Some((t, v)) = extra_long {
            all.push((t, None, v));
        }
        all.sort_by_key(|e| e.0);
        for (tag, v, long) in all {
            out.extend_from_slice(&tag.to_le_bytes());
            match v {
                Some(v) => {
                    out.extend_from_slice(&2u16.to_le_bytes());
                    out.extend_from_slice(&(v.len() as u32).to_le_bytes());
                    if v.len() <= 4 {
                        let mut b = v.clone();
                        b.resize(4, 0);
                        out.extend_from_slice(&b);
                    } else {
                        let off = at + head as u32 + data.len() as u32;
                        out.extend_from_slice(&off.to_le_bytes());
                        data.extend_from_slice(v);
                        if data.len() % 2 == 1 {
                            data.push(0);
                        }
                    }
                }
                None => {
                    out.extend_from_slice(&4u16.to_le_bytes());
                    out.extend_from_slice(&1u32.to_le_bytes());
                    out.extend_from_slice(&long.to_le_bytes());
                }
            }
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&data);
        out
    }
    let ifd0_entries = vec![ascii(0x010F, "SynthCam"), ascii(0x0110, "Model S")];
    // IFD0 at 8; its size decides where the Exif IFD goes.
    let probe = ifd(&ifd0_entries, 8, Some((0x8769, 0)));
    let exif_at = 8 + probe.len() as u32;
    let ifd0 = ifd(&ifd0_entries, 8, Some((0x8769, exif_at)));
    let exif_entries = vec![
        ascii(0x9003, time),
        ascii(0x9291, subsec),
        ascii(0xA431, serial),
    ];
    let exif_ifd = ifd(&exif_entries, exif_at, None);
    let mut tiff = b"II*\0".to_vec();
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&ifd0);
    tiff.extend_from_slice(&exif_ifd);
    let mut app1 = vec![0xFF, 0xE1];
    app1.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&tiff);
    app1
}

fn jpeg(px: &[u8], w: usize, h: usize, q: u8, exif_block: Option<&[u8]>) -> Vec<u8> {
    let j = specular_thumbs::codec::encode_jpeg(px, w, h, q);
    match exif_block {
        Some(e) => {
            let mut v = j[..2].to_vec();
            v.extend_from_slice(e);
            v.extend_from_slice(&j[2..]);
            v
        }
        None => j,
    }
}

fn png(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w as u32, h as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(px).unwrap();
    }
    out
}

/// Ground truth of one file.
#[derive(Clone, Debug)]
struct Truth {
    /// Duplicate set (an original and its copies).
    dup: Option<u32>,
    /// Similar set (a burst, a crop series).
    similar: Option<u32>,
    /// The member a good suggestion keeps (one per set).
    best: bool,
    /// Rated in a sidecar: must be kept.
    protected: bool,
}

fn write(dir: &Path, name: &str, data: &[u8], truth: Truth, all: &mut Vec<(PathBuf, Truth)>) {
    let p = dir.join(name);
    std::fs::write(&p, data).unwrap();
    all.push((p, truth));
}

fn make_corpus(dir: &Path) -> Vec<(PathBuf, Truth)> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let mut all = Vec::new();
    let none = Truth {
        dup: None,
        similar: None,
        best: false,
        protected: false,
    };
    let mut set = 0u32;
    // Duplicates: 16 scenes × original + variants.
    for k in 0..16u64 {
        set += 1;
        let s = scene(1000 + k);
        let px = render(&s, (0.0, 0.0), (0.0, 0.0));
        let t = |best| Truth {
            dup: Some(set),
            best,
            ..none.clone()
        };
        let orig = jpeg(&px, W, H, 95, None);
        // A lossless file of the source pixels beats the JPEG.
        write(
            dir,
            &format!("dup{k:02}.jpg"),
            &orig,
            t(k % 4 != 2),
            &mut all,
        );
        match k % 4 {
            0 => write(
                dir,
                &format!("dup{k:02} copy.jpg"),
                &orig,
                t(false),
                &mut all,
            ),
            1 => {
                let (_, _, dec) = specular_thumbs::codec::decode_jpeg(&orig).unwrap();
                write(
                    dir,
                    &format!("dup{k:02}_export.png"),
                    &png(&dec, W, H),
                    t(false),
                    &mut all,
                );
            }
            2 => write(
                dir,
                &format!("dup{k:02}.png"),
                &png(&px, W, H),
                t(true),
                &mut all,
            ),
            _ => {}
        }
        write(
            dir,
            &format!("dup{k:02}_q70.jpg"),
            &jpeg(&px, W, H, 70, None),
            t(false),
            &mut all,
        );
        let half = resize(&px, W, H, W / 2, H / 2);
        write(
            dir,
            &format!("dup{k:02}-600.jpg"),
            &jpeg(&half, W / 2, H / 2, 85, None),
            t(false),
            &mut all,
        );
        if k % 2 == 0 {
            let r = resize(&px, W, H, 1024, 768);
            write(
                dir,
                &format!("dup{k:02}-1024.jpg"),
                &jpeg(&r, 1024, 768, 80, None),
                t(false),
                &mut all,
            );
        }
    }
    // Bursts: 12 scenes × 5 frames 0.12 s apart, one sharp, the others blurred, smeared or
    // moved. Frames of a burst can look alike, but are not duplicates.
    for k in 0..12u64 {
        set += 1;
        let s = scene(2000 + k);
        let sharp_at = (k % 5) as usize;
        let mut rng = Rng(k + 99);
        for f in 0..5usize {
            let cam = (rng.range(-6.0, 6.0), rng.range(-4.0, 4.0));
            let subj = (f as f32 * 3.0, 0.0);
            let mut px = render(&s, cam, subj);
            if f != sharp_at {
                px = match (f + k as usize) % 3 {
                    0 => blur(&px, W, H, 3, 3),
                    1 => blur(&px, W, H, 7, 0),
                    _ => blur(&px, W, H, 2, 2),
                };
            }
            let sub = format!("{:02}", 10 + f * 12);
            let e = exif(
                &format!("2025:06:{:02} 10:{:02}:07", 1 + k, 10 + k),
                &sub,
                "SN12345",
            );
            let protected = k == 3 && f == (sharp_at + 1) % 5;
            let name = format!("burst{k:02}_{f}.jpg");
            write(
                dir,
                &name,
                &jpeg(&px, W, H, 90, Some(&e)),
                Truth {
                    similar: Some(set),
                    best: f == sharp_at,
                    protected,
                    ..none.clone()
                },
                &mut all,
            );
            if protected {
                std::fs::write(
                    dir.join(format!("{name}.xmp")),
                    "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF><rdf:Description xmp:Rating=\"3\"/></rdf:RDF></x:xmpmeta>",
                )
                .unwrap();
            }
        }
    }
    // Crops: 6 scenes × original + 6 % and 12 % crops (similar, not duplicates).
    for k in 0..6u64 {
        set += 1;
        let s = scene(3000 + k);
        let px = render(&s, (0.0, 0.0), (0.0, 0.0));
        let t = |best| Truth {
            similar: Some(set),
            best,
            ..none.clone()
        };
        write(
            dir,
            &format!("crop{k:02}.jpg"),
            &jpeg(&px, W, H, 92, None),
            t(true),
            &mut all,
        );
        for pct in [6usize, 12] {
            let (cw, ch) = (W * (100 - pct) / 100, H * (100 - pct) / 100);
            let c = crop(&px, W, H, (W - cw) / 2, (H - ch) / 2, cw, ch);
            write(
                dir,
                &format!("crop{k:02}_c{pct}.jpg"),
                &jpeg(&c, cw, ch, 92, None),
                t(false),
                &mut all,
            );
        }
    }
    // Near-misses: 10 pairs sharing background and palette, hours apart.
    for k in 0..10u64 {
        let s = scene(4000 + k);
        let o = near_miss(&s, 5000 + k);
        for (i, sc) in [s, o].iter().enumerate() {
            let e = exif(
                &format!("2025:07:{:02} {:02}:00:00", 1 + k, 8 + 3 * i),
                "00",
                "SN12345",
            );
            let px = render(sc, (0.0, 0.0), (0.0, 0.0));
            write(
                dir,
                &format!("near{k:02}_{i}.jpg"),
                &jpeg(&px, W, H, 90, Some(&e)),
                none.clone(),
                &mut all,
            );
        }
    }
    // Pans: 8 scenes, three shots minutes apart with the camera moved 8 %, 16 %, 30 %.
    // Neither must group nor must not: they show what the sensitivity does.
    for k in 0..8u64 {
        let s = scene(7000 + k);
        for (i, pan) in [0.0f32, 0.08, 0.16, 0.30].iter().enumerate() {
            let e = exif(
                &format!("2025:08:{:02} 12:{:02}:00", 1 + k, 2 * i),
                "00",
                "SN12345",
            );
            let px = render(&s, (pan * W as f32, 0.0), (0.0, 0.0));
            write(
                dir,
                &format!("pan{k:02}_{i}.jpg"),
                &jpeg(&px, W, H, 90, Some(&e)),
                none.clone(),
                &mut all,
            );
        }
    }
    // Singletons.
    for k in 0..20u64 {
        let px = render(&scene(6000 + k), (0.0, 0.0), (0.0, 0.0));
        write(
            dir,
            &format!("single{k:02}.jpg"),
            &jpeg(&px, W, H, 90, None),
            none.clone(),
            &mut all,
        );
    }
    all
}

fn pairs_of(groups: &[Vec<usize>]) -> HashSet<(usize, usize)> {
    let mut s = HashSet::new();
    for g in groups {
        for (i, &a) in g.iter().enumerate() {
            for &b in &g[i + 1..] {
                s.insert((a.min(b), a.max(b)));
            }
        }
    }
    s
}

fn pr(pred: &HashSet<(usize, usize)>, truth: &HashSet<(usize, usize)>) -> (f64, f64, usize, usize) {
    let tp = pred.intersection(truth).count();
    let p = if pred.is_empty() {
        1.0
    } else {
        tp as f64 / pred.len() as f64
    };
    let r = if truth.is_empty() {
        1.0
    } else {
        tp as f64 / truth.len() as f64
    };
    (p, r, pred.len() - tp, truth.len() - tp)
}

fn setup(scratch: &Path, threads: usize) -> ScanSetup {
    let cache = Arc::new(specular_thumbs::DiskCache::new(
        scratch.join("cache"),
        1 << 30,
    ));
    let registry = Arc::new(specular_decode::Registry::new());
    let pool = Arc::new(specular_core::JobPool::new(threads, "thumbs"));
    let keys = specular_thumbs::KeyMemo::default();
    let (engine, wake) = scan::engine(registry, pool, Some(cache), 0, true, keys.clone(), None);
    ScanSetup {
        engine,
        wake,
        store: Arc::new(Mutex::new(Store::open(&scratch.join("similar")))),
        cancel: Arc::new(AtomicBool::new(false)),
        keys,
        tier3: Default::default(),
    }
}

fn report_timings(label: &str, r: &scan::Results) {
    let t = &r.timings;
    println!(
        "{label}: {} units, total {:.2} s (groups at {:.2} s); hashed {} ({:.0} µs each), digests {} ({} from cache, \
         {:.0} µs per look-up), hints {:.0} µs each, reused {}; \
         tier 0: {} files {:.1} ms; verify: {} pairs {:.1} ms; grouping {:.1} ms; \
         focus: {} files {:.1} ms; failed {}",
        r.items.len(),
        t.total.as_secs_f64(),
        t.groups.as_secs_f64(),
        t.hashed,
        t.hash_time.as_secs_f64() * 1e6 / t.hashed.max(1) as f64,
        t.digests,
        t.digests_cached,
        t.digest_time.as_secs_f64() * 1e6 / (t.hashed + t.digests).max(1) as f64,
        t.hint_time.as_secs_f64() * 1e6 / (t.hashed + t.digests).max(1) as f64,
        t.reused,
        t.tier0_files,
        t.tier0_time.as_secs_f64() * 1e3,
        t.verified,
        t.verify_time.as_secs_f64() * 1e3,
        t.group_time.as_secs_f64() * 1e3,
        t.focused,
        t.focus_time.as_secs_f64() * 1e3,
        r.failed,
    );
}

fn corpus(scratch: &Path, real: Option<&Path>) {
    let dir = scratch.join("corpus");
    let t0 = Instant::now();
    let truth = make_corpus(&dir);
    println!(
        "corpus: {} files in {:.1} s",
        truth.len(),
        t0.elapsed().as_secs_f64()
    );
    let _ = std::fs::remove_dir_all(scratch.join("cache"));
    let _ = std::fs::remove_dir_all(scratch.join("similar"));
    let paths: Vec<PathBuf> = truth.iter().map(|t| t.0.clone()).collect();
    let protect = |u: &scan::Unit| scan::sidecar_protect(&u.files);
    let s = setup(scratch, 4);
    let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
    report_timings("cold (no cache, no store)", &r);
    drop(s);
    // Cache warm, store empty: hashing from cached thumbnails.
    let _ = std::fs::remove_dir_all(scratch.join("similar"));
    let s = setup(scratch, 4);
    let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
    report_timings("cache warm, store empty", &r);
    drop(s);
    let s = setup(scratch, 4);
    let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
    report_timings("warm (store hits)", &r);

    let file_of: HashMap<&Path, usize> = truth
        .iter()
        .enumerate()
        .map(|(i, t)| (t.0.as_path(), i))
        .collect();
    let idx = |item: u32| file_of[r.units[item as usize].main.as_path()];
    let truth_pairs = |f: &dyn Fn(&Truth) -> Option<u32>| {
        let mut by: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, t) in truth.iter().enumerate() {
            if let Some(k) = f(&t.1) {
                by.entry(k).or_default().push(i);
            }
        }
        pairs_of(&by.into_values().collect::<Vec<_>>())
    };
    let dup_truth = truth_pairs(&|t| t.dup);
    let sim_truth = truth_pairs(&|t| t.similar);
    let dup_groups: Vec<Vec<usize>> = r
        .duplicates
        .iter()
        .map(|g| g.members.iter().map(|&m| idx(m)).collect())
        .collect();
    let (p, rc, fp, fneg) = pr(&pairs_of(&dup_groups), &dup_truth);
    println!(
        "duplicates: {} groups; pairwise precision {:.3}, recall {:.3} ({} false, {} missed)",
        dup_groups.len(),
        p,
        rc,
        fp,
        fneg
    );
    // SSIM of the 256 px thumbnails by pair kind (calibrates the verify threshold).
    let cache = specular_thumbs::DiskCache::new(scratch.join("cache"), 0);
    let luma = |p: &Path| {
        let k = specular_thumbs::ThumbKey::of_file(p).ok()?;
        let e = cache.get(&k, specular_similar::SIG_CLASS, 0)?;
        Some(specular_similar::Luma::from_rgba(
            e.pixels.width,
            e.pixels.height,
            &e.pixels.rgba,
        ))
    };
    let lumas: Vec<_> = truth.iter().map(|t| luma(&t.0)).collect();
    let mut kinds: HashMap<&str, Vec<f32>> = HashMap::new();
    for a in 0..truth.len() {
        for b in a + 1..truth.len() {
            let (ta, tb) = (&truth[a], &truth[b]);
            let na = ta.0.file_name().unwrap().to_string_lossy();
            let nb = tb.0.file_name().unwrap().to_string_lossy();
            let kind = if ta.1.dup.is_some() && ta.1.dup == tb.1.dup {
                "duplicate"
            } else if ta.1.similar.is_some() && ta.1.similar == tb.1.similar {
                if na.starts_with("burst") {
                    "burst"
                } else {
                    "crop"
                }
            } else if na.starts_with("near") && nb.starts_with("near") && na[..6] == nb[..6] {
                "near-miss"
            } else {
                continue;
            };
            if let (Some(la), Some(lb)) = (&lumas[a], &lumas[b]) {
                kinds
                    .entry(kind)
                    .or_default()
                    .push(specular_similar::ssim::ssim(la, lb));
            }
        }
    }
    for (k, mut v) in kinds {
        v.sort_by(f32::total_cmp);
        println!(
            "ssim {k}: n {} min {:.3} median {:.3} max {:.3}",
            v.len(),
            v[0],
            v[v.len() / 2],
            v[v.len() - 1]
        );
    }
    for (a, b) in pairs_of(&dup_groups).difference(&dup_truth) {
        println!(
            "  false dup: {} ~ {}",
            truth[*a].0.display(),
            truth[*b].0.display()
        );
    }
    let item_of: HashMap<usize, usize> = (0..r.items.len()).map(|i| (idx(i as u32), i)).collect();
    let diag = |a: usize, b: usize| {
        let (Some(&x), Some(&y)) = (item_of.get(&a), item_of.get(&b)) else {
            return "(no item)".to_string();
        };
        let (sa, sb) = (&r.items[x].sig, &r.items[y].sig);
        let v = s.store.lock().unwrap().verified(r.ids[x], r.ids[y]);
        format!(
            "ph {} dh {} col {:.3} asp {:.3} vis {:.3} ssim {:?} t1 {}",
            sa.phash_distance(sb),
            sa.dhash_distance(sb),
            sa.colour_distance(sb),
            sa.aspect_difference(sb),
            sa.visual_distance(sb),
            v,
            sa.tier1 == sb.tier1
        )
    };
    for (a, b) in dup_truth.difference(&pairs_of(&dup_groups)) {
        println!(
            "  missed dup: {} ~ {}: {}",
            truth[*a].0.file_name().unwrap().to_string_lossy(),
            truth[*b].0.file_name().unwrap().to_string_lossy(),
            diag(*a, *b)
        );
    }
    // Best picks of duplicates: the original kept, nothing protected discarded.
    let mut dup_best = 0;
    for g in &r.duplicates {
        if g.members
            .iter()
            .zip(&g.picks)
            .any(|(&m, p)| p.keep && truth[idx(m)].1.best)
        {
            dup_best += 1;
        }
    }
    println!(
        "duplicates: original kept in {dup_best}/{}",
        r.duplicates.len()
    );
    for sens in [0.2f32, 0.35, 0.5, 0.65, 0.8] {
        let split = |_a: u32, _b: u32| false;
        let groups = r.similar(sens, &split);
        let gs: Vec<Vec<usize>> = groups
            .iter()
            .map(|g| g.members.iter().map(|&m| idx(m)).collect())
            .collect();
        // Pairs within one duplicate set are not wrong here (copies look alike).
        let pred: HashSet<_> = pairs_of(&gs)
            .into_iter()
            .filter(|&(a, b)| truth[a].1.dup.is_none() || truth[a].1.dup != truth[b].1.dup)
            .collect();
        let mut best = 0;
        let mut prot_ok = true;
        for g in &groups {
            for (&m, pk) in g.members.iter().zip(&g.picks) {
                let t = &truth[idx(m)].1;
                if t.best && pk.keep {
                    best += 1;
                }
                if t.protected && !pk.keep {
                    prot_ok = false;
                }
            }
        }
        let pans: HashSet<_> = pairs_of(&gs)
            .into_iter()
            .filter(|&(a, b)| {
                let (na, nb) = (
                    truth[a].0.file_name().unwrap().to_string_lossy(),
                    truth[b].0.file_name().unwrap().to_string_lossy(),
                );
                na.starts_with("pan") && na[..5] == nb[..5]
            })
            .collect();
        let pred: HashSet<_> = pred.difference(&pans).copied().collect();
        let (p, rc, fp, fneg) = pr(&pred, &sim_truth);
        println!(
            "similar @ {sens:.2}: {} groups; pairwise precision {:.3}, recall {:.3} ({} false, {} missed); \
             pan pairs grouped {}/48; sharpest/original kept in {best}/{}; protected kept: {prot_ok}",
            groups.len(),
            p,
            rc,
            fp,
            fneg,
            pans.len(),
            18
        );
        if (sens - 0.5).abs() < 1e-3 {
            for (a, b) in pred.difference(&sim_truth).take(12) {
                println!(
                    "  false similar: {} ~ {}",
                    truth[*a].0.display(),
                    truth[*b].0.display()
                );
            }
            for (a, b) in sim_truth.difference(&pred).take(12) {
                println!(
                    "  missed similar: {} ~ {}: {}",
                    truth[*a].0.file_name().unwrap().to_string_lossy(),
                    truth[*b].0.file_name().unwrap().to_string_lossy(),
                    diag(*a, *b)
                );
            }
            for g in groups.iter().take(4) {
                let row: Vec<String> = g
                    .members
                    .iter()
                    .zip(&g.picks)
                    .map(|(&m, p)| {
                        format!(
                            "{} {} {:?} {:.3}",
                            r.units[m as usize]
                                .main
                                .file_name()
                                .unwrap()
                                .to_string_lossy(),
                            if p.keep { "keep" } else { "discard" },
                            p.reason,
                            p.score
                        )
                    })
                    .collect();
                println!("  group: {}", row.join(" | "));
            }
        }
    }
    drop(s);
    if let Some(real) = real {
        real_dir(scratch, real);
    }
}

fn real_dir(scratch: &Path, real: &Path) {
    let mut paths = Vec::new();
    let mut stack = vec![real.to_path_buf()];
    let reg = specular_decode::Registry::new();
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if reg.supports_extension(&p) {
                paths.push(p);
            }
        }
    }
    paths.sort();
    println!("real: {} files (read only)", paths.len());
    let scratch = scratch.join("real");
    let _ = std::fs::remove_dir_all(&scratch);
    let protect = |u: &scan::Unit| scan::sidecar_protect(&u.files);
    for label in ["real cold", "real cache warm, store empty", "real warm"] {
        if label.contains("store empty") {
            let _ = std::fs::remove_dir_all(scratch.join("similar"));
        }
        let s = setup(&scratch, 4);
        let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
        report_timings(label, &r);
        if label == "real warm" {
            let reclaim: u64 = r.duplicates.iter().map(|g| g.reclaim).sum();
            println!(
                "real: {} duplicate groups ({} MB reclaimable), {} edges",
                r.duplicates.len(),
                reclaim >> 20,
                r.edges.len()
            );
            for g in &r.duplicates {
                let names: Vec<String> = g
                    .members
                    .iter()
                    .zip(&g.picks)
                    .map(|(&m, p)| {
                        let k = if p.keep { "keep" } else { "discard" };
                        format!("{} {k} {:?}", r.items[m as usize].name, p.reason)
                    })
                    .collect();
                println!("  real dup: {}", names.join(" | "));
            }
            for sens in [0.2f32, 0.5, 0.8] {
                let t = Instant::now();
                let g = r.similar(sens, &|_, _| false);
                let members: usize = g.iter().map(|g| g.members.len()).sum();
                println!(
                    "real similar @ {sens}: {} groups, {} members, regroup {:.2} ms",
                    g.len(),
                    members,
                    t.elapsed().as_secs_f64() * 1e3
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// Synthetic signatures: clusters of near copies and bursts among unrelated images.
fn synthetic_items(n: usize) -> Vec<Item> {
    let mut r = Rng(42);
    let mut items = Vec::with_capacity(n);
    let mut t = 1_600_000_000_000i64;
    while items.len() < n {
        let base_ph = r.next();
        let base_dh = [r.next(), r.next(), r.next(), r.next()];
        let moments = [
            (r.next() % 256) as u8,
            (r.next() % 256) as u8,
            (r.next() % 256) as u8,
            (r.next() % 128) as u8,
            (r.next() % 64) as u8,
            (r.next() % 64) as u8,
        ];
        t += (r.next() % 600_000) as i64;
        let k = 1 + (r.next() % 4) as usize;
        for j in 0..k {
            let mut ph = base_ph;
            let mut dh = base_dh;
            for _ in 0..(j * 3) {
                ph ^= 1 << (r.next() % 64);
                dh[(r.next() % 4) as usize] ^= 1 << (r.next() % 64);
            }
            items.push(Item {
                name: format!("{}.jpg", items.len()),
                width: 4000,
                height: 3000,
                len: 3_000_000 + r.next() % 1_000_000,
                bytes: 3_000_000,
                sig: Signature {
                    tier1: r.next(),
                    phash: ph,
                    dhash: dh,
                    moments,
                    aspect: 4.0 / 3.0,
                    ..Signature::default()
                },
                hints: Hints {
                    time_ms: Some(t + j as i64 * 150),
                    subsec: true,
                    ..Hints::default()
                },
                ..Item::default()
            });
        }
    }
    items.truncate(n);
    items
}

fn bench() {
    for n in [10_000usize, 100_000] {
        let items = synthetic_items(n);
        let t0 = Instant::now();
        let index = HashIndex::new(items.iter().map(|i| i.sig.phash).collect());
        let t_index = t0.elapsed();
        let t1 = Instant::now();
        let none = |_: u32, _: u32| false;
        let cands = group::duplicate_candidates(&items, &index, &none);
        let t_cands = t1.elapsed();
        let t2 = Instant::now();
        let edges = group::similar_edges(&items, &index, &[]);
        let t_edges = t2.elapsed();
        let t3 = Instant::now();
        let groups = analyse::similar(&items, &edges, 0.5, &none);
        let t_cluster = t3.elapsed();
        let t4 = Instant::now();
        let mut verify = |_: &group::DupCandidate| Some(1.0f32);
        let dups = analyse::duplicates(&items, &index, &none, &mut verify);
        let t_dups = t4.elapsed();
        let _ = Category::Similar;
        println!(
            "{n} items: index {:.1} ms, duplicate candidates {:.1} ms ({}), duplicate groups (verify stubbed) {:.1} ms ({}), \
             similar edges {:.1} ms ({}), regroup at a slider move {:.1} ms ({} groups)",
            t_index.as_secs_f64() * 1e3,
            t_cands.as_secs_f64() * 1e3,
            cands.len(),
            t_dups.as_secs_f64() * 1e3,
            dups.len(),
            t_edges.as_secs_f64() * 1e3,
            edges.len(),
            t_cluster.as_secs_f64() * 1e3,
            groups.len()
        );
    }
    // Hashing cost per 256 px thumbnail.
    let s = scene(1);
    let px = render(&s, (0.0, 0.0), (0.0, 0.0));
    let th = resize(&px, W, H, 256, 192);
    let t = Instant::now();
    let reps = 200;
    for _ in 0..reps {
        std::hint::black_box(Signature::of_rgba(256, 192, std::hint::black_box(&th)));
    }
    println!(
        "signature of a 256×192 thumbnail: {:.0} µs",
        t.elapsed().as_secs_f64() * 1e6 / reps as f64
    );
    let a = specular_similar::Luma::from_rgba(256, 192, &th);
    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(specular_similar::ssim::ssim(&a, std::hint::black_box(&a)));
    }
    println!(
        "SSIM of two 256×192 thumbnails: {:.0} µs",
        t.elapsed().as_secs_f64() * 1e6 / reps as f64
    );
}

/// Files in the `--folder` corpus.
const FOLDER_FILES: usize = 2000;

/// A folder of [`FOLDER_FILES`] images for the scan timings: mostly unrelated shots, with
/// byte copies, re-encodes, resizes, PNG exports and bursts among them. Written once (a
/// `done` marker), so runs before and after a change see the same files.
fn make_folder(dir: &Path) -> Vec<PathBuf> {
    let marker = dir.join("done");
    if !marker.is_file() {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        let t0 = Instant::now();
        // (name, scene seed, variant), rendered on a few threads.
        let mut jobs: Vec<(String, u64, u8)> = Vec::new();
        for k in 0..1500u64 {
            jobs.push((format!("img{k:04}.jpg"), 10_000 + k, 0));
        }
        // Copies of the first 150: byte copy, q70 re-encode, half size.
        for k in 0..150u64 {
            jobs.push((
                format!("img{k:04}_v{}.jpg", k % 3),
                10_000 + k,
                1 + (k % 3) as u8,
            ));
        }
        for k in 0..50u64 {
            jobs.push((format!("img{:04}_export.png", 200 + k), 10_200 + k, 4));
            jobs.push((format!("png{k:04}.png"), 20_000 + k, 5));
        }
        for k in 0..50u64 {
            for f in 0..5u8 {
                jobs.push((format!("burst{k:02}_{f}.jpg"), 30_000 + k, 10 + f));
            }
        }
        assert_eq!(jobs.len(), FOLDER_FILES);
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..3 {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((name, seed, v)) = jobs.get(i) else {
                            break;
                        };
                        let s = scene(*seed);
                        let flat = || render(&s, (0.0, 0.0), (0.0, 0.0));
                        let data = match *v {
                            // 1: a byte copy (the same encode as 0).
                            0 | 1 => jpeg(&flat(), W, H, 90, None),
                            2 => jpeg(&flat(), W, H, 70, None),
                            3 => jpeg(&resize(&flat(), W, H, W / 2, H / 2), W / 2, H / 2, 85, None),
                            4 => {
                                let j = jpeg(&flat(), W, H, 90, None);
                                let (_, _, dec) = specular_thumbs::codec::decode_jpeg(&j).unwrap();
                                png(&dec, W, H)
                            }
                            5 => png(&flat(), W, H),
                            f => {
                                let f = usize::from(f - 10);
                                let mut px =
                                    render(&s, (f as f32 * 2.0, 0.0), (f as f32 * 3.0, 0.0));
                                if f != 2 {
                                    px = blur(&px, W, H, 1 + f, 1);
                                }
                                let e = exif(
                                    &format!("2025:06:{:02} 10:{:02}:07", 1 + seed % 28, seed % 60),
                                    &format!("{:02}", 10 + f * 12),
                                    "SN12345",
                                );
                                jpeg(&px, W, H, 90, Some(&e))
                            }
                        };
                        std::fs::write(dir.join(name), data).unwrap();
                    }
                });
            }
        });
        std::fs::write(&marker, b"").unwrap();
        println!(
            "folder: wrote {FOLDER_FILES} files in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg" || e == "png"))
        .collect();
    paths.sort();
    paths
}

/// The gallery's engine configuration (default, as `gallery_prepare` makes it).
fn gallery_config() -> specular_thumbs::EngineConfig {
    let mut cfg = specular_thumbs::EngineConfig {
        workers: 4,
        ..Default::default()
    };
    cfg.generate.prefer_embedded = true;
    // As the app's gallery: class-256 tiles carry their signatures.
    cfg.digester = Some(scan::digester());
    cfg
}

/// Fills the cache as the gallery does when it shows every file at class [`SIG_CLASS`]
/// (the class of 160 px tiles at scale 1). Returns the seconds taken.
fn gallery_fill(scratch: &Path, paths: &[PathBuf]) -> f64 {
    let cache = Arc::new(specular_thumbs::DiskCache::new(
        scratch.join("cache"),
        1 << 30,
    ));
    let registry = Arc::new(specular_decode::Registry::new());
    let pool = Arc::new(specular_core::JobPool::new(4, "thumbs"));
    let (tx, rx) = crossbeam_channel::bounded::<()>(1);
    let e = specular_thumbs::Engine::new(
        gallery_config(),
        registry,
        pool,
        Arc::new(specular_thumbs::engine::FileOpener),
        Some(cache),
        Arc::new(move || {
            let _ = tx.try_send(());
        }),
    );
    let t0 = Instant::now();
    e.want(
        paths
            .iter()
            .map(|p| specular_thumbs::Request {
                path: p.clone(),
                class: specular_similar::SIG_CLASS,
            })
            .collect(),
        0,
    );
    let mut left = paths.len();
    while left > 0 {
        let _ = rx.recv_timeout(std::time::Duration::from_millis(100));
        for o in e.take_ready() {
            match o {
                specular_thumbs::engine::Outcome::Ready(r) if r.sharp => left -= 1,
                specular_thumbs::engine::Outcome::Failed { .. } => left -= 1,
                _ => {}
            }
        }
    }
    t0.elapsed().as_secs_f64()
}

/// Signatures (hex, by file name) and the groups of a scan, for comparing runs.
fn dump(r: &scan::Results, out: &Path) {
    let names = |m: &[u32]| {
        let mut n: Vec<&str> = m
            .iter()
            .map(|&m| r.items[m as usize].name.as_str())
            .collect();
        n.sort_unstable();
        n.join(" ")
    };
    let mut lines: Vec<String> = r
        .units
        .iter()
        .zip(&r.items)
        .map(|(u, it)| {
            let hex: String = it
                .sig
                .to_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            format!(
                "s\t{}\t{hex}",
                u.main.file_name().unwrap().to_string_lossy()
            )
        })
        .collect();
    lines.sort();
    let mut groups: Vec<String> = r
        .duplicates
        .iter()
        .map(|g| format!("d\t{}", names(&g.members)))
        .collect();
    for sens in [0.2f32, 0.5, 0.8] {
        groups.extend(
            r.similar(sens, &|_, _| false)
                .iter()
                .map(|g| format!("m{sens}\t{}", names(&g.members))),
        );
    }
    groups.sort();
    lines.extend(groups);
    std::fs::write(out, lines.join("\n") + "\n").unwrap();
}

/// `--compare a b`: how two dumps differ (signatures and groups).
fn compare(a: &Path, b: &Path) {
    type Dump = (HashMap<String, Signature>, HashSet<String>);
    let read = |p: &Path| -> Dump {
        let t = std::fs::read_to_string(p).unwrap();
        let mut sigs = HashMap::new();
        let mut groups = HashSet::new();
        for l in t.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f[0] == "s" {
                let bytes: Vec<u8> = (0..f[2].len() / 2)
                    .map(|i| u8::from_str_radix(&f[2][2 * i..2 * i + 2], 16).unwrap())
                    .collect();
                sigs.insert(f[1].to_string(), Signature::from_bytes(&bytes).unwrap());
            } else {
                groups.insert(l.to_string());
            }
        }
        (sigs, groups)
    };
    let (sa, ga) = read(a);
    let (sb, gb) = read(b);
    let (mut same, mut t1, mut n) = (0, 0, 0);
    let (mut ph, mut dh, mut vd) = (Vec::new(), Vec::new(), Vec::new());
    for (k, x) in &sa {
        let Some(y) = sb.get(k) else { continue };
        n += 1;
        same += usize::from(x.to_bytes() == y.to_bytes());
        t1 += usize::from(x.tier1 == y.tier1);
        ph.push(x.phash_distance(y));
        dh.push(x.dhash_distance(y));
        vd.push(x.visual_distance(y));
    }
    ph.sort_unstable();
    dh.sort_unstable();
    vd.sort_by(f32::total_cmp);
    let q = |v: &[u32], f: f64| {
        v.get(((v.len() as f64 - 1.0) * f) as usize)
            .copied()
            .unwrap_or(0)
    };
    println!(
        "{n} common files: {same} identical signatures, {t1} identical tier 1; \
         pHash distance median {} p95 {} max {}; dHash distance median {} p95 {} max {}; \
         visual distance median {:.4} max {:.4}",
        q(&ph, 0.5),
        q(&ph, 0.95),
        q(&ph, 1.0),
        q(&dh, 0.5),
        q(&dh, 0.95),
        q(&dh, 1.0),
        vd.get(vd.len() / 2).copied().unwrap_or(0.0),
        vd.last().copied().unwrap_or(0.0),
    );
    for kind in ["d", "m0.2", "m0.5", "m0.8"] {
        let of = |g: &HashSet<String>| -> HashSet<String> {
            g.iter()
                .filter(|l| l.split('\t').next() == Some(kind))
                .cloned()
                .collect()
        };
        let (x, y) = (of(&ga), of(&gb));
        println!(
            "groups {kind}: {} vs {}, {} only in the first, {} only in the second",
            x.len(),
            y.len(),
            x.difference(&y).count(),
            y.difference(&x).count()
        );
        for l in x.symmetric_difference(&y).take(6) {
            println!("  {l}");
        }
    }
}

/// `--folder <scratch> <tag> [runs]`: scan timings over the generated folder: cold (no
/// thumbnail cache, no store), with the cache the scan left, warm (store hits), and with a
/// cache the gallery filled. The first run writes dumps for `--compare`.
fn folder(scratch: &Path, tag: &str, runs: usize) {
    let paths = make_folder(&scratch.join("folder"));
    println!("folder: {} files", paths.len());
    let protect = |u: &scan::Unit| scan::sidecar_protect(&u.files);
    let mut times: HashMap<&str, Vec<f64>> = HashMap::new();
    let clear = |what: &[&str]| {
        for w in what {
            let _ = std::fs::remove_dir_all(scratch.join(w));
        }
    };
    let run = |label: &'static str, dump_to: Option<&str>, times: &mut HashMap<&str, Vec<f64>>| {
        let s = setup(scratch, 4);
        let t0 = Instant::now();
        let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
        let t = t0.elapsed().as_secs_f64();
        report_timings(label, &r);
        times.entry(label).or_default().push(t);
        times
            .entry(first_groups(label))
            .or_default()
            .push(r.timings.groups.as_secs_f64());
        if let Some(d) = dump_to {
            dump(&r, &scratch.join(format!("dump-{tag}-{d}.txt")));
        }
    };
    for i in 0..runs {
        let first = i == 0;
        clear(&["cache", "similar"]);
        run("cold", first.then_some("cold"), &mut times);
        clear(&["similar"]);
        run(
            "scan cache, store empty",
            first.then_some("scancache"),
            &mut times,
        );
        run("warm (store hits)", None, &mut times);
        clear(&["cache", "similar"]);
        let fill = gallery_fill(scratch, &paths);
        println!(
            "gallery fill (class {}): {fill:.2} s",
            specular_similar::SIG_CLASS
        );
        times.entry("gallery fill").or_default().push(fill);
        run(
            "gallery cache, store empty",
            first.then_some("gallerycache"),
            &mut times,
        );
    }
    let mut keys: Vec<_> = times.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        let mut v = times[k].clone();
        v.sort_by(f64::total_cmp);
        println!(
            "{tag} {k}: median {:.2} s (runs {:?})",
            v[v.len() / 2],
            v.iter()
                .map(|x| (x * 100.0).round() / 100.0)
                .collect::<Vec<_>>()
        );
    }
}

/// The timings label of the time to the first groups of a scan labelled `label` (before the
/// focus pass, [`scan::run_groups`]).
fn first_groups(label: &str) -> &'static str {
    match label {
        "cold" => "cold, first groups",
        "scan cache, store empty" => "scan cache, store empty, first groups",
        "warm (store hits)" => "warm (store hits), first groups",
        "gallery cache, store empty" => "gallery cache, store empty, first groups",
        _ => "first groups",
    }
}

/// `--bestpick` frames: camera size.
const BW: usize = 4000;
const BH: usize = 3000;
/// `--bestpick` bursts of 5.
const BEST_BURSTS: u64 = 8;

fn hash3(x: u64, y: u64, seed: u64) -> u64 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 31;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// A `BW`×`BH` burst frame: the scene (its `W`×`H` layout scaled up) with a fine texture on a
/// 3 px lattice fixed to it, the camera moved by `shift` px, defocused by a box of radius
/// `blur_r` (0: sharp), and sensor noise of about `sigma` added after the optics.
fn big_frame(s: &Scene, shift: usize, blur_r: usize, sigma: f32, seed: u64) -> Vec<u8> {
    let (kx, ky) = (W as f32 / BW as f32, H as f32 / BH as f32);
    let mut px = vec![255u8; BW * BH * 4];
    for y in 0..BH {
        for x in 0..BW {
            let sx = x + shift;
            let c = colour_at(s, sx as f32 * kx, y as f32 * ky, (0.0, 0.0));
            let (gx, gy) = (sx / 3, y / 3);
            let (tx, ty) = ((sx % 3) as f32 / 3.0, (y % 3) as f32 / 3.0);
            let g = |x: usize, y: usize| (hash3(x as u64, y as u64, s.seed) & 31) as f32 - 15.5;
            let tex = (g(gx, gy) * (1.0 - tx) + g(gx + 1, gy) * tx) * (1.0 - ty)
                + (g(gx, gy + 1) * (1.0 - tx) + g(gx + 1, gy + 1) * tx) * ty;
            let o = (y * BW + x) * 4;
            for k in 0..3 {
                px[o + k] = (c[k] + tex).clamp(0.0, 255.0) as u8;
            }
        }
    }
    if blur_r > 0 {
        px = blur(&px, BW, BH, blur_r, blur_r);
    }
    for (i, p) in px.chunks_exact_mut(4).enumerate() {
        let h = hash3(i as u64, 1, seed);
        for (k, v) in p[..3].iter_mut().enumerate() {
            let u = |j: usize| ((h >> (16 * ((j + k) % 4))) & 0xFFFF) as f32 / 65535.0 - 0.5;
            // Sum of four uniforms, unit variance.
            let g = (u(0) + u(1) + u(2) + u(3)) * 3f32.sqrt();
            *v = (f32::from(*v) + sigma * g).clamp(0.0, 255.0) as u8;
        }
    }
    px
}

/// What a `--bestpick` frame is.
#[derive(Clone, Copy, PartialEq, Debug)]
enum BestRole {
    Sharp,
    /// Defocused by a box of this radius.
    Blurred(usize),
    /// Sharp, with three times the noise.
    Noisy,
}

/// Burst `k`, frame `f`: one frame defocused (radius 1 or 2), and in every other burst one
/// sharp frame noisy.
fn best_role(k: u64, f: u64) -> BestRole {
    let b = k % 5;
    if f == b {
        BestRole::Blurred(1 + (k % 2) as usize)
    } else if k % 2 == 1 && f == (b + 2) % 5 {
        BestRole::Noisy
    } else {
        BestRole::Sharp
    }
}

/// `--bestpick <scratch> [runs]`: best-pick on bursts of 12 MP frames (blur under the
/// thumbnail's resolving limit, noisy frames) through the real scan: how often a sharp, clean
/// frame is kept and the blurred one discarded for blur, and what the scan costs.
fn bestpick(scratch: &Path, runs: usize) {
    use specular_similar::pick::Reason;
    let dir = scratch.join("bestpick");
    let marker = dir.join("done");
    if !marker.is_file() {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let t0 = Instant::now();
        let jobs: Vec<(u64, u64)> = (0..BEST_BURSTS)
            .flat_map(|k| (0..5).map(move |f| (k, f)))
            .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..3 {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&(k, f)) = jobs.get(i) else { break };
                        let s = scene(40_000 + k);
                        let (r, sigma) = match best_role(k, f) {
                            BestRole::Sharp => (0, 2.0),
                            BestRole::Blurred(r) => (r, 2.0),
                            BestRole::Noisy => (0, 6.0),
                        };
                        let px = big_frame(&s, 3 * f as usize, r, sigma, k * 10 + f);
                        let e = exif(
                            &format!("2025:07:{:02} 11:{:02}:07", 1 + k, k * 3),
                            &format!("{:02}", 10 + f * 12),
                            "SN12345",
                        );
                        let data = jpeg(&px, BW, BH, 92, Some(&e));
                        std::fs::write(dir.join(format!("b{k}_{f}.jpg")), data).unwrap();
                    }
                });
            }
        });
        std::fs::write(&marker, b"").unwrap();
        println!(
            "bestpick: wrote {} frames in {:.1} s",
            jobs.len(),
            t0.elapsed().as_secs_f64()
        );
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
        .collect();
    paths.sort();
    let protect = |u: &scan::Unit| scan::sidecar_protect(&u.files);
    let mut times: HashMap<&str, Vec<f64>> = HashMap::new();
    for i in 0..runs {
        for (label, cold) in [("cold", true), ("warm (store hits)", false)] {
            if cold {
                let _ = std::fs::remove_dir_all(dir.join("cache"));
                let _ = std::fs::remove_dir_all(dir.join("similar"));
            }
            let s = setup(&dir, 4);
            let t0 = Instant::now();
            let r = scan::run(&s, scan::units(&paths), &protect, &mut |_| {}).unwrap();
            times
                .entry(label)
                .or_default()
                .push(t0.elapsed().as_secs_f64());
            times
                .entry(first_groups(label))
                .or_default()
                .push(r.timings.groups.as_secs_f64());
            report_timings(label, &r);
            if i > 0 || !cold {
                continue;
            }
            let role = |m: u32| {
                let n = r.units[m as usize].main.file_stem().unwrap();
                let n = n.to_string_lossy();
                let (k, f) = n[1..].split_once('_').unwrap();
                (
                    n.to_string(),
                    best_role(k.parse().unwrap(), f.parse().unwrap()),
                )
            };
            let groups = r.similar(0.5, &|_, _| false);
            let (mut sharp_won, mut blur_out, mut noisy_lost, mut members) = (0, 0, 0, 0);
            for g in &groups {
                members += g.members.len();
                let mut line = Vec::new();
                for (&m, p) in g.members.iter().zip(&g.picks) {
                    let (name, role) = role(m);
                    let mark = if p.keep { '+' } else { '-' };
                    line.push(format!("{name} {role:?} {mark}{:?}", p.reason));
                    let best = p.keep && p.reason != Reason::AlsoGood;
                    match role {
                        BestRole::Sharp if best => sharp_won += 1,
                        BestRole::Blurred(_) if !p.keep && p.reason == Reason::Blurrier => {
                            blur_out += 1
                        }
                        BestRole::Noisy if !best => noisy_lost += 1,
                        _ => {}
                    }
                }
                println!("  {}", line.join(", "));
            }
            println!(
                "bestpick: {} groups ({members} of {} frames); a sharp clean frame kept as the \
                 best in {sharp_won}/{BEST_BURSTS}; blurred discarded as blurrier \
                 {blur_out}/{BEST_BURSTS}; noisy not the best {noisy_lost}/{}",
                groups.len(),
                paths.len(),
                BEST_BURSTS / 2
            );
        }
    }
    let mut keys: Vec<_> = times.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        let mut v = times[k].clone();
        v.sort_by(f64::total_cmp);
        println!("bestpick {k}: median {:.2} s (runs {v:?})", v[v.len() / 2]);
    }
}

/// CPU time of this process so far, threads that ended included (Linux, in 10 ms ticks;
/// 0 elsewhere).
fn cpu_ns() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/stat") else {
        return 0;
    };
    // After the command name: state is field 3, utime and stime fields 14 and 15.
    let rest = s.rsplit_once(')').map_or("", |(_, r)| r);
    let f: Vec<u64> = rest
        .split_whitespace()
        .skip(11)
        .take(2)
        .filter_map(|v| v.parse().ok())
        .collect();
    f.iter().sum::<u64>() * 10_000_000
}

/// `--focus <runs> <file>...`: one file's focus measure (a fresh session each run) on 1, 2, 4
/// and 8 threads: median wall and CPU.
fn focus_times(runs: usize, files: &[String]) {
    let registry = specular_decode::Registry::new();
    let stop = AtomicBool::new(false);
    for f in files {
        let path = Path::new(f);
        for threads in [1, 2, 4, 8] {
            let (mut wall, mut cpu) = (Vec::new(), Vec::new());
            let mut got = None;
            for _ in 0..runs.max(1) {
                let (c0, t0) = (cpu_ns(), Instant::now());
                let s = registry.open_path(path).expect("open");
                got = specular_similar::focus::measure_session_with(&*s, None, threads, &stop);
                wall.push(t0.elapsed().as_secs_f64() * 1e3);
                cpu.push((cpu_ns() - c0) as f64 / 1e6);
            }
            let med = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            };
            let g = got.unwrap_or(specular_similar::focus::Focus::UNAVAILABLE);
            println!(
                "focus {} {} threads: wall {:.1} ms (median), cpu {:.1} ms (mean; {} runs; scale {}, {}, detail {:.3})",
                path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default(),
                threads,
                med(&mut wall),
                cpu.iter().sum::<f64>() / cpu.len() as f64,
                runs.max(1),
                g.scale,
                if g.regions { "regions" } else { "whole" },
                g.detail
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--focus") && args.len() >= 3 {
        focus_times(args[1].parse().unwrap_or(5), &args[2..]);
        return;
    }
    if args.first().map(String::as_str) == Some("--bench") {
        bench();
        return;
    }
    if args.first().map(String::as_str) == Some("--folder") && args.len() >= 3 {
        let runs = args.get(3).and_then(|r| r.parse().ok()).unwrap_or(3);
        folder(Path::new(&args[1]), &args[2], runs);
        return;
    }
    if args.first().map(String::as_str) == Some("--bestpick") && args.len() >= 2 {
        let runs = args.get(2).and_then(|r| r.parse().ok()).unwrap_or(3);
        bestpick(Path::new(&args[1]), runs);
        return;
    }
    if args.first().map(String::as_str) == Some("--compare") && args.len() == 3 {
        compare(Path::new(&args[1]), Path::new(&args[2]));
        return;
    }
    let Some(scratch) = args.first() else {
        eprintln!("usage: corpus <scratch dir> [real dir] | --bench");
        std::process::exit(2);
    };
    corpus(Path::new(scratch), args.get(1).map(Path::new));
}
