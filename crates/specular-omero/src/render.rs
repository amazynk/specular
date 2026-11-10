//! Rendering settings and the webgateway URLs that apply them: the `c=` channel string
//! (`1|0:255$FF0000,-2|…`: index from 1, negative when off, window and colour), the model
//! (`m=c` colour, `m=g` greyscale), and the tile grid of a pyramid level.

use crate::image::ImgData;

/// How a channel is rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelRender {
    pub active: bool,
    pub start: f64,
    pub end: f64,
    pub colour: [u8; 3],
}

/// What is asked of the server per image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Render {
    pub channels: Vec<ChannelRender>,
    pub greyscale: bool,
}

/// How rendered pixels come: JPEG (small, lossy; the pyramid's tiles) or PNG (exact).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Jpeg,
    Png,
}

impl Encoding {
    pub fn id(self) -> &'static str {
        match self {
            Encoding::Jpeg => "jpeg",
            Encoding::Png => "png",
        }
    }
    pub fn from_id(s: &str) -> Option<Encoding> {
        match s {
            "jpeg" | "jpg" => Some(Encoding::Jpeg),
            "png" => Some(Encoding::Png),
            _ => None,
        }
    }
    /// The query part selecting it (`format=jpeg&q=0.9`).
    fn query(self) -> &'static str {
        match self {
            Encoding::Jpeg => "format=jpeg&q=0.9",
            Encoding::Png => "format=png",
        }
    }
}

/// A number as the channel string wants it: integers without a fraction.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Characters of the channel string escaped in a query (`|`, `$`, `:`, `,`).
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '|' => out.push_str("%7C"),
            '$' => out.push_str("%24"),
            ':' => out.push_str("%3A"),
            ',' => out.push_str("%2C"),
            c => out.push(c),
        }
    }
    out
}

impl Render {
    /// The image's saved rendering.
    pub fn of(img: &ImgData) -> Render {
        Render {
            channels: img
                .channels
                .iter()
                .map(|c| ChannelRender {
                    active: c.active,
                    start: c.window.start,
                    end: c.window.end,
                    colour: c.colour,
                })
                .collect(),
            greyscale: img.greyscale,
        }
    }

    /// The `c=` value, unescaped: `1|0:255$FF0000,-2|0:4095$00FF00`.
    pub fn channel_string(&self) -> String {
        self.channels
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let [r, g, b] = c.colour;
                format!(
                    "{}{}|{}:{}${r:02X}{g:02X}{b:02X}",
                    if c.active { "" } else { "-" },
                    i + 1,
                    num(c.start),
                    num(c.end)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The query's rendering part: `c=…&m=c`.
    pub fn query(&self) -> String {
        let m = if self.greyscale { "g" } else { "c" };
        if self.channels.is_empty() {
            format!("m={m}")
        } else {
            format!("c={}&m={m}", escape(&self.channel_string()))
        }
    }

    /// A short stable hash of the rendering (cache keys).
    pub fn hash(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.query().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// Turns channel `index` (from 0) on or off; false when there is no such channel.
    pub fn toggle(&mut self, index: usize) -> bool {
        match self.channels.get_mut(index) {
            Some(c) => {
                c.active = !c.active;
                true
            }
            None => false,
        }
    }
}

/// A tile of a level: its column and row, and its size (the edge tiles are cut).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub level: u32,
    pub col: u32,
    pub row: u32,
    /// Where it sits in the level, and its size there.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The tiles of one level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub tile_w: u32,
    pub tile_h: u32,
}

impl Grid {
    pub fn cols(&self) -> u32 {
        self.width.div_ceil(self.tile_w)
    }
    pub fn rows(&self) -> u32 {
        self.height.div_ceil(self.tile_h)
    }
    pub fn tile(&self, col: u32, row: u32) -> Tile {
        let x = col * self.tile_w;
        let y = row * self.tile_h;
        Tile {
            level: self.level,
            col,
            row,
            x,
            y,
            w: self.tile_w.min(self.width.saturating_sub(x)),
            h: self.tile_h.min(self.height.saturating_sub(y)),
        }
    }
    /// The tiles covering `x0..x1 × y0..y1` (level pixels, clipped to the level), row by row.
    pub fn covering(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> Vec<Tile> {
        let (x1, y1) = (x1.min(self.width), y1.min(self.height));
        if x0 >= x1 || y0 >= y1 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for row in y0 / self.tile_h..y1.div_ceil(self.tile_h) {
            for col in x0 / self.tile_w..x1.div_ceil(self.tile_w) {
                out.push(self.tile(col, row));
            }
        }
        out
    }
}

/// `render_image_region` for one tile of plane `z`, `t`. OMERO counts levels from full size
/// as 0 and tiles by index; we ask for the full tile size and the server cuts edge tiles.
pub fn tile_url(
    image: u64,
    z: u32,
    t: u32,
    grid: &Grid,
    tile: &Tile,
    render: &Render,
    enc: Encoding,
) -> String {
    format!(
        "/webgateway/render_image_region/{image}/{z}/{t}/?tile={},{},{},{},{}&{}&{}",
        tile.level,
        tile.col,
        tile.row,
        grid.tile_w,
        grid.tile_h,
        render.query(),
        enc.query()
    )
}

/// `render_image` for a whole plane (images without a pyramid).
pub fn image_url(image: u64, z: u32, t: u32, render: &Render, enc: Encoding) -> String {
    format!(
        "/webgateway/render_image/{image}/{z}/{t}/?{}&{}",
        render.query(),
        enc.query()
    )
}

/// `render_thumbnail` with the saved rendering, longest side `size` (OMERO's default 96).
pub fn thumbnail_url(image: u64, size: u32) -> String {
    format!("/webgateway/render_thumbnail/{image}/{size}/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Render {
        Render {
            channels: vec![
                ChannelRender {
                    active: true,
                    start: 0.0,
                    end: 255.0,
                    colour: [255, 0, 0],
                },
                ChannelRender {
                    active: false,
                    start: 12.5,
                    end: 4095.0,
                    colour: [0, 255, 0],
                },
            ],
            greyscale: false,
        }
    }

    #[test]
    fn channel_strings() {
        let mut r = two();
        assert_eq!(r.channel_string(), "1|0:255$FF0000,-2|12.5:4095$00FF00");
        assert_eq!(
            r.query(),
            "c=1%7C0%3A255%24FF0000%2C-2%7C12.5%3A4095%2400FF00&m=c"
        );
        let h = r.hash();
        assert!(r.toggle(1));
        assert!(!r.toggle(5));
        assert_eq!(r.channel_string(), "1|0:255$FF0000,2|12.5:4095$00FF00");
        assert_ne!(r.hash(), h);
        r.greyscale = true;
        assert!(r.query().ends_with("&m=g"));
        assert_eq!(num(-3.0), "-3");
        assert_eq!(num(0.1), "0.1");
    }

    #[test]
    fn grids_cut_edge_tiles() {
        let g = Grid {
            level: 1,
            width: 500,
            height: 350,
            tile_w: 256,
            tile_h: 256,
        };
        assert_eq!((g.cols(), g.rows()), (2, 2));
        let t = g.tile(1, 1);
        assert_eq!((t.x, t.y, t.w, t.h), (256, 256, 244, 94));
        let c = g.covering(250, 0, 260, 10);
        assert_eq!(c.iter().map(|t| t.col).collect::<Vec<_>>(), [0, 1]);
        assert!(g.covering(500, 0, 600, 10).is_empty());
        assert_eq!(
            tile_url(9, 2, 3, &g, &t, &two(), Encoding::Png),
            "/webgateway/render_image_region/9/2/3/?tile=1,1,1,256,256&c=1%7C0%3A255%24FF0000%2C-2%7C12.5%3A4095%2400FF00&m=c&format=png"
        );
    }
}
