//! HiPS sky surveys (IVOA HiPS 1.0): the `properties` file, the tile order for a zoom, tile
//! paths, and painting tiles into any view whose pixels have sky positions.
//!
//! A HiPS tile of order `k` and index `n` is the HEALPix NESTED cell `n` at `Nside = 2^k`,
//! cut into `w × w` pixels (`w = 2^m`) that are the cells of order `k + m` inside it. In a
//! JPEG or PNG tile (rows top-down) the pixel at column `c`, row `r` is the sub-cell whose
//! face coordinates are `ix = r`, `iy = c`: north is the bottom-right corner, east the
//! bottom-left. FITS tiles store rows bottom-up, so once read top-down they follow the same
//! rule.

use rayon::prelude::*;
use specular_fits::healpix::ang2pix_nest;

use crate::frames::{Rotation, Sky};

/// The deepest tile order read.
pub const MAX_ORDER: u32 = 24;

/// A tile image format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    Jpeg,
    Png,
}

impl TileFormat {
    pub fn ext(self) -> &'static str {
        match self {
            TileFormat::Jpeg => "jpg",
            TileFormat::Png => "png",
        }
    }
}

/// What a survey's `properties` file says.
#[derive(Clone, Debug, PartialEq)]
pub struct Properties {
    pub title: String,
    pub creator: String,
    pub copyright: String,
    pub copyright_url: String,
    /// The deepest and shallowest tile orders.
    pub max_order: u32,
    pub min_order: u32,
    /// Tile width in pixels (a power of two).
    pub tile_width: u32,
    /// The image format used (PNG preferred over JPEG, which has no transparency).
    pub format: TileFormat,
    pub frame: Sky,
}

/// Why a survey cannot be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A catalogue or cube, not an image survey.
    NotImage(String),
    /// Tiles only in a format this viewer does not paint (FITS only, for one).
    Format(String),
    /// A frame other than equatorial or galactic.
    Frame(String),
    /// No usable order or tile width.
    Malformed(&'static str),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NotImage(k) => write!(f, "the survey holds {k} data, not images"),
            Refused::Format(k) => write!(f, "its tiles are only in {k}, which is not shown yet"),
            Refused::Frame(k) => write!(f, "its {k} coordinate frame is not supported"),
            Refused::Malformed(k) => write!(f, "its properties have no usable {k}"),
        }
    }
}

/// Reads a `properties` file (`key = value` lines, `#` comments).
pub fn parse_properties(text: &str) -> Result<Properties, Refused> {
    let kv: Vec<(&str, &str)> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let get = |k: &str| kv.iter().rev().find(|(key, _)| *key == k).map(|(_, v)| *v);
    if let Some(kind) = get("dataproduct_type")
        && !kind.eq_ignore_ascii_case("image")
    {
        return Err(Refused::NotImage(kind.to_owned()));
    }
    let max_order = get("hips_order")
        .or(get("maxOrder"))
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&o| o <= MAX_ORDER)
        .ok_or(Refused::Malformed("order"))?;
    let min_order = get("hips_order_min")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
        .min(max_order);
    let tile_width = get("hips_tile_width")
        .map_or(Some(512), |v| v.parse::<u32>().ok())
        .filter(|w| w.is_power_of_two() && (2..=4096).contains(w))
        .ok_or(Refused::Malformed("tile width"))?;
    let formats = get("hips_tile_format")
        .or(get("format"))
        .unwrap_or("jpeg")
        .to_ascii_lowercase();
    let words: Vec<&str> = formats
        .split([' ', ','])
        .filter(|w| !w.is_empty())
        .collect();
    let format = if words.contains(&"png") {
        TileFormat::Png
    } else if words.iter().any(|w| matches!(*w, "jpeg" | "jpg")) {
        TileFormat::Jpeg
    } else {
        return Err(Refused::Format(formats));
    };
    let frame_word = get("hips_frame")
        .or(get("coordsys"))
        .unwrap_or("equatorial")
        .to_ascii_lowercase();
    let frame = match frame_word.as_str() {
        "equatorial" | "c" | "icrs" => Sky::Icrs,
        "galactic" | "g" => Sky::Galactic,
        _ => return Err(Refused::Frame(frame_word)),
    };
    let s = |k: &str| get(k).unwrap_or("").to_owned();
    Ok(Properties {
        title: s("obs_title"),
        creator: s("hips_creator"),
        copyright: s("obs_copyright"),
        copyright_url: s("obs_copyright_url"),
        max_order,
        min_order,
        tile_width,
        format,
        frame,
    })
}

/// The tile's path under the survey's address: `Norder3/Dir0/Npix123.jpg`.
pub fn tile_path(order: u32, ipix: u64, format: TileFormat) -> String {
    format!(
        "Norder{order}/Dir{}/Npix{ipix}.{}",
        ipix / 10_000 * 10_000,
        format.ext()
    )
}

/// The side of a HEALPix cell of `order`, in degrees (the square root of its area).
pub fn cell_size(order: u32) -> f64 {
    (std::f64::consts::PI / 3.0).sqrt().to_degrees() / 2f64.powi(order.min(60) as i32)
}

impl Properties {
    fn shift(&self) -> u32 {
        self.tile_width.trailing_zeros()
    }

    /// The tile order whose pixels are no larger than `deg_per_px` (the view's pixel), within
    /// the survey's orders.
    pub fn order_for(&self, deg_per_px: f64) -> u32 {
        let m = self.shift();
        let mut k = self.min_order;
        while k < self.max_order && cell_size(k + m) > deg_per_px {
            k += 1;
        }
        k
    }
}

/// Where a sky position falls: the tile of `order` and the pixel in it (column, row,
/// top-down). `lng`, `lat` in degrees in the survey's frame.
pub fn locate(order: u32, tile_width: u32, lng: f64, lat: f64) -> (u64, u32, u32) {
    let m = tile_width.trailing_zeros();
    let nside = 1u64 << (order + m);
    let theta = (90.0 - lat).to_radians();
    let phi = lng.to_radians().rem_euclid(std::f64::consts::TAU);
    let p = ang2pix_nest(nside, theta.clamp(0.0, std::f64::consts::PI), phi);
    let sub = p & ((1u64 << (2 * m)) - 1);
    let (ix, iy) = (compact(sub), compact(sub >> 1));
    (p >> (2 * m), iy as u32, ix as u32)
}

/// The even bits of `v`, packed.
fn compact(mut v: u64) -> u64 {
    v &= 0x5555_5555_5555_5555;
    v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
    v = (v | (v >> 2)) & 0x0f0f_0f0f_0f0f_0f0f;
    v = (v | (v >> 4)) & 0x00ff_00ff_00ff_00ff;
    v = (v | (v >> 8)) & 0x0000_ffff_0000_ffff;
    (v | (v >> 16)) & 0x0000_0000_ffff_ffff
}

/// A tile's RGBA pixels, `tile_width²`, rows top-down.
pub type TileRgba<'a> = &'a [u8];

/// Paints the survey into `out` (`w × h` RGBA, rows top-down): `sky(x, y)` gives the
/// position (degrees, in `frame`) at the centre of output pixel `(x, y)`, `None` where
/// nothing is to be painted (left transparent). `tile(order, ipix)` gives a loaded tile;
/// where the tile of `order` is missing, a coarser one (up to 3 orders up) stands in.
/// Returns the tiles of `order` that were wanted and missing, without repeats.
#[allow(clippy::too_many_arguments)]
pub fn paint<'t, S, T>(
    out: &mut [u8],
    w: usize,
    h: usize,
    frame: Sky,
    props: &Properties,
    order: u32,
    sky: S,
    tile: T,
) -> Vec<u64>
where
    S: Fn(usize, usize) -> Option<(f64, f64, Sky)> + Sync,
    T: Fn(u32, u64) -> Option<TileRgba<'t>> + Sync,
{
    let width = props.tile_width;
    let tw = width as usize;
    let rot_icrs = Rotation::new(Sky::Icrs, frame);
    let rot_fk5 = Rotation::new(Sky::Fk5, frame);
    let rot_fk4 = Rotation::new(Sky::Fk4, frame);
    let rot_gal = Rotation::new(Sky::Galactic, frame);
    let missing: Vec<Vec<u64>> = out
        .par_chunks_mut(w * 4)
        .take(h)
        .enumerate()
        .map(|(y, row)| {
            let mut miss = Vec::new();
            let mut last: Option<u64> = None;
            for x in 0..w {
                let px = &mut row[x * 4..x * 4 + 4];
                px.copy_from_slice(&[0, 0, 0, 0]);
                let Some((lng, lat, from)) = sky(x, y) else {
                    continue;
                };
                let (lng, lat) = match from {
                    Sky::Icrs => rot_icrs.apply(lng, lat),
                    Sky::Fk5 => rot_fk5.apply(lng, lat),
                    Sky::Fk4 => rot_fk4.apply(lng, lat),
                    Sky::Galactic => rot_gal.apply(lng, lat),
                };
                let mut k = order;
                loop {
                    let (n, c, r) = locate(k, width, lng, lat);
                    if let Some(t) = tile(k, n) {
                        let i = (r as usize * tw + c as usize) * 4;
                        if let Some(src) = t.get(i..i + 4) {
                            px.copy_from_slice(src);
                        }
                        break;
                    }
                    if k == order && last != Some(n) {
                        miss.push(n);
                        last = Some(n);
                    }
                    if k == 0 || order - k >= 3 || k <= props.min_order {
                        break;
                    }
                    k -= 1;
                }
            }
            miss
        })
        .collect();
    let mut all: Vec<u64> = missing.into_iter().flatten().collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// A well-known image survey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Survey {
    /// Stable key for settings.
    pub id: &'static str,
    pub name: &'static str,
    /// The survey's address (no trailing slash).
    pub url: &'static str,
    /// The credit its provider asks for.
    pub credit: &'static str,
}

/// Surveys offered by name, all served by CDS (Strasbourg).
pub const SURVEYS: &[Survey] = &[
    Survey {
        id: "dss2-colour",
        name: "DSS2 colour",
        url: "https://alasky.cds.unistra.fr/DSS/DSSColor",
        credit: "Digitized Sky Survey: STScI/NASA; colour and HiPS by CDS",
    },
    Survey {
        id: "2mass-colour",
        name: "2MASS colour",
        url: "https://alasky.cds.unistra.fr/2MASS/Color",
        credit: "2MASS: University of Massachusetts and IPAC/Caltech, NASA and NSF; HiPS by CDS",
    },
    Survey {
        id: "panstarrs-dr1-colour",
        name: "Pan-STARRS DR1 colour",
        url: "https://alasky.cds.unistra.fr/Pan-STARRS/DR1/color-z-zg-g",
        credit: "Pan-STARRS1 Surveys: PS1 Science Consortium, STScI/MAST; HiPS by CDS",
    },
    Survey {
        id: "sdss9-colour",
        name: "SDSS9 colour",
        url: "https://alasky.cds.unistra.fr/SDSS/DR9/color",
        credit: "Sloan Digital Sky Survey (SDSS-III); colour and HiPS by CDS",
    },
];

#[cfg(test)]
mod tests;
