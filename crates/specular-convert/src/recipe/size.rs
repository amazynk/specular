//! Output size: the resize modes of a recipe and the arithmetic behind them.
//!
//! Every mode keeps the aspect ratio except [`Size::Exact`], which fills its box and cuts
//! the overflow equally from both sides. Without "enlarge", a picture is never made larger:
//! the scale stops at 1 (an exact box then keeps its aspect, at the picture's own size).

/// How big the exported picture is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Full,
    /// The longer side becomes this long.
    LongEdge(u32),
    /// The shorter side becomes this long.
    ShortEdge(u32),
    /// About this many pixels, in tenths of a megapixel (24 = 2.4 MP).
    Megapixels(u32),
    /// Exactly this wide and high, the picture filling the box and cropped to its aspect.
    Exact {
        width: u32,
        height: u32,
    },
    Percent(u32),
}

/// Upper bounds of the pixel values (a side), the megapixels (in tenths) and percent.
pub const MAX_SIDE: u32 = 65_000;
pub const MAX_TENTHS: u32 = 4_000;
pub const MAX_PERCENT: u32 = 400;

/// What [`target`] decides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// Output size.
    pub width: u32,
    pub height: u32,
    /// The part of the source used, as fractions of its width and height: x, y, w, h.
    pub crop: [f64; 4],
}

impl Size {
    pub fn normalised(self) -> Size {
        let side = |n: u32| n.clamp(1, MAX_SIDE);
        match self {
            Size::Full => Size::Full,
            Size::LongEdge(n) => Size::LongEdge(side(n)),
            Size::ShortEdge(n) => Size::ShortEdge(side(n)),
            Size::Megapixels(t) => Size::Megapixels(t.clamp(1, MAX_TENTHS)),
            Size::Exact { width, height } => Size::Exact {
                width: side(width),
                height: side(height),
            },
            Size::Percent(p) => Size::Percent(p.clamp(1, MAX_PERCENT)),
        }
    }

    /// `full`, `long_edge 2048`, `short_edge 1080`, `megapixels 2.4`, `exact 1080 1350`,
    /// `percent 50`.
    pub fn to_text(self) -> String {
        match self {
            Size::Full => "full".into(),
            Size::LongEdge(n) => format!("long_edge {n}"),
            Size::ShortEdge(n) => format!("short_edge {n}"),
            Size::Megapixels(t) => format!("megapixels {}.{}", t / 10, t % 10),
            Size::Exact { width, height } => format!("exact {width} {height}"),
            Size::Percent(p) => format!("percent {p}"),
        }
    }

    pub fn parse(s: &str) -> Option<Size> {
        let num = |s: &str| {
            s.parse::<u32>()
                .ok()
                .filter(|&n| (1..=MAX_SIDE).contains(&n))
        };
        let mut it = s.split_whitespace();
        let size = match (it.next(), it.next(), it.next()) {
            (Some("full"), None, None) => Size::Full,
            (Some("long_edge"), Some(n), None) => Size::LongEdge(num(n)?),
            (Some("short_edge"), Some(n), None) => Size::ShortEdge(num(n)?),
            (Some("megapixels"), Some(n), None) => {
                let t = (n.parse::<f64>().ok()? * 10.0).round();
                if !(1.0..=f64::from(MAX_TENTHS)).contains(&t) {
                    return None;
                }
                Size::Megapixels(t as u32)
            }
            (Some("exact"), Some(w), Some(h)) => Size::Exact {
                width: num(w)?,
                height: num(h)?,
            },
            (Some("percent"), Some(n), None) => {
                Size::Percent(num(n).filter(|&p| p <= MAX_PERCENT)?)
            }
            _ => return None,
        };
        it.next().is_none().then_some(size)
    }
}

/// The output of a `w × h` (upright) picture.
pub fn target(w: u32, h: u32, size: Size, enlarge: bool) -> Target {
    let (w, h) = (w.max(1), h.max(1));
    let (wf, hf) = (f64::from(w), f64::from(h));
    let whole = [0.0, 0.0, 1.0, 1.0];
    let scaled = |s: f64| {
        let s = if enlarge { s } else { s.min(1.0) };
        (
            ((wf * s).round() as u32).max(1),
            ((hf * s).round() as u32).max(1),
        )
    };
    let (width, height, crop) = match size.normalised() {
        Size::Full => (w, h, whole),
        Size::LongEdge(n) => {
            let s = f64::from(n) / wf.max(hf);
            let (mut ow, mut oh) = scaled(s);
            // The named side exactly, not off by a rounding.
            if s <= 1.0 || enlarge {
                if w >= h {
                    ow = n;
                } else {
                    oh = n;
                }
            }
            (ow, oh, whole)
        }
        Size::ShortEdge(n) => {
            let s = f64::from(n) / wf.min(hf);
            let (mut ow, mut oh) = scaled(s);
            if s <= 1.0 || enlarge {
                if w <= h {
                    ow = n;
                } else {
                    oh = n;
                }
            }
            (ow, oh, whole)
        }
        Size::Megapixels(t) => {
            let s = (f64::from(t) * 1e5 / (wf * hf)).sqrt();
            let (ow, oh) = scaled(s);
            (ow, oh, whole)
        }
        Size::Percent(p) => {
            let (ow, oh) = scaled(f64::from(p) / 100.0);
            (ow, oh, whole)
        }
        Size::Exact { width, height } => {
            let (bw, bh) = (f64::from(width), f64::from(height));
            // The crop of the source with the box's aspect, as large as fits.
            let aspect = bw / bh;
            let (cw, ch) = if wf / hf > aspect {
                (hf * aspect, hf)
            } else {
                (wf, wf / aspect)
            };
            let crop = [(wf - cw) * 0.5 / wf, (hf - ch) * 0.5 / hf, cw / wf, ch / hf];
            let s = bw / cw;
            if s > 1.0 && !enlarge {
                // The crop at the source's own size.
                ((cw.round() as u32).max(1), (ch.round() as u32).max(1), crop)
            } else {
                (width, height, crop)
            }
        }
    };
    Target {
        width,
        height,
        crop,
    }
}

/// The smallest source size (upright) that serves every target of `sizes` without
/// enlarging it: the largest of the needs, as a fraction of the full size (1.0: full).
pub fn needed_scale(w: u32, h: u32, sizes: &[(Size, bool)]) -> f64 {
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    sizes
        .iter()
        .map(|&(s, enlarge)| {
            let t = target(w, h, s, enlarge);
            let sx = f64::from(t.width) / (wf * t.crop[2]);
            let sy = f64::from(t.height) / (hf * t.crop[3]);
            sx.max(sy)
        })
        .fold(0.0f64, f64::max)
        .min(1.0)
}
