//! Malformed profiles never panic: mutated, truncated and corrupted versions of every
//! installed profile go through parsing, transform building and evaluation.

use specular_color::{
    CmykConverter, ColorSpace, Display, Intent, Profile, Transform, gpu, profiles,
};

/// The synthetic profiles committed as fuzz seeds (tools/fuzz/seeds.py `icc`).
const FUZZ_SEEDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/seeds/profile");

fn seeds() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for dir in [
        "/usr/share/color/icc",
        "/usr/share/color/icc/colord",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data"),
        FUZZ_SEEDS,
    ] {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut names: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if (ext == "icc" || ext == "icm")
                && std::fs::metadata(&p).is_ok_and(|m| m.len() < 2 << 20)
                && let Ok(d) = std::fs::read(&p)
            {
                out.push(d);
            }
        }
    }
    out
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn exercise(data: &[u8], heavy: bool) {
    let Ok(p) = Profile::parse(data) else { return };
    let srgb = profiles::srgb();
    let n = p.color_space.channels();
    for intent in [Intent::Perceptual, Intent::Relative, Intent::Absolute] {
        if let Ok(t) = Transform::new(&p, &srgb, intent, true) {
            let mut o = [0f32; 16];
            for v in [0.0, 0.5, 1.0, -1.0, 2.0, f32::NAN, f32::INFINITY] {
                t.eval(&vec![v; n.max(1)], &mut o);
            }
        }
        if let Ok(t) = Transform::new(&srgb, &p, intent, true) {
            let mut o = [0f32; 16];
            for v in [0.0, 0.5, 1.0, f32::NAN] {
                t.eval(&[v; 3], &mut o);
            }
        }
    }
    let _ = gpu::source_lut_for(&p, Intent::Relative, true);
    if heavy {
        let _ = Display::for_profile(&p, Intent::Relative, true);
        if p.color_space == ColorSpace::Cmyk {
            let _ =
                CmykConverter::new(&p, Intent::Relative, true).map(|c| c.convert([10, 20, 30, 40]));
        }
    }
}

#[test]
fn seeds_parse() {
    for s in seeds() {
        exercise(&s, true);
    }
}

#[test]
fn mutations_never_panic() {
    let seeds = seeds();
    if seeds.is_empty() {
        return;
    }
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let iterations = std::env::var("ICC_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1500);
    for it in 0..iterations {
        let mut d = seeds[rng.below(seeds.len())].clone();
        match rng.below(5) {
            0 => {
                for _ in 0..1 + rng.below(16) {
                    let i = rng.below(d.len());
                    d[i] = rng.next() as u8;
                }
            }
            1 => {
                let l = rng.below(d.len());
                d.truncate(l);
            }
            2 => {
                // Corrupt a tag-table entry (offset or size) with a large or edge value.
                let count = u32::from_be_bytes([d[128], d[129], d[130], d[131]]) as usize;
                if count > 0 {
                    let e = 132 + 12 * rng.below(count) + 4 + 4 * rng.below(2);
                    let v: u32 = match rng.below(4) {
                        0 => u32::MAX,
                        1 => 0,
                        2 => d.len() as u32,
                        _ => rng.next() as u32,
                    };
                    if e + 4 <= d.len() {
                        d[e..e + 4].copy_from_slice(&v.to_be_bytes());
                    }
                }
            }
            3 => {
                // Corrupt bytes inside a tag body: counts, grid sizes, offsets.
                let count = u32::from_be_bytes([d[128], d[129], d[130], d[131]]) as usize;
                if count > 0 {
                    let e = 132 + 12 * rng.below(count);
                    let off = u32::from_be_bytes([d[e + 4], d[e + 5], d[e + 6], d[e + 7]]) as usize;
                    for _ in 0..1 + rng.below(4) {
                        let i = off + rng.below(64);
                        if i < d.len() {
                            d[i] = [0, 1, 0xff, 0x80, rng.next() as u8][rng.below(5)];
                        }
                    }
                }
            }
            _ => {
                // Random garbage behind a valid-looking header.
                let keep = 132.min(d.len());
                d.truncate(keep);
                for _ in 0..rng.below(4096) {
                    d.push(rng.next() as u8);
                }
            }
        }
        exercise(&d, it % 50 == 0);
    }
}

#[test]
fn tiny_and_empty_inputs() {
    for len in 0..200 {
        let mut d = vec![0u8; len];
        if len >= 40 {
            d[36..40].copy_from_slice(b"acsp");
        }
        assert!(Profile::parse(&d).is_err());
    }
}

/// Every committed fuzz seed is a usable profile, so the fuzzer starts from deep paths: it
/// parses, converts to sRGB, and the LUT-based ones keep their tables.
#[test]
fn fuzz_seeds_are_usable() {
    let mut names: Vec<_> = std::fs::read_dir(FUZZ_SEEDS)
        .expect("fuzz seeds")
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    assert!(names.len() >= 10, "{names:?}");
    let srgb = profiles::srgb();
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let p = Profile::parse(&std::fs::read(&path).unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let t = Transform::new(&p, &srgb, Intent::Relative, true)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut o = [0f32; 3];
        t.eval(&[0.5; 4][..p.color_space.channels()], &mut o);
        assert!(o.iter().all(|v| v.is_finite()), "{name}: {o:?}");
        let expect = match &name[..name.find('_').unwrap()] {
            "rgb" => ColorSpace::Rgb,
            "gray" => ColorSpace::Gray,
            "cmyk" => ColorSpace::Cmyk,
            other => panic!("unexpected seed {other}"),
        };
        assert_eq!(p.color_space, expect, "{name}");
        assert_eq!(
            p.version,
            if name.contains("_v4") { 4 } else { 2 },
            "{name}"
        );
        if name.contains("lut") || name.contains("mab") {
            assert!(p.a2b[0].is_some(), "{name}: no A2B0");
        }
    }
}
