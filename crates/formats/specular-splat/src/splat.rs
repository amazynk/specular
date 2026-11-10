//! antimatter15's `.splat`: 32-byte little-endian records, no header. Position (3 × f32),
//! linear scale (3 × f32), colour and opacity (4 × u8), rotation (4 × u8, w x y z, each
//! (b − 128) / 128).

use crate::{FileUp, Limits, Raw, Result, SplatFormat, bad, keep};

const RECORD: usize = 32;

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Raw> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(RECORD) {
        return bad(format!(
            "{} bytes is not a whole number of 32-byte splats",
            bytes.len()
        ));
    }
    let n = (bytes.len() / RECORD) as u64;
    let (kept, stride) = keep(n, limits.max_splats);
    let mut raw = Raw::new(SplatFormat::Splat, n, FileUp::NegY, kept, 0);
    if stride > 1 {
        raw.notes
            .push(format!("every {stride}th of {n} splats shown (the cap)"));
    }
    let f = |r: &[u8], o: usize| f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
    for r in bytes.chunks_exact(RECORD).step_by(stride as usize) {
        raw.positions.push([f(r, 0), f(r, 4), f(r, 8)]);
        raw.scales.push([f(r, 12), f(r, 16), f(r, 20)]);
        let c = |v: u8| f32::from(v) / 255.0;
        raw.colors.push([c(r[24]), c(r[25]), c(r[26]), c(r[27])]);
        let q = |v: u8| (f32::from(v) - 128.0) / 128.0;
        raw.rotations.push([q(r[28]), q(r[29]), q(r[30]), q(r[31])]);
    }
    Ok(raw)
}
