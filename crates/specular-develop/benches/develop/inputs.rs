//! Synthetic raws for the bench: Bayer and X-Trans DNGs (LJ92 tiles, the "fast codec" of
//! §3.9) at 24, 45 and 100 MP, written by the test DNG writer of `specular-raw`. They are
//! generated on demand into `$CARGO_TARGET_DIR/develop-bench/inputs` (never committed) and
//! kept between runs; `DV_BENCH_CLEAN=1` deletes them at the end.

use std::path::{Path, PathBuf};

use crate::common::{DngCodec, DngSpec, Tag, XYZ_TO_SRGB, tag_ascii, write_dng};

/// Bumped when the generator changes (old files are then made again).
const VERSION: u32 = 1;

/// The X-T3 / X-T4 layout (CFAPattern of their DNGs).
const XTRANS: [u8; 36] = [
    1, 1, 2, 1, 1, 0, //
    2, 0, 1, 0, 2, 1, //
    1, 1, 2, 1, 1, 0, //
    1, 1, 0, 1, 1, 2, //
    0, 2, 1, 2, 0, 1, //
    1, 1, 0, 1, 1, 2, //
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mosaic {
    Bayer,
    XTrans,
}

/// One synthetic raw.
#[derive(Clone, Copy, Debug)]
pub struct Raw {
    pub mp: u32,
    pub width: u32,
    pub height: u32,
    pub mosaic: Mosaic,
}

impl Raw {
    pub const fn new(mp: u32, mosaic: Mosaic) -> Raw {
        // 3:2 frames of the usual sensors (X-Trans sides are multiples of 6).
        let (width, height) = match mp {
            24 => (6000, 4002),
            45 => (8256, 5504),
            100 => (11664, 8748),
            _ => (3000, 1998),
        };
        Raw {
            mp,
            width,
            height,
            mosaic,
        }
    }

    pub fn name(&self) -> String {
        let m = match self.mosaic {
            Mosaic::Bayer => "bayer",
            Mosaic::XTrans => "xtrans",
        };
        format!("{}mp-{m}", self.mp)
    }

    /// The file, made when missing.
    pub fn path(&self) -> PathBuf {
        let p = dir().join(format!("{}-v{VERSION}.dng", self.name()));
        if !p.exists() {
            let t = std::time::Instant::now();
            let tmp = p.with_extension("part");
            std::fs::write(&tmp, self.dng()).expect("write a synthetic raw");
            std::fs::rename(&tmp, &p).expect("rename a synthetic raw");
            eprintln!(
                "made {} ({:.0} MB) in {:.1} s",
                p.display(),
                std::fs::metadata(&p).map_or(0, |m| m.len()) as f64 / 1e6,
                t.elapsed().as_secs_f64()
            );
        }
        p
    }

    /// A 14-bit DNG: smooth colour fields, fine texture, a few clipped highlights, and
    /// shot noise, so demosaic, denoise and highlight recovery all have work.
    pub fn dng(&self) -> Vec<u8> {
        let (w, h) = (self.width, self.height);
        let mut s = DngSpec::new(w, h, DngCodec::Lj92Tiles(256));
        s.white = 16383;
        s.black = 512;
        s.color_matrix = XYZ_TO_SRGB;
        s.neutral = [0.55, 1.0, 0.75];
        s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Bench")];
        let pattern: Vec<u8> = match self.mosaic {
            Mosaic::Bayer => vec![0, 1, 1, 2],
            Mosaic::XTrans => {
                s.extra.push(tag_shorts(0x828d, &[6, 6]));
                s.extra.push((0x828e, 1, 36, XTRANS.to_vec()));
                XTRANS.to_vec()
            }
        };
        let side = if self.mosaic == Mosaic::Bayer { 2 } else { 6 };
        let mut data = vec![0u16; (w * h) as usize];
        let gain = [0.55f32, 1.0, 0.75];
        let mut seed = 0x2545_F491u32 ^ self.mp;
        for y in 0..h {
            let fy = y as f32 / h as f32;
            let row = &mut data[(y * w) as usize..][..w as usize];
            for (x, v) in row.iter_mut().enumerate() {
                let fx = x as f32 / w as f32;
                let c = usize::from(pattern[(y as usize % side) * side + x % side]);
                // Scene (linear, relative to white): three colour fields and texture.
                let field = match c {
                    0 => 0.05 + 0.35 * fx,
                    1 => 0.04 + 0.30 * fy,
                    _ => 0.06 + 0.25 * (1.0 - fx) * (1.0 - fy),
                };
                let tex = 0.04 * ((x as f32 * 0.21).sin() * (y as f32 * 0.17).cos());
                // A row of bright discs, clipped in their middle.
                let dx = (fx * 8.0).fract() - 0.5;
                let dy = (fy - 0.2) * 8.0 * h as f32 / w as f32;
                let disc = (1.0 - (dx * dx + dy * dy) * 16.0).max(0.0) * 1.6;
                let lin = ((field + tex + disc) * gain[c]).max(0.0);
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                // Shot noise, about √signal, uniform stand-in.
                let n = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 2.0 * (lin * 40.0).sqrt();
                let dn = 512.0 + lin * 15871.0 + n;
                *v = dn.clamp(0.0, 16383.0) as u16;
            }
        }
        write_dng(&s, &data)
    }
}

fn tag_shorts(tag: u16, v: &[u16]) -> Tag {
    (
        tag,
        3,
        v.len() as u32,
        v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    )
}

/// `$CARGO_TARGET_DIR/develop-bench/inputs` (or `target/` beside the workspace).
pub fn dir() -> PathBuf {
    let d = out_dir().join("inputs");
    std::fs::create_dir_all(&d).expect("make the inputs folder");
    d
}

/// `$CARGO_TARGET_DIR/develop-bench`.
pub fn out_dir() -> PathBuf {
    let t = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
        PathBuf::from,
    );
    t.join("develop-bench")
}

/// Deletes the generated raws.
pub fn clean() {
    let _ = std::fs::remove_dir_all(out_dir().join("inputs"));
}
