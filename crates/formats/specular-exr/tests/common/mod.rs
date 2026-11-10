//! Fixture generation with the `exr` crate (oracle) and comparison helpers.
#![allow(dead_code)]

pub mod dwa;

use std::io::Cursor;

use exr::prelude::*;

/// Deterministic pseudo-random generator (xorshift).
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A smooth gradient with noise, a few negatives and (if `specials`) inf/nan samples.
pub fn values(w: usize, h: usize, seed: u64, specials: bool) -> Vec<f32> {
    let mut rng = Rng(seed | 1);
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let base = (x as f32 / w.max(1) as f32) * 4.0 + (y as f32 / h.max(1) as f32) * 2.0;
            let mut s = base + (rng.f() - 0.5) * 0.3;
            if (x + y * 7) % 97 == 3 {
                s = -s;
            }
            if specials && (x * 13 + y) % 211 == 5 {
                s = [f32::INFINITY, f32::NEG_INFINITY, f32::NAN][(x + y) % 3];
            }
            v.push(s);
        }
    }
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    H,
    F,
    U,
}

pub fn samples(ty: Ty, v: &[f32]) -> FlatSamples {
    match ty {
        Ty::H => FlatSamples::F16(v.iter().map(|&x| f16::from_f32(x)).collect()),
        Ty::F => FlatSamples::F32(v.to_vec()),
        Ty::U => FlatSamples::U32(
            v.iter()
                .map(|&x| {
                    if x.is_finite() {
                        (x.abs() * 1000.0) as u32
                    } else {
                        7
                    }
                })
                .collect(),
        ),
    }
}

pub fn channel(
    name: &str,
    ty: Ty,
    w: usize,
    h: usize,
    seed: u64,
    specials: bool,
) -> AnyChannel<FlatSamples> {
    AnyChannel {
        name: name.into(),
        sample_data: samples(ty, &values(w, h, seed, specials)),
        quantize_linearly: false,
        sampling: Vec2(1, 1),
    }
}

/// Write any `exr` image to memory.
#[macro_export]
macro_rules! write_exr {
    ($image:expr) => {{
        let mut out = Vec::new();
        $image
            .write()
            .to_buffered(std::io::Cursor::new(&mut out))
            .expect("oracle write");
        out
    }};
}

pub fn write_one(layer: Layer<AnyChannels<FlatSamples>>) -> Vec<u8> {
    write_exr!(Image::from_layer(layer))
}

pub fn encoding(
    compression: Compression,
    tiles: Option<(usize, usize)>,
    order: LineOrder,
) -> Encoding {
    Encoding {
        compression,
        blocks: match tiles {
            Some((w, h)) => Blocks::Tiles(Vec2(w, h)),
            None => Blocks::ScanLines,
        },
        line_order: order,
    }
}

/// Oracle read of every layer (part), channel and level, as f32.
pub type OracleImage = Image<Layers<AnyChannels<Levels<FlatSamples>>>>;

pub fn oracle_read(bytes: &[u8]) -> OracleImage {
    read()
        .no_deep_data()
        .all_resolution_levels()
        .all_channels()
        .all_layers()
        .all_attributes()
        .non_parallel()
        .from_buffered(Cursor::new(bytes))
        .expect("oracle read")
}

pub fn flat_f32(s: &FlatSamples) -> Vec<f32> {
    match s {
        FlatSamples::F16(v) => v.iter().map(|x| x.to_f32()).collect(),
        FlatSamples::F32(v) => v.clone(),
        FlatSamples::U32(v) => v.iter().map(|&x| x as f32).collect(),
    }
}

/// The samples of a channel at a level (index into specular's `Part::levels` order).
pub fn oracle_level(levels: &Levels<FlatSamples>, lx: usize, ly: usize) -> Vec<f32> {
    match levels {
        Levels::Singular(s) => flat_f32(s),
        Levels::Mip { level_data, .. } => flat_f32(&level_data[lx]),
        Levels::Rip { level_data, .. } => {
            let i = ly * level_data.level_count.0 + lx;
            flat_f32(&level_data.map_data[i])
        }
    }
}

pub fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// Compare slot `slot` of our RGBA output with `expected`.
pub fn assert_slot(ours: &[f32], slot: usize, expected: &[f32], what: &str) {
    assert_eq!(ours.len() / 4, expected.len(), "{what}: size");
    for (i, (px, e)) in ours.chunks_exact(4).zip(expected).enumerate() {
        assert!(
            same(px[slot], *e),
            "{what}: pixel {i} slot {slot}: ours {} oracle {}",
            px[slot],
            e
        );
    }
}

pub fn chans(list: Vec<AnyChannel<FlatSamples>>) -> AnyChannels<FlatSamples> {
    AnyChannels::sort(SmallVec::from_vec(list))
}

pub fn f32_pixels(img: &specular_exr::ExrImage) -> &[f32] {
    match &img.pixels {
        specular_exr::Pixels::F32(v) => v,
        _ => panic!("expected f32"),
    }
}
