//! LUT files: Resolve/IRIDAS `.cube` (1D and 3D), Sony Imageworks `.spi1d` and `.spi3d`,
//! Autodesk `.3dl`,
//! and Academy/ASC CLF and Autodesk CTF (XML) with Matrix, LUT1D, LUT3D, Range, Exponent,
//! Log and ASC_CDL nodes.

use std::path::Path;

use crate::ops::{Interp, Negative, Op};
use crate::{Error, Result};

/// Entries of one table at most (a 129³ cube, a 65536-entry 1D table).
const MAX_ENTRIES: usize = 129 * 129 * 129;
/// Bytes of one LUT file at most.
const MAX_FILE: u64 = 256 << 20;

/// Reads a LUT file into operations; `interp` is the transform's interpolation.
pub fn read(path: &Path, interp: &str) -> Result<Vec<Op>> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();
    let meta = std::fs::metadata(path)
        .map_err(|_| Error::File(format!("the LUT file {name} is missing")))?;
    if meta.len() > MAX_FILE {
        return Err(Error::File(format!("the LUT file {name} is too large")));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| Error::File(format!("the LUT file {name} is unreadable")))?;
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let i3 = if interp.eq_ignore_ascii_case("tetrahedral") {
        Interp::Tetrahedral
    } else {
        Interp::Trilinear
    };
    let bad = |e: String| Error::File(format!("the LUT file {name}: {e}"));
    match ext.as_str() {
        "cube" => cube(&text, i3).map_err(bad),
        "spi1d" => spi1d(&text).map_err(bad),
        "spi3d" => spi3d(&text, i3).map_err(bad),
        "3dl" => threedl(&text, i3).map_err(bad),
        "clf" | "ctf" => clf(&text, i3).map_err(bad),
        e => Err(Error::Unsupported(format!(
            "LUT files of type .{e} are not supported"
        ))),
    }
}

fn floats(s: &str) -> Option<Vec<f64>> {
    s.split_whitespace().map(|t| t.parse().ok()).collect()
}

/// A Resolve/IRIDAS .cube: optional 1D shaper and/or 3D table, red fastest.
pub fn cube(text: &str, interp: Interp) -> std::result::Result<Vec<Op>, String> {
    let (mut n1, mut n3) = (0usize, 0usize);
    let mut lo = [0.0; 3];
    let mut hi = [1.0; 3];
    let mut lo1 = None;
    let mut hi1 = None;
    let mut data: Vec<[f64; 3]> = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut it = l.split_whitespace();
        let key = it.next().unwrap_or("");
        let rest: Vec<f64> = it.filter_map(|t| t.parse().ok()).collect();
        match key {
            "TITLE" => {}
            "LUT_1D_SIZE" => n1 = rest.first().copied().unwrap_or(0.0) as usize,
            "LUT_3D_SIZE" => n3 = rest.first().copied().unwrap_or(0.0) as usize,
            "DOMAIN_MIN" if rest.len() == 3 => lo = [rest[0], rest[1], rest[2]],
            "DOMAIN_MAX" if rest.len() == 3 => hi = [rest[0], rest[1], rest[2]],
            "LUT_1D_INPUT_RANGE" if rest.len() == 2 => {
                lo1 = Some([rest[0]; 3]);
                hi1 = Some([rest[1]; 3]);
            }
            "LUT_3D_INPUT_RANGE" if rest.len() == 2 => {
                lo = [rest[0]; 3];
                hi = [rest[1]; 3];
            }
            _ => {
                let v = floats(l).ok_or_else(|| format!("unexpected line {l}"))?;
                if v.len() != 3 {
                    return Err(format!("unexpected line {l}"));
                }
                data.push([v[0], v[1], v[2]]);
                if data.len() > MAX_ENTRIES + 65536 {
                    return Err("the table is too large".into());
                }
            }
        }
    }
    let mut ops = Vec::new();
    let mut at = 0;
    if n1 > 0 {
        if data.len() < n1 || n1 < 2 {
            return Err("the 1D table is short".into());
        }
        ops.push(Op::Lut1d {
            size: n1,
            values: data[..n1].to_vec(),
            lo: lo1.unwrap_or(lo),
            hi: hi1.unwrap_or(hi),
            half_domain: false,
            inverse: false,
        });
        at = n1;
    }
    if n3 > 0 {
        let n = n3;
        if !(2..=129).contains(&n) || data.len() < at + n * n * n {
            return Err("the 3D table is short".into());
        }
        // Red fastest in the file; blue fastest in the op.
        let src = &data[at..at + n * n * n];
        let mut values = vec![[0.0; 3]; n * n * n];
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    values[(r * n + g) * n + b] = src[(b * n + g) * n + r];
                }
            }
        }
        ops.push(Op::Lut3d {
            size: n,
            values,
            lo,
            hi,
            interp,
        });
    }
    if ops.is_empty() {
        return Err("no table".into());
    }
    Ok(ops)
}

/// A .spi1d: `From lo hi`, `Length n`, `Components c`, then `{ values }`.
pub fn spi1d(text: &str) -> std::result::Result<Vec<Op>, String> {
    let (mut lo, mut hi, mut n, mut comps) = (0.0, 1.0, 0usize, 1usize);
    let mut values = Vec::new();
    let mut inside = false;
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if inside {
            if l.starts_with('}') {
                break;
            }
            let v = floats(l).ok_or_else(|| format!("unexpected line {l}"))?;
            let e = match (comps, v.len()) {
                (1, 1) => [v[0]; 3],
                (3, 3) => [v[0], v[1], v[2]],
                (2, 2) => [v[0], v[1], 0.0],
                _ => return Err(format!("unexpected line {l}")),
            };
            values.push(e);
            if values.len() > 1 << 20 {
                return Err("the table is too large".into());
            }
            continue;
        }
        let mut it = l.split_whitespace();
        match it.next() {
            Some("From") => {
                let v: Vec<f64> = it.filter_map(|t| t.parse().ok()).collect();
                if v.len() == 2 {
                    (lo, hi) = (v[0], v[1]);
                }
            }
            Some("Length") => n = it.next().and_then(|t| t.parse().ok()).unwrap_or(0),
            Some("Components") => comps = it.next().and_then(|t| t.parse().ok()).unwrap_or(1),
            Some("{") => inside = true,
            _ => {}
        }
    }
    if values.len() < 2 || (n > 0 && values.len() != n) {
        return Err("the table length does not match".into());
    }
    Ok(vec![Op::Lut1d {
        size: values.len(),
        values,
        lo: [lo; 3],
        hi: [hi; 3],
        half_domain: false,
        inverse: false,
    }])
}

/// A .spi3d: `SPILUT 1.0`, `3 3`, `n n n`, then `r g b R G B` index lines.
pub fn spi3d(text: &str, interp: Interp) -> std::result::Result<Vec<Op>, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let head = lines.next().unwrap_or("");
    if !head.starts_with("SPILUT") {
        return Err("not an SPI 3D table".into());
    }
    lines.next();
    let size: Vec<usize> = lines
        .next()
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    let n = *size.first().ok_or("no size")?;
    if !(2..=129).contains(&n) || size.iter().any(|&s| s != n) {
        return Err("the table size is not supported".into());
    }
    let mut values = vec![[0.0; 3]; n * n * n];
    let mut count = 0;
    for l in lines {
        let v = floats(l).ok_or_else(|| format!("unexpected line {l}"))?;
        if v.len() != 6 {
            continue;
        }
        let (r, g, b) = (v[0] as usize, v[1] as usize, v[2] as usize);
        if r < n && g < n && b < n {
            values[(r * n + g) * n + b] = [v[3], v[4], v[5]];
            count += 1;
        }
    }
    if count < n * n * n {
        return Err("the table is short".into());
    }
    Ok(vec![Op::Lut3d {
        size: n,
        values,
        lo: [0.0; 3],
        hi: [1.0; 3],
        interp,
    }])
}

/// The integer bit depth (8, 10, …, 16) whose largest code holds `max`, as OpenColorIO
/// guesses it for .3dl files.
fn likely_depth(max: f64) -> Option<f64> {
    (8..=16)
        .step_by(2)
        .map(|b| f64::from((1u32 << b) - 1))
        .find(|&code| max <= code)
}

/// An Autodesk .3dl (Lustre, Flame, Nuke): an optional `3DMESH`/`Mesh` header, a line of
/// input mesh codes (a ramp from 0, steps even to a code or so, as in `0 64 … 960 1023`),
/// then `n³` integer RGB lines, blue fastest.
/// Input and output bit depths are guessed from the largest code (8, 10, 12, 14 or 16
/// bits), as OpenColorIO does.
pub fn threedl(text: &str, interp: Interp) -> std::result::Result<Vec<Op>, String> {
    let mut mesh: Option<Vec<f64>> = None;
    let mut data: Vec<[f64; 3]> = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        // Lustre headers ("3DMESH", "Mesh 4 12", "LUT8", "gamma 1.0") carry nothing the
        // table itself does not.
        if l.starts_with(|c: char| c.is_ascii_alphabetic()) || l.starts_with("3DMESH") {
            continue;
        }
        let v = floats(l).ok_or_else(|| format!("unexpected line {l}"))?;
        match &mesh {
            None => mesh = Some(v),
            Some(_) if v.len() == 3 => {
                data.push([v[0], v[1], v[2]]);
                if data.len() > MAX_ENTRIES {
                    return Err("the table is too large".into());
                }
            }
            Some(_) => return Err(format!("unexpected line {l}")),
        }
    }
    let mesh = mesh.ok_or("no input mesh")?;
    let n = mesh.len();
    if !(2..=129).contains(&n) {
        return Err("the table size is not supported".into());
    }
    if data.len() < n * n * n {
        return Err("the table is short".into());
    }
    let step = mesh[1] - mesh[0];
    let even = mesh[0] == 0.0
        && step > 0.0
        && mesh.windows(2).all(|w| ((w[1] - w[0]) - step).abs() <= 1.5);
    if !even {
        return Err("an uneven input mesh is not supported".into());
    }
    let in_max = likely_depth(mesh[n - 1]).ok_or("the input codes are out of range")?;
    let out_peak = data
        .iter()
        .flat_map(|c| c.iter().copied())
        .fold(0.0f64, f64::max);
    let out_max = likely_depth(out_peak).ok_or("the output codes are out of range")?;
    let values = data[..n * n * n]
        .iter()
        .map(|c| c.map(|v| v / out_max))
        .collect();
    Ok(vec![Op::Lut3d {
        size: n,
        values,
        lo: [0.0; 3],
        hi: [mesh[n - 1] / in_max; 3],
        interp,
    }])
}

/// The scale a CLF bit depth implies ("10i" → 1023).
fn depth_scale(d: &str) -> f64 {
    match d {
        "8i" => 255.0,
        "10i" => 1023.0,
        "12i" => 4095.0,
        "16i" => 65535.0,
        _ => 1.0,
    }
}

/// A CLF or CTF process list.
pub fn clf(text: &str, interp: Interp) -> std::result::Result<Vec<Op>, String> {
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    )
    .map_err(|e| format!("unreadable XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "ProcessList" {
        return Err("not a ProcessList".into());
    }
    let mut ops = Vec::new();
    for node in root.children().filter(|n| n.is_element()) {
        let name = node.tag_name().name();
        if matches!(
            name,
            "Description" | "InputDescriptor" | "OutputDescriptor" | "Info"
        ) {
            continue;
        }
        let in_scale = depth_scale(node.attribute("inBitDepth").unwrap_or("32f"));
        let out_scale = depth_scale(node.attribute("outBitDepth").unwrap_or("32f"));
        let inverse = node.attribute("inverse") == Some("true")
            || name == "InverseLUT1D"
            || node
                .attribute("style")
                .is_some_and(|s| s.to_ascii_lowercase().contains("rev") || s == "inverse");
        let child = |n: &str| node.children().find(|c| c.tag_name().name() == n);
        let array = || -> Option<(Vec<usize>, Vec<f64>)> {
            let a = child("Array")?;
            let dim: Vec<usize> = a
                .attribute("dim")?
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            Some((dim, floats(a.text().unwrap_or(""))?))
        };
        // Integer depths: inputs arrive normalised; tables give output codes.
        let pre = |ops: &mut Vec<Op>| {
            if in_scale != 1.0 {
                ops.push(Op::Range {
                    scale: in_scale,
                    offset: 0.0,
                    lo: None,
                    hi: None,
                });
            }
        };
        match name {
            "Matrix" => {
                let (dim, v) = array().ok_or("a Matrix without values")?;
                let (rows, cols) = (
                    dim.first().copied().unwrap_or(3),
                    dim.get(1).copied().unwrap_or(3),
                );
                if v.len() < rows * cols || rows < 3 {
                    return Err("a short Matrix".into());
                }
                let mut m = [0.0; 9];
                let mut off = [0.0; 3];
                for r in 0..3 {
                    for c in 0..3 {
                        m[r * 3 + c] = v[r * cols + c] * out_scale.recip() * in_scale;
                    }
                    if cols >= 4 {
                        off[r] = v[r * cols + 3] / out_scale;
                    }
                }
                ops.push(Op::Matrix { m, offset: off });
            }
            "LUT1D" | "InverseLUT1D" => {
                let (dim, v) = array().ok_or("a LUT1D without values")?;
                let n = *dim.first().ok_or("a LUT1D without size")?;
                let comps = dim.get(1).copied().unwrap_or(1);
                if n < 2 || v.len() < n * comps || n > 65536 {
                    return Err("a short LUT1D".into());
                }
                let values: Vec<[f64; 3]> = (0..n)
                    .map(|i| {
                        let e = |c: usize| v[i * comps + c.min(comps - 1)] / out_scale;
                        [e(0), e(1), e(2)]
                    })
                    .collect();
                let half = node.attribute("halfDomain") == Some("true");
                ops.push(Op::Lut1d {
                    size: n,
                    values,
                    lo: [0.0; 3],
                    hi: [1.0; 3],
                    half_domain: half,
                    inverse,
                });
            }
            "LUT3D" => {
                let (dim, v) = array().ok_or("a LUT3D without values")?;
                let n = *dim.first().ok_or("a LUT3D without size")?;
                if !(2..=129).contains(&n) || v.len() < n * n * n * 3 {
                    return Err("a short LUT3D".into());
                }
                // CLF stores blue fastest.
                let values = (0..n * n * n)
                    .map(|i| {
                        [
                            v[i * 3] / out_scale,
                            v[i * 3 + 1] / out_scale,
                            v[i * 3 + 2] / out_scale,
                        ]
                    })
                    .collect();
                let i3 = match node.attribute("interpolation") {
                    Some("tetrahedral") => Interp::Tetrahedral,
                    Some("trilinear") => Interp::Trilinear,
                    _ => interp,
                };
                ops.push(Op::Lut3d {
                    size: n,
                    values,
                    lo: [0.0; 3],
                    hi: [1.0; 3],
                    interp: i3,
                });
            }
            "Range" => {
                let f = |n: &str| child(n).and_then(|c| c.text()?.trim().parse::<f64>().ok());
                let (min_in, max_in, min_out, max_out) = (
                    f("minInValue"),
                    f("maxInValue"),
                    f("minOutValue"),
                    f("maxOutValue"),
                );
                let clamp = node.attribute("style") != Some("noClamp");
                pre(&mut ops);
                match (min_in, max_in, min_out, max_out) {
                    (Some(a), Some(b), Some(c), Some(d)) => {
                        let s = (d - c) / (b - a);
                        ops.push(Op::Range {
                            scale: s / out_scale,
                            offset: (c - a * s) / out_scale,
                            lo: clamp.then_some(c / out_scale),
                            hi: clamp.then_some(d / out_scale),
                        });
                    }
                    (a, b, _, _) => ops.push(Op::Range {
                        scale: 1.0 / out_scale,
                        offset: 0.0,
                        lo: a.map(|a| a / out_scale),
                        hi: b.map(|b| b / out_scale),
                    }),
                }
            }
            "Exponent" => {
                let p = child("ExponentParams");
                let e = p
                    .and_then(|p| p.attribute("exponent"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.0);
                let off = p
                    .and_then(|p| p.attribute("offset"))
                    .and_then(|v| v.parse::<f64>().ok());
                let style = node.attribute("style").unwrap_or("basicFwd");
                let rev = style.contains("Rev");
                if let Some(o) = off.filter(|_| style.starts_with("moncurve")) {
                    ops.push(Op::ExpLinear {
                        g: [e; 3],
                        o: [o; 3],
                        neg: Negative::Linear,
                        inverse: rev,
                    });
                } else {
                    let neg = if style.contains("Mirror") {
                        Negative::Mirror
                    } else if style.contains("PassThru") {
                        Negative::PassThru
                    } else {
                        Negative::Clamp
                    };
                    ops.push(Op::Exponent {
                        e: [e; 3],
                        neg,
                        inverse: rev,
                    });
                }
            }
            "Log" => {
                let style = node.attribute("style").unwrap_or("log2");
                let (base, inv) = match style {
                    "log10" => (10.0, false),
                    "antiLog10" => (10.0, true),
                    "log2" => (2.0, false),
                    "antiLog2" => (2.0, true),
                    "linToLog" | "cameraLinToLog" => (
                        child("LogParams")
                            .and_then(|p| p.attribute("base")?.parse().ok())
                            .unwrap_or(2.0),
                        false,
                    ),
                    _ => (
                        child("LogParams")
                            .and_then(|p| p.attribute("base")?.parse().ok())
                            .unwrap_or(2.0),
                        true,
                    ),
                };
                let p = child("LogParams");
                let a = |n: &str, d: f64| p.and_then(|p| p.attribute(n)?.parse().ok()).unwrap_or(d);
                ops.push(Op::Log {
                    base,
                    log_slope: [a("logSideSlope", 1.0); 3],
                    log_offset: [a("logSideOffset", 0.0); 3],
                    lin_slope: [a("linSideSlope", 1.0); 3],
                    lin_offset: [a("linSideOffset", 0.0); 3],
                    lin_break: p
                        .and_then(|p| p.attribute("linSideBreak")?.parse().ok())
                        .map(|b: f64| [b; 3]),
                    linear_slope: p
                        .and_then(|p| p.attribute("linearSlope")?.parse().ok())
                        .map(|b: f64| [b; 3]),
                    inverse: inv,
                });
            }
            "ASC_CDL" => {
                let sop = child("SOPNode");
                let get = |n: &str, d: f64| -> [f64; 3] {
                    sop.and_then(|s| s.children().find(|c| c.tag_name().name() == n))
                        .and_then(|c| floats(c.text()?))
                        .filter(|v| v.len() == 3)
                        .map_or([d; 3], |v| [v[0], v[1], v[2]])
                };
                let sat = child("SatNode")
                    .and_then(|s| s.children().find(|c| c.tag_name().name() == "Saturation"))
                    .and_then(|c| c.text()?.trim().parse().ok())
                    .unwrap_or(1.0);
                let style = node.attribute("style").unwrap_or("Fwd");
                ops.push(Op::Cdl {
                    slope: get("Slope", 1.0),
                    offset: get("Offset", 0.0),
                    power: get("Power", 1.0),
                    sat,
                    clamp: !style.contains("NoClamp"),
                    inverse: style.starts_with("Rev"),
                });
            }
            other => return Err(format!("the {other} node is not supported")),
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_and_clf() {
        // A 2³ cube that swaps red and blue.
        let mut t = String::from("TITLE \"swap\"\nLUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    t += &format!("{b} {g} {r}\n");
                }
            }
        }
        let ops = cube(&t, Interp::Trilinear).unwrap();
        let mut c = [0.25, 0.5, 0.75];
        ops[0].apply(&mut c);
        assert!((c[0] - 0.75).abs() < 1e-12 && (c[2] - 0.25).abs() < 1e-12);
        let spi = "Version 1\nFrom 0.0 1.0\nLength 3\nComponents 1\n{\n0\n0.25\n1\n}\n";
        let ops = spi1d(spi).unwrap();
        let mut c = [0.75, 0.5, 0.25];
        ops[0].apply(&mut c);
        assert!((c[1] - 0.25).abs() < 1e-12 && (c[0] - 0.625).abs() < 1e-12);
        let x = r#"<?xml version="1.0"?><ProcessList id="a" compCLFversion="3.0">
          <Matrix inBitDepth="32f" outBitDepth="32f"><Array dim="3 3">2 0 0 0 2 0 0 0 2</Array></Matrix>
          <Range inBitDepth="32f" outBitDepth="32f"><minInValue>0</minInValue><maxInValue>1</maxInValue><minOutValue>0</minOutValue><maxOutValue>1</maxOutValue></Range>
          <ASC_CDL style="Fwd" inBitDepth="32f" outBitDepth="32f"><SOPNode><Slope>1 1 1</Slope><Offset>0 0 0</Offset><Power>2 2 2</Power></SOPNode><SatNode><Saturation>1</Saturation></SatNode></ASC_CDL>
        </ProcessList>"#;
        let ops = clf(x, Interp::Trilinear).unwrap();
        let mut c = [0.25, 0.4, 0.9];
        for o in &ops {
            o.apply(&mut c);
        }
        assert!((c[0] - 0.25).abs() < 1e-12 && (c[1] - 0.64).abs() < 1e-12 && c[2] == 1.0);
    }
}
