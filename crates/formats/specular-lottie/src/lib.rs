//! Lottie animations (Bodymovin JSON, the Lottie specification's model) and `.lottie`
//! packages (a ZIP with a manifest and `animations/*.json`), read into a layer tree and
//! drawn frame by frame as `specular-drawpage` pages.
//!
//! Drawn: shape layers (groups with transforms; rectangles, ellipses, polystars and paths;
//! solid and gradient fills and strokes with dashes; trim paths), solid layers, image
//! layers with embedded pictures, precompositions (with their own time and size), null
//! layers as parents, layer transforms (anchor, position or split position, scale,
//! rotation, skew, opacity) through the parent chain, masks (add, subtract, intersect;
//! lighten and difference as add, darken as intersect) and
//! track mattes (alpha and inverted alpha; luma mattes as alpha). Keyframes ease along
//! their cubic beziers; positions follow their spatial tangents.
//!
//! Not drawn, and counted: text layers, effects, repeaters, merge paths, expressions
//! (their static values are used). Nothing is fetched: pictures referenced by URL are
//! left out.

mod prop;
mod render;

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use specular_drawpage::ImageDecoder;

pub use render::frame_page;

use prop::{PathProp, Prop};

/// Why an animation was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "damaged Lottie animation: {s}"),
            Error::Unsupported(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

/// Largest JSON read.
const MAX_JSON: usize = 128 << 20;
/// Most frames played.
pub const MAX_FRAMES: u32 = 36_000;
/// Deepest nesting of groups and precompositions followed.
const MAX_DEPTH: usize = 32;

/// Whether `d` is a Lottie animation: a JSON object naming the version, frame rate, in and
/// out points and size near its start, or near its end (exporters that write the assets
/// first put them last).
pub fn is_lottie_json(d: &[u8]) -> bool {
    let h = &d[..d.len().min(4096)];
    let start = h
        .iter()
        .position(|b| !b.is_ascii_whitespace() && *b != 0xEF && *b != 0xBB && *b != 0xBF);
    if start.is_none_or(|i| h[i] != b'{') {
        return false;
    }
    let keys = |h: &[u8]| {
        let has = |k: &[u8]| h.windows(k.len()).any(|w| w == k);
        has(b"\"v\"")
            && has(b"\"fr\"")
            && has(b"\"ip\"")
            && has(b"\"op\"")
            && has(b"\"w\"")
            && has(b"\"h\"")
    };
    keys(h) || keys(&d[d.len().saturating_sub(4096)..])
}

/// Whether `d` is a `.lottie` package: a ZIP holding `manifest.json` and an animation.
pub fn is_dotlottie(d: &[u8]) -> bool {
    d.starts_with(b"PK\x03\x04")
        && specular_ora::zip::Zip::parse(d).is_ok_and(|z| {
            z.find("manifest.json").is_some()
                && z.entries
                    .iter()
                    .any(|e| e.name.starts_with("animations/") && e.name.ends_with(".json"))
        })
}

/// A layer transform.
#[derive(Clone, Debug)]
pub(crate) struct Transform {
    pub anchor: Prop,
    pub position: Prop,
    /// Split position: x and y apart.
    pub split: Option<(Prop, Prop)>,
    pub scale: Prop,
    pub rotation: Prop,
    pub opacity: Prop,
    pub skew: Prop,
    pub skew_axis: Prop,
}

impl Transform {
    fn parse(v: Option<&Value>) -> Transform {
        let g = |k: &str| v.and_then(|v| v.get(k));
        let p = g("p");
        let split = p
            .filter(|p| p.get("s").and_then(Value::as_bool) == Some(true))
            .map(|p| {
                (
                    Prop::parse(p.get("x"), &[0.0]),
                    Prop::parse(p.get("y"), &[0.0]),
                )
            });
        Transform {
            anchor: Prop::parse(g("a"), &[0.0, 0.0]),
            position: Prop::parse(p, &[0.0, 0.0]),
            split,
            scale: Prop::parse(g("s"), &[100.0, 100.0]),
            rotation: Prop::parse(g("r").or(g("rz")), &[0.0]),
            opacity: Prop::parse(g("o"), &[100.0]),
            skew: Prop::parse(g("sk"), &[0.0]),
            skew_axis: Prop::parse(g("sa"), &[0.0]),
        }
    }
}

/// A gradient: start, end, colour stops (count, values), radial or linear.
#[derive(Clone, Debug)]
pub(crate) struct Gradient {
    pub radial: bool,
    pub start: Prop,
    pub end: Prop,
    pub count: usize,
    pub stops: Prop,
}

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Group {
        items: Vec<Item>,
        transform: Option<Transform>,
    },
    Rect {
        position: Prop,
        size: Prop,
        round: Prop,
    },
    Ellipse {
        position: Prop,
        size: Prop,
    },
    Star {
        star: bool,
        points: Prop,
        position: Prop,
        rotation: Prop,
        inner: Prop,
        outer: Prop,
    },
    Path(PathProp),
    Fill {
        colour: Prop,
        opacity: Prop,
        even_odd: bool,
    },
    GradientFill {
        g: Gradient,
        opacity: Prop,
        even_odd: bool,
    },
    Stroke {
        colour: Prop,
        opacity: Prop,
        width: Prop,
        cap: u8,
        join: u8,
        dashes: Vec<(char, Prop)>,
    },
    GradientStroke {
        g: Gradient,
        opacity: Prop,
        width: Prop,
        cap: u8,
        join: u8,
    },
    Trim {
        start: Prop,
        end: Prop,
        offset: Prop,
    },
    /// Not drawn (repeaters, merge paths, rounded corners, …).
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct Mask {
    pub mode: char,
    pub inverted: bool,
    pub path: PathProp,
    pub opacity: Prop,
}

#[derive(Clone, Debug)]
pub(crate) enum Content {
    Shapes(Vec<Item>),
    Solid {
        colour: [u8; 4],
        w: f64,
        h: f64,
    },
    Image(String),
    Precomp {
        id: String,
        w: f64,
        h: f64,
        remap: Option<Prop>,
    },
    Null,
    /// Text and others: not drawn.
    Unsupported,
}

#[derive(Clone, Debug)]
pub(crate) struct Layer {
    pub index: Option<i64>,
    pub parent: Option<i64>,
    pub content: Content,
    pub transform: Transform,
    pub in_point: f64,
    pub out_point: f64,
    pub start: f64,
    pub stretch: f64,
    pub hidden: bool,
    pub masks: Vec<Mask>,
    /// Matte type of this layer (1 alpha, 2 inverted alpha, 3 luma, 4 inverted luma).
    pub matte: u8,
    /// This layer is the matte of the next.
    pub is_matte: bool,
}

/// A read animation.
pub struct Animation {
    pub width: f64,
    pub height: f64,
    pub frame_rate: f64,
    pub in_point: f64,
    pub out_point: f64,
    pub name: String,
    pub version: String,
    pub(crate) layers: Vec<Layer>,
    pub(crate) precomps: HashMap<String, Vec<Layer>>,
    pub(crate) images: HashMap<String, Arc<specular_drawpage::Image>>,
    /// Layers and features read but not drawn (text, effects, repeaters, missing pictures).
    pub not_drawn: Vec<String>,
}

impl Animation {
    /// Frames played (out point less in point, at most [`MAX_FRAMES`]).
    pub fn frames(&self) -> u32 {
        ((self.out_point - self.in_point).ceil().max(1.0) as u32).min(MAX_FRAMES)
    }

    /// Seconds a frame.
    pub fn frame_seconds(&self) -> f64 {
        1.0 / self.frame_rate
    }
}

fn colour01(v: &[f64]) -> [u8; 4] {
    let c = |i: usize, d: f64| {
        let x = v.get(i).copied().unwrap_or(d);
        // Some exports write 0–255.
        let x = if v.iter().take(3).any(|&y| y > 1.0) {
            x / 255.0
        } else {
            x
        };
        (x.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    [c(0, 0.0), c(1, 0.0), c(2, 0.0), c(3, 1.0)]
}

fn parse_items(v: Option<&Value>, depth: usize, notes: &mut Vec<String>) -> Vec<Item> {
    let Some(Value::Array(items)) = v else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        if it.get("hd").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let g = |k: &str| it.get(k);
        let ty = it.get("ty").and_then(Value::as_str).unwrap_or("");
        let fill_rule = g("r").and_then(Value::as_i64) == Some(2);
        let gradient = || Gradient {
            radial: g("t").and_then(Value::as_i64) == Some(2),
            start: Prop::parse(g("s"), &[0.0, 0.0]),
            end: Prop::parse(g("e"), &[0.0, 0.0]),
            count: it.pointer("/g/p").and_then(Value::as_u64).unwrap_or(0) as usize,
            stops: Prop::parse(it.pointer("/g/k"), &[]),
        };
        let cap = g("lc").and_then(Value::as_u64).unwrap_or(2) as u8;
        let join = g("lj").and_then(Value::as_u64).unwrap_or(2) as u8;
        out.push(match ty {
            "gr" if depth < MAX_DEPTH => {
                let mut items = parse_items(g("it"), depth + 1, notes);
                // The group's transform is its "tr" item.
                let tr = it
                    .get("it")
                    .and_then(Value::as_array)
                    .and_then(|a| {
                        a.iter()
                            .find(|i| i.get("ty").and_then(Value::as_str) == Some("tr"))
                    })
                    .map(|t| Transform::parse(Some(t)));
                items.retain(|i| !matches!(i, Item::Other));
                Item::Group {
                    items,
                    transform: tr,
                }
            }
            "rc" => Item::Rect {
                position: Prop::parse(g("p"), &[0.0, 0.0]),
                size: Prop::parse(g("s"), &[0.0, 0.0]),
                round: Prop::parse(g("r"), &[0.0]),
            },
            "el" => Item::Ellipse {
                position: Prop::parse(g("p"), &[0.0, 0.0]),
                size: Prop::parse(g("s"), &[0.0, 0.0]),
            },
            "sr" => Item::Star {
                star: g("sy").and_then(Value::as_i64) != Some(2),
                points: Prop::parse(g("pt"), &[5.0]),
                position: Prop::parse(g("p"), &[0.0, 0.0]),
                rotation: Prop::parse(g("r"), &[0.0]),
                inner: Prop::parse(g("ir"), &[0.0]),
                outer: Prop::parse(g("or"), &[0.0]),
            },
            "sh" => Item::Path(PathProp::parse(g("ks"))),
            "fl" => Item::Fill {
                colour: Prop::parse(g("c"), &[0.0, 0.0, 0.0, 1.0]),
                opacity: Prop::parse(g("o"), &[100.0]),
                even_odd: fill_rule,
            },
            "gf" => Item::GradientFill {
                g: gradient(),
                opacity: Prop::parse(g("o"), &[100.0]),
                even_odd: fill_rule,
            },
            "st" => Item::Stroke {
                colour: Prop::parse(g("c"), &[0.0, 0.0, 0.0, 1.0]),
                opacity: Prop::parse(g("o"), &[100.0]),
                width: Prop::parse(g("w"), &[1.0]),
                cap,
                join,
                dashes: g("d")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|d| {
                                let n = d.get("n").and_then(Value::as_str).unwrap_or("d");
                                (
                                    n.chars().next().unwrap_or('d'),
                                    Prop::parse(d.get("v"), &[0.0]),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            "gs" => Item::GradientStroke {
                g: gradient(),
                opacity: Prop::parse(g("o"), &[100.0]),
                width: Prop::parse(g("w"), &[1.0]),
                cap,
                join,
            },
            "tm" => Item::Trim {
                start: Prop::parse(g("s"), &[0.0]),
                end: Prop::parse(g("e"), &[100.0]),
                offset: Prop::parse(g("o"), &[0.0]),
            },
            "tr" => Item::Other,
            other => {
                let what = match other {
                    "rp" => "repeaters",
                    "mm" => "merge paths",
                    "rd" => "rounded corners",
                    "pb" => "puckers and bloats",
                    "tw" => "twists",
                    "zz" => "zig-zags",
                    "op" => "offset paths",
                    _ => "unknown shape items",
                };
                if !notes.iter().any(|n| n == what) {
                    notes.push(what.into());
                }
                Item::Other
            }
        });
    }
    out
}

fn parse_layers(v: Option<&Value>, notes: &mut Vec<String>) -> Vec<Layer> {
    let Some(Value::Array(layers)) = v else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(layers.len());
    for l in layers {
        let g = |k: &str| l.get(k);
        let num = |k: &str, d: f64| g(k).and_then(Value::as_f64).unwrap_or(d);
        let ty = g("ty").and_then(Value::as_i64).unwrap_or(-1);
        let content = match ty {
            0 => Content::Precomp {
                id: g("refId").and_then(Value::as_str).unwrap_or("").to_owned(),
                w: num("w", 0.0),
                h: num("h", 0.0),
                remap: g("tm").map(|t| Prop::parse(Some(t), &[0.0])),
            },
            1 => Content::Solid {
                colour: g("sc")
                    .and_then(Value::as_str)
                    .and_then(hex)
                    .unwrap_or([0, 0, 0, 255]),
                w: num("sw", 0.0),
                h: num("sh", 0.0),
            },
            2 => Content::Image(g("refId").and_then(Value::as_str).unwrap_or("").to_owned()),
            3 => Content::Null,
            4 => Content::Shapes(parse_items(g("shapes"), 0, notes)),
            5 => {
                if !notes.iter().any(|n| n == "text layers") {
                    notes.push("text layers".into());
                }
                Content::Unsupported
            }
            _ => Content::Unsupported,
        };
        if g("ef")
            .and_then(Value::as_array)
            .is_some_and(|e| !e.is_empty())
            && !notes.iter().any(|n| n == "effects")
        {
            notes.push("effects".into());
        }
        let masks = g("masksProperties")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|m| Mask {
                        mode: m
                            .get("mode")
                            .and_then(Value::as_str)
                            .and_then(|s| s.chars().next())
                            .unwrap_or('a'),
                        inverted: m.get("inv").and_then(Value::as_bool).unwrap_or(false),
                        path: PathProp::parse(m.get("pt")),
                        opacity: Prop::parse(m.get("o"), &[100.0]),
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Layer {
            index: g("ind").and_then(Value::as_i64),
            parent: g("parent").and_then(Value::as_i64),
            content,
            transform: Transform::parse(g("ks")),
            in_point: num("ip", f64::NEG_INFINITY),
            out_point: num("op", f64::INFINITY),
            start: num("st", 0.0),
            stretch: num("sr", 1.0).max(1e-3),
            hidden: g("hd").and_then(Value::as_bool) == Some(true),
            masks,
            matte: g("tt").and_then(Value::as_u64).unwrap_or(0) as u8,
            is_matte: g("td").and_then(Value::as_u64).unwrap_or(0) == 1,
        });
    }
    out
}

fn hex(s: &str) -> Option<[u8; 4]> {
    let h = s.strip_prefix('#')?;
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([v(0)?, v(2)?, v(4)?, 255])
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Reads a Lottie JSON animation; `images` decodes embedded pictures; `files` gives a
/// package's own image files by name (`.lottie`).
pub fn open_json(
    d: &[u8],
    images: Option<&ImageDecoder>,
    files: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Animation, Error> {
    if d.len() > MAX_JSON {
        return Err(Error::Unsupported(format!(
            "Lottie files over {} MB are not read",
            MAX_JSON >> 20
        )));
    }
    let v: Value =
        serde_json::from_slice(d).map_err(|e| Error::Malformed(format!("not valid JSON: {e}")))?;
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    let (Some(w), Some(h), Some(fr), Some(ip), Some(op)) =
        (num("w"), num("h"), num("fr"), num("ip"), num("op"))
    else {
        return Err(Error::Malformed(
            "no size, frame rate or in and out points".into(),
        ));
    };
    if !(w > 0.0 && h > 0.0 && w <= 16384.0 && h <= 16384.0) {
        return Err(Error::Malformed(format!("canvas {w}×{h}")));
    }
    if !(fr > 0.0 && fr <= 1000.0) || op <= ip {
        return Err(Error::Malformed("frame rate or in and out points".into()));
    }
    let mut notes = Vec::new();
    let mut precomps = HashMap::new();
    let mut pics = HashMap::new();
    let mut missing = 0;
    for a in v
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = a.get("id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(l) = a.get("layers") {
            precomps.insert(id.to_owned(), parse_layers(Some(l), &mut notes));
            continue;
        }
        let p = a.get("p").and_then(Value::as_str).unwrap_or("");
        let u = a.get("u").and_then(Value::as_str).unwrap_or("");
        let bytes = if let Some(rest) = p.strip_prefix("data:") {
            rest.split_once(',').and_then(|(_, b)| base64(b))
        } else {
            files(&format!("{u}{p}")).or_else(|| files(p))
        };
        match (bytes, images) {
            (Some(b), Some(dec)) => match specular_drawpage::Image::decode(dec, &b) {
                Some(img) => {
                    pics.insert(id.to_owned(), Arc::new(img));
                }
                None => missing += 1,
            },
            _ => missing += 1,
        }
    }
    if missing > 0 {
        notes.push(format!("{missing} pictures"));
    }
    let layers = parse_layers(v.get("layers"), &mut notes);
    if v.get("layers").and_then(Value::as_array).is_none() {
        return Err(Error::Malformed("no layers".into()));
    }
    Ok(Animation {
        width: w,
        height: h,
        frame_rate: fr,
        in_point: ip,
        out_point: op,
        name: v.get("nm").and_then(Value::as_str).unwrap_or("").to_owned(),
        version: v.get("v").and_then(Value::as_str).unwrap_or("").to_owned(),
        layers,
        precomps,
        images: pics,
        not_drawn: notes,
    })
}

/// Reads a `.lottie` package's first (or manifest-named) animation.
pub fn open_dotlottie(d: &[u8], images: Option<&ImageDecoder>) -> Result<Animation, Error> {
    use specular_ora::zip::Zip;
    let z = Zip::parse(d).map_err(|e| Error::Malformed(format!("ZIP: {e}")))?;
    let read = |name: &str| -> Option<Vec<u8>> {
        let e = z.find(name)?;
        z.read(d, e, MAX_JSON as u64).ok().map(|c| c.into_owned())
    };
    let manifest: Option<Value> =
        read("manifest.json").and_then(|m| serde_json::from_slice(&m).ok());
    let first_id = manifest
        .as_ref()
        .and_then(|m| m.pointer("/animations/0/id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let name = first_id
        .map(|id| format!("animations/{id}.json"))
        .filter(|n| z.find(n).is_some())
        .or_else(|| {
            z.entries
                .iter()
                .find(|e| e.name.starts_with("animations/") && e.name.ends_with(".json"))
                .map(|e| e.name.clone())
        })
        .ok_or_else(|| Error::Malformed("a .lottie without animations".into()))?;
    let json = read(&name).ok_or_else(|| Error::Malformed("animation unreadable".into()))?;
    let files = |p: &str| {
        let p = p.trim_start_matches('/');
        read(&format!("images/{}", p.rsplit('/').next().unwrap_or(p))).or_else(|| read(p))
    };
    open_json(&json, images, &files)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const SQUARE: &str = r#"{"v":"5.7.4","fr":30,"ip":0,"op":30,"w":100,"h":100,"nm":"sq","ddd":0,"assets":[],
      "layers":[{"ddd":0,"ind":1,"ty":4,"nm":"box","sr":1,"ks":{
        "o":{"a":0,"k":100},"r":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0],"y":[0]},"i":{"x":[1],"y":[1]}},{"t":30,"s":[90]}]},
        "p":{"a":0,"k":[50,50,0]},"a":{"a":0,"k":[0,0,0]},"s":{"a":0,"k":[100,100,100]}},
        "ao":0,"shapes":[{"ty":"gr","it":[
          {"ty":"rc","d":1,"s":{"a":0,"k":[40,40]},"p":{"a":0,"k":[0,0]},"r":{"a":0,"k":0}},
          {"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100},"r":1},
          {"ty":"tr","p":{"a":0,"k":[0,0]},"a":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}],
        "ip":0,"op":30,"st":0,"bm":0}]}"#;

    #[test]
    fn reads_a_square() {
        assert!(is_lottie_json(SQUARE.as_bytes()));
        let a = open_json(SQUARE.as_bytes(), None, &|_| None).unwrap();
        assert_eq!(a.frames(), 30);
        assert_eq!((a.width, a.height), (100.0, 100.0));
        let p = frame_page(&a, 0.0);
        assert!(!p.ops.is_empty());
        assert!(!is_lottie_json(b"{\"a\": 1}"));
    }
}
