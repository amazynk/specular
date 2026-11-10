//! Raster base maps over the offline vector map: which tiles a camera needs, where each
//! goes on screen, and the open tile sources Specular knows by name.
//!
//! Nothing here fetches anything. Online base maps are off unless the user turns them on;
//! the app explains what the tile server sees before the first request and fetches through
//! `specular-remote`. Tiles can also come from the user's own MBTiles or PMTiles file.

use crate::{Camera, MAX_ZOOM, TILE};

/// An online tile source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    /// Its id in settings.
    pub id: &'static str,
    pub name: &'static str,
    /// URL template with `{z}`, `{x}` and `{y}`.
    pub url: &'static str,
    /// The credit its terms ask for, shown while its tiles are on screen.
    pub attribution: &'static str,
    pub max_zoom: u8,
}

/// OpenStreetMap's standard tiles (data under the ODbL; the tile servers' usage policy asks
/// for this credit, a cache, and no bulk downloading).
pub const OPENSTREETMAP: Provider = Provider {
    id: "openstreetmap",
    name: "OpenStreetMap",
    url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    attribution: "© OpenStreetMap contributors",
    max_zoom: 19,
};

/// A tile address: zoom, column, row (row 0 at the north edge).
pub type TileKey = (u8, u32, u32);

/// The URL of a tile from a template (`{z}`, `{x}`, `{y}`; `{-y}` counts rows from the
/// south as TMS servers do). `None` unless it is an http(s) URL with all three.
pub fn tile_url(template: &str, (z, x, y): TileKey) -> Option<String> {
    let t = template.trim();
    let lower = t.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return None;
    }
    let tms = t.contains("{-y}");
    if !(t.contains("{z}") && t.contains("{x}") && (t.contains("{y}") || tms)) {
        return None;
    }
    let row = if tms {
        (1u32 << z.min(31)).saturating_sub(1).saturating_sub(y)
    } else {
        y
    };
    Some(
        t.replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{-y}", &row.to_string())
            .replace("{y}", &y.to_string()),
    )
}

/// Whether a template is usable (http or https with `{z}`, `{x}` and `{y}`).
pub fn valid_template(template: &str) -> bool {
    tile_url(template, (0, 0, 0)).is_some()
}

/// The host a URL goes to ("tile.openstreetmap.org"), lower case.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?.1;
    let auth = rest.split(['/', '?', '#']).next()?;
    let host = auth.rsplit('@').next()?;
    let host = if host.starts_with('[') {
        host.split(']').next()?.trim_start_matches('[')
    } else {
        host.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The tile zoom level for a camera: one tile pixel per device pixel or finer, at most
/// `max_zoom`.
pub fn level(cam: &Camera, device_scale: f64, max_zoom: u8) -> u8 {
    let z = cam.zoom + device_scale.max(0.5).log2();
    z.round()
        .clamp(0.0, f64::from(max_zoom.min(MAX_ZOOM as u8 + 2))) as u8
}

/// A tile to draw: its address and its rectangle on screen (logical px from the view's top
/// left: x, y, w, h).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub key: TileKey,
    pub rect: (f64, f64, f64, f64),
}

/// The tiles at level `z` covering the camera's view, with copies of the world east and
/// west wrapped to their column, nearest the centre first.
pub fn visible(cam: &Camera, z: u8) -> Vec<Placed> {
    let n = 1u32 << z.min(30);
    let nf = f64::from(n);
    let (x0, y0, x1, y1) = cam.visible();
    let tx0 = (x0 * nf).floor() as i64;
    let tx1 = (x1 * nf).ceil() as i64;
    let ty0 = (y0.clamp(0.0, 1.0) * nf).floor() as i64;
    let ty1 = (y1.clamp(0.0, 1.0) * nf).ceil() as i64;
    let s = cam.scale() / nf;
    let mut out = Vec::new();
    // At most a screenful of tiles plus the world's width (guards an extreme aspect).
    let cols = (tx1 - tx0).clamp(0, i64::from(n) + 2);
    for ty in ty0..ty1.min(i64::from(n)) {
        for tx in tx0..tx0 + cols {
            let wx = tx as f64 / nf;
            let wy = ty as f64 / nf;
            // Screen position of this copy: relative to the centre, not wrapped.
            let sx = (wx - cam.center.0) * cam.scale() + cam.size.0 / 2.0;
            let sy = (wy - cam.center.1) * cam.scale() + cam.size.1 / 2.0;
            out.push(Placed {
                key: (z, tx.rem_euclid(i64::from(n)) as u32, ty as u32),
                rect: (sx, sy, s, s),
            });
        }
    }
    let (cx, cy) = (cam.size.0 / 2.0, cam.size.1 / 2.0);
    out.sort_by(|a, b| {
        let d = |p: &Placed| {
            let (x, y) = (
                p.rect.0 + p.rect.2 / 2.0 - cx,
                p.rect.1 + p.rect.3 / 2.0 - cy,
            );
            x * x + y * y
        };
        d(a).total_cmp(&d(b))
    });
    out
}

/// The ancestor of a tile `up` levels up, and the part of it the tile covers (u0, v0, u1,
/// v1 in 0..1), to draw while the tile itself is not in.
pub fn ancestor((z, x, y): TileKey, up: u8) -> Option<(TileKey, [f32; 4])> {
    if up == 0 || up > z {
        return None;
    }
    let k = 1u32 << up;
    let (px, py) = (x >> up, y >> up);
    let (ox, oy) = (x - (px << up), y - (py << up));
    let f = 1.0 / k as f32;
    Some((
        (z - up, px, py),
        [
            ox as f32 * f,
            oy as f32 * f,
            (ox + 1) as f32 * f,
            (oy + 1) as f32 * f,
        ],
    ))
}

/// Tile pixels at zoom 0 (the usual web map tile).
pub const TILE_PX: f64 = TILE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates() {
        assert_eq!(
            tile_url(OPENSTREETMAP.url, (3, 4, 2)).as_deref(),
            Some("https://tile.openstreetmap.org/3/4/2.png")
        );
        assert_eq!(
            tile_url("http://h/{z}/{x}/{-y}.png", (2, 1, 0)).as_deref(),
            Some("http://h/2/1/3.png")
        );
        assert!(!valid_template("file:///etc/{z}/{x}/{y}"));
        assert!(!valid_template("https://h/{z}/{x}"));
        assert_eq!(
            host_of("https://user@Tile.Example.org:8443/a/{z}").as_deref(),
            Some("tile.example.org")
        );
        assert_eq!(host_of("http://[::1]:80/x").as_deref(), Some("::1"));
    }

    #[test]
    fn tiles_cover_the_view() {
        let mut cam = Camera::new((800.0, 600.0));
        cam.zoom = 3.0;
        cam.center = (0.5, 0.5);
        let z = level(&cam, 1.0, 19);
        assert_eq!(z, 3);
        let v = visible(&cam, z);
        // 2048 px of world, 800 × 600 px of view: 256-px tiles, 4 (or 5) × 3 (or 4).
        assert!((12..=20).contains(&v.len()), "{}", v.len());
        let first = v[0];
        assert!(first.rect.2 > 255.9 && first.rect.2 < 256.1);
        // Every tile overlaps the view.
        for p in &v {
            let (x, y, w, h) = p.rect;
            assert!(
                x < 800.0 && y < 600.0 && x + w > 0.0 && y + h > 0.0,
                "{p:?}"
            );
        }
        // Across the antimeridian: columns wrap.
        cam.center = (0.001, 0.5);
        let v = visible(&cam, 3);
        assert!(v.iter().any(|p| p.key.1 == 7) && v.iter().any(|p| p.key.1 == 0));
        assert_eq!(level(&cam, 2.0, 19), 4);
        assert_eq!(level(&cam, 1.0, 2), 2);
    }

    #[test]
    fn ancestors() {
        assert_eq!(
            ancestor((3, 5, 6), 1),
            Some(((2, 2, 3), [0.5, 0.0, 1.0, 0.5]))
        );
        assert_eq!(ancestor((1, 1, 1), 2), None);
    }
}
