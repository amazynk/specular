//! GeoTIFF: the raster-to-model transform (ModelTiepoint + ModelPixelScale, or
//! ModelTransformation), the GeoKey directory with its EPSG codes, user-defined projections
//! and citations, and GDAL's GDAL_METADATA and GDAL_NODATA tags.

use crate::crs::{self, Crs, CrsKind, LinearUnit};
use crate::proj::{Ellipsoid, Method, Projection};

/// An affine map from raster to model coordinates: `X = a·col + b·row + c`,
/// `Y = d·col + e·row + f`, where (col, row) are continuous pixel coordinates with (0, 0)
/// at the top-left corner of the top-left pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine2 {
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        )
    }

    pub fn inverse(&self) -> Option<Affine2> {
        let det = self.a * self.e - self.b * self.d;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (a, b, d, e) = (self.e / det, -self.b / det, -self.d / det, self.a / det);
        Some(Affine2 {
            a,
            b,
            c: -(a * self.c + b * self.f),
            d,
            e,
            f: -(d * self.c + e * self.f),
        })
    }

    /// Model units per pixel along x and y (the column lengths).
    pub fn pixel_size(&self) -> (f64, f64) {
        (self.a.hypot(self.d), self.b.hypot(self.e))
    }

    /// No rotation or shear.
    pub fn is_north_up(&self) -> bool {
        self.b == 0.0 && self.d == 0.0
    }

    /// This transform followed by `next`.
    pub fn then(&self, next: &Affine2) -> Affine2 {
        Affine2 {
            a: next.a * self.a + next.b * self.d,
            b: next.a * self.b + next.b * self.e,
            c: next.a * self.c + next.b * self.f + next.c,
            d: next.d * self.a + next.e * self.d,
            e: next.d * self.b + next.e * self.e,
            f: next.d * self.c + next.e * self.f + next.f,
        }
    }

    /// The least-squares affine transform taking each `from` point to its `to` point (at
    /// least three, not on one line).
    pub fn fit(pairs: &[([f64; 2], [f64; 2])]) -> Option<Affine2> {
        if pairs.len() < 3 {
            return None;
        }
        // Normal equations for v = p x + q y + r, centred for conditioning.
        let n = pairs.len() as f64;
        let (mx, my) = pairs
            .iter()
            .fold((0.0, 0.0), |(a, b), (f, _)| (a + f[0] / n, b + f[1] / n));
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        let (mut sxu, mut syu, mut sxv, mut syv, mut su, mut sv) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for (f, t) in pairs {
            let (x, y) = (f[0] - mx, f[1] - my);
            sxx += x * x;
            sxy += x * y;
            syy += y * y;
            sxu += x * t[0];
            syu += y * t[0];
            sxv += x * t[1];
            syv += y * t[1];
            su += t[0];
            sv += t[1];
        }
        let det = sxx * syy - sxy * sxy;
        if det.abs() <= 1e-12 * (sxx * syy).max(f64::MIN_POSITIVE) || !det.is_finite() {
            return None;
        }
        let a = (sxu * syy - syu * sxy) / det;
        let b = (syu * sxx - sxu * sxy) / det;
        let d = (sxv * syy - syv * sxy) / det;
        let e = (syv * sxx - sxv * sxy) / det;
        Some(Affine2 {
            a,
            b,
            c: su / n - a * mx - b * my,
            d,
            e,
            f: sv / n - d * mx - e * my,
        })
    }
}

/// What the GeoTIFF tags say.
#[derive(Clone, Debug, Default)]
pub struct GeoTiffInfo {
    pub crs: Option<Crs>,
    /// Raster (pixel corner) to model coordinates.
    pub transform: Option<Affine2>,
    /// RasterPixelIsPoint (the tiepoints name pixel centres).
    pub pixel_is_point: bool,
    /// Ground control points without a transform (not used to place the image).
    pub gcps: usize,
    /// GTCitation, or the PCS/geographic citation.
    pub citation: Option<String>,
    pub vertical: Option<String>,
    /// Every GeoKey, by name, with its value as text: for the all-fields view.
    pub keys: Vec<(String, String)>,
    /// GeoKey directory version "1.1.0" etc.
    pub version: Option<String>,
}

/// GDAL's per-dataset and per-band metadata (GDAL_METADATA) and no-data value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GdalMetadata {
    /// (name, band, role, value); band `None` for dataset items.
    pub items: Vec<GdalItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GdalItem {
    pub name: String,
    pub sample: Option<u32>,
    pub role: Option<String>,
    pub value: String,
}

impl GdalMetadata {
    /// Parse `<GDALMetadata><Item name=".." sample="0" role="scale">value</Item>...`.
    pub fn parse(xml: &str) -> GdalMetadata {
        let xml = xml.trim_end_matches('\0');
        if !specular_limits::xml_within_depth(xml.as_bytes(), specular_limits::MAX_XML_DEPTH) {
            return GdalMetadata::default();
        }
        let Ok(doc) = roxmltree::Document::parse(xml) else {
            return GdalMetadata::default();
        };
        let items = doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "Item")
            .take(4096)
            .map(|n| GdalItem {
                name: n.attribute("name").unwrap_or_default().to_string(),
                sample: n.attribute("sample").and_then(|v| v.parse().ok()),
                role: n.attribute("role").map(str::to_string),
                value: n
                    .descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect(),
            })
            .collect();
        GdalMetadata { items }
    }

    /// A band's item with this role (GDAL's "scale", "offset", "unittype", "description").
    pub fn band_role(&self, band: u32, role: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|i| i.sample == Some(band) && i.role.as_deref() == Some(role))
            .map(|i| i.value.as_str())
    }

    /// A dataset or band item by name.
    pub fn named(&self, name: &str, band: Option<u32>) -> Option<&str> {
        self.items
            .iter()
            .find(|i| i.name == name && i.sample == band && i.role.is_none())
            .map(|i| i.value.as_str())
    }
}

/// GDAL_NODATA: a number as text ("nan", "-9999").
pub fn parse_nodata(s: &str) -> Option<f64> {
    let t = s.trim().trim_end_matches('\0');
    match t.to_ascii_lowercase().as_str() {
        "nan" | "-nan" => Some(f64::NAN),
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ => t.parse().ok(),
    }
}

fn key_name(id: u16) -> String {
    match id {
        1024 => "GTModelTypeGeoKey",
        1025 => "GTRasterTypeGeoKey",
        1026 => "GTCitationGeoKey",
        2048 => "GeographicTypeGeoKey",
        2049 => "GeogCitationGeoKey",
        2050 => "GeogGeodeticDatumGeoKey",
        2051 => "GeogPrimeMeridianGeoKey",
        2052 => "GeogLinearUnitsGeoKey",
        2053 => "GeogLinearUnitSizeGeoKey",
        2054 => "GeogAngularUnitsGeoKey",
        2055 => "GeogAngularUnitSizeGeoKey",
        2056 => "GeogEllipsoidGeoKey",
        2057 => "GeogSemiMajorAxisGeoKey",
        2058 => "GeogSemiMinorAxisGeoKey",
        2059 => "GeogInvFlatteningGeoKey",
        2060 => "GeogAzimuthUnitsGeoKey",
        2061 => "GeogPrimeMeridianLongGeoKey",
        2062 => "GeogTOWGS84GeoKey",
        3072 => "ProjectedCSTypeGeoKey",
        3073 => "PCSCitationGeoKey",
        3074 => "ProjectionGeoKey",
        3075 => "ProjCoordTransGeoKey",
        3076 => "ProjLinearUnitsGeoKey",
        3077 => "ProjLinearUnitSizeGeoKey",
        3078 => "ProjStdParallel1GeoKey",
        3079 => "ProjStdParallel2GeoKey",
        3080 => "ProjNatOriginLongGeoKey",
        3081 => "ProjNatOriginLatGeoKey",
        3082 => "ProjFalseEastingGeoKey",
        3083 => "ProjFalseNorthingGeoKey",
        3084 => "ProjFalseOriginLongGeoKey",
        3085 => "ProjFalseOriginLatGeoKey",
        3086 => "ProjFalseOriginEastingGeoKey",
        3087 => "ProjFalseOriginNorthingGeoKey",
        3088 => "ProjCenterLongGeoKey",
        3089 => "ProjCenterLatGeoKey",
        3090 => "ProjCenterEastingGeoKey",
        3091 => "ProjCenterNorthingGeoKey",
        3092 => "ProjScaleAtNatOriginGeoKey",
        3093 => "ProjScaleAtCenterGeoKey",
        3094 => "ProjAzimuthAngleGeoKey",
        3095 => "ProjStraightVertPoleLongGeoKey",
        4096 => "VerticalCSTypeGeoKey",
        4097 => "VerticalCitationGeoKey",
        4098 => "VerticalDatumGeoKey",
        4099 => "VerticalUnitsGeoKey",
        _ => return format!("GeoKey {id}"),
    }
    .to_string()
}

#[derive(Clone, Debug)]
enum KeyVal {
    Short(u16),
    Doubles(Vec<f64>),
    Ascii(String),
}

/// Interpret the GeoTIFF tags (as stored) of an image of `width` × `height` pixels.
pub fn interpret(
    pixel_scale: &[f64],
    tiepoints: &[f64],
    transformation: &[f64],
    key_directory: &[u16],
    double_params: &[f64],
    ascii_params: &str,
) -> GeoTiffInfo {
    let mut info = GeoTiffInfo::default();
    let mut keys: Vec<(u16, KeyVal)> = Vec::new();
    if key_directory.len() >= 4 {
        let n = key_directory[3] as usize;
        info.version = Some(format!(
            "{}.{}.{}",
            key_directory[0], key_directory[1], key_directory[2]
        ));
        for k in key_directory[4..].chunks_exact(4).take(n) {
            let (id, loc, count, off) = (k[0], k[1], k[2] as usize, k[3] as usize);
            let v = match loc {
                0 => KeyVal::Short(off as u16),
                34736 => match double_params.get(off..off + count) {
                    Some(d) => KeyVal::Doubles(d.to_vec()),
                    None => continue,
                },
                34737 => match ascii_params.get(off..(off + count).min(ascii_params.len())) {
                    Some(s) => KeyVal::Ascii(s.trim_end_matches(['|', '\0']).replace('|', "; ")),
                    None => continue,
                },
                34735 => match key_directory.get(off) {
                    Some(&s) => KeyVal::Short(s),
                    None => continue,
                },
                _ => continue,
            };
            keys.push((id, v));
        }
    }
    let short = |id| {
        keys.iter().find_map(|(k, v)| match v {
            KeyVal::Short(s) if *k == id => Some(*s),
            _ => None,
        })
    };
    let dbl = |id| {
        keys.iter().find_map(|(k, v)| match v {
            KeyVal::Doubles(d) if *k == id => d.first().copied(),
            _ => None,
        })
    };
    let ascii = |id| {
        keys.iter().find_map(|(k, v)| match v {
            KeyVal::Ascii(s) if *k == id && !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => None,
        })
    };
    info.keys = keys
        .iter()
        .map(|(k, v)| {
            (
                key_name(*k),
                match v {
                    KeyVal::Short(s) => s.to_string(),
                    KeyVal::Doubles(d) => d
                        .iter()
                        .map(|x| format!("{x}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    KeyVal::Ascii(s) => s.clone(),
                },
            )
        })
        .collect();
    info.pixel_is_point = short(1025) == Some(2);
    info.citation = ascii(1026).or_else(|| ascii(3073)).or_else(|| ascii(2049));
    info.vertical = match short(4096) {
        Some(c) if c != 0 && c != 32767 => Some(format!("EPSG:{c}")),
        _ => ascii(4097),
    };

    // Raster to model.
    let half = if info.pixel_is_point { 0.5 } else { 0.0 };
    if transformation.len() >= 16 {
        let t = transformation;
        let m = Affine2 {
            a: t[0],
            b: t[1],
            c: t[3],
            d: t[4],
            e: t[5],
            f: t[7],
        };
        // Pixel-is-point: the transform maps pixel centres; shift by half a pixel.
        info.transform = Some(Affine2 {
            c: m.c - half * (m.a + m.b),
            f: m.f - half * (m.d + m.e),
            ..m
        });
    } else if tiepoints.len() >= 6 && pixel_scale.len() >= 2 {
        let (i, j, x, y) = (tiepoints[0], tiepoints[1], tiepoints[3], tiepoints[4]);
        // A negative ScaleY is a writer's sign mistake far more often than a south-up image
        // (GDAL ignores the sign too).
        let (sx, sy) = (pixel_scale[0], pixel_scale[1].abs());
        if sx != 0.0 && sy != 0.0 && [i, j, x, y, sx, sy].iter().all(|v| v.is_finite()) {
            let (i, j) = (i + half, j + half);
            info.transform = Some(Affine2 {
                a: sx,
                b: 0.0,
                c: x - i * sx,
                d: 0.0,
                e: -sy,
                f: y + j * sy,
            });
        }
    } else if tiepoints.len() >= 12 {
        info.gcps = tiepoints.len() / 6;
    }

    // The CRS.
    let model = short(1024);
    let pcs = short(3072).filter(|&c| c != 0 && c != 32767);
    let gcs = short(2048).filter(|&c| c != 0 && c != 32767);
    let mut crs = match (model, pcs, gcs) {
        (_, Some(code), _) => crs::from_epsg(code as u32).or_else(|| {
            let mut c = Crs::named(
                ascii(3073).unwrap_or_else(|| format!("EPSG:{code}")),
                CrsKind::Projected,
            );
            c.epsg = Some(code as u32);
            Some(c)
        }),
        (Some(1), None, _) => user_projected(&short, &dbl, &ascii, gcs),
        (Some(2) | None, None, Some(code)) => crs::from_epsg(code as u32).or_else(|| {
            let mut c = Crs::named(
                ascii(2049).unwrap_or_else(|| format!("EPSG:{code}")),
                CrsKind::Geographic,
            );
            c.epsg = Some(code as u32);
            c.ellipsoid = user_ellipsoid(&short, &dbl);
            Some(c)
        }),
        (Some(2), None, None) => {
            let mut c = Crs::named(
                ascii(2049).unwrap_or_else(|| "Unknown geographic CRS".into()),
                CrsKind::Geographic,
            );
            c.ellipsoid = user_ellipsoid(&short, &dbl);
            // GeogGeodeticDatumGeoKey: datum 6xxx is the datum of geographic CRS 4xxx.
            if let Some(d @ 6001..=6999) = short(2050) {
                c.datum_shift = crate::epsg::to_wgs84(u32::from(d) - 2000);
            }
            Some(c)
        }
        (Some(3), ..) => Some(Crs::named(
            ascii(1026).unwrap_or_else(|| "Geocentric".into()),
            CrsKind::Other,
        )),
        _ => None,
    };
    // A projected CRS by EPSG code whose file states another linear unit (IMAGINE writes
    // metre-based state planes with coordinates in feet): the file's unit wins, as in GDAL.
    if pcs.is_some()
        && let Some(c) = crs.as_mut()
        && c.kind == CrsKind::Projected
        && let Some(u) = file_unit(&short, &dbl, &ascii)
        && c.unit
            .as_ref()
            .is_none_or(|cu| (cu.metres - u.metres).abs() > 1e-12)
    {
        c.unit = Some(u);
    }
    // ESRI writes user-defined CRSs with their WKT in a citation ("ESRI PE String = ...").
    if crs.as_ref().is_none_or(|c| c.epsg.is_none())
        && let Some(wkt) = [3073, 1026, 2049]
            .into_iter()
            .filter_map(&ascii)
            .find_map(|c| {
                c.split_once("ESRI PE String = ")
                    .map(|(_, w)| w.to_string())
            })
        && let Some(c) = crate::wkt::crs_from_wkt(&wkt)
    {
        crs = Some(c);
    }
    // A citation naming a well-known CRS when the keys gave none.
    if crs.as_ref().is_none_or(|c| c.epsg.is_none())
        && let Some(code) = [1026, 3073]
            .into_iter()
            .filter_map(&ascii)
            .find_map(|c| crs::epsg_from_esri_name(c.trim_start_matches("PCS Name = ").trim()))
    {
        crs = crs::from_epsg(code).or(crs);
    }
    // GeogTOWGS84GeoKey: a Helmert to WGS 84 (3 or 7 values, position vector).
    if let Some(c) = crs.as_mut()
        && let Some(v) = keys.iter().find_map(|(k, v)| match v {
            KeyVal::Doubles(d) if *k == 2062 => Some(d.clone()),
            _ => None,
        })
        && let Some(h) = crate::datum::Helmert::from_towgs84(&v)
    {
        c.datum_shift = crate::datum::ToWgs84::One(h);
    }
    info.crs = crs;
    info
}

fn user_ellipsoid(
    short: &dyn Fn(u16) -> Option<u16>,
    dbl: &dyn Fn(u16) -> Option<f64>,
) -> Ellipsoid {
    match short(2056) {
        Some(7030) => return Ellipsoid::WGS84,
        Some(7019) => return Ellipsoid::GRS80,
        Some(7008) => return Ellipsoid::CLARKE1866,
        Some(7022) => return Ellipsoid::INTL1924,
        Some(7001) => return Ellipsoid::AIRY1830,
        Some(7004) => return Ellipsoid::BESSEL1841,
        Some(7024) => return Ellipsoid::KRASSOVSKY,
        _ => {}
    }
    match (dbl(2057), dbl(2059), dbl(2058)) {
        (Some(a), Some(rf), _) => Ellipsoid::inv(a, rf),
        (Some(a), None, Some(b)) if b > 0.0 && a > 0.0 => Ellipsoid { a, f: (a - b) / a },
        (Some(a), None, None) => Ellipsoid { a, f: 0.0 },
        _ => match short(2050) {
            Some(6269 | 6258 | 6283 | 6171) => Ellipsoid::GRS80,
            Some(6267) => Ellipsoid::CLARKE1866,
            Some(6230) => Ellipsoid::INTL1924,
            _ => Ellipsoid::WGS84,
        },
    }
}

/// The linear unit the file states: ProjLinearUnitsGeoKey, else the "Units = ..." line
/// of an IMAGINE citation (as GDAL reads it).
fn file_unit(
    short: &dyn Fn(u16) -> Option<u16>,
    dbl: &dyn Fn(u16) -> Option<f64>,
    ascii: &dyn Fn(u16) -> Option<String>,
) -> Option<LinearUnit> {
    match short(3076) {
        Some(9001) => return Some(LinearUnit::metre()),
        Some(9002) => return Some(LinearUnit::foot()),
        Some(9003) => return Some(LinearUnit::us_survey_foot()),
        Some(_) => {
            return dbl(3077).filter(|m| *m > 0.0).map(|metres| LinearUnit {
                name: "unit".into(),
                metres,
            });
        }
        None => {}
    }
    let citation = ascii(1026)?;
    let units = citation
        .lines()
        .find_map(|l| l.trim().strip_prefix("Units = "))?
        .trim_end_matches('|')
        .to_ascii_lowercase();
    if units.contains("survey") {
        Some(LinearUnit::us_survey_foot())
    } else if units.contains("feet") || units.contains("foot") {
        Some(LinearUnit::foot())
    } else if units.starts_with("met") {
        Some(LinearUnit::metre())
    } else {
        None
    }
}

/// A user-defined projected CRS: ProjectionGeoKey (UTM codes) or ProjCoordTransGeoKey with
/// its parameters.
fn user_projected(
    short: &dyn Fn(u16) -> Option<u16>,
    dbl: &dyn Fn(u16) -> Option<f64>,
    ascii: &dyn Fn(u16) -> Option<String>,
    gcs: Option<u16>,
) -> Option<Crs> {
    let base = gcs.and_then(|c| crs::from_epsg(c as u32));
    let ell = base
        .as_ref()
        .map(|b| b.ellipsoid)
        .unwrap_or_else(|| user_ellipsoid(short, dbl));
    let unit = file_unit(short, dbl, ascii).unwrap_or_else(LinearUnit::metre);
    let name = ascii(3073)
        .or_else(|| ascii(1026))
        .unwrap_or_else(|| "User-defined projected CRS".into());
    // ProjectionGeoKey: Proj_UTM_zone_{1..60}{N,S} = 16001.. / 16101..
    let method = match short(3074) {
        Some(c @ 16001..=16060) => Some(Method::utm((c - 16000) as u32, true)),
        Some(c @ 16101..=16160) => Some(Method::utm((c - 16100) as u32, false)),
        // EPSG:3856, Popular Visualisation Pseudo-Mercator: EPSG:3857 on WGS 84.
        Some(3856) if matches!(gcs, Some(4326) | None) => return crs::from_epsg(3857),
        Some(3856) => Some(Method::WebMercator {
            lon0: 0.0,
            fe: 0.0,
            fn_: 0.0,
        }),
        _ => None,
    };
    // Lengths in the GeoKeys are in the linear unit; angles in degrees (GeogAngularUnits
    // other than degrees are rare and not handled).
    let len = |id| dbl(id).map(|v| v * unit.metres);
    let method = method.or_else(|| {
        let lat_nat = dbl(3081).or(dbl(3085)).or(dbl(3089)).unwrap_or(0.0);
        let lon_nat = dbl(3080).or(dbl(3084)).or(dbl(3088)).unwrap_or(0.0);
        let fe = len(3082).or(len(3086)).or(len(3090)).unwrap_or(0.0);
        let fn_ = len(3083).or(len(3087)).or(len(3091)).unwrap_or(0.0);
        let k0 = dbl(3092).or(dbl(3093)).unwrap_or(1.0);
        // GeoTIFF 1.0 transformation codes, or the EPSG method codes newer writers use.
        let ct = match short(3075)? {
            9807 => 1,
            9804 | 9805 => 7,
            9802 => 8,
            9801 => 9,
            9820 => 10,
            9822 => 11,
            9810 | 9829 => 15,
            9809 => 16,
            1028 | 1029 => 17,
            9806 => 18,
            9808 => 27,
            9812 => 3,
            9815 => 9815,
            c => c,
        };
        Some(match ct {
            1 => Method::TransverseMercator {
                lat0: lat_nat,
                lon0: lon_nat,
                k0,
                fe,
                fn_,
            },
            7 => match dbl(3078) {
                Some(l1) if l1 != 0.0 => Method::MercatorB {
                    lon0: lon_nat,
                    lat1: l1,
                    fe,
                    fn_,
                },
                _ => Method::Mercator {
                    lon0: lon_nat,
                    k0,
                    fe,
                    fn_,
                },
            },
            8 => Method::LambertConic {
                lat0: lat_nat,
                lon0: lon_nat,
                lat1: dbl(3078)?,
                lat2: dbl(3079)?,
                k0: 1.0,
                fe,
                fn_,
            },
            9 => Method::LambertConic {
                lat0: lat_nat,
                lon0: lon_nat,
                lat1: lat_nat,
                lat2: lat_nat,
                k0,
                fe,
                fn_,
            },
            11 => Method::Albers {
                lat0: lat_nat,
                lon0: lon_nat,
                lat1: dbl(3078)?,
                lat2: dbl(3079)?,
                fe,
                fn_,
            },
            // CT_ObliqueMercator (Hotine, variant B as GDAL writes it: centre, azimuth,
            // rectified grid angle).
            3 | 9815 => {
                let alpha = dbl(3094)?;
                Method::HotineObliqueMercator {
                    latc: dbl(3089).unwrap_or(lat_nat),
                    lonc: dbl(3088).unwrap_or(lon_nat),
                    alpha,
                    gamma: dbl(3096).unwrap_or(alpha),
                    kc: k0,
                    fe,
                    fn_,
                    centre: ct == 9815,
                }
            }
            10 => Method::LambertAzimuthal {
                lat0: lat_nat,
                lon0: lon_nat,
                fe,
                fn_,
            },
            15 => {
                let lon0 = dbl(3095).unwrap_or(lon_nat);
                if (lat_nat.abs() - 90.0).abs() < 1e-12 {
                    Method::PolarStereographicA {
                        south: lat_nat < 0.0,
                        lon0,
                        k0,
                        fe,
                        fn_,
                    }
                } else {
                    Method::PolarStereographic {
                        lat_ts: lat_nat,
                        lon0,
                        south: lat_nat < 0.0,
                        fe,
                        fn_,
                    }
                }
            }
            16 => Method::ObliqueStereographic {
                lat0: lat_nat,
                lon0: lon_nat,
                k0,
                fe,
                fn_,
            },
            // On an ellipsoid as PROJ evaluates it (the meridian arc for y).
            17 => Method::EquidistantCylindrical {
                lat_ts: dbl(3078).unwrap_or(0.0),
                lon0: lon_nat,
                fe,
                fn_,
            },
            18 => Method::CassiniSoldner {
                lat0: lat_nat,
                lon0: lon_nat,
                fe,
                fn_,
            },
            27 => Method::TransverseMercatorSouth {
                lat0: lat_nat,
                lon0: lon_nat,
                k0,
                fe,
                fn_,
            },
            _ => return None,
        })
    });
    let mut c = match method {
        Some(m) => Crs::custom_projected(name, ell, m, unit),
        None => {
            let mut c = Crs::named(name, CrsKind::Projected);
            c.unit = Some(unit);
            c.ellipsoid = ell;
            c.method_name = short(3075).map(|t| format!("GeoTIFF coordinate transformation {t}"));
            c
        }
    };
    if let Some(b) = base {
        c.geographic = Some(b.name.clone());
        c.datum = b.datum.clone();
        c.ellipsoid_name = b.ellipsoid_name.clone();
        c.datum_shift = b.datum_shift.clone();
        c.pm = b.pm;
    }
    if let Some(p) = c.projection.as_mut() {
        *p = Projection::new(p.method, ell);
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_unit_from_key_or_imagine_citation() {
        let none_s = |_: u16| None;
        let none_d = |_: u16| None;
        let usft = |k: u16| (k == 3076).then_some(9003);
        assert_eq!(
            file_unit(&usft, &none_d, &|_| None),
            Some(LinearUnit::us_survey_foot())
        );
        let cite = |k: u16| {
            (k == 1026).then(|| {
                "IMAGINE GeoTIFF Support\nProjection Name = X\nUnits = international_FeeT\nGeoTIFF Units = international_FeeT|".to_string()
            })
        };
        assert_eq!(file_unit(&none_s, &none_d, &cite), Some(LinearUnit::foot()));
        assert_eq!(file_unit(&none_s, &none_d, &|_| None), None);
    }

    #[test]
    fn utm_tiepoint() {
        let keys = [1, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32633];
        let g = interpret(
            &[30.0, 30.0, 0.0],
            &[0.0, 0.0, 0.0, 440_720.0, 3_751_320.0, 0.0],
            &[],
            &keys,
            &[],
            "",
        );
        let c = g.crs.unwrap();
        assert_eq!(c.epsg, Some(32633));
        let t = g.transform.unwrap();
        assert_eq!(t.apply(1.0, 1.0), (440_750.0, 3_751_290.0));
    }

    #[test]
    fn gdal_metadata() {
        let m = GdalMetadata::parse(
            r#"<GDALMetadata><Item name="STATISTICS_MAXIMUM" sample="0">10</Item><Item name="SCALE" sample="0" role="scale">0.5</Item><Item name="DESCRIPTION">a &amp; b</Item></GDALMetadata>"#,
        );
        assert_eq!(m.band_role(0, "scale"), Some("0.5"));
        assert_eq!(m.named("DESCRIPTION", None), Some("a & b"));
        assert!(parse_nodata("nan").unwrap().is_nan());
        assert_eq!(parse_nodata("-9999"), Some(-9999.0));
    }
}
