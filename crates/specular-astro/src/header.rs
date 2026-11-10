//! The FITS keywords subframes are sorted and grouped by, as capture software writes them
//! (N.I.N.A., SGP, MaxIm DL, ASIAIR, KStars/Ekos, SharpCap, APT; XISF files carry the same
//! keywords), and the plate scale.

/// What a subframe's header says about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubframeHeader {
    /// `OBJECT`.
    pub object: Option<String>,
    /// `FILTER` (`FILTNAM1`, `FILTER1`).
    pub filter: Option<String>,
    /// `IMAGETYP` (`FRAME`) as stored: "Light Frame", "LIGHT", "Dark", "Flat Field"…
    pub frame_type: Option<String>,
    /// `EXPTIME` (`EXPOSURE`), seconds.
    pub exposure: Option<f64>,
    /// `GAIN` (camera gain setting, as the driver numbers it).
    pub gain: Option<f64>,
    /// `CCD-TEMP` (`CCD_TEMP`, `SENSOR-TEMP`, `TEMPERAT`), °C.
    pub ccd_temp: Option<f64>,
    /// `DATE-OBS` (`DATE-BEG`) as stored.
    pub date_obs: Option<String>,
    /// `FOCALLEN` (mm), `XPIXSZ` (µm), `XBINNING`.
    pub focal_mm: Option<f64>,
    pub pixel_um: Option<f64>,
    pub binning: Option<f64>,
}

impl SubframeHeader {
    /// Reads the keywords through `get` (a keyword's value as text, strings without quotes).
    pub fn from_keywords(get: impl Fn(&str) -> Option<String>) -> SubframeHeader {
        let s = |keys: &[&str]| {
            keys.iter().find_map(|k| {
                get(k)
                    .map(|v| v.trim().to_owned())
                    .filter(|v| !v.is_empty())
            })
        };
        let f = |keys: &[&str]| {
            keys.iter().find_map(|k| {
                get(k).and_then(|v| {
                    let v = v.trim().replace(['D', 'd'], "E");
                    v.parse::<f64>().ok().filter(|x| x.is_finite())
                })
            })
        };
        SubframeHeader {
            object: s(&["OBJECT", "OBJNAME"]),
            filter: s(&["FILTER", "FILTNAM1", "FILTER1"]),
            frame_type: s(&["IMAGETYP", "FRAME", "FRAMETYP"]),
            exposure: f(&["EXPTIME", "EXPOSURE"]),
            gain: f(&["GAIN"]),
            ccd_temp: f(&["CCD-TEMP", "CCD_TEMP", "SENSOR-TEMP", "TEMPERAT"]),
            date_obs: s(&["DATE-OBS", "DATE-BEG"]),
            focal_mm: f(&["FOCALLEN"]).filter(|v| *v > 0.0),
            pixel_um: f(&["XPIXSZ", "PIXSIZE1", "PIXSIZE"]).filter(|v| *v > 0.0),
            binning: f(&["XBINNING"]).filter(|v| *v >= 1.0),
        }
    }

    /// Whether anything a subframe is sorted by was found.
    pub fn is_empty(&self) -> bool {
        self.object.is_none()
            && self.filter.is_none()
            && self.frame_type.is_none()
            && self.exposure.is_none()
            && self.gain.is_none()
            && self.ccd_temp.is_none()
            && self.date_obs.is_none()
    }

    /// The plate scale (arc seconds per pixel) from the focal length and pixel size.
    /// `XPIXSZ` is the binned pixel's size by the SBIG convention most software follows; one
    /// that equals the sensor's unbinned size cannot be told apart, so it is taken as written.
    pub fn optics_scale(&self) -> Option<f64> {
        let (f, p) = (self.focal_mm?, self.pixel_um?);
        Some(206.264_806 * p / f)
    }

    /// The frame's kind in one word, from `IMAGETYP`: `light`, `dark`, `flat`, `bias`,
    /// `darkflat`, or the value itself lower-cased.
    pub fn kind(&self) -> Option<String> {
        let t = self.frame_type.as_deref()?.to_ascii_lowercase();
        let t = t.trim();
        let word = if t.contains("dark") && t.contains("flat") {
            "darkflat"
        } else if t.contains("light") || t == "object" || t == "science" {
            "light"
        } else if t.contains("dark") {
            "dark"
        } else if t.contains("flat") {
            "flat"
        } else if t.contains("bias") || t.contains("offset") || t == "zero" {
            "bias"
        } else {
            return Some(t.to_owned());
        };
        Some(word.to_owned())
    }
}

/// The plate scale (arc seconds per pixel) from a WCS's `CDi_j` or `CDELTi` (with `CROTA2` or
/// `PCi_j`, which keep the scale): the square root of the pixel's area on the sky.
pub fn wcs_scale(get: impl Fn(&str) -> Option<String>) -> Option<f64> {
    let f = |k: &str| {
        get(k).and_then(|v| {
            v.trim()
                .replace(['D', 'd'], "E")
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
        })
    };
    let det = match (f("CD1_1"), f("CD1_2"), f("CD2_1"), f("CD2_2")) {
        (Some(a), b, c, Some(d)) => a * d - b.unwrap_or(0.0) * c.unwrap_or(0.0),
        _ => {
            let (c1, c2) = (f("CDELT1")?, f("CDELT2")?);
            let pc = match (f("PC1_1"), f("PC1_2"), f("PC2_1"), f("PC2_2")) {
                (Some(a), b, c, Some(d)) => a * d - b.unwrap_or(0.0) * c.unwrap_or(0.0),
                _ => 1.0,
            };
            c1 * c2 * pc
        }
    };
    let s = det.abs().sqrt() * 3600.0;
    (s > 0.0 && s.is_finite()).then_some(s)
}
