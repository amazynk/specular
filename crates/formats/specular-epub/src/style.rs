//! Computed styles: the browser defaults of each element, then the book's CSS, inherited
//! down the tree.

use crate::css::Decl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    None,
    ListItem,
    Table,
    TableRow,
    TableCell,
    /// thead/tbody/tfoot.
    RowGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListType {
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    None,
    Upper,
    Lower,
    Capitalize,
}

/// A length as written: resolved against the font size or the containing width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Len {
    Px(f32),
    Em(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone)]
pub struct Style {
    pub display: Display,
    /// CSS family names, in order, lower-cased.
    pub families: Vec<String>,
    pub size: f32,
    pub weight: f32,
    pub italic: bool,
    pub color: [u8; 4],
    pub align: Align,
    pub indent: f32,
    pub line_height: f32,
    /// top, right, bottom, left (px).
    pub margin: [f32; 4],
    pub padding: [f32; 4],
    pub break_before: bool,
    pub break_after: bool,
    pub underline: bool,
    pub strike: bool,
    /// Baseline shift in px (negative up): `sup`, `sub`, `vertical-align`.
    pub shift: f32,
    pub list: ListType,
    pub pre: bool,
    pub rtl: Option<bool>,
    pub lang: Option<String>,
    pub hidden: bool,
    pub case: Case,
    pub width: Len,
    pub height: Len,
    pub border: bool,
    pub small_caps: bool,
}

impl Style {
    pub fn root(size: f32) -> Style {
        Style {
            display: Display::Block,
            families: vec!["serif".into()],
            size,
            weight: 400.0,
            italic: false,
            color: [0, 0, 0, 255],
            align: Align::Start,
            indent: 0.0,
            line_height: 1.4,
            margin: [0.0; 4],
            padding: [0.0; 4],
            break_before: false,
            break_after: false,
            underline: false,
            strike: false,
            shift: 0.0,
            list: ListType::Disc,
            pre: false,
            rtl: None,
            lang: None,
            hidden: false,
            case: Case::None,
            width: Len::Auto,
            height: Len::Auto,
            border: false,
            small_caps: false,
        }
    }

    /// What a child inherits (box properties reset).
    fn inherit(&self) -> Style {
        Style {
            display: Display::Inline,
            margin: [0.0; 4],
            padding: [0.0; 4],
            break_before: false,
            break_after: false,
            width: Len::Auto,
            height: Len::Auto,
            border: false,
            shift: 0.0,
            ..self.clone()
        }
    }

    pub fn resolve(&self, l: Len, container: f32) -> Option<f32> {
        match l {
            Len::Px(v) => Some(v),
            Len::Em(v) => Some(v * self.size),
            Len::Percent(v) => Some(v / 100.0 * container),
            Len::Auto => None,
        }
    }
}

pub fn parse_len(s: &str) -> Option<Len> {
    let s = s.trim().to_ascii_lowercase();
    if s == "auto" {
        return Some(Len::Auto);
    }
    let num_end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(s.len());
    let v: f32 = s[..num_end].parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let v = v.clamp(-10_000.0, 10_000.0);
    Some(match &s[num_end..] {
        "" | "px" => Len::Px(v),
        "pt" => Len::Px(v * 4.0 / 3.0),
        "pc" => Len::Px(v * 16.0),
        "in" => Len::Px(v * 96.0),
        "cm" => Len::Px(v * 96.0 / 2.54),
        "mm" => Len::Px(v * 96.0 / 25.4),
        "em" | "rem" => Len::Em(v),
        "ex" | "ch" => Len::Em(v * 0.5),
        "%" => Len::Percent(v),
        "vw" | "vh" => Len::Percent(v),
        _ => return None,
    })
}

pub fn parse_weight(s: &str, parent: f32) -> Option<f32> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "normal" => 400.0,
        "bold" => 700.0,
        "bolder" => (parent + 300.0).min(900.0),
        "lighter" => (parent - 300.0).max(100.0),
        n => n
            .parse::<f32>()
            .ok()
            .filter(|v| (1.0..=1000.0).contains(v))?,
    })
}

pub fn parse_color(s: &str) -> Option<[u8; 4]> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
        return match h.len() {
            3 => Some([v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17, 255]),
            6 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, 255]),
            8 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?, v(6, 2)?]),
            _ => None,
        };
    }
    if let Some(inner) = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner
            .split([',', ' ', '/'])
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let ch = |p: &str| -> Option<u8> {
            if let Some(pc) = p.strip_suffix('%') {
                Some((pc.parse::<f32>().ok()? * 2.55).clamp(0.0, 255.0) as u8)
            } else {
                Some(p.parse::<f32>().ok()?.clamp(0.0, 255.0) as u8)
            }
        };
        let a = parts
            .get(3)
            .and_then(|a| a.parse::<f32>().ok())
            .map_or(255, |a| (a.clamp(0.0, 1.0) * 255.0) as u8);
        return Some([ch(parts[0])?, ch(parts[1])?, ch(parts[2])?, a]);
    }
    Some(match s.as_str() {
        "black" => [0, 0, 0, 255],
        "white" => [255, 255, 255, 255],
        "red" => [255, 0, 0, 255],
        "maroon" => [128, 0, 0, 255],
        "green" => [0, 128, 0, 255],
        "lime" => [0, 255, 0, 255],
        "blue" => [0, 0, 255, 255],
        "navy" => [0, 0, 128, 255],
        "gray" | "grey" => [128, 128, 128, 255],
        "silver" => [192, 192, 192, 255],
        "darkgray" | "darkgrey" => [169, 169, 169, 255],
        "lightgray" | "lightgrey" => [211, 211, 211, 255],
        "purple" => [128, 0, 128, 255],
        "teal" => [0, 128, 128, 255],
        "olive" => [128, 128, 0, 255],
        "yellow" => [255, 255, 0, 255],
        "orange" => [255, 165, 0, 255],
        "brown" => [165, 42, 42, 255],
        "darkred" => [139, 0, 0, 255],
        "darkblue" => [0, 0, 139, 255],
        "darkgreen" => [0, 100, 0, 255],
        "inherit" | "currentcolor" => return None,
        "transparent" => [0, 0, 0, 0],
        _ => return None,
    })
}

fn font_families(v: &str) -> Vec<String> {
    v.split(',')
        .map(|f| {
            f.trim()
                .trim_matches(['"', '\''])
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|f| !f.is_empty() && f != "inherit")
        .collect()
}

fn font_size(v: &str, parent: f32, root: f32) -> Option<f32> {
    let px = match v.trim().to_ascii_lowercase().as_str() {
        "xx-small" => root * 9.0 / 16.0,
        "x-small" => root * 10.0 / 16.0,
        "small" => root * 13.0 / 16.0,
        "medium" => root,
        "large" => root * 18.0 / 16.0,
        "x-large" => root * 1.5,
        "xx-large" => root * 2.0,
        "smaller" => parent / 1.2,
        "larger" => parent * 1.2,
        _ => match parse_len(v)? {
            Len::Px(p) => p,
            Len::Em(e) => {
                if v.trim().ends_with("rem") {
                    e * root
                } else {
                    e * parent
                }
            }
            Len::Percent(p) => p / 100.0 * parent,
            Len::Auto => return None,
        },
    };
    Some(px.clamp(2.0, 400.0))
}

/// Margin or padding shorthand: 1 to 4 values.
fn box4(v: &str) -> Option<[Len; 4]> {
    let parts: Vec<Len> = v.split_whitespace().map(parse_len).collect::<Option<_>>()?;
    Some(match parts.as_slice() {
        [a] => [*a, *a, *a, *a],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d, ..] => [*a, *b, *c, *d],
        [] => return None,
    })
}

const BLOCK: &[&str] = &[
    "html",
    "body",
    "div",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "blockquote",
    "pre",
    "section",
    "article",
    "aside",
    "header",
    "footer",
    "nav",
    "figure",
    "figcaption",
    "address",
    "center",
    "main",
    "hgroup",
    "hr",
    "form",
    "fieldset",
    "details",
    "summary",
    "caption",
    "menu",
    "dir",
    "legend",
    "noscript",
    "xmp",
    "listing",
];
const HIDDEN: &[&str] = &[
    "head", "title", "script", "style", "meta", "link", "template", "rp", "base", "object",
    "audio", "video", "iframe", "input", "button", "select", "textarea", "map", "area",
];

/// Margins (top, bottom in em) and other defaults of an element.
fn defaults(tag: &str, s: &mut Style, parent: &Style, cw: f32) {
    let em = |s: &Style, v: f32| s.size * v;
    // The inline start side: right in right-to-left text.
    let start = if s.rtl == Some(true) { 1 } else { 3 };
    if BLOCK.contains(&tag) {
        s.display = Display::Block;
    }
    if HIDDEN.contains(&tag) {
        s.display = Display::None;
    }
    let heading = |s: &mut Style, scale: f32, m: f32| {
        s.size = parent.size * scale;
        s.weight = 700.0;
        let v = em(s, m);
        s.margin[0] = v;
        s.margin[2] = v;
    };
    match tag {
        "p" | "dl" | "figure" => {
            let v = em(s, 1.0);
            s.margin[0] = v;
            s.margin[2] = v;
        }
        "h1" => heading(s, 2.0, 0.67),
        "h2" => heading(s, 1.5, 0.83),
        "h3" => heading(s, 1.17, 1.0),
        "h4" => heading(s, 1.0, 1.33),
        "h5" => heading(s, 0.83, 1.67),
        "h6" => heading(s, 0.67, 2.33),
        "b" | "strong" => s.weight = 700.0,
        "i" | "em" | "cite" | "var" | "dfn" | "address" => s.italic = true,
        "u" | "ins" => s.underline = true,
        "s" | "strike" | "del" => s.strike = true,
        "sup" => {
            s.size = parent.size * 0.83;
            s.shift = -parent.size * 0.38;
        }
        "sub" => {
            s.size = parent.size * 0.83;
            s.shift = parent.size * 0.22;
        }
        "small" => s.size = parent.size * 0.83,
        "big" => s.size = parent.size * 1.17,
        "code" | "kbd" | "samp" | "tt" => {
            s.families = vec!["monospace".into()];
            s.size = parent.size * 0.9;
        }
        "pre" | "xmp" | "listing" => {
            s.families = vec!["monospace".into()];
            s.size = parent.size * 0.9;
            s.pre = true;
            let v = em(s, 1.0);
            s.margin[0] = v;
            s.margin[2] = v;
        }
        "blockquote" => {
            let v = em(s, 1.0);
            s.margin = [v, 40.0f32.min(cw * 0.1), v, 40.0f32.min(cw * 0.1)];
        }
        "ul" | "ol" | "menu" | "dir" => {
            let nested = matches!(parent.display, Display::ListItem);
            let v = if nested { 0.0 } else { em(s, 1.0) };
            s.margin[0] = v;
            s.margin[2] = v;
            s.padding[start] = 32.0f32.min(cw * 0.1);
            s.list = if tag == "ol" {
                ListType::Decimal
            } else if nested {
                ListType::Circle
            } else {
                ListType::Disc
            };
        }
        "li" => s.display = Display::ListItem,
        "dd" => s.margin[start] = 32.0f32.min(cw * 0.1),
        "dt" => s.weight = s.weight.max(600.0),
        "center" => s.align = Align::Center,
        "hr" => {
            let v = em(s, 0.5);
            s.margin[0] = v;
            s.margin[2] = v;
        }
        "table" => s.display = Display::Table,
        "tr" => s.display = Display::TableRow,
        "td" => s.display = Display::TableCell,
        "th" => {
            s.display = Display::TableCell;
            s.weight = 700.0;
            s.align = Align::Center;
        }
        "thead" | "tbody" | "tfoot" => s.display = Display::RowGroup,
        "figcaption" | "caption" => s.size = parent.size * 0.9,
        "a" | "span" | "img" | "svg" | "br" => {}
        _ => {}
    }
}

/// The style of an element with the local name `tag` under `parent`, with its matching
/// declarations (lowest priority first) and its presentational attributes.
pub fn compute(
    tag: &str,
    attrs: &[(String, String)],
    decls: &[Decl],
    parent: &Style,
    cw: f32,
    root: f32,
) -> Style {
    let mut s = parent.inherit();
    let attr = |k: &str| {
        attrs
            .iter()
            .find(|(n, _)| n == k || crate::xml::local_of(n) == k)
            .map(|(_, v)| v.as_str())
    };
    match attr("dir") {
        Some(d) if d.eq_ignore_ascii_case("rtl") => s.rtl = Some(true),
        Some(d) if d.eq_ignore_ascii_case("ltr") => s.rtl = Some(false),
        _ => {}
    }
    defaults(tag, &mut s, parent, cw);
    if let Some(l) = attr("lang") {
        s.lang = Some(l.to_owned());
    }
    if let Some(a) = attr("align") {
        s.align = match a.to_ascii_lowercase().as_str() {
            "center" | "middle" => Align::Center,
            "right" => Align::Right,
            "left" => Align::Left,
            "justify" => Align::Justify,
            _ => s.align,
        };
    }
    if tag == "img" || tag == "svg" || tag == "image" {
        if let Some(w) = attr("width").and_then(parse_len) {
            s.width = w;
        }
        if let Some(h) = attr("height").and_then(parse_len) {
            s.height = h;
        }
    }
    if tag == "table" && attr("border").is_some_and(|b| b.trim() != "0") {
        s.border = true;
    }
    for d in decls {
        apply(&mut s, d, parent, cw, root);
    }
    s
}

fn apply(s: &mut Style, d: &Decl, parent: &Style, cw: f32, root: f32) {
    let v = d.value.trim();
    let lower = v.to_ascii_lowercase();
    let len_of = |s: &Style, v: &str| -> Option<f32> {
        match parse_len(v)? {
            Len::Auto => Some(0.0),
            l => s.resolve(l, cw),
        }
    };
    match d.name.as_str() {
        "display" => {
            s.display = match lower.as_str() {
                "none" => Display::None,
                "inline" | "inline-block" | "inline-flex" | "contents" => Display::Inline,
                "list-item" => Display::ListItem,
                "table" | "inline-table" => Display::Table,
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-row-group" | "table-header-group" | "table-footer-group" => {
                    Display::RowGroup
                }
                "block" | "flex" | "grid" | "flow-root" | "table-caption" => Display::Block,
                _ => s.display,
            }
        }
        "visibility" => s.hidden = lower == "hidden" || lower == "collapse",
        "font-family" => {
            let f = font_families(v);
            if !f.is_empty() {
                s.families = f;
            }
        }
        "font-size" => {
            if let Some(px) = font_size(v, parent.size, root) {
                s.size = px;
            }
        }
        "font-weight" => {
            if let Some(w) = parse_weight(v, parent.weight) {
                s.weight = w;
            }
        }
        "font-style" => s.italic = matches!(lower.as_str(), "italic" | "oblique"),
        "font-variant" | "font-variant-caps" => s.small_caps = lower.contains("small-caps"),
        "font" => font_shorthand(s, v, parent, root),
        "line-height" => {
            if lower == "normal" {
                s.line_height = 1.4;
            } else if let Ok(n) = lower.parse::<f32>() {
                s.line_height = n.clamp(0.5, 5.0);
            } else if let Some(l) = parse_len(v) {
                let px = match l {
                    Len::Px(p) => p,
                    Len::Em(e) => e * s.size,
                    Len::Percent(p) => p / 100.0 * s.size,
                    Len::Auto => 1.4 * s.size,
                };
                s.line_height = (px / s.size.max(1.0)).clamp(0.5, 5.0);
            }
        }
        "color" => {
            if let Some(c) = parse_color(v) {
                s.color = c;
            }
        }
        "text-align" => {
            s.align = match lower.as_str() {
                "center" => Align::Center,
                "right" => Align::Right,
                "left" => Align::Left,
                "justify" => Align::Justify,
                "end" => Align::End,
                "start" => Align::Start,
                _ => s.align,
            }
        }
        "text-indent" => {
            if let Some(px) = len_of(s, v) {
                s.indent = px;
            }
        }
        "text-decoration" | "text-decoration-line" => {
            if lower.contains("none") {
                s.underline = false;
                s.strike = false;
            }
            if lower.contains("underline") {
                s.underline = true;
            }
            if lower.contains("line-through") {
                s.strike = true;
            }
        }
        "text-transform" => {
            s.case = match lower.as_str() {
                "uppercase" => Case::Upper,
                "lowercase" => Case::Lower,
                "capitalize" => Case::Capitalize,
                _ => Case::None,
            }
        }
        "white-space" => s.pre = lower.starts_with("pre") && lower != "pre-line",
        "direction" => s.rtl = Some(lower == "rtl"),
        "vertical-align" => match lower.as_str() {
            "super" => s.shift = -parent.size * 0.38,
            "sub" => s.shift = parent.size * 0.22,
            "baseline" => s.shift = 0.0,
            _ => {}
        },
        "list-style-type" | "list-style" => {
            for w in lower.split_whitespace() {
                s.list = match w {
                    "disc" => ListType::Disc,
                    "circle" => ListType::Circle,
                    "square" => ListType::Square,
                    "decimal" | "decimal-leading-zero" => ListType::Decimal,
                    "lower-alpha" | "lower-latin" => ListType::LowerAlpha,
                    "upper-alpha" | "upper-latin" => ListType::UpperAlpha,
                    "lower-roman" => ListType::LowerRoman,
                    "upper-roman" => ListType::UpperRoman,
                    "none" => ListType::None,
                    _ => continue,
                };
            }
        }
        "page-break-before" | "break-before" => {
            s.break_before = matches!(
                lower.as_str(),
                "always" | "page" | "left" | "right" | "recto" | "verso"
            );
        }
        "page-break-after" | "break-after" => {
            s.break_after = matches!(
                lower.as_str(),
                "always" | "page" | "left" | "right" | "recto" | "verso"
            );
        }
        "width" => {
            if let Some(l) = parse_len(v) {
                s.width = l;
            }
        }
        "height" => {
            if let Some(l) = parse_len(v) {
                s.height = l;
            }
        }
        "border" | "border-top" | "border-bottom" | "border-width" | "border-style" => {
            s.border =
                !(lower.contains("none") || lower.starts_with('0') || lower.contains("hidden"));
        }
        name => {
            let (is_margin, rest) = if let Some(r) = name.strip_prefix("margin") {
                (true, r)
            } else if let Some(r) = name.strip_prefix("padding") {
                (false, r)
            } else {
                return;
            };
            let target = if is_margin {
                &mut s.margin
            } else {
                &mut s.padding
            };
            let one = |v: &str, size: f32| -> Option<f32> {
                match parse_len(v)? {
                    Len::Px(p) => Some(p),
                    Len::Em(e) => Some(e * size),
                    Len::Percent(p) => Some(p / 100.0 * cw),
                    Len::Auto => Some(0.0),
                }
            };
            let size = s.size;
            let idx = match rest {
                "" => {
                    if let Some(b) = box4(v) {
                        for (i, l) in b.iter().enumerate() {
                            target[i] = match *l {
                                Len::Px(p) => p,
                                Len::Em(e) => e * size,
                                Len::Percent(p) => p / 100.0 * cw,
                                Len::Auto => 0.0,
                            };
                        }
                    }
                    return;
                }
                "-top" | "-block-start" => 0,
                "-right" | "-inline-end" => 1,
                "-bottom" | "-block-end" => 2,
                "-left" | "-inline-start" => 3,
                _ => return,
            };
            if let Some(px) = one(v, size) {
                target[idx] = px;
            }
            if !is_margin {
                for p in target.iter_mut() {
                    *p = p.max(0.0);
                }
            }
        }
    }
    // Keep everything finite and in a sane range.
    for m in s.margin.iter_mut().chain(s.padding.iter_mut()) {
        *m = if m.is_finite() {
            m.clamp(-cw, cw.max(1.0) * 2.0)
        } else {
            0.0
        };
    }
    s.indent = if s.indent.is_finite() {
        s.indent.clamp(-cw, cw)
    } else {
        0.0
    };
}

fn font_shorthand(s: &mut Style, v: &str, parent: &Style, root: f32) {
    // [style] [variant] [weight] size[/line-height] family
    let mut words = v.split_whitespace().peekable();
    let mut rest = String::new();
    while let Some(w) = words.next() {
        let lw = w.to_ascii_lowercase();
        match lw.as_str() {
            "italic" | "oblique" => s.italic = true,
            "normal" => {}
            "small-caps" => s.small_caps = true,
            "bold" | "bolder" | "lighter" => {
                s.weight = parse_weight(&lw, parent.weight).unwrap_or(s.weight);
            }
            _ if lw.parse::<f32>().is_ok() && words.peek().is_some() => {
                s.weight = parse_weight(&lw, parent.weight).unwrap_or(s.weight);
            }
            _ => {
                let (size, lh) = match w.split_once('/') {
                    Some((a, b)) => (a, Some(b)),
                    None => (w, None),
                };
                if let Some(px) = font_size(size, parent.size, root) {
                    s.size = px;
                    if let Some(lh) = lh.and_then(|l| l.parse::<f32>().ok()) {
                        s.line_height = lh.clamp(0.5, 5.0);
                    }
                    rest = words.collect::<Vec<_>>().join(" ");
                }
                break;
            }
        }
    }
    let f = font_families(&rest);
    if !f.is_empty() {
        s.families = f;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(parse_len("12pt"), Some(Len::Px(16.0)));
        assert_eq!(parse_len("1.5em"), Some(Len::Em(1.5)));
        assert_eq!(parse_color("#f00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("rgb(0, 128, 255)"), Some([0, 128, 255, 255]));
        let root = Style::root(16.0);
        let d =
            crate::css::parse_decls("font: italic bold 12px/1.5 'My Font', serif; margin: 1em 2px");
        let s = compute("p", &[], &d, &root, 600.0, 16.0);
        assert!(s.italic);
        assert_eq!(s.weight, 700.0);
        assert_eq!(s.size, 12.0);
        assert_eq!(s.families, ["my font", "serif"]);
        assert_eq!(s.margin, [12.0, 2.0, 12.0, 2.0]);
    }
}
