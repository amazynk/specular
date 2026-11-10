//! GIMP 3 layer effects (non-destructive filters): each is a GEGL or GIMP operation with its
//! arguments, stored after the layer's mask pointer as a list of pointers to records:
//! name, icon and operation strings, a version string, properties (visibility, opacity, blend
//! mode, and one `PROP_FILTER_ARGUMENT` per operation argument), then a pointer to the
//! filter's mask channel.
//!
//! The operations drawn here are the common colour and pixel ones; their results were
//! compared with GIMP's composite of files it wrote with each filter (tests/oracle.rs over
//! `tools/corpus/xcf-filters.py`). A layer whose filters include another operation is drawn
//! without that filter, and [`crate::Info::undrawn_filters`] names it.

use crate::pixel::{linear_to_srgb, srgb_to_linear};
use crate::read::Reader;
use crate::{Error, Result};

const PROP_END: u32 = 0;
const PROP_MODE: u32 = 7;
const PROP_VISIBLE: u32 = 8;
const PROP_FLOAT_OPACITY: u32 = 33;
const PROP_FILTER_ARGUMENT: u32 = 45;

/// One operation argument.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Arg {
    Int(i32),
    Bool(bool),
    Float(f32),
    Enum(i32),
    /// A serialised GIMP config object (`(name value)` lines).
    Config(String),
    /// Straight R'G'B'A (sRGB-coded).
    Color([f32; 4]),
    Other,
}

/// One layer effect.
#[derive(Clone, Debug)]
pub(crate) struct Filter {
    pub op: String,
    pub visible: bool,
    pub opacity: f32,
    pub mode: u32,
    pub args: Vec<(String, Arg)>,
}

impl Filter {
    fn arg(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    fn float(&self, name: &str, default: f32) -> f32 {
        match self.arg(name) {
            Some(Arg::Float(v)) if v.is_finite() => *v,
            Some(Arg::Int(v) | Arg::Enum(v)) => *v as f32,
            _ => default,
        }
    }

    fn int(&self, name: &str, default: i32) -> i32 {
        match self.arg(name) {
            Some(Arg::Int(v) | Arg::Enum(v)) => *v,
            Some(Arg::Bool(b)) => i32::from(*b),
            Some(Arg::Float(v)) if v.is_finite() => *v as i32,
            _ => default,
        }
    }

    fn color(&self, name: &str, default: [f32; 4]) -> [f32; 4] {
        match self.arg(name) {
            Some(Arg::Color(c)) => *c,
            _ => default,
        }
    }

    /// A number from the serialised config, `(key value)`.
    fn config(&self, key: &str) -> Option<f64> {
        let Some(Arg::Config(s)) = self.arg("config") else {
            return None;
        };
        config_value(s, key)
    }

    /// A record that could not be read: left out, and named in the notes.
    pub(crate) fn unreadable() -> Filter {
        Filter {
            op: "unreadable".into(),
            visible: true,
            opacity: 1.0,
            mode: 0,
            args: Vec::new(),
        }
    }

    /// Whether this crate draws the operation.
    pub(crate) fn drawn(&self) -> bool {
        SUPPORTED.contains(&self.op.as_str())
    }
}

/// The value after `(key ` at the top level of a config string.
fn config_value(s: &str, key: &str) -> Option<f64> {
    let pat = format!("({key} ");
    let i = s.find(&pat)? + pat.len();
    let rest = &s[i..];
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

const SUPPORTED: &[&str] = &[
    "gegl:invert-gamma",
    "gegl:invert-linear",
    "gimp:desaturate",
    "gimp:threshold",
    "gimp:posterize",
    "gimp:brightness-contrast",
    "gegl:mono-mixer",
    "gegl:gaussian-blur",
    "gegl:pixelize",
    "gegl:dropshadow",
    "gegl:color-to-alpha",
    "gegl:opacity",
    "gimp:curves",
    "gimp:levels",
    "gimp:hue-saturation",
    "gimp:colorize",
    "gimp:color-balance",
    "gegl:exposure",
    "gegl:color-overlay",
];

/// Reads the filter record at `ptr`.
pub(crate) fn read(data: &[u8], ptr: u64, wide: bool) -> Result<Filter> {
    let mut r = Reader::at(data, ptr, wide, "layer effect")?;
    let _name = r.string("effect name")?;
    let _icon = r.string("effect icon")?;
    let op = r.string("effect operation")?;
    let _version = r.string("effect version")?;
    let mut f = Filter {
        op,
        visible: true,
        opacity: 1.0,
        mode: 0,
        args: Vec::new(),
    };
    for _ in 0..4096 {
        let ty = r.u32("effect properties")?;
        let len = r.u32("effect properties")? as usize;
        let body = r.bytes(len, "effect property")?;
        let mut b = Reader::new(body, false);
        match ty {
            PROP_END => return Ok(f),
            PROP_VISIBLE => f.visible = b.u32("visible")? != 0,
            PROP_FLOAT_OPACITY => {
                let o = b.f32("opacity")?;
                f.opacity = if o.is_finite() {
                    o.clamp(0.0, 1.0)
                } else {
                    1.0
                };
            }
            PROP_MODE => f.mode = b.u32("mode")?,
            PROP_FILTER_ARGUMENT => {
                let name = b.string("argument name")?;
                let ty = b.u32("argument type")?;
                let v = match ty {
                    1 => Arg::Int(b.i32("argument")?),
                    2 => Arg::Bool(b.u32("argument")? != 0),
                    3 => Arg::Float(b.f32("argument")?),
                    5 => Arg::Enum(b.i32("argument")?),
                    6 => Arg::Config(b.string("argument")?),
                    8 => color(&mut b).unwrap_or(Arg::Other),
                    _ => Arg::Other,
                };
                f.args.push((name, v));
            }
            _ => {}
        }
    }
    Err(Error::Malformed("layer effect properties".into()))
}

/// A colour argument: babl format name, the pixel bytes, then an ICC profile (ignored).
fn color(b: &mut Reader) -> Option<Arg> {
    let format = b.string("colour format").ok()?;
    let n = b.u32("colour").ok()? as usize;
    let px = b.bytes(n, "colour").ok()?;
    let f = |i: usize| -> Option<f32> {
        let c = px.get(i * 4..i * 4 + 4)?;
        Some(f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
    };
    let d = |i: usize| -> Option<f32> {
        let c = px.get(i * 8..i * 8 + 8)?;
        Some(f64::from_le_bytes(c.try_into().ok()?) as f32)
    };
    let (vals, linear) = match format.as_str() {
        "R'G'B'A float" => ([f(0)?, f(1)?, f(2)?, f(3)?], false),
        "R'G'B' float" => ([f(0)?, f(1)?, f(2)?, 1.0], false),
        "RGB float" => ([f(0)?, f(1)?, f(2)?, 1.0], true),
        // babl's HSV and HSL are of the sRGB-coded values.
        "HSV float" | "HSVA float" => {
            let a = if format.starts_with("HSVA") {
                f(3)?
            } else {
                1.0
            };
            let c = hsv_to_rgb(f(0)?, f(1)?, f(2)?);
            ([c[0], c[1], c[2], a], false)
        }
        "HSL float" | "HSLA float" => {
            let a = if format.starts_with("HSLA") {
                f(3)?
            } else {
                1.0
            };
            let c = hsl_to_rgb(f(0)?, f(1)?, f(2)?);
            ([c[0], c[1], c[2], a], false)
        }
        "RGBA float" => ([f(0)?, f(1)?, f(2)?, f(3)?], true),
        "R'G'B'A double" => ([d(0)?, d(1)?, d(2)?, d(3)?], false),
        "RGBA double" => ([d(0)?, d(1)?, d(2)?, d(3)?], true),
        _ => return None,
    };
    let mut c = vals;
    if linear {
        for v in &mut c[..3] {
            *v = linear_to_srgb(*v);
        }
    }
    Some(Arg::Color(c))
}

/// A layer's pixels while its filters run: straight R'G'B'A (sRGB-coded) floats, `w × h`.
pub(crate) struct Pixels<'a> {
    pub buf: &'a mut [f32],
    pub w: usize,
    pub h: usize,
}

/// Runs the visible, drawn filters of `list` over `px`, bottom (first) to top.
pub(crate) fn apply(list: &[Filter], px: &mut Pixels) {
    for f in list.iter().filter(|f| f.visible && f.drawn()) {
        let before = (f.opacity < 1.0).then(|| px.buf.to_vec());
        run(f, px);
        if let Some(before) = before {
            // The filter's opacity mixes its result with its input (linear, premultiplied).
            let k = f.opacity;
            for (o, i) in px.buf.chunks_exact_mut(4).zip(before.chunks_exact(4)) {
                let (ao, ai) = (o[3], i[3]);
                let a = ai + (ao - ai) * k;
                for c in 0..3 {
                    let (lo, li) = (srgb_to_linear(o[c]) * ao, srgb_to_linear(i[c]) * ai);
                    let v = li + (lo - li) * k;
                    o[c] = if a > 0.0 { linear_to_srgb(v / a) } else { 0.0 };
                }
                o[3] = a;
            }
        }
    }
}

fn each(px: &mut Pixels, f: impl Fn(&mut [f32])) {
    px.buf.chunks_exact_mut(4).for_each(f);
}

fn lin(p: &[f32]) -> [f32; 3] {
    [
        srgb_to_linear(p[0]),
        srgb_to_linear(p[1]),
        srgb_to_linear(p[2]),
    ]
}

fn set_lin(p: &mut [f32], c: [f32; 3]) {
    for k in 0..3 {
        p[k] = linear_to_srgb(c[k]);
    }
}

/// babl's luminance weights for sRGB primaries (adapted to D50).
const Y: [f32; 3] = [0.222_488_4, 0.716_903_7, 0.060_607_9];

fn run(f: &Filter, px: &mut Pixels) {
    match f.op.as_str() {
        "gegl:invert-gamma" => each(px, |p| {
            for v in &mut p[..3] {
                *v = 1.0 - *v;
            }
        }),
        "gegl:invert-linear" => each(px, |p| {
            let c = lin(p);
            set_lin(p, [1.0 - c[0], 1.0 - c[1], 1.0 - c[2]]);
        }),
        "gimp:desaturate" => {
            let mode = f.int("mode", 3);
            each(px, |p| {
                let c = lin(p);
                let g = match mode {
                    // Lightness (HSL), on the coded values.
                    0 => {
                        let (mx, mn) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
                        srgb_to_linear((mx + mn) / 2.0)
                    }
                    // Luma: the weights on the coded values.
                    1 => srgb_to_linear(Y[0] * p[0] + Y[1] * p[1] + Y[2] * p[2]),
                    // Average of the coded values.
                    2 => srgb_to_linear((p[0] + p[1] + p[2]) / 3.0),
                    // Value: the largest.
                    4 => c[0].max(c[1]).max(c[2]),
                    // Luminance (linear).
                    _ => Y[0] * c[0] + Y[1] * c[1] + Y[2] * c[2],
                };
                set_lin(p, [g; 3]);
            });
        }
        "gimp:threshold" => {
            let (lo, hi) = (f.float("low", 0.5), f.float("high", 1.0));
            let channel = f.int("channel", 0);
            each(px, |p| {
                let v = match channel {
                    1 => p[0],
                    2 => p[1],
                    3 => p[2],
                    4 => p[3],
                    _ => p[0].max(p[1]).max(p[2]),
                };
                let o = if v >= lo && v <= hi { 1.0 } else { 0.0 };
                if channel == 4 {
                    p[3] = o;
                } else {
                    p[..3].fill(o);
                }
            });
        }
        "gimp:posterize" => {
            let n = (f.int("levels", 3).max(2) - 1) as f32;
            // Colour and alpha alike.
            each(px, |p| {
                for v in p.iter_mut() {
                    *v = (v.clamp(0.0, 1.0) * n + 0.5).floor() / n;
                }
            });
        }
        "gimp:brightness-contrast" => {
            let b = f.config("brightness").unwrap_or(0.0) as f32;
            let c = f.config("contrast").unwrap_or(0.0) as f32;
            // In linear light: half the brightness towards white (or black), then the
            // contrast as a slope about the middle (fitted to GIMP's composites).
            let slant = ((c + 1.0) * std::f32::consts::FRAC_PI_4).tan();
            let b = b / 2.0;
            each(px, |p| {
                let mut c = lin(p);
                for x in &mut c {
                    let v = if b < 0.0 {
                        *x * (1.0 + b)
                    } else {
                        *x + (1.0 - *x) * b
                    };
                    // Not clamped: GIMP keeps the overshoot until the composite.
                    *x = (v - 0.5) * slant + 0.5;
                }
                set_lin(p, c);
            });
        }
        "gegl:mono-mixer" => {
            let (r, g, b) = (
                f.float("red", 0.333),
                f.float("green", 0.333),
                f.float("blue", 0.333),
            );
            let keep = f.int("preserve-luminosity", 0) != 0;
            let s = if keep && (r + g + b).abs() > 1e-6 {
                1.0 / (r + g + b)
            } else {
                1.0
            };
            each(px, |p| {
                let c = lin(p);
                let v = (c[0] * r + c[1] * g + c[2] * b) * s;
                set_lin(p, [v; 3]);
            });
        }
        "gegl:opacity" => {
            let k = f.float("value", 1.0);
            each(px, |p| p[3] *= k);
        }
        "gegl:color-to-alpha" => color_to_alpha(f, px),
        "gegl:gaussian-blur" => {
            let (sx, sy) = (f.float("std-dev-x", 1.5), f.float("std-dev-y", 1.5));
            let fir = f.int("filter", 0) == 1;
            let mut pm = premultiplied(px);
            blur(&mut pm, px.w, px.h, sx, sy, fir);
            straight(&pm, px);
        }
        "gegl:pixelize" => pixelize(f, px),
        "gimp:curves" => curves(f, px),
        "gimp:levels" => levels(f, px),
        "gimp:hue-saturation" => hue_saturation(f, px),
        "gimp:colorize" => {
            let (h, s, l) = (
                f.float("hue", 0.5),
                f.float("saturation", 0.5),
                f.float("lightness", 0.0),
            );
            // On linear light: the luminance moved by the lightness, the hue and
            // saturation given.
            each(px, |p| {
                let c = lin(p);
                let mut y = Y[0] * c[0] + Y[1] * c[1] + Y[2] * c[2];
                if l > 0.0 {
                    y = y * (1.0 - l) + l;
                } else if l < 0.0 {
                    y *= l + 1.0;
                }
                p[..3].copy_from_slice(&hsl_to_rgb(h, s, y));
                let c = [p[0], p[1], p[2]];
                set_lin(p, c);
            });
        }
        "gimp:color-balance" => color_balance(f, px),
        "gegl:exposure" => {
            // On linear light: (v − black) / (2^−exposure − black).
            let (black, e) = (f.float("black-level", 0.0), f.float("exposure", 0.0));
            let white = (-e).exp2();
            let d = if (white - black).abs() > 1e-6 {
                1.0 / (white - black)
            } else {
                1e6
            };
            each(px, |p| {
                let c = lin(p).map(|v| (v - black) * d);
                set_lin(p, c);
            });
        }
        "gegl:color-overlay" => {
            let c = f.color("value", [0.0, 0.0, 0.0, 0.0]);
            let (cl, a) = (
                [0, 1, 2].map(|i| srgb_to_linear(c[i])),
                c[3].clamp(0.0, 1.0),
            );
            each(px, |p| {
                let v = lin(p);
                set_lin(p, [0, 1, 2].map(|i| v[i] + (cl[i] - v[i]) * a));
            });
        }
        "gegl:dropshadow" => dropshadow(f, px),
        _ => {}
    }
}

/// The settings of a config string in sections: every `(marker name)` starts one, and the
/// `(key value…)` lines after it belong to it; lines before the first marker go to `""`.
fn sections<'a>(cfg: &'a str, marker: &str) -> Vec<(&'a str, Vec<(&'a str, &'a str)>)> {
    let mut out: Vec<(&str, Vec<(&str, &str)>)> = vec![("", Vec::new())];
    let pat = format!("({marker} ");
    let mut rest = cfg;
    while let Some(i) = rest.find('(') {
        rest = &rest[i..];
        // A balanced item.
        let mut depth = 0usize;
        let mut end = rest.len();
        for (k, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let item = &rest[1..end.saturating_sub(1).max(1)];
        if rest.starts_with(&pat) {
            out.push((item[marker.len()..].trim(), Vec::new()));
        } else if let Some((k, v)) = item.split_once(char::is_whitespace)
            && let Some(last) = out.last_mut()
        {
            last.1.push((k, v.trim()));
        }
        rest = &rest[end.min(rest.len())..];
        if out.len() > 64 {
            break;
        }
    }
    out
}

fn get<'a>(kv: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    kv.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn getf(kv: &[(&str, &str)], key: &str, default: f32) -> f32 {
    get(kv, key)
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

/// The config string of a GIMP tool operation.
fn config_text(f: &Filter) -> &str {
    match f.arg("config") {
        Some(Arg::Config(s)) => s,
        _ => "",
    }
}

/// Whether a GIMP point filter works on linear values (`(trc linear)`).
fn trc_linear(cfg: &str) -> bool {
    sections(cfg, "channel")[0]
        .1
        .iter()
        .any(|&(k, v)| k == "trc" && v == "linear")
}

/// Runs `map` over each pixel's colour in the filter's working coding.
fn point(px: &mut Pixels, linear: bool, map: impl Fn(&mut [f32; 3], &mut f32)) {
    each(px, |p| {
        let mut c = if linear { lin(p) } else { [p[0], p[1], p[2]] };
        let mut a = p[3];
        map(&mut c, &mut a);
        if linear {
            set_lin(p, c);
        } else {
            p[..3].copy_from_slice(&c);
        }
        p[3] = a;
    });
}

/// GIMP's curves: per channel (value, red, green, blue, alpha) the stored samples, mapped
/// with linear interpolation; colour channels through their own curve, then the value one.
fn curves(f: &Filter, px: &mut Pixels) {
    let cfg = config_text(f);
    let linear = trc_linear(cfg);
    let mut lut: [Option<Vec<f32>>; 5] = Default::default();
    for (name, kv) in sections(cfg, "channel") {
        let slot = match name {
            "value" => 0,
            "red" => 1,
            "green" => 2,
            "blue" => 3,
            "alpha" => 4,
            _ => continue,
        };
        let Some(s) = get(&kv, "curve") else { continue };
        let Some(i) = s.find("(samples ") else {
            continue;
        };
        let nums: Vec<f32> = s[i + 9..]
            .split(|c: char| c.is_whitespace() || c == ')')
            .filter(|t| !t.is_empty())
            .skip(1)
            .map_while(|t| t.parse::<f32>().ok())
            .collect();
        let identity = nums
            .iter()
            .enumerate()
            .all(|(k, &v)| (v - k as f32 / (nums.len().max(2) - 1) as f32).abs() < 1e-4);
        if nums.len() >= 2 && !identity {
            lut[slot] = Some(nums);
        }
    }
    let map = |t: &Option<Vec<f32>>, v: f32| -> f32 {
        let Some(t) = t else { return v };
        let x = v.clamp(0.0, 1.0) * (t.len() - 1) as f32;
        let i = (x as usize).min(t.len() - 2);
        t[i] + (t[i + 1] - t[i]) * (x - i as f32)
    };
    point(px, linear, |c, a| {
        for k in 0..3 {
            c[k] = map(&lut[0], map(&lut[k + 1], c[k]));
        }
        *a = map(&lut[4], *a);
    });
}

/// GIMP's levels: per channel (value, red, green, blue, alpha) input range, gamma and output
/// range; colour channels through their own levels, then the value ones.
fn levels(f: &Filter, px: &mut Pixels) {
    let cfg = config_text(f);
    let linear = trc_linear(cfg);
    let head = &sections(cfg, "channel")[0].1;
    let clamp_in = get(head, "clamp-input") == Some("yes");
    let clamp_out = get(head, "clamp-output") == Some("yes");
    let mut ch = [[0.0f32, 1.0, 1.0, 0.0, 1.0]; 5];
    for (name, kv) in sections(cfg, "channel") {
        let slot = match name {
            "value" => 0,
            "red" => 1,
            "green" => 2,
            "blue" => 3,
            "alpha" => 4,
            _ => continue,
        };
        ch[slot] = [
            getf(&kv, "low-input", 0.0),
            getf(&kv, "high-input", 1.0),
            getf(&kv, "gamma", 1.0),
            getf(&kv, "low-output", 0.0),
            getf(&kv, "high-output", 1.0),
        ];
    }
    let map = |p: &[f32; 5], v: f32| -> f32 {
        let [li, hi, g, lo, ho] = *p;
        let mut v = if hi != li {
            (v - li) / (hi - li)
        } else {
            v - li
        };
        if clamp_in {
            v = v.clamp(0.0, 1.0);
        }
        if g > 0.0 && g != 1.0 && v > 0.0 {
            v = v.powf(1.0 / g);
        }
        let v = v * (ho - lo) + lo;
        if clamp_out { v.clamp(0.0, 1.0) } else { v }
    };
    point(px, linear, |c, a| {
        for k in 0..3 {
            c[k] = map(&ch[0], map(&ch[k + 1], c[k]));
        }
        *a = map(&ch[4], *a);
    });
}

/// GIMP's hue-saturation: per hue range (all, then red, yellow, green, cyan, blue, magenta)
/// a hue turn, a saturation and a lightness change, ranges blended over their overlap; on
/// the sRGB-coded HSL values.
fn hue_saturation(f: &Filter, px: &mut Pixels) {
    let cfg = config_text(f);
    let mut hsl = [[0.0f32; 3]; 7];
    let mut overlap = 0.0f32;
    for (name, kv) in sections(cfg, "range") {
        let slot = match name {
            "all" => 0,
            "red" => 1,
            "yellow" => 2,
            "green" => 3,
            "cyan" => 4,
            "blue" => 5,
            "magenta" => 6,
            _ => {
                overlap = getf(&kv, "overlap", overlap);
                continue;
            }
        };
        hsl[slot] = [
            getf(&kv, "hue", 0.0),
            getf(&kv, "saturation", 0.0),
            getf(&kv, "lightness", 0.0),
        ];
        overlap = getf(&kv, "overlap", overlap);
    }
    let overlap = overlap / 2.0;
    let hue = |r: usize, v: f32| (v + (hsl[0][0] + hsl[r][0]) / 2.0).rem_euclid(1.0);
    let sat = |r: usize, v: f32| (v * (hsl[0][1] + hsl[r][1] + 1.0)).clamp(0.0, 1.0);
    let light = |r: usize, v: f32| {
        let k = (hsl[0][2] + hsl[r][2]) / 2.0;
        if k < 0.0 {
            v * (k + 1.0)
        } else {
            v + k * (1.0 - v)
        }
    };
    each(px, |p| {
        let (h, s, l) = rgb_to_hsl([p[0], p[1], p[2]]);
        let x = h * 6.0;
        let mut primary = 0usize;
        let mut secondary = None;
        for r in 0..7 {
            let t = r as f32 + 0.5;
            if x < t + overlap {
                primary = r;
                if overlap > 0.0 && x > t - overlap {
                    secondary = Some(((r + 1) % 6, (x - t + overlap) / (2.0 * overlap)));
                }
                break;
            }
        }
        let primary = primary % 6 + 1;
        let (h, s, l) = match secondary {
            Some((r2, k)) => {
                let r2 = r2 + 1;
                (
                    hue(primary, h) * (1.0 - k) + hue(r2, h) * k,
                    sat(primary, s) * (1.0 - k) + sat(r2, s) * k,
                    light(primary, l) * (1.0 - k) + light(r2, l) * k,
                )
            }
            None => (hue(primary, h), sat(primary, s), light(primary, l)),
        };
        p[..3].copy_from_slice(&hsl_to_rgb(h, s, l));
    });
}

/// GIMP's colour balance: cyan–red, magenta–green and yellow–blue shifts for shadows,
/// midtones and highlights weighted by the pixel's lightness, optionally keeping it.
fn color_balance(f: &Filter, px: &mut Pixels) {
    let cfg = config_text(f);
    let mut adj = [[0.0f32; 3]; 3];
    let mut keep = false;
    for (name, kv) in sections(cfg, "range") {
        let slot = match name {
            "shadows" => 0,
            "midtones" => 1,
            "highlights" => 2,
            _ => continue,
        };
        adj[slot] = [
            getf(&kv, "cyan-red", 0.0),
            getf(&kv, "magenta-green", 0.0),
            getf(&kv, "yellow-blue", 0.0),
        ];
        keep |= get(&kv, "preserve-luminosity") == Some("yes");
    }
    keep |= cfg.contains("(preserve-luminosity yes)");
    let map = |v: f32, l: f32, c: usize| -> f32 {
        let (a, b, scale) = (0.25f32, 0.333f32, 0.7f32);
        let sh = adj[0][c] * ((l - b) / -a + 0.5).clamp(0.0, 1.0) * scale;
        let mid = adj[1][c]
            * ((l - b) / a + 0.5).clamp(0.0, 1.0)
            * ((l + b - 1.0) / -a + 0.5).clamp(0.0, 1.0)
            * scale;
        let hi = adj[2][c] * ((l + b - 1.0) / a + 0.5).clamp(0.0, 1.0) * scale;
        (v + sh + mid + hi).clamp(0.0, 1.0)
    };
    each(px, |p| {
        let (_, _, l) = rgb_to_hsl([p[0], p[1], p[2]]);
        let c = [map(p[0], l, 0), map(p[1], l, 1), map(p[2], l, 2)];
        let c = if keep {
            let (h, s, _) = rgb_to_hsl(c);
            hsl_to_rgb(h, s, l)
        } else {
            c
        };
        p[..3].copy_from_slice(&c);
    });
}

fn rgb_to_hsl([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d <= 1e-7 {
        return (0.0, 0.0, l);
    }
    let s = if l <= 0.5 {
        d / (mx + mn)
    } else {
        d / (2.0 - mx - mn)
    };
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn chroma_rgb(h: f32, c: f32, m: f32) -> [f32; 3] {
    let hp = h.rem_euclid(1.0) * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    chroma_rgb(h, c, l - c / 2.0)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    chroma_rgb(h, c, v - c)
}

/// Linear, premultiplied RGBA of `px`.
fn premultiplied(px: &Pixels) -> Vec<f32> {
    let mut out = vec![0.0; px.buf.len()];
    for (o, p) in out.chunks_exact_mut(4).zip(px.buf.chunks_exact(4)) {
        let c = lin(p);
        let a = p[3];
        o[0] = c[0] * a;
        o[1] = c[1] * a;
        o[2] = c[2] * a;
        o[3] = a;
    }
    out
}

/// Back from [`premultiplied`].
fn straight(pm: &[f32], px: &mut Pixels) {
    for (p, o) in px.buf.chunks_exact_mut(4).zip(pm.chunks_exact(4)) {
        let a = o[3];
        if a > 1e-7 {
            set_lin(p, [o[0] / a, o[1] / a, o[2] / a]);
        } else {
            p[..3].fill(0.0);
        }
        p[3] = a;
    }
}

/// GEGL's Gaussian blur: recursive (Young and van Vliet's IIR, as GEGL picks for deviations
/// over 1) unless `fir`, edges clamped.
fn blur(buf: &mut [f32], w: usize, h: usize, sx: f32, sy: f32, fir: bool) {
    if fir {
        gauss(buf, w, h, sx, sy);
        return;
    }
    let (fx, fy) = if sx > 1.0 { (0.0, sx) } else { (sx, 0.0) };
    let (gx, gy) = if sy > 1.0 { (0.0, sy) } else { (sy, 0.0) };
    gauss(buf, w, h, fx, gx);
    iir(buf, w, h, 1, w, fy);
    iir(buf, h, w, w, 1, gy);
}

/// Young and van Vliet's recursive Gaussian of deviation `s` (> 1) along lines of `n`
/// 4-channel pixels (`lines` of them, pixels `step` apart, lines `line` apart), edges clamped.
fn iir(buf: &mut [f32], n: usize, lines: usize, step: usize, line: usize, s: f32) {
    if s <= 1.0 || n == 0 {
        return;
    }
    let s = f64::from(s);
    let q = if s >= 2.5 {
        0.98711 * s - 0.96330
    } else {
        3.97156 - 4.14554 * (1.0 - 0.26891 * s).sqrt()
    };
    let b0 = 1.57825 + 2.44413 * q + 1.4281 * q * q + 0.422205 * q * q * q;
    let b1 = (2.44413 * q + 2.85619 * q * q + 1.26661 * q * q * q) / b0;
    let b2 = -(1.4281 * q * q + 1.26661 * q * q * q) / b0;
    let b3 = 0.422205 * q * q * q / b0;
    let bb = 1.0 - (b1 + b2 + b3);
    let pad = (s * 6.0).ceil() as usize + 8;
    let len = n + 2 * pad;
    let mut x = vec![[0f64; 4]; len];
    let mut wv = vec![[0f64; 4]; len];
    for l in 0..lines {
        let at = |i: usize| (l * line + i * step) * 4;
        for (i, v) in x.iter_mut().enumerate() {
            let k = at(i.saturating_sub(pad).min(n - 1));
            for c in 0..4 {
                v[c] = f64::from(buf[k + c]);
            }
        }
        for c in 0..4 {
            let (mut p1, mut p2, mut p3) = (x[0][c], x[0][c], x[0][c]);
            for i in 0..len {
                let v = bb * x[i][c] + b1 * p1 + b2 * p2 + b3 * p3;
                wv[i][c] = v;
                (p3, p2, p1) = (p2, p1, v);
            }
            let last = wv[len - 1][c];
            let (mut n1, mut n2, mut n3) = (last, last, last);
            for i in (0..len).rev() {
                let v = bb * wv[i][c] + b1 * n1 + b2 * n2 + b3 * n3;
                x[i][c] = v;
                (n3, n2, n1) = (n2, n1, v);
            }
        }
        for i in 0..n {
            let k = at(i);
            for c in 0..4 {
                buf[k + c] = x[i + pad][c] as f32;
            }
        }
    }
}

/// A Gaussian of standard deviations `sx`, `sy` over 4-channel rows, edges clamped.
fn gauss(buf: &mut [f32], w: usize, h: usize, sx: f32, sy: f32) {
    fn kernel(s: f32) -> Vec<f32> {
        if s < 0.05 {
            return vec![1.0];
        }
        let r = (s * 3.5).ceil() as i32;
        let k: Vec<f32> = (-r..=r)
            .map(|i| (-(i * i) as f32 / (2.0 * s * s)).exp())
            .collect();
        let t: f32 = k.iter().sum();
        k.iter().map(|v| v / t).collect()
    }
    let pass = |buf: &mut [f32], n: usize, lines: usize, step: usize, line: usize, k: &[f32]| {
        if k.len() == 1 {
            return;
        }
        let r = (k.len() / 2) as isize;
        let mut tmp = vec![0f32; n * 4];
        for l in 0..lines {
            let at = |i: isize| l * line + (i.clamp(0, n as isize - 1) as usize) * step;
            for (i, t) in tmp.chunks_exact_mut(4).enumerate() {
                let mut acc = [0f32; 4];
                for (j, kv) in k.iter().enumerate() {
                    let s = at(i as isize + j as isize - r) * 4;
                    for c in 0..4 {
                        acc[c] += buf[s + c] * kv;
                    }
                }
                t.copy_from_slice(&acc);
            }
            for (i, t) in tmp.chunks_exact(4).enumerate() {
                let d = at(i as isize) * 4;
                buf[d..d + 4].copy_from_slice(t);
            }
        }
    };
    pass(buf, w, h, 1, w, &kernel(sx));
    pass(buf, h, w, w, 1, &kernel(sy));
}

fn pixelize(f: &Filter, px: &mut Pixels) {
    let (bw, bh) = (
        f.int("size-x", 16).max(1) as usize,
        f.int("size-y", 16).max(1) as usize,
    );
    let (ox, oy) = (f.int("offset-x", 0), f.int("offset-y", 0));
    let pm = premultiplied(px);
    let w = px.w;
    let mut out = pm.clone();
    // Blocks start at the offset (GEGL's grid is anchored to the layer's origin).
    let first = |o: i32, b: usize| -> isize { -(((o.rem_euclid(b as i32)) as isize) % b as isize) };
    let (fx, fy) = (first(-ox, bw), first(-oy, bh));
    let mut y = fy;
    while y < px.h as isize {
        let (y0, y1) = (y.max(0) as usize, ((y + bh as isize) as usize).min(px.h));
        let mut x = fx;
        while x < w as isize {
            let (x0, x1) = (x.max(0) as usize, ((x + bw as isize) as usize).min(w));
            let mut s = [0f32; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * w + xx) * 4;
                    for c in 0..4 {
                        s[c] += pm[i + c];
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)).max(1) as f32;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * w + xx) * 4;
                    for c in 0..4 {
                        out[i + c] = s[c] / n;
                    }
                }
            }
            x += bw as isize;
        }
        y += bh as isize;
    }
    straight(&out, px);
}

fn dropshadow(f: &Filter, px: &mut Pixels) {
    let (dx, dy) = (
        f.float("x", 20.0).round() as isize,
        f.float("y", 20.0).round() as isize,
    );
    let radius = f.float("radius", 10.0);
    let col = f.color("color", [0.0, 0.0, 0.0, 1.0]);
    let opacity = f.float("opacity", 0.5);
    let (w, h) = (px.w, px.h);
    // The shadow: the alpha moved, blurred, in the colour at the opacity.
    let mut sh = vec![0f32; w * h * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let (sx, sy) = (x - dx, y - dy);
            if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                sh[(y as usize * w + x as usize) * 4 + 3] =
                    px.buf[(sy as usize * w + sx as usize) * 4 + 3];
            }
        }
    }
    blur(&mut sh, w, h, radius, radius, false);
    let cl = lin(&col);
    let pm = premultiplied(px);
    let mut out = pm.clone();
    for (o, (s, p)) in out
        .chunks_exact_mut(4)
        .zip(sh.chunks_exact(4).zip(pm.chunks_exact(4)))
    {
        let sa = (s[3] * col[3] * opacity).clamp(0.0, 1.0);
        let pa = p[3];
        for c in 0..3 {
            o[c] = p[c] + cl[c] * sa * (1.0 - pa);
        }
        o[3] = pa + sa * (1.0 - pa);
    }
    straight(&out, px);
}

fn color_to_alpha(f: &Filter, px: &mut Pixels) {
    let col = f.color("color", [1.0, 1.0, 1.0, 1.0]);
    let t0 = f.float("transparency-threshold", 0.0);
    let t1 = f.float("opacity-threshold", 1.0).max(t0 + 1e-6);
    // On the coded values: per channel, the distance to the colour over the room there is
    // on that side; the largest is the least alpha that gives the pixel over the colour.
    each(px, |p| {
        let mut alpha = 0f32;
        for k in 0..3 {
            let d = p[k] - col[k];
            let room = if d > 0.0 { 1.0 - col[k] } else { col[k] };
            let a = if room > 1e-6 { d.abs() / room } else { 1.0 };
            alpha = alpha.max(a);
        }
        let alpha = ((alpha - t0) / (t1 - t0)).clamp(0.0, 1.0);
        if alpha > 1e-6 {
            for k in 0..3 {
                p[k] = ((p[k] - col[k]) / alpha + col[k]).clamp(0.0, 1.0);
            }
        }
        p[3] *= alpha;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_values_are_read() {
        let s = "(time 0)\n(brightness 0.2)\n(contrast -0.5)\n";
        assert_eq!(config_value(s, "brightness"), Some(0.2));
        assert_eq!(config_value(s, "contrast"), Some(-0.5));
        assert_eq!(config_value(s, "gamma"), None);
    }

    fn tool(op: &str, cfg: &str) -> Filter {
        Filter {
            op: op.into(),
            visible: true,
            opacity: 1.0,
            mode: 0,
            args: vec![("config".into(), Arg::Config(cfg.into()))],
        }
    }

    fn run1(f: &Filter, rgba: [f32; 4]) -> [f32; 4] {
        let mut buf = rgba.to_vec();
        run(
            f,
            &mut Pixels {
                buf: &mut buf,
                w: 1,
                h: 1,
            },
        );
        [buf[0], buf[1], buf[2], buf[3]]
    }

    #[test]
    fn tool_configs_split_into_sections() {
        let s = sections(
            "(time 0)\n(trc linear)\n(channel value)\n(low-input 0.25)\n(channel red)\n(curve\n    (samples 3 0 0.5 1))\n",
            "channel",
        );
        assert_eq!(s[0].1, [("time", "0"), ("trc", "linear")]);
        assert_eq!(s[1].0, "value");
        assert_eq!(s[1].1, [("low-input", "0.25")]);
        assert_eq!(s[2].0, "red");
        assert!(s[2].1[0].1.contains("(samples 3 0 0.5 1)"));
    }

    #[test]
    fn levels_curves_and_hue_saturation() {
        // Levels: red input 0.2..0.6 stretched, value untouched (perceptual coding).
        let l = tool(
            "gimp:levels",
            "(trc non-linear)\n(channel value)\n(low-input 0)\n(high-input 1)\n(gamma 1)\n(low-output 0)\n(high-output 1)\n(channel red)\n(low-input 0.2)\n(high-input 0.6)\n(gamma 1)\n(low-output 0)\n(high-output 1)\n",
        );
        let o = run1(&l, [0.4, 0.4, 0.4, 1.0]);
        assert!((o[0] - 0.5).abs() < 1e-5 && (o[1] - 0.4).abs() < 1e-5);
        // Curves: a red curve of three samples (0, 0.25, 1) halves the middle.
        let c = tool(
            "gimp:curves",
            "(trc non-linear)\n(channel red)\n(curve\n    (curve-type smooth)\n    (n-samples 3)\n    (samples 3 0 0.25 1))\n",
        );
        let o = run1(&c, [0.5, 0.5, 0.5, 1.0]);
        assert!((o[0] - 0.25).abs() < 1e-5 && (o[2] - 0.5).abs() < 1e-5);
        // Hue-saturation: a turn of the master hue by 1/3 (120°) makes red green.
        let h = tool(
            "gimp:hue-saturation",
            "(range all)\n(hue 0.66666666)\n(saturation 0)\n(lightness 0)\n(overlap 0)\n",
        );
        let o = run1(&h, [1.0, 0.0, 0.0, 1.0]);
        assert!(
            o[0] < 1e-4 && (o[1] - 1.0).abs() < 1e-4 && o[2] < 1e-4,
            "{o:?}"
        );
        // Colour balance with nothing set keeps the colour.
        let b = tool(
            "gimp:color-balance",
            "(range midtones)\n(cyan-red 0)\n(magenta-green 0)\n(yellow-blue 0)\n(preserve-luminosity yes)\n",
        );
        let o = run1(&b, [0.3, 0.5, 0.7, 1.0]);
        assert!((o[0] - 0.3).abs() < 1e-4 && (o[2] - 0.7).abs() < 1e-4);
    }

    fn string(v: &mut Vec<u8>, s: &str) {
        v.extend_from_slice(&(s.len() as u32 + 1).to_be_bytes());
        v.extend_from_slice(s.as_bytes());
        v.push(0);
    }

    fn prop(v: &mut Vec<u8>, id: u32, body: &[u8]) {
        v.extend_from_slice(&id.to_be_bytes());
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(body);
    }

    #[test]
    fn records_are_read() {
        let mut v = Vec::new();
        string(&mut v, "gegl:gaussian-blur");
        string(&mut v, "gimp-image");
        string(&mut v, "gegl:gaussian-blur");
        string(&mut v, "0:0");
        prop(&mut v, PROP_VISIBLE, &0u32.to_be_bytes());
        prop(&mut v, PROP_FLOAT_OPACITY, &0.5f32.to_be_bytes());
        let mut arg = Vec::new();
        string(&mut arg, "std-dev-x");
        arg.extend_from_slice(&3u32.to_be_bytes());
        arg.extend_from_slice(&2.5f32.to_be_bytes());
        prop(&mut v, PROP_FILTER_ARGUMENT, &arg);
        let mut arg = Vec::new();
        string(&mut arg, "color");
        arg.extend_from_slice(&8u32.to_be_bytes());
        string(&mut arg, "R'G'B'A float");
        arg.extend_from_slice(&16u32.to_be_bytes());
        for c in [1.0f32, 0.5, 0.0, 1.0] {
            arg.extend_from_slice(&c.to_le_bytes());
        }
        arg.extend_from_slice(&0u32.to_be_bytes());
        prop(&mut v, PROP_FILTER_ARGUMENT, &arg);
        prop(&mut v, PROP_END, &[]);
        let f = read(&v, 0, false).unwrap();
        assert_eq!(f.op, "gegl:gaussian-blur");
        assert!(!f.visible);
        assert_eq!(f.opacity, 0.5);
        assert_eq!(f.float("std-dev-x", 0.0), 2.5);
        assert_eq!(f.color("color", [0.0; 4]), [1.0, 0.5, 0.0, 1.0]);
        assert!(f.drawn());
        // Cut short: an error, not a panic.
        assert!(read(&v[..v.len() - 5], 0, false).is_err());
    }

    #[test]
    fn invert_runs_on_coded_values() {
        let mut buf = vec![0.25, 0.5, 1.0, 1.0];
        let f = Filter {
            op: "gegl:invert-gamma".into(),
            visible: true,
            opacity: 1.0,
            mode: 0,
            args: Vec::new(),
        };
        apply(
            &[f],
            &mut Pixels {
                buf: &mut buf,
                w: 1,
                h: 1,
            },
        );
        assert_eq!(buf, [0.75, 0.5, 0.0, 1.0]);
    }
}
