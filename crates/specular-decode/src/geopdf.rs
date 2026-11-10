//! GeoPDF: a PDF page's georeference from its ISO 32000 geospatial measure (the largest
//! viewport's GPTS/LPTS fitted to the page, with the GCS's EPSG code or WKT) or its OGC best
//! practice LGIDict (CTM or registration points, with a geographic or UTM projection).
//! The page's rotation and crop box are applied, so stored pixels map to the map.

use specular_gis::crs::{self, Crs, CrsKind};
use specular_gis::geotiff::Affine2;
use specular_pdf::Document;
use specular_pdf::geo::Geo;

use crate::geo::{GeoRef, RasterGeo};

/// A PDF matrix (a b c d e f: x' = a x + c y + e, y' = b x + d y + f) as an [`Affine2`].
fn pdf_matrix(m: [f64; 6]) -> Affine2 {
    Affine2 {
        a: m[0],
        b: m[2],
        c: m[4],
        d: m[1],
        e: m[3],
        f: m[5],
    }
}

/// The CRS an LGIDict projection names: geographic or UTM on WGS 84, NAD83 or NAD27.
fn lgi_crs(p: &[(String, String)]) -> (Option<Crs>, Option<String>) {
    let get = |k: &str| {
        p.iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.trim().to_ascii_uppercase())
    };
    let datum = get("Datum").unwrap_or_default();
    // DMA datum codes: WGE (WGS 84), NAR (NAD83), NAS (NAD27), with region suffixes.
    let base = match datum.split('-').next().unwrap_or("") {
        "WGE" | "WGS84" | "WGS 84" => Some((4326, 32600, 32700)),
        "NAR" | "NAD83" => Some((4269, 26900, 0)),
        "NAS" | "NAD27" => Some((4267, 26700, 0)),
        _ => None,
    };
    let ty = get("ProjectionType").unwrap_or_default();
    let Some((geog, north, south)) = base else {
        return (None, Some(format!("LGIDict datum {datum}")));
    };
    match ty.as_str() {
        "GEOGRAPHIC" | "GEODETIC" => (crs::from_epsg(geog), None),
        "UT" => {
            let zone: u32 = get("Zone").and_then(|z| z.parse().ok()).unwrap_or(0);
            let south_hemi = get("Hemisphere").is_some_and(|h| h.starts_with('S'));
            let base = if south_hemi { south } else { north };
            if !(1..=60).contains(&zone) || base == 0 {
                return (None, Some(format!("LGIDict UTM zone {zone}")));
            }
            (crs::from_epsg(base + zone), None)
        }
        other => (
            None,
            Some(format!("LGIDict projection {other} is not supported")),
        ),
    }
}

/// User space to map coordinates, the CRS and what the facts list.
type Placement = (Affine2, Option<Crs>, Option<String>, Vec<(String, String)>);

fn from_measure(g: &Geo) -> Option<Placement> {
    let Geo::Measure {
        bbox,
        gpts,
        lpts,
        epsg,
        wkt,
        name,
        ..
    } = g
    else {
        return None;
    };
    let by_code = epsg
        .and_then(|c| u32::try_from(c).ok())
        .and_then(crs::from_epsg);
    let mut note = None;
    // The points are longitude/latitude; without a known CRS the map is drawn in them.
    let crs = by_code
        .or_else(|| wkt.as_deref().and_then(specular_gis::wkt::crs_from_wkt))
        .or_else(|| {
            note = Some(match epsg {
                Some(e) => {
                    format!("code {e} is not in the EPSG dataset; shown in longitude and latitude")
                }
                None => "no CRS given; shown in longitude and latitude".to_string(),
            });
            crs::from_epsg(4326)
        });
    let geographic = crs.as_ref().is_none_or(|c| c.kind == CrsKind::Geographic);
    let [x0, y0, x1, y1] = *bbox;
    let mut pairs = Vec::new();
    for (g, l) in gpts.chunks_exact(2).zip(lpts.chunks_exact(2)) {
        let (lat, lon) = (g[0], g[1]);
        let user = [x0 + l[0] * (x1 - x0), y0 + l[1] * (y1 - y0)];
        let map = if geographic {
            [lon, lat]
        } else {
            let (x, y) = crs.as_ref()?.from_lonlat(lon, lat)?;
            [x, y]
        };
        pairs.push((user, map));
    }
    let t = Affine2::fit(&pairs)?;
    let mut keys = vec![("Encoding".into(), "ISO 32000 measure".into())];
    if let Some(n) = name.as_ref().filter(|n| !n.trim().is_empty()) {
        keys.push(("Viewport".into(), n.trim().to_string()));
    }
    if let Some(e) = epsg {
        keys.push(("EPSG".into(), e.to_string()));
    }
    Some((t, crs, note, keys))
}

fn from_lgi(g: &Geo) -> Option<Placement> {
    let Geo::Lgi {
        ctm,
        registration,
        projection,
        description,
    } = g
    else {
        return None;
    };
    let t = match ctm {
        Some(m) => pdf_matrix(*m),
        None => Affine2::fit(
            &registration
                .iter()
                .map(|r| ([r[0], r[1]], [r[2], r[3]]))
                .collect::<Vec<_>>(),
        )?,
    };
    let (crs, note) = lgi_crs(projection);
    let mut keys = vec![("Encoding".into(), "OGC best practice (LGIDict)".into())];
    if let Some(d) = description.as_ref().filter(|d| !d.trim().is_empty()) {
        keys.push(("Description".into(), d.trim().to_string()));
    }
    for (k, v) in projection {
        if k != "Type" {
            keys.push((k.clone(), v.clone()));
        }
    }
    Some((t, crs, note, keys))
}

/// The georeference of page `page`, rendered at `px_per_pt` stored pixels per point to a
/// `size` raster.
pub(crate) fn page_georef(
    doc: &Document,
    page: usize,
    px_per_pt: (f64, f64),
    size: (u32, u32),
) -> Option<GeoRef> {
    let geos = doc.geospatial(page);
    let area = |g: &Geo| match g {
        Geo::Measure { bbox, .. } => ((bbox[2] - bbox[0]) * (bbox[3] - bbox[1])).abs(),
        Geo::Lgi { .. } => 0.0,
    };
    let measure = geos
        .iter()
        .filter(|g| matches!(g, Geo::Measure { .. }))
        .max_by(|a, b| area(a).total_cmp(&area(b)));
    let (user_to_map, crs, note, keys) = measure
        .and_then(from_measure)
        .or_else(|| geos.iter().find_map(from_lgi))?;
    let page_from_user = pdf_matrix(doc.user_to_page(page)?);
    let stored_to_page = Affine2 {
        a: 1.0 / px_per_pt.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 1.0 / px_per_pt.1,
        f: 0.0,
    };
    let to_map = stored_to_page
        .then(&page_from_user.inverse()?)
        .then(&user_to_map);
    Some(crate::geo::raster(
        to_map,
        crs,
        note,
        size,
        RasterGeo {
            levels: vec![size],
            keys,
            keys_kind: "header",
            ..Default::default()
        },
    ))
}
