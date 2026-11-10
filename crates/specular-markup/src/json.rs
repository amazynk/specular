//! The sidecar of an image or a drawing that is not a PDF: JSON in the image's pixels (y
//! down, pixel (i, j) covering i..i+1 × j..j+1).
//!
//! ```json
//! { "format": "specular-markup", "version": 1, "file": "plan.dxf",
//!   "space": "pixels", "width": 4096, "height": 2896,
//!   "pages": [ { "page": 0, "items": [
//!     { "id": "spec-…", "kind": "cloud", "points": [[10, 20], [200, 20], [200, 90]],
//!       "colour": "#E53935", "width": 3, "size": 24, "text": "",
//!       "author": "Marc", "date": "2026-10-24T14:03:00Z" },
//!     { "kind": "stamp", "points": [[400, 300]], "stamp": "approved", … },
//!     { "kind": "pin", "points": [[640, 480]], "photo": "/home/…/IMG_2041.jpg", … } ] } ] }
//! ```
//!
//! `width` and `height` are the image's size when it was marked up: if the file is shown at
//! another size later, the markup is scaled to it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::Value;

use crate::{Markup, P, Shape, Stamp, hex_colour, parse_colour};

fn n(v: f64) -> String {
    specular_pdf::annotate::num(v)
}

/// The JSON of `pages` for the file named `file`, `size` its pixel size.
pub fn write(file: &str, size: (u32, u32), pages: &[(u32, &[Markup])]) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\n  \"format\": \"specular-markup\",\n  \"version\": 1,\n  \"file\": {},\n  \
         \"space\": \"pixels\",\n  \"width\": {},\n  \"height\": {},\n  \"pages\": [",
        Value::String(file.to_owned()),
        size.0,
        size.1
    );
    for (pi, &(page, items)) in pages.iter().enumerate() {
        let _ = write!(
            s,
            "{}\n    {{ \"page\": {page}, \"items\": [",
            if pi > 0 { "," } else { "" }
        );
        for (i, it) in items.iter().enumerate() {
            let pts: Vec<String> = it
                .shape
                .points()
                .iter()
                .map(|p| format!("[{}, {}]", n(p.x), n(p.y)))
                .collect();
            let _ = write!(
                s,
                "{}\n      {{ \"id\": {}, \"kind\": \"{}\", \"points\": [{}], \"colour\": \"{}\", \
                 \"width\": {}, \"size\": {}, \"text\": {}, \"author\": {}, \"date\": {}",
                if i > 0 { "," } else { "" },
                Value::String(it.id.clone()),
                it.shape.kind(),
                pts.join(", "),
                hex_colour(it.colour),
                n(it.width),
                n(it.size),
                Value::String(it.text.clone()),
                Value::String(it.author.clone()),
                Value::String(it.date.clone()),
            );
            match &it.shape {
                Shape::Stamp { kind, .. } => {
                    let _ = write!(s, ", \"stamp\": \"{}\"", kind.id());
                }
                Shape::Pin { photo, .. } => {
                    let _ = write!(
                        s,
                        ", \"photo\": {}",
                        Value::String(photo.to_string_lossy().into_owned())
                    );
                }
                _ => {}
            }
            s.push_str(" }");
        }
        s.push_str(" ] }");
    }
    s.push_str("\n  ]\n}\n");
    s
}

/// The markup of a JSON sidecar, by page, scaled to an image of `size` (or as written when the
/// file does not say its size). Objects of unknown kinds are skipped.
pub fn read(text: &str, size: (u32, u32)) -> Result<BTreeMap<u32, Vec<Markup>>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if v.get("format").and_then(Value::as_str) != Some("specular-markup") {
        return Err("not a Specular markup file".into());
    }
    let dim = |k: &str| v.get(k).and_then(Value::as_f64).filter(|d| *d > 0.0);
    let (sx, sy) = match (dim("width"), dim("height")) {
        (Some(w), Some(h)) if size.0 > 0 && size.1 > 0 => {
            (f64::from(size.0) / w, f64::from(size.1) / h)
        }
        _ => (1.0, 1.0),
    };
    let k = (sx * sy).sqrt();
    let mut out: BTreeMap<u32, Vec<Markup>> = BTreeMap::new();
    for pg in v
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(page) = pg.get("page").and_then(Value::as_u64) else {
            continue;
        };
        let page = u32::try_from(page).unwrap_or(u32::MAX);
        for it in pg
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let s = |k: &str| it.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
            let f = |k: &str| {
                it.get(k)
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite() && *v >= 0.0)
            };
            let pts: Vec<P> = it
                .get("points")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let a = p.as_array()?;
                    let x = a.first()?.as_f64()?;
                    let y = a.get(1)?.as_f64()?;
                    (x.is_finite() && y.is_finite()).then_some(P::new(x * sx, y * sy))
                })
                .collect();
            let first = pts.first().copied();
            let two = (pts.len() == 2).then(|| (pts[0], pts[1]));
            let shape = match s("kind").as_str() {
                "cloud" if pts.len() >= 2 => Some(Shape::Cloud(pts)),
                "area" if pts.len() >= 3 => Some(Shape::Area(pts)),
                "text" => first.map(Shape::Text),
                "arrow" => two.map(|(tail, head)| Shape::Arrow { tail, head }),
                "length" => two.map(|(a, b)| Shape::Length(a, b)),
                "stamp" => first.map(|at| Shape::Stamp {
                    at,
                    kind: Stamp::from_id(&s("stamp")).unwrap_or(Stamp::Custom),
                }),
                "pin" if !s("photo").is_empty() => first.map(|at| Shape::Pin {
                    at,
                    photo: PathBuf::from(s("photo")),
                }),
                _ => None,
            };
            let Some(shape) = shape else { continue };
            let id = s("id");
            out.entry(page).or_default().push(Markup {
                id: if id.is_empty() { crate::new_id() } else { id },
                shape,
                colour: parse_colour(&s("colour")).unwrap_or([229, 57, 53]),
                width: f("width").unwrap_or(2.0) * k,
                size: f("size").filter(|v| *v > 0.0).unwrap_or(16.0) * k,
                text: s("text"),
                author: s("author"),
                date: s("date"),
            });
        }
    }
    Ok(out)
}
