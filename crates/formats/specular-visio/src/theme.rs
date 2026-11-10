//! The drawing's theme (`visio/theme/theme1.xml`): colours, the variant colour and style
//! schemes, the fill, line and font style matrices that "Themed" cells and quick styles
//! resolve through, and the theme fonts.

use specular_drawpage::Rgba;
use specular_drawpage::xml::roxmltree::{Document, Node};

#[derive(Debug, Clone)]
pub(crate) enum Spec {
    Srgb([u8; 3], Vec<(String, f64)>),
    Scheme(String, Vec<(String, f64)>),
}

#[derive(Debug, Clone)]
pub(crate) enum FillStyle {
    None,
    Solid(Spec),
    /// Stops (offset 0–1, colour) and the angle in degrees (0: left to right, clockwise).
    Grad(Vec<(f32, Spec)>, f64),
}

#[derive(Debug, Clone)]
pub(crate) struct LineStyle {
    /// Points.
    pub width: Option<f64>,
    pub fill: FillStyle,
    pub dash: Option<String>,
}

/// Style indices (1-based) of one variant style.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct VarStyle {
    pub fill: usize,
    pub line: usize,
    pub font: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Scheme {
    pub fills: Vec<FillStyle>,
    pub lines: Vec<LineStyle>,
    pub fonts: Vec<Option<Spec>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Theme {
    /// dk1, lt1, dk2, lt2, accent1–6, hlink, folHlink.
    pub colors: Vec<(String, [u8; 3])>,
    pub variants: Vec<Vec<[u8; 3]>>,
    pub var_styles: Vec<Vec<VarStyle>>,
    pub shapes: Scheme,
    pub connectors: Scheme,
    pub minor_font: Option<String>,
    pub major_font: Option<String>,
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn hex3(s: &str) -> Option<[u8; 3]> {
    let s = s.trim();
    if s.len() != 6 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([v(0)?, v(2)?, v(4)?])
}

fn mods(n: Node) -> Vec<(String, f64)> {
    n.children()
        .filter(Node::is_element)
        .filter_map(|m| {
            let v: f64 = m.attribute("val")?.parse().ok()?;
            Some((m.tag_name().name().to_string(), v / 100_000.0))
        })
        .collect()
}

/// A colour element's spec: the first `srgbClr`, `schemeClr` or `sysClr` child of `n`.
fn spec_in(n: Node) -> Option<Spec> {
    for c in n.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "srgbClr" => return Some(Spec::Srgb(hex3(c.attribute("val")?)?, mods(c))),
            "schemeClr" => return Some(Spec::Scheme(c.attribute("val")?.to_string(), mods(c))),
            "sysClr" => {
                let rgb = c.attribute("lastClr").and_then(hex3).unwrap_or(
                    if c.attribute("val") == Some("window") {
                        [255, 255, 255]
                    } else {
                        [0, 0, 0]
                    },
                );
                return Some(Spec::Srgb(rgb, mods(c)));
            }
            _ => {}
        }
    }
    None
}

fn fill_of(n: Node) -> FillStyle {
    match n.tag_name().name() {
        "solidFill" => spec_in(n).map_or(FillStyle::None, FillStyle::Solid),
        "gradFill" => {
            let mut stops = Vec::new();
            if let Some(l) = child(n, "gsLst") {
                for gs in l.children().filter(|c| c.is_element()) {
                    let pos = gs
                        .attribute("pos")
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0;
                    if let Some(s) = spec_in(gs) {
                        stops.push((pos as f32, s));
                    }
                }
            }
            let ang = child(n, "lin")
                .and_then(|l| l.attribute("ang"))
                .and_then(|a| a.parse::<f64>().ok())
                .unwrap_or(0.0)
                / 60_000.0;
            if stops.is_empty() {
                FillStyle::None
            } else {
                FillStyle::Grad(stops, ang)
            }
        }
        _ => FillStyle::None,
    }
}

fn scheme_of(n: Node) -> Scheme {
    let mut s = Scheme::default();
    if let Some(l) = child(n, "fillStyleLst") {
        s.fills = l.children().filter(Node::is_element).map(fill_of).collect();
    }
    if let Some(l) = child(n, "lnStyleLst") {
        s.lines = l
            .children()
            .filter(Node::is_element)
            .map(|ln| LineStyle {
                width: ln
                    .attribute("w")
                    .and_then(|w| w.parse::<f64>().ok())
                    .map(|w| w / 12_700.0),
                fill: ln
                    .children()
                    .filter(Node::is_element)
                    .map(fill_of)
                    .find(|f| !matches!(f, FillStyle::None))
                    .unwrap_or(FillStyle::None),
                dash: child(ln, "prstDash")
                    .and_then(|d| d.attribute("val"))
                    .map(str::to_string),
            })
            .collect();
    }
    s
}

fn font_styles(n: Node) -> Vec<Option<Spec>> {
    n.children()
        .filter(|c| c.is_element() && c.tag_name().name() == "fontProps")
        .map(|p| child(p, "color").and_then(spec_in))
        .collect()
}

impl Theme {
    pub fn parse(doc: &Document) -> Theme {
        let mut t = Theme::default();
        let el = |name: &'static str| {
            doc.descendants()
                .find(move |n| n.is_element() && n.tag_name().name() == name)
        };
        if let Some(cs) = el("clrScheme") {
            for c in cs.children().filter(Node::is_element) {
                let name = c.tag_name().name();
                if name == "extLst" {
                    continue;
                }
                if let Some(Spec::Srgb(rgb, _)) = spec_in(c) {
                    t.colors.push((name.to_string(), rgb));
                }
            }
        }
        for v in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "variationClrScheme")
        {
            let cols: Vec<[u8; 3]> = v
                .children()
                .filter(Node::is_element)
                .filter_map(|c| match spec_in(c) {
                    Some(Spec::Srgb(rgb, _)) => Some(rgb),
                    _ => None,
                })
                .collect();
            t.variants.push(cols);
        }
        for v in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "variationStyleScheme")
        {
            let idx = |s: Node, a: &str| {
                s.attribute(a)
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(0)
            };
            t.var_styles.push(
                v.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "varStyle")
                    .map(|s| VarStyle {
                        fill: idx(s, "fillIdx"),
                        line: idx(s, "lineIdx"),
                        font: idx(s, "fontIdx"),
                    })
                    .collect(),
            );
        }
        if let Some(fs) = el("fmtScheme") {
            t.shapes = scheme_of(fs);
        }
        if let Some(fs) = el("fmtConnectorScheme") {
            t.connectors = scheme_of(fs);
        }
        if let Some(g) = el("fontStylesGroup") {
            if let Some(f) = child(g, "fontStyles") {
                t.shapes.fonts = font_styles(f);
            }
            if let Some(f) = child(g, "connectorFontStyles") {
                t.connectors.fonts = font_styles(f);
            }
        }
        let latin = |name: &'static str| {
            el(name)
                .and_then(|f| child(f, "latin"))
                .and_then(|l| l.attribute("typeface"))
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        t.minor_font = latin("minorFont");
        t.major_font = latin("majorFont");
        t
    }

    pub fn scheme_color(&self, name: &str) -> Option<[u8; 3]> {
        let name = match name {
            "tx1" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            n => n,
        };
        self.colors.iter().find(|(n, _)| n == name).map(|c| c.1)
    }

    /// A quick-style colour index: 0 dk1, 1 lt1, 2–7 accents, 8 dk2, 9 lt2; 100–106 (and
    /// 200–206) the variant's colours 1–7.
    pub fn quick_color(&self, index: i64, variant: usize) -> Option<[u8; 3]> {
        let names = [
            "dk1", "lt1", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "dk2",
            "lt2",
        ];
        match index {
            0..=9 => self.scheme_color(names[index as usize]),
            100..=106 | 200..=206 => {
                let i = (index % 100) as usize;
                self.variants
                    .get(variant)
                    .or_else(|| self.variants.first())
                    .and_then(|v| v.get(i).copied())
                    .or_else(|| self.scheme_color(names[2 + i.min(5)]))
            }
            _ => None,
        }
    }

    /// The 1-based style index a quick-style matrix value means: 1–6 as is, 100–103 through
    /// the variant style scheme. `pick` chooses fill, line or font.
    pub fn matrix(&self, value: i64, variant: usize, pick: fn(&VarStyle) -> usize) -> usize {
        match value {
            1..=6 => value as usize,
            100..=103 => self
                .var_styles
                .get(variant)
                .or_else(|| self.var_styles.first())
                .and_then(|v| v.get((value - 100) as usize))
                .map(pick)
                .unwrap_or(1),
            _ => 0,
        }
    }

    /// The colour of `spec` with `ph` for the placeholder.
    pub fn resolve(&self, spec: &Spec, ph: [u8; 3]) -> Rgba {
        let (rgb, m) = match spec {
            Spec::Srgb(c, m) => (*c, m),
            Spec::Scheme(n, m) => (
                if n == "phClr" {
                    ph
                } else {
                    self.scheme_color(n).unwrap_or(ph)
                },
                m,
            ),
        };
        apply_mods(rgb, m)
    }
}

/// DrawingML colour transforms, approximated in sRGB / HSL.
pub(crate) fn apply_mods(rgb: [u8; 3], mods: &[(String, f64)]) -> Rgba {
    let mut c = [
        f64::from(rgb[0]) / 255.0,
        f64::from(rgb[1]) / 255.0,
        f64::from(rgb[2]) / 255.0,
    ];
    let mut alpha = 1.0;
    for (name, v) in mods {
        match name.as_str() {
            "tint" => c = c.map(|x| x * v + (1.0 - v)),
            "shade" => c = c.map(|x| x * v),
            "alpha" => alpha = *v,
            "lumMod" | "lumOff" | "satMod" => {
                let (h, s, l) = hsl(c);
                let (s, l) = match name.as_str() {
                    "lumMod" => (s, l * v),
                    "lumOff" => (s, l + v),
                    _ => ((s * v).clamp(0.0, 1.0), l),
                };
                c = from_hsl(h, s, l.clamp(0.0, 1.0));
            }
            _ => {}
        }
    }
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [b(c[0]), b(c[1]), b(c[2]), b(alpha)]
}

fn hsl(c: [f64; 3]) -> (f64, f64, f64) {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    let l = (max + min) / 2.0;
    if max - min < 1e-12 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if max == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    if s <= 0.0 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_mods() {
        assert_eq!(apply_mods([200, 100, 0], &[]), [200, 100, 0, 255]);
        assert_eq!(
            apply_mods([200, 100, 0], &[("shade".into(), 0.5)]),
            [100, 50, 0, 255]
        );
        let t = apply_mods([0, 0, 0], &[("tint".into(), 0.2)]);
        assert_eq!(t, [204, 204, 204, 255]);
        let l = apply_mods([255, 0, 0], &[("lumMod".into(), 0.5)]);
        assert_eq!(l, [128, 0, 0, 255]);
    }
}
