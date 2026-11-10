//! specular-jpeg runner (feature `specular`). Times read file + `decode` to planes; specular leaves
//! colour conversion/upsampling to the GPU, so this is less work than the RGB output of the
//! other decoders (see README).

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use specular_jpeg::{DecodeOptions, Parallelism, Scale, choose_scale, decode, read_info};

use crate::decoders::Outcome;

pub fn decode_file(path: &Path, view: Option<(u32, u32)>, single: bool) -> Result<Outcome> {
    let t = Instant::now();
    let data = std::fs::read(path)?;
    let scale = match view {
        Some((vw, vh)) => choose_scale(&read_info(&data)?, vw, vh),
        None => Scale::Full,
    };
    let opts = DecodeOptions {
        scale,
        parallelism: if single {
            Parallelism::Single
        } else {
            Parallelism::Auto
        },
        ..Default::default()
    };
    let p = decode(&data, &opts)?;
    let elapsed = t.elapsed();
    let mut note = Vec::new();
    if p.truncated {
        note.push("truncated".to_string());
    }
    if view.is_some() {
        note.push(format!("scale 1/{}", scale.factor()));
    }
    Ok(Outcome {
        elapsed,
        out_w: p.width,
        out_h: p.height,
        note: (!note.is_empty()).then(|| note.join(", ")),
    })
}
