//! An image's rendering metadata from `/webgateway/imgData/<id>/`: sizes, pixel type,
//! physical sizes, channels with their colours and windows, the rendering defaults, and
//! whether the server serves it as a pyramid of tiles.

use serde_json::Value;
use specular_remote::CancelToken;

use crate::api::{Omero, OmeroError, Result};

/// The largest image side and plane count accepted (beyond any real microscope image).
const MAX_SIDE: u64 = 1 << 24;
const MAX_PLANES: u64 = 1 << 20;
const MAX_CHANNELS: usize = 1024;
const MAX_LEVELS: u32 = 32;

/// A channel's rendering window and range.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    /// The pixel type's or the channel's range.
    pub min: f64,
    pub max: f64,
    /// Values mapped to black and full colour.
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    pub label: String,
    /// "RRGGBB".
    pub colour: [u8; 3],
    pub active: bool,
    pub window: Window,
    pub emission: Option<f64>,
}

/// What imgData says about an image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImgData {
    pub id: u64,
    pub name: String,
    pub description: String,
    /// "First Last".
    pub author: Option<String>,
    pub project: Option<(u64, String)>,
    pub dataset: Option<(u64, String)>,
    pub well_id: Option<u64>,
    /// The acquisition (or import) time, seconds since 1970.
    pub timestamp: Option<i64>,
    pub pixels_type: String,
    /// X, Y, Z, C, T.
    pub size: [u32; 5],
    /// µm per pixel along X, Y, Z.
    pub pixel_size: [Option<f64>; 3],
    pub magnification: Option<f64>,
    pub channels: Vec<Channel>,
    /// The server serves the image as tiles of a pyramid (else `render_image` whole).
    pub tiled: bool,
    pub tile: (u32, u32),
    /// Pyramid levels (1 without a pyramid).
    pub levels: u32,
    /// Scale of each level against full size, level 0 (full size) first.
    pub scaling: Vec<f64>,
    pub default_z: u32,
    pub default_t: u32,
    /// Greyscale rendering model.
    pub greyscale: bool,
}

impl Omero {
    /// The image's imgData.
    pub fn img_data(&self, id: u64, cancel: &CancelToken) -> Result<ImgData> {
        let v = self.get_json(&format!("/webgateway/imgData/{id}/"), cancel)?;
        parse(&v)
    }
}

/// `"FF0000"` as RGB; white for anything else.
pub fn parse_colour(s: &str) -> [u8; 3] {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6
        && let Ok(v) = u32::from_str_radix(s, 16)
    {
        return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
    }
    [255, 255, 255]
}

fn f(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn s(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_string()
}

/// Parses imgData's JSON.
pub fn parse(v: &Value) -> Result<ImgData> {
    let bad = |m: &str| OmeroError::Malformed(format!("imgData: {m}"));
    let id = v
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad("no id"))?;
    let meta = v.get("meta");
    let m = |k: &str| meta.and_then(|m| m.get(k));
    let size = v.get("size").ok_or_else(|| bad("no size"))?;
    let dim = |k: &str| -> Result<u32> {
        let n = size.get(k).and_then(Value::as_u64).unwrap_or(1);
        if n == 0 || n > MAX_SIDE {
            return Err(bad("size out of range"));
        }
        Ok(n as u32)
    };
    let size = [
        dim("width")?,
        dim("height")?,
        dim("z")?,
        dim("c")?,
        dim("t")?,
    ];
    if u64::from(size[2]) * u64::from(size[4]) > MAX_PLANES {
        return Err(bad("too many planes"));
    }
    let channels: Vec<Channel> = v
        .get("channels")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .take(MAX_CHANNELS)
                .map(|c| {
                    let w = c.get("window");
                    let wf = |k: &str| f(w.and_then(|w| w.get(k)));
                    let (min, max) = (wf("min").unwrap_or(0.0), wf("max").unwrap_or(255.0));
                    Channel {
                        label: s(c.get("label")),
                        colour: parse_colour(&s(c.get("color"))),
                        active: c.get("active").and_then(Value::as_bool).unwrap_or(true),
                        window: Window {
                            min,
                            max,
                            start: wf("start").unwrap_or(min),
                            end: wf("end").unwrap_or(max),
                        },
                        emission: f(c.get("emissionWave")),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let tiled = v.get("tiles").and_then(Value::as_bool).unwrap_or(false);
    let ts = v.get("tile_size");
    let tdim = |k: &str| {
        ts.and_then(|t| t.get(k))
            .and_then(Value::as_u64)
            .filter(|&n| (16..=4096).contains(&n))
            .unwrap_or(256) as u32
    };
    let levels = v
        .get("levels")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, u64::from(MAX_LEVELS)) as u32;
    let zs = v.get("zoomLevelScaling");
    let mut scaling: Vec<f64> = (0..levels)
        .map_while(|l| f(zs.and_then(|z| z.get(l.to_string()))))
        .collect();
    if scaling.len() != levels as usize || scaling.first().is_some_and(|&s| (s - 1.0).abs() > 1e-6)
    {
        // Missing or odd: halving per level, as OMERO builds pyramids.
        scaling = (0..levels).map(|l| 0.5f64.powi(l as i32)).collect();
    }
    let ps = v.get("pixel_size");
    let px = |k: &str| f(ps.and_then(|p| p.get(k))).filter(|&x| x > 0.0);
    let rdefs = v.get("rdefs");
    let r = |k: &str| rdefs.and_then(|r| r.get(k));
    let opt_pair = |idk: &str, namek: &str| {
        let id = m(idk).and_then(Value::as_u64)?;
        Some((id, s(m(namek))))
    };
    Ok(ImgData {
        id,
        name: s(m("imageName")),
        description: s(m("imageDescription")),
        author: Some(s(m("imageAuthor"))).filter(|a| !a.trim().is_empty()),
        project: opt_pair("projectId", "projectName"),
        dataset: opt_pair("datasetId", "datasetName"),
        well_id: m("wellId").and_then(Value::as_u64),
        timestamp: m("imageTimestamp")
            .and_then(Value::as_f64)
            .map(|t| t as i64),
        pixels_type: s(m("pixelsType")),
        size,
        pixel_size: [px("x"), px("y"), px("z")],
        magnification: f(v.get("nominalMagnification")).filter(|&m| m > 0.0),
        channels,
        tiled,
        tile: if tiled {
            (tdim("width"), tdim("height"))
        } else {
            (0, 0)
        },
        levels: if tiled { levels } else { 1 },
        scaling: if tiled { scaling } else { vec![1.0] },
        default_z: (r("defaultZ").and_then(Value::as_u64).unwrap_or(0) as u32).min(size[2] - 1),
        default_t: (r("defaultT").and_then(Value::as_u64).unwrap_or(0) as u32).min(size[4] - 1),
        greyscale: r("model").and_then(Value::as_str) == Some("greyscale"),
    })
}

impl ImgData {
    /// The size of pyramid level `level` (0 is full size).
    pub fn level_size(&self, level: u32) -> (u32, u32) {
        let s = self.scaling.get(level as usize).copied().unwrap_or(1.0);
        let d = |n: u32| ((f64::from(n) * s).round() as u32).max(1);
        (d(self.size[0]), d(self.size[1]))
    }

    /// The level's downsampling against full size, when it is a power of two (levels of
    /// other factors are not used).
    pub fn level_factor(&self, level: u32) -> Option<u32> {
        let s = *self.scaling.get(level as usize)?;
        if s <= 0.0 {
            return None;
        }
        let f = (1.0 / s).round();
        let p = f as u32;
        ((1.0 / s - f).abs() / f < 0.02 && p.is_power_of_two()).then_some(p)
    }

    /// Z × T planes.
    pub fn planes(&self) -> u32 {
        self.size[2].saturating_mul(self.size[4])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_tiled_image() {
        let v = json!({
            "id": 5, "meta": {"imageName": "slide.svs", "imageAuthor": "Ada Lovelace",
                "projectName": "P", "projectId": 1, "datasetName": "D", "datasetId": 2,
                "imageTimestamp": 1459513800.0, "pixelsType": "uint8"},
            "tiles": true, "tile_size": {"width": 256, "height": 256}, "levels": 3,
            "zoomLevelScaling": {"0": 1.0, "1": 0.5, "2": 0.25},
            "size": {"width": 1000, "height": 700, "z": 1, "t": 1, "c": 3},
            "pixel_size": {"x": 0.25, "y": 0.25, "z": null},
            "channels": [{"label": "R", "color": "FF0000", "active": true,
                          "window": {"min": 0, "max": 255, "start": 10, "end": 200}}],
            "rdefs": {"model": "color", "defaultZ": 0, "defaultT": 0}
        });
        let d = parse(&v).unwrap();
        assert!(d.tiled);
        assert_eq!(d.tile, (256, 256));
        assert_eq!(d.level_size(1), (500, 350));
        assert_eq!(d.level_size(2), (250, 175));
        assert_eq!(d.level_factor(2), Some(4));
        assert_eq!(d.project, Some((1, "P".into())));
        assert_eq!(d.pixel_size, [Some(0.25), Some(0.25), None]);
        assert_eq!(d.channels[0].colour, [255, 0, 0]);
        assert_eq!(d.channels[0].window.start, 10.0);
    }

    #[test]
    fn refuses_nonsense() {
        assert!(parse(&json!({"id": 1, "size": {"width": 0, "height": 1}})).is_err());
        assert!(parse(&json!({"size": {"width": 1, "height": 1}})).is_err());
        assert!(
            parse(
                &json!({"id": 1, "size": {"width": 1, "height": 1, "z": 1_000_000, "t": 1_000_000}})
            )
            .is_err()
        );
        assert_eq!(parse_colour("zz"), [255, 255, 255]);
    }
}
