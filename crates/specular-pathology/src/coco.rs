//! COCO instances JSON: one image (the slide), a category per class and an annotation per
//! region or point, in level-0 pixel coordinates.
//!
//! - A polygon or rectangle is a `segmentation` of polygons (`[x1, y1, x2, y2, …]`, one per
//!   part) with its `bbox` (`[x, y, width, height]`) and geometric `area`. A polygon with
//!   holes, which COCO polygons cannot express, is an uncompressed RLE mask instead (column-
//!   major counts over the whole image, pixels whose centres are inside, as [`crate::mask`]
//!   paints them).
//! - A point is a `keypoints` triple (`[x, y, 2]`) with an empty `segmentation` and a
//!   zero-size `bbox`; several points are one annotation with several keypoints.
//! - Lines have no COCO form and are left out (counted).
//!
//! `attributes.name` keeps an annotation's own name and `categories[].color` the class
//! colour (extensions COCO readers ignore); `specular` on an annotation keeps the exact
//! shape, so Specular reads its own files back without loss. The reader also takes COCO
//! files from other tools: polygons, keypoints and boxes (an RLE mask from another tool
//! comes back as its box, counted).

use serde_json::Value;

use crate::annot::{Annotation, Class, Geometry, Poly, Pt, class_index, classes, open_ring};
use crate::json::{self, num, round};
use crate::mask;

/// The slide the annotations are on.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub file_name: String,
    /// Level-0 size.
    pub width: u32,
    pub height: u32,
}

/// What [`write`] left out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skipped {
    pub lines: usize,
    pub invalid: usize,
}

fn flat(ring: &[Pt]) -> String {
    let mut s = String::from("[");
    for (i, p) in ring.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&num(round(p.0, 3)));
        s.push(',');
        s.push_str(&num(round(p.1, 3)));
    }
    s.push(']');
    s
}

/// Column-major RLE counts (COCO's uncompressed form) of the pixels whose centres are in
/// `polys` on a `w` × `h` image.
pub fn rle(polys: &[Poly], w: u32, h: u32) -> Vec<u64> {
    // Column-major is row-major over the transposed image: columns become rows.
    let swap = |r: &Vec<Pt>| r.iter().map(|&(x, y)| (y, x)).collect::<Vec<Pt>>();
    let t: Vec<Poly> = polys
        .iter()
        .map(|p| Poly {
            exterior: swap(&p.exterior),
            holes: p.holes.iter().map(swap).collect(),
        })
        .collect();
    // Runs per transposed row (image column), merged across parts.
    let mut rows: std::collections::BTreeMap<u32, Vec<(u32, u32)>> = Default::default();
    for p in &t {
        mask::spans(p, (0.0, 0.0), 1.0, h, w, |j, a, b| {
            rows.entry(j).or_default().push((a, b))
        });
    }
    let mut counts = Vec::new();
    let mut pos = 0u64; // pixels accounted for, column-major
    let mut zeros = 0u64;
    for (col, mut runs) in rows {
        runs.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::new();
        for (a, b) in runs {
            match merged.last_mut() {
                Some(l) if a <= l.1 => l.1 = l.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        let base = u64::from(col) * u64::from(h);
        for (a, b) in merged {
            let start = base + u64::from(a);
            zeros += start - pos;
            if counts.is_empty() || zeros > 0 {
                counts.push(zeros);
                counts.push(u64::from(b - a));
            } else if let Some(l) = counts.last_mut() {
                // Touching the previous run (the end of one column, the top of the next).
                *l += u64::from(b - a);
            }
            zeros = 0;
            pos = base + u64::from(b);
        }
    }
    let total = u64::from(w) * u64::from(h);
    let tail = total - pos;
    if counts.is_empty() {
        counts.push(total);
    } else if tail > 0 {
        counts.push(tail);
    }
    counts
}

fn exact(g: &Geometry) -> String {
    let pts = |v: &[Pt]| {
        let mut s = String::from("[");
        for (i, p) in v.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{}]", num(p.0), num(p.1)));
        }
        s.push(']');
        s
    };
    match g {
        Geometry::Area(ps) => {
            let parts: Vec<String> = ps
                .iter()
                .map(|p| {
                    let holes: Vec<String> = p.holes.iter().map(|h| pts(h)).collect();
                    format!(
                        "{{\"exterior\":{},\"holes\":[{}]}}",
                        pts(&p.exterior),
                        holes.join(",")
                    )
                })
                .collect();
            format!("{{\"type\":\"polygon\",\"parts\":[{}]}}", parts.join(","))
        }
        &Geometry::Rect { x0, y0, x1, y1 } => format!(
            "{{\"type\":\"rectangle\",\"box\":[{},{},{},{}]}}",
            num(x0),
            num(y0),
            num(x1),
            num(y1)
        ),
        Geometry::Point(p) => format!("{{\"type\":\"point\",\"points\":{}}}", pts(&[*p])),
        Geometry::Points(v) => format!("{{\"type\":\"points\",\"points\":{}}}", pts(v)),
        Geometry::Line(v) => format!("{{\"type\":\"line\",\"points\":{}}}", pts(v)),
    }
}

/// The COCO file for `items` on `image`; `software` and `created` (RFC 3339) go in `info`.
pub fn write(
    items: &[Annotation],
    image: &Image,
    software: &str,
    created: &str,
) -> (String, Skipped) {
    let mut skipped = Skipped::default();
    let cls = classes(items);
    let mut s = String::new();
    s.push_str("{\"info\":{");
    s.push_str(&format!(
        "\"description\":{},\"version\":\"1.0\",\"contributor\":{},\"date_created\":{}",
        json::string(&format!("Annotations of {}", image.file_name)),
        json::string(software),
        json::string(created)
    ));
    if let Some(y) = created.get(..4).and_then(|y| y.parse::<u32>().ok()) {
        s.push_str(&format!(",\"year\":{y}"));
    }
    s.push_str("},\"licenses\":[],\"images\":[");
    s.push_str(&format!(
        "{{\"id\":1,\"file_name\":{},\"width\":{},\"height\":{}}}",
        json::string(&image.file_name),
        image.width,
        image.height
    ));
    s.push_str("],\"categories\":[");
    for (i, c) in cls.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"id\":{},\"name\":{},\"supercategory\":\"annotation\"",
            i + 1,
            json::string(&c.name)
        ));
        if let Some(col) = c.colour {
            s.push_str(&format!(",\"color\":{}", json::rgb(col)));
        }
        s.push('}');
    }
    s.push_str("],\"annotations\":[");
    let mut id = 0;
    for a in items {
        let g = &a.geometry;
        if matches!(g, Geometry::Line(_)) {
            skipped.lines += 1;
            continue;
        }
        if !g.is_valid() {
            skipped.invalid += 1;
            continue;
        }
        id += 1;
        if id > 1 {
            s.push(',');
        }
        let cat = class_index(&cls, a) + 1;
        let (x0, y0, x1, y1) = g.bounds();
        let bbox = format!(
            "[{},{},{},{}]",
            num(round(x0, 3)),
            num(round(y0, 3)),
            num(round(x1 - x0, 3)),
            num(round(y1 - y0, 3))
        );
        let (seg, extra) = match g {
            Geometry::Point(_) | Geometry::Points(_) => {
                let pts = g.points();
                let kp: Vec<String> = pts
                    .iter()
                    .map(|p| format!("{},{},2", num(round(p.0, 3)), num(round(p.1, 3))))
                    .collect();
                (
                    "[]".to_owned(),
                    format!(
                        ",\"keypoints\":[{}],\"num_keypoints\":{}",
                        kp.join(","),
                        pts.len()
                    ),
                )
            }
            _ => {
                let polys = g.polys();
                if polys.iter().any(|p| !p.holes.is_empty()) {
                    let counts: Vec<String> = rle(&polys, image.width, image.height)
                        .iter()
                        .map(u64::to_string)
                        .collect();
                    (
                        format!(
                            "{{\"size\":[{},{}],\"counts\":[{}]}}",
                            image.height,
                            image.width,
                            counts.join(",")
                        ),
                        String::new(),
                    )
                } else {
                    let rings: Vec<String> = polys.iter().map(|p| flat(&p.exterior)).collect();
                    (format!("[{}]", rings.join(",")), String::new())
                }
            }
        };
        s.push_str(&format!(
            "{{\"id\":{id},\"image_id\":1,\"category_id\":{cat},\"segmentation\":{seg},\"area\":{},\"bbox\":{bbox},\"iscrowd\":0{extra}",
            num(round(g.area(), 3))
        ));
        if !a.name.is_empty() {
            s.push_str(&format!(
                ",\"attributes\":{{\"name\":{}}}",
                json::string(&a.name)
            ));
        }
        s.push_str(&format!(",\"specular\":{}}}", exact(g)));
    }
    s.push_str("]}\n");
    (s, skipped)
}

/// What [`read`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    pub items: Vec<Annotation>,
    /// The first image's entry.
    pub image: Option<Image>,
    /// Annotations of other images, left out.
    pub other_images: usize,
    /// RLE masks from other tools, read as their boxes.
    pub rle_as_boxes: usize,
    /// Entries without a usable shape.
    pub skipped: usize,
}

/// Why a COCO file could not be read.
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    /// Valid JSON without `images`, `categories` or `annotations`.
    NotCoco,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json(e) => write!(f, "{e}"),
            Error::NotCoco => f.write_str("not a COCO instances file"),
        }
    }
}

fn f(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite())
}

fn pair(v: &Value) -> Option<Pt> {
    let a = v.as_array()?;
    Some((f(a.first()?)?, f(a.get(1)?)?))
}

fn pairs(v: &Value) -> Option<Vec<Pt>> {
    v.as_array()?.iter().map(pair).collect()
}

/// The exact shape Specular wrote.
fn read_exact(v: &Value) -> Option<Geometry> {
    let g = match v.get("type")?.as_str()? {
        "polygon" => Geometry::Area(
            v.get("parts")?
                .as_array()?
                .iter()
                .map(|p| {
                    Some(Poly {
                        exterior: pairs(p.get("exterior")?)?,
                        holes: p
                            .get("holes")?
                            .as_array()?
                            .iter()
                            .map(pairs)
                            .collect::<Option<_>>()?,
                    })
                })
                .collect::<Option<_>>()?,
        ),
        "rectangle" => {
            let b: Vec<f64> = v.get("box")?.as_array()?.iter().filter_map(f).collect();
            if b.len() != 4 {
                return None;
            }
            Geometry::Rect {
                x0: b[0],
                y0: b[1],
                x1: b[2],
                y1: b[3],
            }
        }
        "point" => Geometry::Point(*pairs(v.get("points")?)?.first()?),
        "points" => Geometry::Points(pairs(v.get("points")?)?),
        "line" => Geometry::Line(pairs(v.get("points")?)?),
        _ => return None,
    };
    g.is_valid().then_some(g)
}

/// Reads a COCO instances file: the first image's annotations.
pub fn read(text: &str) -> Result<Read, Error> {
    let v: Value = serde_json::from_str(text).map_err(Error::Json)?;
    let (Some(images), Some(cats), Some(anns)) = (
        v.get("images").and_then(Value::as_array),
        v.get("categories").and_then(Value::as_array),
        v.get("annotations").and_then(Value::as_array),
    ) else {
        return Err(Error::NotCoco);
    };
    let mut out = Read::default();
    let first = images.first();
    let image_id = first.and_then(|i| i.get("id")).cloned();
    out.image = first.and_then(|i| {
        Some(Image {
            file_name: i
                .get("file_name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            width: u32::try_from(i.get("width")?.as_u64()?).ok()?,
            height: u32::try_from(i.get("height")?.as_u64()?).ok()?,
        })
    });
    let class_of = |id: &Value| -> Option<Class> {
        let c = cats.iter().find(|c| c.get("id") == Some(id))?;
        let colour = c.get("color").and_then(|a| {
            let a = a.as_array()?;
            let ch = |i: usize| a.get(i)?.as_u64().and_then(|v| u8::try_from(v).ok());
            Some([ch(0)?, ch(1)?, ch(2)?])
        });
        Some(Class {
            name: c
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            colour,
        })
    };
    for a in anns {
        if image_id.is_some() && a.get("image_id") != image_id.as_ref() {
            out.other_images += 1;
            continue;
        }
        let class = a
            .get("category_id")
            .and_then(class_of)
            .filter(|c| c.name != crate::annot::UNCLASSIFIED);
        let name = a
            .get("attributes")
            .and_then(|x| x.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let geometry = a.get("specular").and_then(read_exact).or_else(|| {
            let seg = a.get("segmentation");
            if let Some(polys) = seg.and_then(Value::as_array).filter(|p| !p.is_empty()) {
                let parts: Option<Vec<Poly>> = polys
                    .iter()
                    .map(|r| {
                        let xs: Vec<f64> = r.as_array()?.iter().filter_map(f).collect();
                        (xs.len() >= 6 && xs.len().is_multiple_of(2)).then(|| {
                            Poly::new(open_ring(
                                xs.chunks_exact(2).map(|c| (c[0], c[1])).collect(),
                            ))
                        })
                    })
                    .collect();
                return parts.map(Geometry::Area);
            }
            if let Some(kp) = a
                .get("keypoints")
                .and_then(Value::as_array)
                .filter(|k| !k.is_empty())
            {
                let pts: Vec<Pt> = kp
                    .chunks_exact(3)
                    .filter(|c| c[2].as_f64().unwrap_or(0.0) > 0.0)
                    .filter_map(|c| Some((f(&c[0])?, f(&c[1])?)))
                    .collect();
                return match pts.len() {
                    0 => None,
                    1 => Some(Geometry::Point(pts[0])),
                    _ => Some(Geometry::Points(pts)),
                };
            }
            let b: Vec<f64> = a.get("bbox")?.as_array()?.iter().filter_map(f).collect();
            if b.len() != 4 || b[2] <= 0.0 || b[3] <= 0.0 {
                return None;
            }
            if seg.is_some_and(Value::is_object) {
                out.rle_as_boxes += 1;
            }
            Some(Geometry::rect((b[0], b[1]), (b[0] + b[2], b[1] + b[3])))
        });
        match geometry.filter(Geometry::is_valid) {
            Some(geometry) => out.items.push(Annotation {
                name,
                class,
                geometry,
            }),
            None => out.skipped += 1,
        }
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests;
