//! Masks and blends in readable form (after the model's masks).
//!
//! - **Shapes:** `<spd:masks><rdf:Seq>`, one `rdf:li` per shape: `spd:id`, `spd:shape`
//!   (`ellipse`, `rectangle`, `polygon`, `linear`, `radial`, `brush`) and its points as
//!   numbers separated by spaces. Brush strokes are compact base64 (`spd:strokes`).
//! - **Blend:** a module item's `<spd:blend>` with `spd:mode`, `spd:opacity`, `spd:invert`
//!   (only when set), the refine fields and `spd:mask`, an expression in parentheses:
//!   `(all)`, `(shape 1)`, `(range luminance input 0.0 0.1 0.6 0.8)`,
//!   `(colours 0.1 0.05 r g b …)`, `(module exposure 1)`, `(ai "sky" <64 hex>)`,
//!   `(not E)`, `(union E E)` (also `intersect`, `subtract`, `exclude`), `(opacity 0.5 E)`,
//!   `(details 0.25 0.2)`. `spd:radius` only when not 0.02.

use crate::model::fields::Choice;
use crate::model::mask::{
    AI_MAX_POINTS, AiKind, AiMask, AiPoint, ColourRange, Combine, DepthRange, Details, PathPoint,
    REFINE_RADIUS, Range, RangeChannel, Stroke,
};
use crate::model::{
    Blend, BlendMode, Edit, MaskExpr, MaskShape, ModuleKind, Refine, Registry, Shape,
};

use super::{Error, NS, b64, bad, esc, float, module_kind, module_name};

fn floats(v: &[f32]) -> String {
    v.iter().map(|f| float(*f)).collect::<Vec<_>>().join(" ")
}

fn parse_floats(s: &str) -> Result<Vec<f32>, Error> {
    s.split_ascii_whitespace()
        .map(|t| t.parse::<f32>().map_err(|_| bad("a mask number")))
        .collect()
}

/// The shapes as a `spd:masks` element, indented by `ind`.
pub(super) fn masks_xml(o: &mut String, masks: &[MaskShape], ind: usize) {
    let pad = " ".repeat(ind);
    o.push_str(&format!("{pad}<spd:masks>\n{pad} <rdf:Seq>\n"));
    for m in masks {
        o.push_str(&format!("{pad}  <rdf:li spd:id=\"{}\"", m.id));
        let a = |o: &mut String, n: &str, v: String| o.push_str(&format!(" spd:{n}=\"{v}\""));
        match &m.shape {
            Shape::Ellipse {
                centre,
                radii,
                angle_deg,
                feather,
            } => {
                a(o, "shape", "ellipse".into());
                a(o, "centre", floats(centre));
                a(o, "radii", floats(radii));
                a(o, "angle", float(*angle_deg));
                a(o, "feather", float(*feather));
            }
            Shape::Rectangle {
                centre,
                half,
                angle_deg,
                round,
                feather,
            } => {
                a(o, "shape", "rectangle".into());
                a(o, "centre", floats(centre));
                a(o, "half", floats(half));
                a(o, "angle", float(*angle_deg));
                a(o, "round", float(*round));
                a(o, "feather", float(*feather));
            }
            Shape::Polygon { points, closed } => {
                a(o, "shape", "polygon".into());
                let v: Vec<f32> = points.iter().flat_map(|p| [p.x, p.y, p.feather]).collect();
                a(o, "points", floats(&v));
                a(o, "closed", if *closed { "True" } else { "False" }.into());
            }
            Shape::Linear { start, end } => {
                a(o, "shape", "linear".into());
                a(o, "start", floats(start));
                a(o, "end", floats(end));
            }
            Shape::Radial {
                centre,
                inner,
                outer,
                angle_deg,
            } => {
                a(o, "shape", "radial".into());
                a(o, "centre", floats(centre));
                a(o, "inner", floats(inner));
                a(o, "outer", floats(outer));
                a(o, "angle", float(*angle_deg));
            }
            Shape::Brush { .. } => {
                a(o, "shape", "brush".into());
                let mut e = Edit::empty(Default::default());
                e.masks.push(m.clone());
                a(
                    o,
                    "strokes",
                    b64::encode(&crate::model::codec::encode_edit(&e)),
                );
            }
        }
        o.push_str("/>\n");
    }
    o.push_str(&format!("{pad} </rdf:Seq>\n{pad}</spd:masks>\n"));
}

/// Reads a `spd:masks` element. A shape this build cannot read is left out and `partial`
/// set (the file is then never overwritten, see [`super::write_version`]).
pub(super) fn read_masks(n: roxmltree::Node, reg: &Registry, partial: &mut bool) -> Vec<MaskShape> {
    let mut out = Vec::new();
    for li in super::module_items(n) {
        match read_shape(li, reg) {
            Ok(m) => out.push(m),
            Err(_) => *partial = true,
        }
    }
    out
}

fn read_shape(li: roxmltree::Node, reg: &Registry) -> Result<MaskShape, Error> {
    {
        let get = |k: &str| {
            li.attribute((NS, k))
                .ok_or_else(|| bad("a mask without its points"))
        };
        let pt = |k: &str| -> Result<[f32; 2], Error> {
            let v = parse_floats(get(k)?)?;
            v.try_into().map_err(|_| bad("a mask point"))
        };
        let num =
            |k: &str| -> Result<f32, Error> { get(k)?.parse().map_err(|_| bad("a mask number")) };
        let id: u32 = get("id")?.parse().map_err(|_| bad("a mask id"))?;
        let shape = match get("shape")? {
            "ellipse" => Shape::Ellipse {
                centre: pt("centre")?,
                radii: pt("radii")?,
                angle_deg: num("angle")?,
                feather: num("feather")?,
            },
            "rectangle" => Shape::Rectangle {
                centre: pt("centre")?,
                half: pt("half")?,
                angle_deg: num("angle")?,
                round: num("round")?,
                feather: num("feather")?,
            },
            "polygon" => {
                let v = parse_floats(get("points")?)?;
                if v.len() % 3 != 0 {
                    return Err(bad("a polygon's points"));
                }
                Shape::Polygon {
                    points: v
                        .chunks_exact(3)
                        .map(|c| PathPoint {
                            x: c[0],
                            y: c[1],
                            feather: c[2],
                        })
                        .collect(),
                    closed: get("closed")? == "True",
                }
            }
            "linear" => Shape::Linear {
                start: pt("start")?,
                end: pt("end")?,
            },
            "radial" => Shape::Radial {
                centre: pt("centre")?,
                inner: pt("inner")?,
                outer: pt("outer")?,
                angle_deg: num("angle")?,
            },
            "brush" => {
                let b = b64::decode(get("strokes")?).ok_or_else(|| bad("bad base64"))?;
                let (e, _) = crate::model::codec::decode_edit(&b, reg)?;
                match e.masks.into_iter().next() {
                    Some(MaskShape {
                        shape: s @ Shape::Brush { .. },
                        ..
                    }) => s,
                    _ => Shape::Brush {
                        strokes: Vec::<Stroke>::new(),
                    },
                }
            }
            _ => return Err(bad("a mask shape this build does not know")),
        };
        Ok(MaskShape { id, shape })
    }
}

/// A blend as a `spd:blend` element.
pub(super) fn blend_xml(o: &mut String, b: &Blend, reg: &Registry, pad: &str) {
    o.push_str(&format!(
        "{pad}<spd:blend spd:mode=\"{}\" spd:opacity=\"{}\"",
        b.mode.name(),
        float(b.opacity)
    ));
    if b.invert {
        o.push_str(" spd:invert=\"True\"");
    }
    let r = &b.refine;
    for (n, v) in [
        ("feather", r.feather),
        ("blur", r.blur),
        ("contrast", r.contrast),
        ("offset", r.offset),
        ("edgeAware", r.edge_aware),
        ("details", r.details),
    ] {
        o.push_str(&format!(" spd:{n}=\"{}\"", float(v)));
    }
    // Only when not the original fixed 2 %.
    if r.radius != REFINE_RADIUS {
        o.push_str(&format!(" spd:radius=\"{}\"", float(r.radius)));
    }
    o.push_str(&format!(
        "\n{pad}  spd:mask=\"{}\"/>\n",
        esc(&expr_text(&b.mask, reg))
    ));
}

/// Reads a `spd:blend` element.
pub(super) fn read_blend(n: roxmltree::Node, reg: &Registry) -> Result<Blend, Error> {
    let f = |k: &str, d: f32| -> Result<f32, Error> {
        match n.attribute((NS, k)) {
            Some(s) => s.parse().map_err(|_| bad("a blend number")),
            None => Ok(d),
        }
    };
    let mode = n
        .attribute((NS, "mode"))
        .map_or(Some(BlendMode::Normal), BlendMode::from_name)
        .ok_or_else(|| bad("a blend mode this build does not know"))?;
    Ok(Blend {
        mode,
        opacity: f("opacity", 1.0)?,
        invert: n.attribute((NS, "invert")) == Some("True"),
        refine: Refine {
            feather: f("feather", 0.0)?,
            blur: f("blur", 0.0)?,
            contrast: f("contrast", 0.0)?,
            offset: f("offset", 0.0)?,
            edge_aware: f("edgeAware", 0.0)?,
            details: f("details", 0.0)?,
            radius: f("radius", REFINE_RADIUS)?,
        },
        mask: match n.attribute((NS, "mask")) {
            Some(s) => parse_expr(s, reg)?,
            None => MaskExpr::All,
        },
    })
}

/// A mask expression as text.
pub fn expr_text(e: &MaskExpr, reg: &Registry) -> String {
    match e {
        MaskExpr::All => "(all)".into(),
        MaskExpr::Shape(id) => format!("(shape {id})"),
        MaskExpr::Range(r) => format!(
            "(range {} {} {})",
            r.channel.name(),
            if r.on_output { "output" } else { "input" },
            floats(&r.limits)
        ),
        MaskExpr::ColourRange(c) => {
            let mut v = vec![c.distance, c.falloff];
            v.extend(c.colours.iter().flatten());
            format!("(colours {})", floats(&v))
        }
        MaskExpr::FromModule { kind, instance } => {
            format!("(module {} {instance})", module_name(reg, *kind))
        }
        MaskExpr::Ai(m) => {
            let mut s = format!(
                "(ai {} \"{}\"",
                m.kind.name(),
                m.model.replace('\\', "\\\\").replace('"', "\\\"")
            );
            if let Some(r) = &m.rect {
                s += &format!(" box {}", floats(r));
            }
            for p in &m.points {
                let sign = if p.positive { "+" } else { "-" };
                s += &format!(" {sign} {}", floats(&[p.x, p.y]));
            }
            s + ")"
        }
        MaskExpr::Invert(a) => format!("(not {})", expr_text(a, reg)),
        MaskExpr::Combine(op, a, b) => {
            format!(
                "({} {} {})",
                op.name(),
                expr_text(a, reg),
                expr_text(b, reg)
            )
        }
        MaskExpr::Opacity(k, a) => format!("(opacity {} {})", float(*k), expr_text(a, reg)),
        MaskExpr::Details(d) => format!("(details {})", floats(&[d.threshold, d.softness])),
        MaskExpr::Depth(d) => format!(
            "(depth \"{}\" {})",
            d.model.replace('\\', "\\\\").replace('"', "\\\""),
            floats(&[d.near, d.far, d.feather])
        ),
    }
}

#[derive(Debug, PartialEq)]
enum Tok {
    Open,
    Close,
    Atom(String),
    Quoted(String),
}

fn tokens(s: &str) -> Result<Vec<Tok>, Error> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '(' => out.push(Tok::Open),
            ')' => out.push(Tok::Close),
            c if c.is_whitespace() => {}
            '"' => {
                let mut q = String::new();
                loop {
                    match it.next() {
                        Some('\\') => q.push(it.next().ok_or_else(|| bad("a mask text"))?),
                        Some('"') => break,
                        Some(c) => q.push(c),
                        None => return Err(bad("a mask text")),
                    }
                }
                out.push(Tok::Quoted(q));
            }
            c => {
                let mut a = String::from(c);
                while let Some(&n) = it.peek() {
                    if n == '(' || n == ')' || n.is_whitespace() {
                        break;
                    }
                    a.push(n);
                    it.next();
                }
                out.push(Tok::Atom(a));
            }
        }
    }
    Ok(out)
}

/// Reads a mask expression's text.
pub fn parse_expr(s: &str, reg: &Registry) -> Result<MaskExpr, Error> {
    let t = tokens(s)?;
    let mut at = 0;
    let e = expr(&t, &mut at, reg)?;
    if at != t.len() {
        return Err(bad("text after a mask"));
    }
    Ok(e)
}

fn expr(t: &[Tok], at: &mut usize, reg: &Registry) -> Result<MaskExpr, Error> {
    let next = |at: &mut usize| -> Result<&Tok, Error> {
        let x = t.get(*at).ok_or_else(|| bad("a mask ends early"))?;
        *at += 1;
        Ok(x)
    };
    let atom = |at: &mut usize| -> Result<String, Error> {
        match next(at)? {
            Tok::Atom(a) => Ok(a.clone()),
            _ => Err(bad("a mask word")),
        }
    };
    if next(at)? != &Tok::Open {
        return Err(bad("a mask starts with ("));
    }
    let head = atom(at)?;
    // Numbers up to the closing parenthesis.
    let rest_floats = |at: &mut usize| -> Result<Vec<f32>, Error> {
        let mut v = Vec::new();
        while let Some(Tok::Atom(a)) = t.get(*at) {
            v.push(a.parse().map_err(|_| bad("a mask number"))?);
            *at += 1;
        }
        Ok(v)
    };
    let e = match head.as_str() {
        "all" => MaskExpr::All,
        "shape" => MaskExpr::Shape(atom(at)?.parse().map_err(|_| bad("a mask id"))?),
        "range" => {
            let channel =
                RangeChannel::from_name(&atom(at)?).ok_or_else(|| bad("a range channel"))?;
            let on_output = match atom(at)?.as_str() {
                "output" => true,
                "input" => false,
                _ => return Err(bad("a range's side")),
            };
            let limits: [f32; 4] = rest_floats(at)?
                .try_into()
                .map_err(|_| bad("a range's limits"))?;
            MaskExpr::Range(Range {
                channel,
                on_output,
                limits,
            })
        }
        "colours" => {
            let v = rest_floats(at)?;
            if v.len() < 2 || (v.len() - 2) % 3 != 0 {
                return Err(bad("a colour range"));
            }
            MaskExpr::ColourRange(ColourRange {
                distance: v[0],
                falloff: v[1],
                colours: v[2..].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            })
        }
        "module" => {
            let kind: ModuleKind =
                module_kind(reg, &atom(at)?).ok_or_else(|| bad("a mask's module"))?;
            let instance = atom(at)?.parse().map_err(|_| bad("a mask's instance"))?;
            MaskExpr::FromModule { kind, instance }
        }
        "ai" => match next(at)? {
            // The older placeholder form `(ai "LABEL" KEY)`: the label names the kind.
            Tok::Quoted(label) => {
                let h = atom(at)?;
                if h.len() != 64 || u128::from_str_radix(&h[..32], 16).is_err() {
                    return Err(bad("an AI mask's key"));
                }
                MaskExpr::Ai(AiMask::new(
                    AiKind::from_name(label).unwrap_or_default(),
                    "",
                ))
            }
            Tok::Atom(kind) => {
                let kind = AiKind::from_name(kind).ok_or_else(|| bad("an AI mask's kind"))?;
                let model = match next(at)? {
                    Tok::Quoted(q) => q.clone(),
                    _ => return Err(bad("an AI mask's model")),
                };
                let mut m = AiMask::new(kind, &model);
                let num = |at: &mut usize| -> Result<f32, Error> {
                    atom(at)?.parse().map_err(|_| bad("a mask number"))
                };
                while let Some(Tok::Atom(w)) = t.get(*at) {
                    *at += 1;
                    match w.as_str() {
                        "box" => m.rect = Some([num(at)?, num(at)?, num(at)?, num(at)?]),
                        "+" | "-" => m.points.push(AiPoint {
                            x: num(at)?,
                            y: num(at)?,
                            positive: w == "+",
                        }),
                        _ => return Err(bad("an AI mask's clicks")),
                    }
                }
                if m.prompt_len() > AI_MAX_POINTS {
                    return Err(bad("an AI mask's clicks"));
                }
                MaskExpr::Ai(m)
            }
            _ => return Err(bad("an AI mask's kind")),
        },
        "details" => {
            let [threshold, softness]: [f32; 2] = rest_floats(at)?
                .try_into()
                .map_err(|_| bad("a details mask"))?;
            MaskExpr::Details(Details {
                threshold,
                softness,
            })
        }
        "depth" => {
            let model = match next(at)? {
                Tok::Quoted(q) => q.clone(),
                _ => return Err(bad("a depth mask's model")),
            };
            let [near, far, feather]: [f32; 3] = rest_floats(at)?
                .try_into()
                .map_err(|_| bad("a depth mask"))?;
            MaskExpr::Depth(DepthRange {
                model,
                near,
                far,
                feather,
            })
        }
        "not" => MaskExpr::Invert(Box::new(expr(t, at, reg)?)),
        "opacity" => {
            let k = atom(at)?.parse().map_err(|_| bad("a mask number"))?;
            MaskExpr::Opacity(k, Box::new(expr(t, at, reg)?))
        }
        op => {
            let op =
                Combine::from_name(op).ok_or_else(|| bad("a mask this build does not know"))?;
            let a = expr(t, at, reg)?;
            let b = expr(t, at, reg)?;
            MaskExpr::Combine(op, Box::new(a), Box::new(b))
        }
    };
    if next(at)? != &Tok::Close {
        return Err(bad("a mask ends with )"));
    }
    Ok(e)
}
