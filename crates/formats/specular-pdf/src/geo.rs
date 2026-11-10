//! Geospatial PDF dictionaries, read as given (the caller turns them into a georeference):
//! ISO 32000 measure dictionaries (`/VP` viewports with `/Measure /Subtype /GEO`: GPTS,
//! LPTS, Bounds and the `/GCS` with its EPSG code or WKT) and the OGC best practice
//! `/LGIDict` (a CTM or registration points with a `/Projection` dictionary).

use hayro::hayro_syntax::object::{Array, Dict, Name, Object};

use crate::Document;

/// One geospatial dictionary of a page.
#[derive(Clone, Debug, PartialEq)]
pub enum Geo {
    /// An ISO 32000 viewport with a GEO measure.
    Measure {
        /// The viewport's box in user space (x0, y0, x1, y1).
        bbox: [f64; 4],
        /// Latitude, longitude pairs.
        gpts: Vec<f64>,
        /// The matching points, normalised within `bbox` (x, y pairs).
        lpts: Vec<f64>,
        epsg: Option<i32>,
        wkt: Option<String>,
        /// PROJCS or GEOGCS.
        gcs_type: Option<String>,
        name: Option<String>,
    },
    /// An OGC best practice LGIDict.
    Lgi {
        /// User space to map coordinates (a b c d e f: x' = a x + c y + e, y' = b x + d y + f).
        ctm: Option<[f64; 6]>,
        /// (page x, page y, map x, map y) points.
        registration: Vec<[f64; 4]>,
        /// The Projection dictionary's entries as text.
        projection: Vec<(String, String)>,
        description: Option<String>,
    },
}

/// A number given as a number or as a string (LGIDict writes both).
fn num(o: &Object<'_>) -> Option<f64> {
    match o {
        Object::Number(n) => Some(n.as_f64()),
        Object::String(s) => std::str::from_utf8(s).ok()?.trim().parse().ok(),
        _ => None,
    }
}

fn nums(a: &Array<'_>) -> Vec<f64> {
    a.iter::<Object<'_>>().filter_map(|o| num(&o)).collect()
}

fn text(o: &Object<'_>) -> Option<String> {
    match o {
        Object::String(s) => crate::decode_text_string(s),
        Object::Name(n) => Some(n.as_str().to_string()),
        Object::Number(n) => Some(n.as_f64().to_string()),
        _ => None,
    }
}

fn measure(vp: &Dict<'_>) -> Option<Geo> {
    let m = vp.get::<Dict<'_>>(b"Measure")?;
    if m.get::<Name<'_>>(b"Subtype")
        .is_some_and(|n| n.as_str() != "GEO")
    {
        return None;
    }
    let bbox = vp.get::<Vec<f64>>(b"BBox")?;
    let [x0, y0, x1, y1] = bbox.try_into().ok()?;
    let gpts = m.get::<Array<'_>>(b"GPTS").map(|a| nums(&a))?;
    let lpts = m
        .get::<Array<'_>>(b"LPTS")
        .map(|a| nums(&a))
        .or_else(|| m.get::<Array<'_>>(b"Bounds").map(|a| nums(&a)))?;
    let gcs = m.get::<Dict<'_>>(b"GCS");
    Some(Geo::Measure {
        bbox: [x0, y0, x1, y1],
        gpts,
        lpts,
        epsg: gcs.as_ref().and_then(|g| g.get::<i32>(b"EPSG")),
        wkt: gcs
            .as_ref()
            .and_then(|g| g.get::<Object<'_>>(b"WKT"))
            .and_then(|o| text(&o)),
        gcs_type: gcs
            .as_ref()
            .and_then(|g| g.get::<Name<'_>>(b"Type"))
            .map(|n| n.as_str().to_string()),
        name: vp.get::<Object<'_>>(b"Name").and_then(|o| text(&o)),
    })
}

fn lgi(d: &Dict<'_>) -> Option<Geo> {
    let ctm = d
        .get::<Array<'_>>(b"CTM")
        .map(|a| nums(&a))
        .and_then(|v| <[f64; 6]>::try_from(v).ok());
    let registration: Vec<[f64; 4]> = d
        .get::<Array<'_>>(b"Registration")
        .map(|a| {
            a.iter::<Array<'_>>()
                .filter_map(|p| <[f64; 4]>::try_from(nums(&p)).ok())
                .collect()
        })
        .unwrap_or_default();
    let projection = d
        .get::<Dict<'_>>(b"Projection")
        .map(|p| {
            p.keys()
                .filter_map(|k| {
                    let v = p.get::<Object<'_>>(k.as_ref())?;
                    Some((k.as_str().to_string(), text(&v)?))
                })
                .collect()
        })
        .unwrap_or_default();
    if ctm.is_none() && registration.is_empty() {
        return None;
    }
    Some(Geo::Lgi {
        ctm,
        registration,
        projection,
        description: d.get::<Object<'_>>(b"Description").and_then(|o| text(&o)),
    })
}

impl Document {
    /// The geospatial dictionaries of page `index`: its ISO 32000 viewports with GEO
    /// measures, then its LGIDicts.
    pub fn geospatial(&self, index: usize) -> Vec<Geo> {
        let Some(page) = self.pdf.pages().get(index) else {
            return Vec::new();
        };
        let raw = page.raw();
        let mut out = Vec::new();
        if let Some(vps) = raw.get::<Array<'_>>(b"VP") {
            out.extend(vps.iter::<Dict<'_>>().filter_map(|vp| measure(&vp)));
        }
        if let Some(d) = raw.get::<Dict<'_>>(b"LGIDict") {
            out.extend(lgi(&d));
        } else if let Some(a) = raw.get::<Array<'_>>(b"LGIDict") {
            out.extend(a.iter::<Dict<'_>>().filter_map(|d| lgi(&d)));
        }
        out
    }

    /// The transform from page `index`'s user space (points, y up) to the rendered page
    /// (points, y down, origin at the top left; rotation and crop box applied), as
    /// (a, b, c, d, e, f): x' = a x + c y + e, y' = b x + d y + f.
    pub fn user_to_page(&self, index: usize) -> Option<[f64; 6]> {
        let page = self.pdf.pages().get(index)?;
        Some(page.initial_transform(true).as_coeffs())
    }
}
