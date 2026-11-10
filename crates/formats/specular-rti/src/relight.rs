//! CNR-ISTI Relight folders: `info.json` and `plane_0.jpg`, `plane_1.jpg`, … each holding
//! three coefficient planes in its red, green and blue. A plane's value is `(byte / 255 −
//! bias) · scale` with the per-plane `scale` and `bias` of `materials[0]`.
//!
//! The analytic bases (`ptm`, `hsh`, `sh`) come as `rgb` (one image per term) or `lrgb` (the
//! colour in the first image, the luminance terms in the rest). The learned bases (`rbf`,
//! `bilinear`) come as `mrgb` or `mycc`: their per-light weights are a table (`basis` in
//! the JSON, or `materials.bin` beside it) of bytes, the first block the mean (`byte /
//! 255`), the others `(byte − 127) / range`.

use serde_json::Value;

use crate::{Basis, Colour, Container, Error, Layer, MAX_LAYERS, Result, Rti, check_size};

const TYPES: [&str; 5] = ["ptm", "hsh", "sh", "rbf", "bilinear"];

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("Relight info.json: {what}"))
}

/// True when `data` is a Relight `info.json`.
pub fn is_relight_info(data: &[u8]) -> bool {
    let head = &data[..data.len().min(4096)];
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start();
    if !t.starts_with('{') || !t.contains("\"colorspace\"") || !t.contains("\"type\"") {
        return false;
    }
    serde_json::from_slice::<Value>(data).is_ok_and(|v| {
        v.get("type")
            .and_then(Value::as_str)
            .is_some_and(|s| TYPES.contains(&s))
            && v.get("colorspace").and_then(Value::as_str).is_some()
            && v.get("width").is_some()
    })
}

/// The files beside `info.json` a Relight image needs, in loading order.
pub fn relight_files(info: &[u8]) -> Option<Vec<String>> {
    let v: Value = serde_json::from_slice(info).ok()?;
    let planes = plane_count(&v).ok()?;
    let mut out: Vec<String> = (0..planes.div_ceil(3))
        .map(|j| format!("plane_{j}.jpg"))
        .collect();
    let colour = v.get("colorspace").and_then(Value::as_str).unwrap_or("");
    if matches!(colour, "mrgb" | "mycc") && v.get("basis").is_none() {
        out.push("materials.bin".into());
    }
    Some(out)
}

fn int(v: &Value, key: &str) -> Result<u64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_u64().ok_or_else(|| malformed(key)),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| malformed(key)),
        _ => Err(malformed(&format!("no {key}"))),
    }
}

fn floats(v: &Value, key: &str) -> Option<Vec<f32>> {
    v.get(key)?
        .as_array()?
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect()
}

fn plane_count(v: &Value) -> Result<usize> {
    if v.get("colorspace").and_then(Value::as_str) == Some("mycc") {
        let y = v
            .get("yccplanes")
            .and_then(Value::as_array)
            .ok_or_else(|| malformed("no yccplanes"))?;
        let n: Option<Vec<u64>> = y.iter().map(Value::as_u64).collect();
        let n = n.ok_or_else(|| malformed("yccplanes"))?;
        if n.len() != 3 {
            return Err(malformed("yccplanes"));
        }
        Ok(n.iter().sum::<u64>() as usize)
    } else {
        Ok(int(v, "nplanes")? as usize)
    }
}

pub(crate) fn read(info: &[u8], load: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Result<Rti> {
    let v: Value = serde_json::from_slice(info).map_err(|e| malformed(&e.to_string()))?;
    let kind = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("no type"))?
        .to_string();
    if !TYPES.contains(&kind.as_str()) {
        return Err(Error::Unsupported(format!("Relight type {kind:?}")));
    }
    let colorspace = v
        .get("colorspace")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("no colorspace"))?
        .to_string();
    let format = v.get("format").and_then(Value::as_str).unwrap_or("jpg");
    if format != "jpg" {
        return Err(Error::Unsupported(format!("Relight planes in {format}")));
    }
    if let Some(layout) = v.get("layout").and_then(Value::as_str)
        && layout != "image"
    {
        return Err(Error::Unsupported(format!("Relight {layout} tiles")));
    }
    let width = int(&v, "width")? as u32;
    let height = int(&v, "height")? as u32;
    check_size(width, height)?;
    let planes = plane_count(&v)?;
    let jpegs = planes.div_ceil(3);
    if planes == 0 || jpegs > MAX_LAYERS {
        return Err(Error::Unsupported(format!("Relight with {planes} planes")));
    }
    let material = v
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|m| m.first())
        .ok_or_else(|| malformed("no materials"))?;
    let scale = floats(material, "scale").ok_or_else(|| malformed("no material scale"))?;
    let bias = floats(material, "bias").ok_or_else(|| malformed("no material bias"))?;
    if scale.len() < planes || bias.len() < planes {
        return Err(malformed("fewer scales or biases than planes"));
    }

    let (basis, colour) = match (kind.as_str(), colorspace.as_str()) {
        ("ptm" | "hsh" | "sh", "rgb" | "lrgb") => {
            let lrgb = colorspace == "lrgb";
            let terms = if lrgb {
                planes.saturating_sub(3)
            } else {
                planes / 3
            };
            if !lrgb && planes % 3 != 0 {
                return Err(malformed("RGB planes not a multiple of three"));
            }
            let basis = match kind.as_str() {
                "ptm" if terms == 6 => Basis::Ptm {
                    relight_order: true,
                },
                "hsh" if matches!(terms, 4 | 9 | 16) => Basis::Hsh { terms },
                "sh" if (1..=9).contains(&terms) => Basis::Sh { terms },
                _ => {
                    return Err(Error::Unsupported(format!(
                        "Relight {kind} with {terms} terms"
                    )));
                }
            };
            (basis, if lrgb { Colour::Lrgb } else { Colour::Rgb })
        }
        ("rbf" | "bilinear", "mrgb" | "mycc") => {
            let range = floats(material, "range").ok_or_else(|| malformed("no material range"))?;
            if range.len() < planes {
                return Err(malformed("fewer ranges than planes"));
            }
            let bytes: Vec<u8> = match v.get("basis").and_then(Value::as_array) {
                Some(b) => b
                    .iter()
                    .map(|x| x.as_u64().and_then(|n| u8::try_from(n).ok()))
                    .collect::<Option<_>>()
                    .ok_or_else(|| malformed("basis is not bytes"))?,
                None => load("materials.bin")?,
            };
            let lights = floats(&v, "lights").unwrap_or_default();
            let dims = if kind == "rbf" {
                lights.len() / 3
            } else {
                let r = int(&v, "resolution")? as usize;
                if !(2..=256).contains(&r) {
                    return Err(malformed("bilinear resolution"));
                }
                r * r
            };
            if dims == 0 || bytes.len() < (planes + 1) * dims * 3 {
                return Err(malformed("basis smaller than planes × lights"));
            }
            let table: Vec<f32> = (0..(planes + 1) * dims * 3)
                .map(|o| {
                    let p = o / (dims * 3);
                    let b = bytes[o] as f32;
                    if p == 0 {
                        b / 255.0
                    } else {
                        (b - 127.0) / range[p - 1]
                    }
                })
                .collect();
            let basis = if kind == "rbf" {
                let sigma = v
                    .get("sigma")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| malformed("no sigma"))? as f32;
                Basis::Rbf {
                    lights: lights.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                    sigma,
                    basis: table,
                }
            } else {
                Basis::Bilinear {
                    resolution: (dims as f64).sqrt() as usize,
                    basis: table,
                }
            };
            let colour = if colorspace == "mycc" {
                let chroma = v
                    .get("yccplanes")
                    .and_then(Value::as_array)
                    .and_then(|y| y.get(1))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                Colour::Mycc { chroma }
            } else {
                Colour::Mrgb
            };
            (basis, colour)
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "Relight {kind} in {colorspace}"
            )));
        }
    };

    let _layers = crate::reserve_layers(jpegs, width, height)?;
    let mut layers = Vec::with_capacity(jpegs);
    for j in 0..jpegs {
        let name = format!("plane_{j}.jpg");
        let bytes = load(&name)?;
        crate::jpeg_fits(&bytes, width, height).map_err(|e| match e {
            Error::Jpeg(m) => Error::Jpeg(format!("{name}: {m}")),
            e => e,
        })?;
        let opts = specular_jpeg::DecodeOptions::default();
        let d = specular_jpeg::decode(&bytes, &opts)
            .map_err(|e| Error::Jpeg(format!("{name}: {e}")))?;
        if d.width != width || d.height != height {
            return Err(malformed(&format!(
                "{name} is {}×{}, the image {width}×{height}",
                d.width, d.height
            )));
        }
        let rgba = specular_jpeg::planes_to_rgba(&d);
        let mut data = Vec::with_capacity(rgba.len() / 4 * 3);
        for px in rgba.chunks_exact(4) {
            data.extend_from_slice(&px[..3]);
        }
        let mut s = [0.0; 3];
        let mut o = [0.0; 3];
        for c in 0..3 {
            let p = 3 * j + c;
            if p < planes {
                s[c] = scale[p] / 255.0;
                o[c] = -bias[p] * scale[p];
            }
        }
        layers.push(Layer {
            data,
            scale: s,
            offset: o,
        });
    }
    let quality = v.get("quality").and_then(Value::as_u64).map(|q| q as u32);
    let lights = match &basis {
        Basis::Rbf { lights, .. } => lights.len(),
        _ => floats(&v, "lights").map_or(0, |l| l.len() / 3),
    };
    Ok(Rti {
        width,
        height,
        container: Container::Relight {
            kind,
            colorspace,
            planes,
            quality,
            lights,
        },
        basis,
        colour,
        planes,
        layers,
    })
}
