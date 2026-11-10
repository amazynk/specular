//! `specular-planetary`: planetary science image formats, read with own code.
//!
//! * **VICAR** ([`vicar`]): JPL's labelled image files (`.vic`, `.img`), with binary headers
//!   and line prefixes skipped, all sample formats and host byte orders (VAX reals too),
//!   BSQ/BIL/BIP, and end-of-file labels.
//! * **PDS3** ([`pds3`]): ODL labels ([`odl`]) attached to the data or detached (`.lbl`),
//!   `IMAGE` and `QUBE` objects found through `^` pointers by record or byte, every
//!   `SAMPLE_TYPE` of the standard (MSB, LSB, PC, VAX, IEEE, complex), bands in any storage
//!   order, line prefix and suffix bytes, scaling, missing and invalid constants.
//! * **PDS4** ([`pds4`]): XML labels of observational products, `Array_2D` and `Array_3D`
//!   arrays in any axis order, every element type, special constants, scaling and offset.
//!
//! Each gives [`Facts`]: the mission, spacecraft, instrument, target, observation times and
//! the map projection, for the info panel.

pub mod odl;
pub mod pds3;
pub mod pds4;
pub mod raster;
pub mod sample;
pub mod vicar;

pub use raster::{Interleave, Layout, NoData};
pub use sample::{Real, Sample};

/// Errors of the readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a file of this format")]
    NotThisFormat,
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

/// What a label says about the observation, for the info panel. Text as the label writes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub mission: Option<String>,
    pub spacecraft: Option<String>,
    pub instrument: Option<String>,
    pub target: Option<String>,
    pub start: Option<String>,
    pub stop: Option<String>,
    pub product_id: Option<String>,
    pub title: Option<String>,
    pub map: Option<MapFacts>,
}

impl Facts {
    /// Fills what `self` lacks from `other`.
    pub fn or(mut self, other: Facts) -> Facts {
        macro_rules! fill {
            ($($f:ident),*) => {$(if self.$f.is_none() { self.$f = other.$f; })*};
        }
        fill!(
            mission, spacecraft, instrument, target, start, stop, product_id, title, map
        );
        self
    }
}

/// A map-projected image's projection, as the label states it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapFacts {
    /// "SIMPLE CYLINDRICAL", "EQUIRECTANGULAR", "POLAR STEREOGRAPHIC"…
    pub projection: String,
    /// Metres per pixel.
    pub metres_per_pixel: Option<f64>,
    /// Pixels per degree.
    pub pixels_per_degree: Option<f64>,
    /// Latitude and longitude of the projection centre, degrees.
    pub centre: Option<(f64, f64)>,
    /// Latitudes covered, degrees (south, north).
    pub latitudes: Option<(f64, f64)>,
    /// Longitudes covered, degrees (west, east as the label gives them).
    pub longitudes: Option<(f64, f64)>,
    /// The body's equatorial radius, kilometres.
    pub radius_km: Option<f64>,
    /// "EAST" or "WEST" (which way longitudes grow).
    pub positive_longitude: Option<String>,
}

/// A label value that says nothing ("N/A", "UNK", "NULL", empty).
pub(crate) fn meaningful(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()
        && !["N/A", "UNK", "NULL", "UNKNOWN", "NONE"]
            .iter()
            .any(|n| t.eq_ignore_ascii_case(n)))
    .then(|| t.to_owned())
}

/// A length in `unit` as metres (`default` when the label gives no unit).
pub(crate) fn metres(v: f64, unit: Option<&str>, default: f64) -> Option<f64> {
    let u = unit.map(|u| u.trim().to_ascii_uppercase().replace(' ', ""));
    let f = match u.as_deref() {
        None | Some("") => default,
        Some(u) => {
            let per = u.split('/').next().unwrap_or("");
            match per {
                "KM" | "KILOMETERS" | "KILOMETRES" | "KILOMETER" => 1000.0,
                "M" | "METERS" | "METRES" | "METER" => 1.0,
                "CM" => 0.01,
                "MM" => 0.001,
                _ => return None,
            }
        }
    };
    let m = v * f;
    (m.is_finite() && m > 0.0).then_some(m)
}

/// The map facts from a label's projection keys (PDS3 `IMAGE_MAP_PROJECTION`, a VICAR `MAP`
/// property), read through `get`.
pub(crate) fn map_facts(get: impl Fn(&str) -> Option<odl::Value>) -> Option<MapFacts> {
    let text = |k: &str| get(k).and_then(|v| v.as_str().and_then(meaningful));
    let num = |k: &str| get(k).and_then(|v| v.as_f64()).filter(|v| v.is_finite());
    let projection = text("MAP_PROJECTION_TYPE")?;
    let scale = get("MAP_SCALE").and_then(|v| {
        let n = v.as_f64()?;
        metres(n, v.unit(), 1000.0)
    });
    let pair = |a: &str, b: &str| num(a).zip(num(b));
    Some(MapFacts {
        projection: projection.replace('_', " "),
        metres_per_pixel: scale,
        pixels_per_degree: num("MAP_RESOLUTION").filter(|v| *v > 0.0),
        centre: pair("CENTER_LATITUDE", "CENTER_LONGITUDE"),
        latitudes: pair("MINIMUM_LATITUDE", "MAXIMUM_LATITUDE"),
        longitudes: pair("WESTERNMOST_LONGITUDE", "EASTERNMOST_LONGITUDE"),
        radius_km: get("A_AXIS_RADIUS").and_then(|v| {
            let n = v.as_f64()?;
            metres(n, v.unit(), 1000.0).map(|m| m / 1000.0)
        }),
        positive_longitude: text("POSITIVE_LONGITUDE_DIRECTION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_words() {
        assert_eq!(metres(0.5, Some("KM/PIXEL"), 1.0), Some(500.0));
        assert_eq!(metres(25.0, Some("METERS/PIXEL"), 1000.0), Some(25.0));
        assert_eq!(metres(2.0, None, 1000.0), Some(2000.0));
        assert_eq!(metres(2.0, Some("PIX/DEG"), 1000.0), None);
        assert_eq!(meaningful(" N/A "), None);
        assert_eq!(meaningful("MARS").as_deref(), Some("MARS"));
    }
}
