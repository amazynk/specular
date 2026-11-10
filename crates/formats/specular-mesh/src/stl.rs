//! STL, binary and ASCII. Binary: an 80-byte header, a triangle count, then 50 bytes per
//! triangle (facet normal, three corners, a 16-bit attribute that some writers use for a
//! colour). ASCII: `solid … facet normal … outer loop vertex … endloop endfacet … endsolid`.
//! Corners at identical positions are merged; facets are shaded flat.

use rayon::prelude::*;

use crate::builder::{Builder, weld};
use crate::text::{f32_of, lines, tokens, trim};
use crate::{Limits, Mesh, Result, Up, bad};

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut b = Builder::new(limits);
    b.up = Up::Z;
    b.flat = true;
    if is_binary(bytes) {
        binary(bytes, &mut b)?;
    } else {
        ascii(bytes, &mut b)?;
    }
    b.finish()
}

fn is_binary(bytes: &[u8]) -> bool {
    if bytes.len() < 84 {
        return false;
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as u64;
    let exact = 84 + 50 * n == bytes.len() as u64;
    let text = trim(&bytes[..bytes.len().min(512)]).starts_with(b"solid")
        && bytes[..bytes.len().min(4096)]
            .windows(5)
            .any(|w| w == b"facet");
    exact || !text
}

fn binary(bytes: &[u8], b: &mut Builder) -> Result<()> {
    if bytes.len() < 84 {
        return bad("STL shorter than its header");
    }
    let header = &bytes[..80];
    let declared = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let present = (bytes.len() - 84) / 50;
    let n = declared.min(present);
    if declared > present {
        b.note(format!(
            "file is cut short: {present} of {declared} triangles present"
        ));
    } else if present > declared && declared > 0 {
        b.note("data after the last triangle ignored");
    }
    if n == 0 {
        return bad("STL with no triangles");
    }
    if n > b.limits.max_triangles {
        return Err(crate::Error::TooLarge(format!("{n} triangles")));
    }
    if let Some(name) = header_name(header) {
        b.title = Some(name);
    }
    let body = &bytes[84..84 + n * 50];
    let f = |r: &[u8], o: usize| f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
    // Colours: Materialise writes `COLOR=` in the header and marks a face coloured with bit
    // 15 clear (red in the low bits); VisCAM/SolidView mark it with bit 15 set (blue low).
    let materialise = header.windows(6).any(|w| w == b"COLOR=");
    let default_color = materialise
        .then(|| {
            let i = header.windows(6).position(|w| w == b"COLOR=")? + 6;
            let c = header.get(i..i + 4)?;
            Some([c[0], c[1], c[2], c[3]])
        })
        .flatten()
        .unwrap_or([255; 4]);
    let face_color = |a: u16| -> Option<[u8; 4]> {
        let five = |v: u16| ((v & 31) as u32 * 255 / 31) as u8;
        if materialise {
            (a & 0x8000 == 0).then(|| [five(a), five(a >> 5), five(a >> 10), 255])
        } else {
            (a & 0x8000 != 0).then(|| [five(a >> 10), five(a >> 5), five(a), 255])
        }
    };
    let coloured = materialise
        || body
            .par_chunks_exact(50)
            .any(|r| face_color(u16::from_le_bytes([r[48], r[49]])).is_some());
    let corners: Vec<[f32; 3]> = body
        .par_chunks_exact(50)
        .flat_map_iter(|r| {
            (0..3).map(move |k| {
                let o = 12 + k * 12;
                [f(r, o), f(r, o + 4), f(r, o + 8)]
            })
        })
        .collect();
    if coloured {
        // Per-face colours: corners stay unmerged so each face keeps its own.
        let colors: Vec<[u8; 4]> = body
            .par_chunks_exact(50)
            .flat_map_iter(|r| {
                let c = face_color(u16::from_le_bytes([r[48], r[49]])).unwrap_or(default_color);
                [c; 3]
            })
            .collect();
        b.positions = corners;
        b.colors = Some(colors);
        b.indices = (0..(n * 3) as u32).collect();
    } else {
        let (positions, remap) = weld(&corners);
        drop(corners);
        b.positions = positions;
        b.indices = remap;
    }
    b.normals_given(false);
    Ok(())
}

/// The header's text when it reads as a name (not binary noise, not the exporter's
/// boilerplate).
fn header_name(h: &[u8]) -> Option<String> {
    let end = h.iter().position(|&c| c == 0).unwrap_or(h.len());
    let s = std::str::from_utf8(&h[..end]).ok()?.trim();
    let s = s.strip_prefix("solid").unwrap_or(s).trim();
    (!s.is_empty() && s.chars().all(|c| !c.is_control())).then(|| s.to_owned())
}

fn ascii(bytes: &[u8], b: &mut Builder) -> Result<()> {
    // At least ~12 bytes per corner line; reserve for that and cap.
    let guess = (bytes.len() / 40).min(b.limits.max_vertices);
    let mut corners: Vec<[f32; 3]> = Vec::with_capacity(guess);
    let mut loop_start = 0usize;
    let mut in_loop = false;
    let mut indices: Vec<u32> = Vec::with_capacity(guess);
    let mut skipped = 0usize;
    let mut name = None;
    for line in lines(bytes) {
        let mut t = tokens(line);
        let Some(kw) = t.next() else { continue };
        match kw {
            b"vertex" => {
                let mut v = || t.next().and_then(f32_of);
                match (v(), v(), v()) {
                    (Some(x), Some(y), Some(z)) => {
                        if corners.len() >= b.limits.max_vertices {
                            return Err(crate::Error::TooLarge("too many vertices".into()));
                        }
                        corners.push([x, y, z]);
                    }
                    _ => skipped += 1,
                }
                if !in_loop {
                    in_loop = true;
                    loop_start = corners.len().saturating_sub(1);
                }
            }
            b"outer" => {
                in_loop = true;
                loop_start = corners.len();
            }
            b"endloop" | b"endfacet" if in_loop => {
                in_loop = false;
                // A fan over the loop (normally exactly three corners).
                let k = corners.len() - loop_start;
                for i in 2..k {
                    if indices.len() / 3 >= b.limits.max_triangles {
                        return Err(crate::Error::TooLarge("too many triangles".into()));
                    }
                    indices.extend_from_slice(&[
                        loop_start as u32,
                        (loop_start + i - 1) as u32,
                        (loop_start + i) as u32,
                    ]);
                }
                loop_start = corners.len();
            }
            b"solid" if name.is_none() => {
                let n = trim(crate::text::rest_after_keyword(line));
                if !n.is_empty() {
                    name = Some(crate::text::string(n));
                }
            }
            _ => {}
        }
    }
    if indices.is_empty() {
        return bad("no facets");
    }
    if skipped > 0 {
        b.note(format!("{skipped} unreadable vertex lines skipped"));
    }
    b.title = name;
    let (positions, remap) = weld(&corners);
    b.indices = indices.into_par_iter().map(|i| remap[i as usize]).collect();
    b.positions = positions;
    b.normals_given(false);
    Ok(())
}
