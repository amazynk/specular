//! Web map tile pyramids (MBTiles, PMTiles): what they hold, and the XYZ tile grid of Web
//! Mercator (EPSG:3857) their tiles sit on.
//!
//! Tiles are addressed z/x/y with y counting down from the north edge (the "XYZ" scheme);
//! MBTiles stores y from the south (TMS), which [`crate::mbtiles`] turns round.

/// What a tile's bytes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFormat {
    Png,
    Jpeg,
    WebP,
    Avif,
    /// Mapbox Vector Tile (protobuf), see [`crate::mvt`].
    Mvt,
    Unknown,
}

impl TileFormat {
    /// From an MBTiles `format` value or a PMTiles tile type.
    pub fn from_name(s: &str) -> TileFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" => TileFormat::Png,
            "jpg" | "jpeg" => TileFormat::Jpeg,
            "webp" => TileFormat::WebP,
            "avif" => TileFormat::Avif,
            "pbf" | "mvt" => TileFormat::Mvt,
            _ => TileFormat::Unknown,
        }
    }

    /// From a tile's first bytes.
    pub fn sniff(b: &[u8]) -> TileFormat {
        if b.starts_with(b"\x89PNG") {
            TileFormat::Png
        } else if b.starts_with(&[0xFF, 0xD8]) {
            TileFormat::Jpeg
        } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
            TileFormat::WebP
        } else if b.len() >= 12 && &b[4..8] == b"ftyp" {
            TileFormat::Avif
        } else {
            TileFormat::Unknown
        }
    }

    pub fn is_raster(self) -> bool {
        matches!(
            self,
            TileFormat::Png | TileFormat::Jpeg | TileFormat::WebP | TileFormat::Avif
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            TileFormat::Png => "PNG",
            TileFormat::Jpeg => "JPEG",
            TileFormat::WebP => "WebP",
            TileFormat::Avif => "AVIF",
            TileFormat::Mvt => "vector (MVT)",
            TileFormat::Unknown => "unknown",
        }
    }
}

/// What a tile pyramid says about itself.
#[derive(Clone, Debug, PartialEq)]
pub struct TileMeta {
    /// "MBTiles" or "PMTiles".
    pub container: &'static str,
    pub format: TileFormat,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// West, south, east, north in degrees (the whole world when not stated).
    pub bounds: [f64; 4],
    pub name: Option<String>,
    pub description: Option<String>,
    /// The credit line to show with the tiles (HTML stripped).
    pub attribution: Option<String>,
    /// Vector tile layer names, from the metadata's `vector_layers`.
    pub vector_layers: Vec<String>,
    /// Number of tiles, when the container says.
    pub tiles: Option<u64>,
    /// Other metadata entries (key, value), for the info panel.
    pub extra: Vec<(String, String)>,
}

impl Default for TileMeta {
    fn default() -> Self {
        TileMeta {
            container: "",
            format: TileFormat::Unknown,
            min_zoom: 0,
            max_zoom: 0,
            bounds: WORLD,
            name: None,
            description: None,
            attribution: None,
            vector_layers: Vec::new(),
            tiles: None,
            extra: Vec::new(),
        }
    }
}

/// The Web Mercator world in degrees.
pub const WORLD: [f64; 4] = [-180.0, -85.051_128_779_806_59, 180.0, 85.051_128_779_806_59];

/// Half the width of the Web Mercator world, metres.
pub const HALF: f64 = 20_037_508.342_789_244;

/// A tile's bounds in EPSG:3857 metres (min x, min y, max x, max y).
pub fn tile_bounds(z: u8, x: u32, y: u32) -> [f64; 4] {
    let n = f64::from(1u32 << z.min(31));
    let size = 2.0 * HALF / n;
    let x0 = -HALF + f64::from(x) * size;
    let y1 = HALF - f64::from(y) * size;
    [x0, y1 - size, x0 + size, y1]
}

/// The tile holding a longitude and latitude at zoom `z` (clamped to the grid).
pub fn tile_of(z: u8, lon: f64, lat: f64) -> (u32, u32) {
    let n = f64::from(1u32 << z.min(31));
    let lat = lat.clamp(WORLD[1], WORLD[3]).to_radians();
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let y = ((1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor();
    let max = n - 1.0;
    (x.clamp(0.0, max) as u32, y.clamp(0.0, max) as u32)
}

/// The tile ranges (x0..=x1, y0..=y1) covering degree bounds at zoom `z`.
pub fn range(z: u8, b: [f64; 4]) -> (u32, u32, u32, u32) {
    let (x0, y0) = tile_of(z, b[0], b[3]);
    let (x1, y1) = tile_of(z, b[2], b[1]);
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

/// Degree bounds to EPSG:3857 metres.
pub fn bounds_3857(b: [f64; 4]) -> [f64; 4] {
    let f = |lon: f64, lat: f64| {
        let lat = lat.clamp(WORLD[1], WORLD[3]).to_radians();
        (
            lon.to_radians() * 6_378_137.0,
            (std::f64::consts::FRAC_PI_4 + lat / 2.0).tan().ln() * 6_378_137.0,
        )
    };
    let (x0, y0) = f(b[0], b[1]);
    let (x1, y1) = f(b[2], b[3]);
    [x0, y0, x1, y1]
}

/// HTML attribution text as plain text: tags dropped, the usual entities decoded.
pub fn plain_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&copy;", "©")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Names of the vector layers in a TileJSON-style `vector_layers` metadata value.
pub fn vector_layer_names(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    v.get("vector_layers")
        .and_then(|l| l.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|l| l.get("id")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A store of tiles: MBTiles or PMTiles.
pub trait TileStore: Send + Sync {
    fn meta(&self) -> &TileMeta;
    /// Tile z/x/y (XYZ scheme), uncompressed; `Ok(None)` when absent.
    fn tile(&self, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>, String>;
    /// The tiles present at zoom `z` within x0..=x1, y0..=y1, at most `max`.
    fn tiles_in(
        &self,
        z: u8,
        (x0, y0, x1, y1): (u32, u32, u32, u32),
        max: usize,
    ) -> Result<Vec<(u32, u32, Vec<u8>)>, String> {
        let mut out = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if out.len() >= max {
                    return Ok(out);
                }
                if let Some(t) = self.tile(z, x, y)? {
                    out.push((x, y, t));
                }
            }
        }
        Ok(out)
    }
}

/// Undo gzip (or zlib) compression of a tile or directory, bounded to `limit` bytes.
pub fn gunzip(b: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    let r = if b.starts_with(&[0x1F, 0x8B]) {
        flate2::read::MultiGzDecoder::new(b)
            .take(limit as u64 + 1)
            .read_to_end(&mut out)
    } else {
        flate2::read::ZlibDecoder::new(b)
            .take(limit as u64 + 1)
            .read_to_end(&mut out)
    };
    r.map_err(|e| format!("compressed tile data: {e}"))?;
    if out.len() > limit {
        return Err("a tile larger than 64 MB".into());
    }
    Ok(out)
}

/// The most a tile may expand to.
pub const TILE_LIMIT: usize = 64 << 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid() {
        assert_eq!(tile_of(0, 10.0, 50.0), (0, 0));
        assert_eq!(tile_of(1, 10.0, 50.0), (1, 0));
        assert_eq!(tile_of(1, -10.0, -50.0), (0, 1));
        let b = tile_bounds(1, 1, 0);
        assert!((b[0]).abs() < 1e-6 && (b[3] - HALF).abs() < 1e-6);
        assert_eq!(range(2, WORLD), (0, 0, 3, 3));
        assert_eq!(
            plain_text(r#"<a href="x">&copy; OpenStreetMap</a>  contributors"#),
            "© OpenStreetMap contributors"
        );
    }
}
