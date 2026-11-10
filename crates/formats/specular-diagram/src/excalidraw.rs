//! Excalidraw scenes (`.excalidraw`): JSON with `"type": "excalidraw"`, its `elements`
//! (deleted ones skipped), `appState.viewBackgroundColor` and `files` (images as data
//! URLs). Elements are drawn cleanly: hachure and cross-hatch fills as a light solid fill,
//! no hand-drawn wobble.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use specular_drawpage::kurbo::{Ellipse, RoundedRect};
use specular_drawpage::{
    Affine, BezPath, Cap, HAlign, Image, ImageDecoder, Join, Op, Paint, Point, Rect, Shape, Stroke,
    VAlign,
};

use crate::{Diagram, EMPTY_BOUNDS, Error, MAX_ELEMENTS, colour, faded, grow, page, text_block};

/// Whether `d` is an Excalidraw scene: JSON naming `"type": "excalidraw"` near its start.
pub fn is_excalidraw(d: &[u8]) -> bool {
    let h = String::from_utf8_lossy(&d[..d.len().min(1024)]);
    let t = h.trim_start_matches('\u{feff}').trim_start();
    t.starts_with('{')
        && t.split_once("\"type\"")
            .map(|(_, r)| r.trim_start().trim_start_matches(':').trim_start())
            .is_some_and(|r| r.starts_with("\"excalidraw\""))
}

fn f(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(d)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The font for Excalidraw's family numbers, drawn cleanly (no hand lettering).
fn family(n: i64) -> &'static str {
    match n {
        3 | 8 => "monospace",
        6 => "Nunito",
        7 => "Lilita One",
        _ => "Helvetica",
    }
}

/// Reads an Excalidraw scene as one page.
pub fn open_excalidraw(d: &[u8], images: Option<ImageDecoder>) -> Result<Diagram, Error> {
    let v: Value =
        serde_json::from_slice(d).map_err(|e| Error::Malformed(format!("not valid JSON: {e}")))?;
    if s(&v, "type") != "excalidraw" {
        return Err(Error::Malformed("not an Excalidraw scene".into()));
    }
    let background = v
        .pointer("/appState/viewBackgroundColor")
        .and_then(Value::as_str)
        .and_then(colour)
        .unwrap_or([255, 255, 255, 255]);
    let files: HashMap<&str, &str> = v
        .get("files")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, f)| Some((k.as_str(), f.get("dataURL")?.as_str()?)))
                .collect()
        })
        .unwrap_or_default();
    let mut out = Diagram {
        pages: Vec::new(),
        shapes: 0,
        connectors: 0,
        missing_pictures: 0,
        approximated: 0,
    };
    let mut ops = Vec::new();
    let mut bounds = EMPTY_BOUNDS;
    let elements = v
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for e in elements.iter().take(MAX_ELEMENTS) {
        if e.get("isDeleted").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let (x, y, w, h) = (
            f(e, "x", 0.0),
            f(e, "y", 0.0),
            f(e, "width", 0.0),
            f(e, "height", 0.0),
        );
        let angle = f(e, "angle", 0.0);
        let rot = Affine::rotate_about(angle, (x + w / 2.0, y + h / 2.0));
        let opacity = f(e, "opacity", 100.0) / 100.0;
        let stroke_c = colour(s(e, "strokeColor")).map(|c| faded(c, opacity));
        let fill_c = colour(s(e, "backgroundColor")).map(|c| {
            // Hachure and cross-hatch drawn as a lighter solid.
            let light = if s(e, "fillStyle") == "solid" {
                1.0
            } else {
                0.45
            };
            faded(c, opacity * light)
        });
        let width = f(e, "strokeWidth", 1.0);
        let mut stroke = Stroke::new(width)
            .with_caps(Cap::Round)
            .with_join(Join::Round);
        match s(e, "strokeStyle") {
            "dashed" => {
                stroke = stroke.with_dashes(0.0, [8.0 * width.max(1.0), 6.0 * width.max(1.0)])
            }
            "dotted" => stroke = stroke.with_dashes(0.0, [0.1, 4.0 * width.max(1.0)]),
            _ => {}
        }
        let rounded = !e.get("roundness").is_none_or(Value::is_null);
        let kind = s(e, "type");
        let grow_box = |b: &mut (f64, f64, f64, f64)| {
            for (px, py) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                let p = rot * Point::new(px, py);
                grow(b, p.x, p.y);
            }
        };
        match kind {
            "rectangle" | "ellipse" | "diamond" | "frame" | "magicframe" | "embeddable"
            | "iframe" => {
                out.shapes += 1;
                grow_box(&mut bounds);
                let r = Rect::new(x, y, x + w, y + h);
                let path = match kind {
                    "ellipse" => Ellipse::from_rect(r).to_path(0.1),
                    "diamond" => {
                        let mut p = BezPath::new();
                        p.move_to((x + w / 2.0, y));
                        p.line_to((x + w, y + h / 2.0));
                        p.line_to((x + w / 2.0, y + h));
                        p.line_to((x, y + h / 2.0));
                        p.close_path();
                        p
                    }
                    _ if rounded => {
                        RoundedRect::from_rect(r, (w.min(h) * 0.25).min(32.0)).to_path(0.1)
                    }
                    _ => r.to_path(0.1),
                };
                let path = rot * path;
                if let Some(c) = fill_c {
                    ops.push(Op::Fill {
                        path: path.clone(),
                        even_odd: false,
                        paint: Paint::Solid(c),
                    });
                }
                let frame = kind.contains("frame");
                let sc = if frame {
                    Some([187, 187, 187, 255])
                } else {
                    stroke_c
                };
                if let Some(c) = sc {
                    ops.push(Op::Stroke {
                        path,
                        stroke: stroke.clone(),
                        paint: Paint::Solid(c),
                    });
                }
                if frame {
                    let name = e.get("name").and_then(Value::as_str).unwrap_or("Frame");
                    if let Some(t) = text_block(
                        name,
                        (x, y - 20.0, w.max(80.0), 18.0),
                        "Helvetica",
                        12.0,
                        [120, 120, 120, 255],
                        false,
                        false,
                        HAlign::Left,
                        VAlign::Bottom,
                        0.0,
                    ) {
                        ops.push(t);
                    }
                }
            }
            "line" | "arrow" | "freedraw" => {
                out.connectors += usize::from(kind == "arrow");
                out.shapes += usize::from(kind != "arrow");
                let pts: Vec<Point> = e
                    .get("points")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|p| {
                        let a = p.as_array()?;
                        Some(rot * Point::new(x + a.first()?.as_f64()?, y + a.get(1)?.as_f64()?))
                    })
                    .collect();
                if pts.len() < 2 {
                    continue;
                }
                let mut p = BezPath::new();
                p.move_to(pts[0]);
                if kind == "freedraw" || (rounded && pts.len() > 2) {
                    // Through the points, smoothly (midpoint quadratics).
                    for i in 1..pts.len() - 1 {
                        p.quad_to(pts[i], pts[i].midpoint(pts[i + 1]));
                    }
                    p.line_to(pts[pts.len() - 1]);
                } else {
                    for q in &pts[1..] {
                        p.line_to(*q);
                    }
                }
                for q in &pts {
                    grow(&mut bounds, q.x, q.y);
                }
                let closed =
                    kind == "line" && pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 1.0;
                if closed && let Some(c) = fill_c {
                    let mut fp = p.clone();
                    fp.close_path();
                    ops.push(Op::Fill {
                        path: fp,
                        even_odd: false,
                        paint: Paint::Solid(c),
                    });
                }
                let Some(c) = stroke_c else { continue };
                let paint = Paint::Solid(c);
                let st = if kind == "freedraw" {
                    Stroke::new(width * 1.5)
                        .with_caps(Cap::Round)
                        .with_join(Join::Round)
                } else {
                    stroke.clone()
                };
                ops.push(Op::Stroke {
                    path: p,
                    stroke: st.clone(),
                    paint: paint.clone(),
                });
                let size = 10.0 + width * 3.0;
                let n = pts.len();
                let head = |k: &str| match k {
                    "arrow" => "open",
                    "triangle" => "classic",
                    "triangle_outline" => "open",
                    "dot" | "circle" => "dot",
                    "circle_outline" => "circle_outline",
                    "bar" => "bar",
                    "diamond" | "diamond_outline" => "diamond",
                    _ => "none",
                };
                if let Some(k) = e.get("endArrowhead").and_then(Value::as_str) {
                    crate::arrowhead(&mut ops, pts[n - 1], pts[n - 2], head(k), size, &paint, &st);
                }
                if let Some(k) = e.get("startArrowhead").and_then(Value::as_str) {
                    crate::arrowhead(&mut ops, pts[0], pts[1], head(k), size, &paint, &st);
                }
            }
            "text" => {
                grow_box(&mut bounds);
                let align = match s(e, "textAlign") {
                    "center" => HAlign::Centre,
                    "right" => HAlign::Right,
                    _ => HAlign::Left,
                };
                let valign = match s(e, "verticalAlign") {
                    "middle" => VAlign::Middle,
                    "bottom" => VAlign::Bottom,
                    _ => VAlign::Top,
                };
                let text = e
                    .get("text")
                    .or_else(|| e.get("originalText"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let size = f(e, "fontSize", 20.0);
                let fam = family(e.get("fontFamily").and_then(Value::as_i64).unwrap_or(1));
                if let Some(t) = text_block(
                    text,
                    (x, y, w.max(size), h.max(size)),
                    fam,
                    size,
                    stroke_c.unwrap_or([30, 30, 30, 255]),
                    false,
                    false,
                    align,
                    valign,
                    angle.to_degrees(),
                ) {
                    ops.push(t);
                }
            }
            "image" => {
                out.shapes += 1;
                grow_box(&mut bounds);
                let url = files.get(s(e, "fileId")).copied().unwrap_or("");
                match (crate::data_url(url), images.as_ref()) {
                    (Some(b), Some(dec)) => match Image::decode(dec, &b) {
                        Some(img) => {
                            let (iw, ih) = (f64::from(img.width), f64::from(img.height));
                            ops.push(Op::Image {
                                image: Arc::new(img),
                                transform: rot
                                    * Affine::translate((x, y))
                                    * Affine::scale_non_uniform(w / iw, h / ih),
                                opacity: opacity as f32,
                            });
                        }
                        None => out.missing_pictures += 1,
                    },
                    _ => out.missing_pictures += 1,
                }
            }
            _ => {}
        }
    }
    out.pages
        .push(page("Scene".into(), bounds, ops, background));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene() {
        let j = br##"{"type":"excalidraw","version":2,"elements":[
          {"type":"rectangle","x":0,"y":0,"width":100,"height":50,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","roughness":1,"opacity":100,"roundness":{"type":3}},
          {"type":"arrow","x":100,"y":25,"width":100,"height":0,"points":[[0,0],[100,0]],"strokeColor":"#1e1e1e","backgroundColor":"transparent","strokeWidth":2,"opacity":100,"endArrowhead":"arrow"},
          {"type":"text","x":10,"y":10,"width":80,"height":25,"text":"Hello","fontSize":20,"fontFamily":1,"textAlign":"center","verticalAlign":"middle","strokeColor":"#1e1e1e","opacity":100},
          {"type":"ellipse","x":0,"y":0,"width":5,"height":5,"isDeleted":true}],
          "appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;
        assert!(is_excalidraw(j));
        let d = open_excalidraw(j, None).unwrap();
        assert_eq!((d.shapes, d.connectors), (1, 1));
        let p = &d.pages[0];
        assert!((p.size.0 - 240.0).abs() < 1e-6, "{:?}", p.size);
        assert!(p.ops.iter().any(|o| matches!(o, Op::Text(_))));
        assert!(!is_excalidraw(b"{\"type\": \"other\"}"));
    }
}
