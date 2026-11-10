//! Block layout and pagination.
//!
//! Each spine document is walked once: block elements stack, inline content gathers into
//! paragraphs that parley shapes and breaks into lines at the block's width, tables get
//! column widths from their cells' narrowest and widest layouts, pictures are scaled to the
//! page. The result is a list of vertical boxes (a line, a picture, a table row) with
//! collapsible margins between them, which the paginator stacks onto pages: a box that does
//! not fit goes to the next page, a heading stays with what follows it, a paragraph leaves
//! no single line behind or ahead. Positioning, floats and columns are not laid out.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, InlineBox, InlineBoxKind,
    LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

use crate::css::Styles;
use crate::fonts::BookFonts;
use crate::package::{dir_of, resolve};
use crate::style::{Align, Case, Display, ListType, Style, compute};
use crate::xml::{Doc, Kind, NodeId};
use crate::{Error, ImageDecoder, Inner, Layout, MAX_BINARY_PART, MAX_TEXT_PART, PageSetup};

/// Most pages a book may have.
const MAX_PAGES: usize = 100_000;
/// Deepest nesting of tables in tables.
const MAX_TABLE_DEPTH: u32 = 3;
/// Elements nested deeper than this are not laid out (the parser keeps 256 levels; laying
/// out each takes several stack frames, and worker threads have 2 MiB).
const MAX_LAYOUT_DEPTH: u32 = 64;
/// Most columns a table is laid out with.
const MAX_COLUMNS: usize = 64;
/// The inline box standing for a paragraph's first-line indent.
const INDENT_BOX: u64 = u64::MAX;

/// The brush of a run: its colour and baseline shift (`sup`, `sub`).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Ink {
    color: [u8; 4],
    shift: f32,
}

pub(crate) struct FontEntry {
    pub data: parley::FontData,
    pub upem: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct GlyphRun {
    pub font: u32,
    pub size: f32,
    pub coords: Vec<i16>,
    pub color: [u8; 4],
    /// Synthetic slant in degrees (italic of a font without one).
    pub skew: f32,
    pub bold: bool,
    /// Glyph id, x, baseline y (CSS px on the page, y down).
    pub glyphs: Vec<(u32, f32, f32)>,
}

#[derive(Debug, Clone)]
pub(crate) enum Item {
    Glyphs(GlyphRun),
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [u8; 4],
    },
    Image {
        res: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

impl Item {
    fn shift(&mut self, dx: f32, dy: f32) {
        match self {
            Item::Glyphs(g) => {
                for p in &mut g.glyphs {
                    p.1 += dx;
                    p.2 += dy;
                }
            }
            Item::Rect { x, y, .. } | Item::Image { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
}

/// A picture of the book.
#[derive(Debug, Clone)]
pub(crate) struct ImageRes {
    pub path: String,
    pub media: String,
    /// Pixel size (1 px = 1 CSS px).
    pub size: (u32, u32),
    /// An inline `<svg>` written out, for pictures that are not files.
    pub inline: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PageData {
    pub size: (f32, f32),
    pub spine: Option<usize>,
    pub items: Vec<Item>,
}

/// A laid-out book.
pub struct Pages {
    pub(crate) pages: Vec<PageData>,
    pub(crate) fonts: Vec<FontEntry>,
    /// The memory budget's share of the book's font files the text stack keeps.
    _fonts_held: Vec<specular_core::Reservation>,
    pub(crate) images: Vec<ImageRes>,
    anchors: HashMap<(usize, String), usize>,
    spine_first: Vec<Option<usize>>,
    toc_pages: Vec<Option<usize>>,
    pub(crate) inner: Arc<Inner>,
    pub(crate) decoder: Arc<dyn ImageDecoder>,
    pub(crate) cache: crate::render::Cache,
}

impl std::fmt::Debug for Pages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pages")
            .field("pages", &self.pages.len())
            .field("fonts", &self.fonts.len())
            .field("images", &self.images.len())
            .finish()
    }
}

impl Pages {
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Page size in CSS px.
    pub fn page_size(&self, i: usize) -> Option<(f32, f32)> {
        self.pages.get(i).map(|p| p.size)
    }

    /// The spine document page `i` belongs to; `None` for the cover made from the cover
    /// picture.
    pub fn spine_of(&self, i: usize) -> Option<usize> {
        self.pages.get(i).and_then(|p| p.spine)
    }

    /// The page showing `fragment` of spine document `spine` (its first page without one,
    /// or when the fragment is not found).
    pub fn page_of(&self, spine: usize, fragment: Option<&str>) -> Option<usize> {
        if let Some(f) = fragment
            && let Some(&p) = self.anchors.get(&(spine, f.to_owned()))
        {
            return Some(p);
        }
        self.spine_first.get(spine).copied().flatten()
    }

    /// The page the table of contents' entry `toc_index` points to.
    pub fn toc_page(&self, toc_index: usize) -> Option<usize> {
        self.toc_pages.get(toc_index).copied().flatten()
    }

    /// Byte sizes of the font files the pages draw with (tests: a book's own font is used).
    #[doc(hidden)]
    pub fn font_data_lens(&self) -> Vec<usize> {
        self.fonts
            .iter()
            .map(|f| f.data.data.data().len())
            .collect()
    }

    /// How many glyphs and pictures page `i` draws.
    pub fn item_counts(&self, i: usize) -> Option<(usize, usize)> {
        let p = self.pages.get(i)?;
        let glyphs = p
            .items
            .iter()
            .map(|it| match it {
                Item::Glyphs(g) => g.glyphs.len(),
                _ => 0,
            })
            .sum();
        let images = p
            .items
            .iter()
            .filter(|it| matches!(it, Item::Image { .. }))
            .count();
        Some((glyphs, images))
    }
}

/// A vertical box of the flow.
enum Atom {
    /// A collapsible margin.
    Space(f32),
    /// A forced page break.
    Break,
    Box {
        h: f32,
        items: Vec<Item>,
        anchors: Vec<String>,
        /// Keep with the next box (headings; first and penultimate lines).
        keep: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct SpanStyle {
    families: String,
    size: f32,
    weight: f32,
    italic: bool,
    ink: Ink,
    underline: bool,
    strike: bool,
    lang: Option<String>,
}

struct InlineImg {
    at: usize,
    res: u32,
    w: f32,
    h: f32,
}

struct Para {
    text: String,
    spans: Vec<(usize, usize, SpanStyle)>,
    boxes: Vec<InlineImg>,
    anchors: Vec<(usize, String)>,
    block: Style,
    /// The last thing added was white space (or nothing visible yet).
    space: bool,
}

struct Flow {
    origin: f32,
    left: f32,
    width: f32,
    /// Width pictures are fitted to (the table's, while measuring cells).
    fit_w: f32,
    atoms: Vec<Atom>,
    para: Option<Para>,
    block: Style,
    pending_anchors: Vec<String>,
    marker: Option<(String, Style)>,
    keep: bool,
    max_w: f32,
}

impl Flow {
    fn new(left: f32, width: f32, block: Style) -> Flow {
        Flow {
            origin: left,
            left,
            width: width.max(1.0),
            fit_w: width.max(1.0),
            atoms: Vec::new(),
            para: None,
            block,
            pending_anchors: Vec::new(),
            marker: None,
            keep: false,
            max_w: 0.0,
        }
    }

    fn space(&mut self, h: f32) {
        if h <= 0.0 || !h.is_finite() {
            return;
        }
        if let Some(Atom::Space(s)) = self.atoms.last_mut() {
            *s = s.max(h);
        } else {
            self.atoms.push(Atom::Space(h));
        }
    }

    fn push_box(&mut self, h: f32, items: Vec<Item>, mut anchors: Vec<String>, keep: bool) {
        let mut a = std::mem::take(&mut self.pending_anchors);
        a.append(&mut anchors);
        self.atoms.push(Atom::Box {
            h: h.max(0.0),
            items,
            anchors: a,
            keep,
        });
    }

    fn anchor(&mut self, id: &str) {
        if let Some(p) = &mut self.para {
            p.anchors.push((p.text.len(), id.to_owned()));
        } else {
            self.pending_anchors.push(id.to_owned());
        }
    }
}

struct Ctx<'a> {
    inner: &'a Inner,
    fonts: BookFonts,
    lcx: LayoutContext<Ink>,
    font_table: Vec<FontEntry>,
    font_ids: HashMap<(u64, u32), u32>,
    images: Vec<ImageRes>,
    image_ids: HashMap<String, Option<u32>>,
    decoder: &'a dyn ImageDecoder,
    cancel: &'a dyn Fn() -> bool,
    cancelled: bool,
    root_size: f32,
    /// Content height pictures are fitted to.
    page_h: f32,
    css: HashMap<String, Option<Arc<String>>>,
    table_depth: u32,
    /// Elements being laid out, outermost first ([`MAX_LAYOUT_DEPTH`]).
    depth: u32,
    paragraphs: u32,
}

impl Ctx<'_> {
    fn font_id(&mut self, f: &parley::FontData) -> u32 {
        let key = (f.data.id(), f.index);
        if let Some(&i) = self.font_ids.get(&key) {
            return i;
        }
        let upem = skrifa::FontRef::from_index(f.data.data(), f.index)
            .map(|r| {
                r.metrics(Size::unscaled(), LocationRef::default())
                    .units_per_em
            })
            .ok()
            .filter(|&u| u > 0)
            .map_or(1000.0, f32::from);
        let i = self.font_table.len() as u32;
        self.font_table.push(FontEntry {
            data: f.clone(),
            upem,
        });
        self.font_ids.insert(key, i);
        i
    }

    /// A picture file of the book, with its size.
    fn image(&mut self, path: &str) -> Option<(u32, (u32, u32))> {
        if let Some(r) = self.image_ids.get(path) {
            return r.map(|i| (i, self.images[i as usize].size));
        }
        let media = media_of(self.inner, path);
        let size = self
            .inner
            .read(path, MAX_BINARY_PART)
            .and_then(|b| self.decoder.size(&b, &media))
            .filter(|s| s.0 > 0 && s.1 > 0);
        let id = size.map(|size| {
            self.images.push(ImageRes {
                path: path.to_owned(),
                media,
                size,
                inline: None,
            });
            (self.images.len() - 1) as u32
        });
        self.image_ids.insert(path.to_owned(), id);
        id.map(|i| (i, self.images[i as usize].size))
    }

    fn inline_svg(&mut self, bytes: Vec<u8>, size: (u32, u32)) -> u32 {
        self.images.push(ImageRes {
            path: String::new(),
            media: "image/svg+xml".into(),
            size,
            inline: Some(Arc::new(bytes)),
        });
        (self.images.len() - 1) as u32
    }

    fn stylesheet(&mut self, path: &str) -> Option<Arc<String>> {
        if let Some(c) = self.css.get(path) {
            return c.clone();
        }
        let s = self
            .inner
            .read(path, MAX_TEXT_PART)
            .map(|b| Arc::new(crate::xml::decode_text(&b)));
        self.css.insert(path.to_owned(), s.clone());
        s
    }

    fn check_cancel(&mut self) -> bool {
        self.paragraphs += 1;
        if !self.cancelled && self.paragraphs.is_multiple_of(64) && (self.cancel)() {
            self.cancelled = true;
        }
        self.cancelled
    }
}

fn media_of(inner: &Inner, path: &str) -> String {
    let p = &inner.pkg;
    p.item_by_path(path)
        .map(|i| p.manifest[i].media_type.clone())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| crate::media_type_from_name(path).to_owned())
}

/// A document being laid out.
struct DocCtx<'a> {
    doc: &'a Doc,
    styles: Styles,
    base: String,
}

fn apply_case(t: &str, case: Case, small_caps: bool) -> Cow<'_, str> {
    match case {
        Case::Upper => Cow::Owned(t.to_uppercase()),
        Case::Lower => Cow::Owned(t.to_lowercase()),
        Case::Capitalize => {
            let mut out = String::with_capacity(t.len());
            let mut start = true;
            for c in t.chars() {
                if start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
                start = c.is_whitespace();
            }
            Cow::Owned(out)
        }
        Case::None if small_caps => Cow::Owned(t.to_uppercase()),
        Case::None => Cow::Borrowed(t),
    }
}

fn span_style(ctx: &Ctx, s: &Style) -> SpanStyle {
    SpanStyle {
        families: ctx.fonts.stack(&s.families),
        size: if s.small_caps { s.size * 0.85 } else { s.size },
        weight: s.weight,
        italic: s.italic,
        ink: Ink {
            color: if s.hidden { [0, 0, 0, 0] } else { s.color },
            shift: s.shift,
        },
        underline: s.underline,
        strike: s.strike,
        lang: s.lang.clone(),
    }
}

/// Appends text in style `s` to the flow's paragraph (white space collapsed unless `pre`).
fn add_text(ctx: &Ctx, f: &mut Flow, raw: &str, s: &Style) {
    let t = apply_case(raw, s.case, s.small_caps);
    let mut out = String::new();
    let mut space = f.para.as_ref().is_none_or(|p| p.space);
    if s.pre {
        for c in t.chars() {
            match c {
                '\r' => {}
                '\t' => out.push_str("    "),
                c => out.push(c),
            }
        }
        if !out.is_empty() {
            space = out.ends_with([' ', '\n']);
        }
    } else {
        for c in t.chars() {
            if c.is_whitespace() && c != '\u{a0}' && c != '\u{2007}' && c != '\u{202f}' {
                if !space {
                    out.push(' ');
                    space = true;
                }
            } else if c != '\u{ad}' {
                out.push(c);
                space = false;
            }
        }
    }
    if out.is_empty() || (f.para.is_none() && out.trim().is_empty()) {
        return;
    }
    let st = span_style(ctx, s);
    let p = f.para.get_or_insert_with(|| {
        let mut p = Para {
            text: String::new(),
            spans: Vec::new(),
            boxes: Vec::new(),
            anchors: Vec::new(),
            block: f.block.clone(),
            space: true,
        };
        match f.block.rtl {
            Some(true) => p.text.push('\u{200F}'),
            Some(false) => p.text.push('\u{200E}'),
            None => {}
        }
        p
    });
    let start = p.text.len();
    p.text.push_str(&out);
    p.space = space;
    let end = p.text.len();
    match p.spans.last_mut() {
        Some(last) if last.2 == st && last.1 == start => last.1 = end,
        _ => p.spans.push((start, end, st)),
    }
}

fn add_inline_image(f: &mut Flow, res: u32, w: f32, h: f32) {
    let p = f.para.get_or_insert_with(|| Para {
        text: String::new(),
        spans: Vec::new(),
        boxes: Vec::new(),
        anchors: Vec::new(),
        block: f.block.clone(),
        space: true,
    });
    p.boxes.push(InlineImg {
        at: p.text.len(),
        res,
        w,
        h,
    });
    p.space = false;
}

fn parley_align(a: Align) -> Alignment {
    match a {
        Align::Start => Alignment::Start,
        Align::End => Alignment::End,
        Align::Left => Alignment::Left,
        Align::Right => Alignment::Right,
        Align::Center => Alignment::Center,
        Align::Justify => Alignment::Justify,
    }
}

/// A laid-out line: height, baseline (from its top), items, byte range.
type ShapedLine = (f32, f32, Vec<Item>, std::ops::Range<usize>);

/// Lays `text` out in `spans` at `width`: its lines, as boxes (items relative to each
/// line's top), with the byte range of each.
fn shape(
    ctx: &mut Ctx,
    text: &str,
    spans: &[(usize, usize, SpanStyle)],
    boxes: &[InlineImg],
    block: &Style,
    indent: f32,
    width: Option<f32>,
) -> (Vec<ShapedLine>, f32) {
    let base = span_style(ctx, block);
    let mut b = ctx.lcx.ranged_builder(&mut ctx.fonts.fcx, text, 1.0, false);
    b.push_default(StyleProperty::FontFamily(FontFamily::Source(Cow::Owned(
        base.families.clone(),
    ))));
    b.push_default(StyleProperty::FontSize(base.size));
    b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
        block.line_height,
    )));
    b.push_default(StyleProperty::Brush(base.ink.clone()));
    for (s, e, st) in spans {
        let r = *s..*e;
        if st.families != base.families {
            b.push(
                StyleProperty::FontFamily(FontFamily::Source(Cow::Owned(st.families.clone()))),
                r.clone(),
            );
        }
        b.push(StyleProperty::FontSize(st.size), r.clone());
        b.push(
            StyleProperty::FontWeight(FontWeight::new(st.weight)),
            r.clone(),
        );
        if st.italic {
            b.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
        }
        b.push(StyleProperty::Brush(st.ink.clone()), r.clone());
        if st.underline {
            b.push(StyleProperty::Underline(true), r.clone());
        }
        if st.strike {
            b.push(StyleProperty::Strikethrough(true), r.clone());
        }
        if let Some(l) = st
            .lang
            .as_deref()
            .and_then(|l| parley::Language::parse(l).ok())
        {
            b.push(StyleProperty::Locale(Some(l)), r.clone());
        }
    }
    if indent > 0.0 {
        b.push_inline_box(InlineBox {
            id: INDENT_BOX,
            kind: InlineBoxKind::InFlow,
            index: 0,
            width: indent,
            height: 0.0,
        });
    }
    for (i, bx) in boxes.iter().enumerate() {
        b.push_inline_box(InlineBox {
            id: i as u64,
            kind: InlineBoxKind::InFlow,
            index: bx.at.min(text.len()),
            width: bx.w,
            height: bx.h,
        });
    }
    let mut layout = b.build(text);
    layout.break_all_lines(width);
    layout.align(parley_align(block.align), AlignmentOptions::default());
    let mut out = Vec::new();
    for line in layout.lines() {
        let m = line.metrics();
        let top = m.block_min_coord;
        let h = (m.block_max_coord - m.block_min_coord).max(0.0);
        let mut items = Vec::new();
        for it in line.items() {
            match it {
                PositionedLayoutItem::GlyphRun(gr) => {
                    let run = gr.run();
                    let font = ctx.font_id(run.font());
                    let syn = run.synthesis();
                    let ink = &gr.style().brush;
                    let dy = ink.shift - top;
                    let glyphs: Vec<(u32, f32, f32)> = gr
                        .positioned_glyphs()
                        .map(|g| (g.id, g.x, g.y + dy))
                        .collect();
                    if ink.color[3] == 0 || glyphs.is_empty() {
                        continue;
                    }
                    let rm = run.metrics();
                    let x0 = gr.offset();
                    let w = gr.advance();
                    let base_y = gr.baseline() + dy;
                    if gr.style().underline.is_some() {
                        let t = rm.underline_size.max(0.6);
                        items.push(Item::Rect {
                            x: x0,
                            y: base_y - rm.underline_offset - t / 2.0,
                            w,
                            h: t,
                            color: ink.color,
                        });
                    }
                    if gr.style().strikethrough.is_some() {
                        let t = rm.strikethrough_size.max(0.6);
                        items.push(Item::Rect {
                            x: x0,
                            y: base_y - rm.strikethrough_offset - t / 2.0,
                            w,
                            h: t,
                            color: ink.color,
                        });
                    }
                    items.push(Item::Glyphs(GlyphRun {
                        font,
                        size: run.font_size(),
                        coords: run.normalized_coords().to_vec(),
                        color: ink.color,
                        skew: syn.skew().unwrap_or(0.0),
                        bold: syn.embolden(),
                        glyphs,
                    }));
                }
                PositionedLayoutItem::InlineBox(ib) => {
                    if ib.id == INDENT_BOX {
                        continue;
                    }
                    if let Some(bx) = boxes.get(ib.id as usize) {
                        items.push(Item::Image {
                            res: bx.res,
                            x: ib.x,
                            y: ib.y - top,
                            w: ib.width,
                            h: ib.height,
                        });
                    }
                }
            }
        }
        out.push((h, m.baseline - top, items, line.text_range()));
    }
    (out, layout.width())
}

fn flush(ctx: &mut Ctx, f: &mut Flow) {
    let Some(mut p) = f.para.take() else { return };
    if ctx.check_cancel() {
        return;
    }
    // Trailing white space and line breaks do not make lines.
    let trimmed = p.text.trim_end_matches([' ', '\n']).len();
    let last_box = p.boxes.iter().map(|b| b.at).max().unwrap_or(0);
    let keep_len = trimmed.max(last_box);
    p.text.truncate(keep_len);
    for s in &mut p.spans {
        s.1 = s.1.min(keep_len);
    }
    p.spans.retain(|s| s.0 < s.1);
    let visible = p
        .text
        .chars()
        .any(|c| !c.is_whitespace() && !matches!(c, '\u{200E}' | '\u{200F}'));
    if !visible && p.boxes.is_empty() {
        f.pending_anchors.extend(p.anchors.into_iter().map(|a| a.1));
        return;
    }
    if !visible && p.boxes.len() == 1 {
        // A paragraph that is a picture: a picture block, centred unless aligned.
        let bx = &p.boxes[0];
        let x = match p.block.align {
            Align::Left => 0.0,
            Align::Right => f.width - bx.w,
            Align::End if p.block.rtl != Some(true) => f.width - bx.w,
            _ => (f.width - bx.w) / 2.0,
        };
        let items = vec![Item::Image {
            res: bx.res,
            x: f.left + x.max(0.0),
            y: 0.0,
            w: bx.w,
            h: bx.h,
        }];
        f.max_w = f.max_w.max(f.left - f.origin + bx.w);
        let anchors = p.anchors.into_iter().map(|a| a.1).collect();
        let keep = f.keep;
        f.push_box(bx.h, items, anchors, keep);
        return;
    }
    let indent = p.block.indent.max(0.0);
    let (lines, width) = shape(
        ctx,
        &p.text,
        &p.spans,
        &p.boxes,
        &p.block,
        indent,
        Some(f.width),
    );
    f.max_w = f.max_w.max(f.left - f.origin + width);
    let n = lines.len();
    let mut anchors = p.anchors.into_iter().peekable();
    let marker = f.marker.take();
    for (i, (h, baseline, mut items, range)) in lines.into_iter().enumerate() {
        for it in &mut items {
            it.shift(f.left, 0.0);
        }
        let mut here = Vec::new();
        while let Some((at, _)) = anchors.peek() {
            if *at < range.end || i + 1 == n {
                here.push(anchors.next().map(|a| a.1).unwrap_or_default());
            } else {
                break;
            }
        }
        if i == 0
            && let Some((text, style)) = &marker
        {
            let st = span_style(ctx, style);
            let len = text.len();
            let (ml, mw) = shape(ctx, text, &[(0, len, st)], &[], style, 0.0, None);
            if let Some((_, mb, mitems, _)) = ml.into_iter().next() {
                let gap = style.size * 0.5;
                let x = if p.block.rtl == Some(true) {
                    f.left + f.width + gap
                } else {
                    f.left - gap - mw
                };
                for mut it in mitems {
                    it.shift(x, baseline - mb);
                    items.push(it);
                }
            }
        }
        let keep = f.keep || (n >= 2 && i == 0) || (n >= 3 && i + 2 == n);
        f.push_box(h, items, here, keep);
    }
}

/// The picture size of `s` (CSS width/height, else the picture's own) fitted into `max_w` ×
/// `max_h`.
fn picture_size(s: &Style, own: (u32, u32), container: f32, max_w: f32, max_h: f32) -> (f32, f32) {
    let (iw, ih) = (own.0.max(1) as f32, own.1.max(1) as f32);
    let w = s.resolve(s.width, container).filter(|v| *v > 0.0);
    let h = s.resolve(s.height, container).filter(|v| *v > 0.0);
    let (mut w, mut h) = match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w * ih / iw),
        (None, Some(h)) => (h * iw / ih, h),
        (None, None) => (iw, ih),
    };
    if w > max_w {
        h *= max_w / w;
        w = max_w;
    }
    if h > max_h {
        w *= max_h / h;
        h = max_h;
    }
    (w.max(1.0), h.max(1.0))
}

/// `true` for text that shows something.
fn visible_text(t: &str) -> bool {
    t.chars().any(|c| !c.is_whitespace())
}

/// The one picture a document is made of, when it has no text: an `<img>`, an `<svg>` with
/// an `<image>`, or a drawing `<svg>`.
fn sole_picture(doc: &Doc, body: NodeId) -> Option<NodeId> {
    let mut found = None;
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        match &doc.node(n).kind {
            Kind::Text(t) if visible_text(t) => return None,
            Kind::Element { .. } => {
                let l = doc.local(n).unwrap_or("");
                if matches!(l, "script" | "style" | "head" | "title") {
                    continue;
                }
                if l == "img" || l == "svg" {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(n);
                    if l == "svg" {
                        // Text inside an SVG drawing is the drawing's.
                        continue;
                    }
                }
                stack.extend(doc.children(n).iter().rev().copied());
            }
            _ => stack.extend(doc.children(n).iter().rev().copied()),
        }
    }
    found
}

/// The first picture a document shows (`<img src>` or SVG `<image href>`), resolved.
pub(crate) fn first_image(doc: &Doc, base: &str) -> Option<String> {
    let mut stack = vec![0];
    while let Some(n) = stack.pop() {
        match doc.local(n) {
            Some("img") => {
                if let Some(p) = doc.attr(n, "src").and_then(|s| resolve(base, s)) {
                    return Some(p.0);
                }
            }
            Some("image") => {
                if let Some(p) = doc.attr(n, "href").and_then(|s| resolve(base, s)) {
                    return Some(p.0);
                }
            }
            _ => {}
        }
        stack.extend(doc.children(n).iter().rev().copied());
    }
    None
}

pub(crate) fn cover_path(inner: &Inner) -> Option<String> {
    let p = &inner.pkg;
    if let Some(i) = p.cover_image {
        return Some(p.manifest[i].path.clone());
    }
    let first_in = |doc_path: &str| -> Option<String> {
        let bytes = inner.read(doc_path, MAX_TEXT_PART)?;
        first_image(&Doc::parse(&bytes), dir_of(doc_path))
    };
    if let Some(c) = &p.cover_page
        && let Some(i) = first_in(c)
    {
        return Some(i);
    }
    let first = p.spine.first()?;
    let m = &p.manifest[first.manifest];
    if m.media_type.starts_with("image/") {
        return Some(m.path.clone());
    }
    first_in(&m.path)
}

/// Writes an SVG subtree out as a standalone document.
fn serialize(doc: &Doc, n: NodeId, out: &mut String, depth: u32) {
    if depth > 200 || out.len() > 16 << 20 {
        return;
    }
    match &doc.node(n).kind {
        Kind::Text(t) => out.push_str(&escape(t)),
        Kind::Element { name, attrs } => {
            out.push('<');
            out.push_str(name);
            let has_ns = attrs.iter().any(|(k, _)| k == "xmlns");
            if depth == 0 && !has_ns {
                out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
            }
            if depth == 0 && !attrs.iter().any(|(k, _)| k == "xmlns:xlink") {
                out.push_str(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
            }
            for (k, v) in attrs {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape(v));
                out.push('"');
            }
            out.push('>');
            for &c in doc.children(n) {
                serialize(doc, c, out, depth + 1);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        Kind::Root => {}
    }
}

fn escape(s: &str) -> Cow<'_, str> {
    if !s.contains(['<', '>', '&', '"']) {
        return Cow::Borrowed(s);
    }
    Cow::Owned(
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;"),
    )
}

/// The picture an `<img>` or `<svg>` element shows: its resource and pixel size.
fn picture_of(ctx: &mut Ctx, d: &DocCtx, n: NodeId) -> Option<(u32, (u32, u32))> {
    let doc = d.doc;
    if doc.is(n, "img") {
        let (path, _) = resolve(&d.base, doc.attr(n, "src")?)?;
        return ctx.image(&path);
    }
    // <svg>: a picture inside, else the drawing itself.
    if let Some(im) = doc.find(n, "image")
        && let Some((path, _)) = doc.attr(im, "href").and_then(|h| resolve(&d.base, h))
        && let Some(r) = ctx.image(&path)
    {
        return Some(r);
    }
    let size = svg_size(doc, n).unwrap_or((300, 150));
    let mut s = String::new();
    serialize(doc, n, &mut s, 0);
    Some((ctx.inline_svg(s.into_bytes(), size), size))
}

fn svg_size(doc: &Doc, n: NodeId) -> Option<(u32, u32)> {
    let num = |v: &str| v.trim().trim_end_matches("px").parse::<f32>().ok();
    let vb = doc.attr(n, "viewbox").and_then(|v| {
        let p: Vec<f32> = v
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (p.len() == 4 && p[2] > 0.0 && p[3] > 0.0).then(|| (p[2], p[3]))
    });
    let w = doc.attr(n, "width").and_then(num);
    let h = doc.attr(n, "height").and_then(num);
    let (w, h) = match (w, h, vb) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        (_, _, Some(v)) => v,
        _ => return None,
    };
    (w >= 1.0 && h >= 1.0 && w < 1e5 && h < 1e5).then_some((w as u32, h as u32))
}

fn walk(ctx: &mut Ctx, f: &mut Flow, d: &DocCtx, node: NodeId, parent: &Style) {
    if ctx.cancelled || ctx.depth >= MAX_LAYOUT_DEPTH {
        return;
    }
    ctx.depth += 1;
    walk_children(ctx, f, d, node, parent);
    ctx.depth -= 1;
}

fn walk_children(ctx: &mut Ctx, f: &mut Flow, d: &DocCtx, node: NodeId, parent: &Style) {
    let doc = d.doc;
    for &c in doc.children(node) {
        match &doc.node(c).kind {
            Kind::Text(t) => add_text(ctx, f, t, parent),
            Kind::Element { attrs, .. } => {
                let tag = doc.local(c).unwrap_or("").to_owned();
                let decls = d.styles.matching(doc, c);
                let s = compute(&tag, attrs, &decls, parent, f.width, ctx.root_size);
                if s.display == Display::None {
                    continue;
                }
                if let Some(id) = doc.attr_exact(c, "id") {
                    f.anchor(id);
                } else if tag == "a"
                    && let Some(name) = doc.attr_exact(c, "name")
                {
                    f.anchor(name);
                }
                match tag.as_str() {
                    "br" => {
                        add_text(
                            ctx,
                            f,
                            "\n",
                            &Style {
                                pre: true,
                                ..s.clone()
                            },
                        );
                        if let Some(p) = &mut f.para {
                            p.space = true;
                        }
                        continue;
                    }
                    "img" | "svg" => {
                        if s.hidden {
                            continue;
                        }
                        let Some((res, own)) = picture_of(ctx, d, c) else {
                            if let Some(alt) = doc.attr(c, "alt").filter(|a| visible_text(a)) {
                                add_text(ctx, f, &format!(" {alt} "), &s);
                            }
                            continue;
                        };
                        let (w, h) = picture_size(&s, own, f.width, f.fit_w, ctx.page_h);
                        if s.display == Display::Block {
                            flush(ctx, f);
                            add_inline_image(f, res, w, h);
                            flush(ctx, f);
                        } else {
                            add_inline_image(f, res, w, h);
                        }
                        continue;
                    }
                    "hr" => {
                        flush(ctx, f);
                        f.space(s.margin[0]);
                        let items = vec![Item::Rect {
                            x: f.left,
                            y: 0.0,
                            w: f.width,
                            h: 1.0,
                            color: [150, 150, 150, 255],
                        }];
                        let keep = f.keep;
                        f.push_box(1.0, items, Vec::new(), keep);
                        f.space(s.margin[2]);
                        continue;
                    }
                    "math" => {
                        // MathML is shown as its text.
                        walk(ctx, f, d, c, &s);
                        continue;
                    }
                    _ => {}
                }
                match s.display {
                    Display::Inline => walk(ctx, f, d, c, &s),
                    Display::Table if ctx.table_depth < MAX_TABLE_DEPTH => {
                        table(ctx, f, d, c, &s);
                    }
                    _ => block(ctx, f, d, c, &s),
                }
            }
            Kind::Root => {}
        }
    }
}

fn roman(mut n: u32, upper: bool) -> String {
    const T: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

fn alpha(mut n: u32, upper: bool) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    let s: String = s.into_iter().rev().collect();
    if upper { s.to_uppercase() } else { s }
}

fn marker_text(doc: &Doc, li: NodeId, list: ListType) -> Option<String> {
    let parent = doc.node(li).parent;
    let mut n = doc
        .attr(parent, "start")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(1);
    for &c in doc.children(parent) {
        if c == li {
            break;
        }
        if doc.is(c, "li") {
            n += 1;
        }
    }
    if let Some(v) = doc
        .attr(li, "value")
        .and_then(|v| v.trim().parse::<i64>().ok())
    {
        n = v;
    }
    let n = n.clamp(0, 1_000_000) as u32;
    Some(match list {
        ListType::None => return None,
        ListType::Disc => "\u{2022}".into(),
        ListType::Circle => "\u{25E6}".into(),
        ListType::Square => "\u{25AA}".into(),
        ListType::Decimal => format!("{n}."),
        ListType::LowerAlpha => format!("{}.", alpha(n, false)),
        ListType::UpperAlpha => format!("{}.", alpha(n, true)),
        ListType::LowerRoman => format!("{}.", roman(n, false)),
        ListType::UpperRoman => format!("{}.", roman(n, true)),
    })
}

fn block(ctx: &mut Ctx, f: &mut Flow, d: &DocCtx, n: NodeId, s: &Style) {
    flush(ctx, f);
    if s.break_before && f.atoms.iter().any(|a| matches!(a, Atom::Box { .. })) {
        f.atoms.push(Atom::Break);
    }
    f.space(s.margin[0]);
    let saved = (f.left, f.width, f.block.clone(), f.keep);
    let dl = s.margin[3] + s.padding[3];
    let dw = dl + s.margin[1] + s.padding[1];
    if f.width - dw >= 40.0 {
        f.left += dl;
        f.width -= dw;
    }
    f.space(s.padding[0]);
    f.block = s.clone();
    if d.doc
        .local(n)
        .is_some_and(|t| t.len() == 2 && t.starts_with('h') && t.as_bytes()[1].is_ascii_digit())
    {
        f.keep = true;
    }
    if s.display == Display::ListItem {
        f.marker = marker_text(d.doc, n, s.list).map(|t| (t, s.clone()));
    }
    walk(ctx, f, d, n, s);
    flush(ctx, f);
    f.marker = None;
    f.space(s.padding[2]);
    (f.left, f.width, f.block, f.keep) = saved;
    f.space(s.margin[2]);
    if s.break_after {
        f.atoms.push(Atom::Break);
    }
}

/// Stacks a flow's boxes: their items moved into place, and the height.
fn flatten(atoms: Vec<Atom>) -> (Vec<Item>, f32, Vec<String>) {
    let mut out = Vec::new();
    let mut anchors = Vec::new();
    let mut y = 0.0;
    let mut pend = 0.0f32;
    let mut started = false;
    for a in atoms {
        match a {
            Atom::Space(h) => {
                if started {
                    pend = pend.max(h);
                }
            }
            Atom::Break => {}
            Atom::Box {
                h,
                items,
                anchors: mut an,
                ..
            } => {
                y += pend;
                pend = 0.0;
                for mut it in items {
                    it.shift(0.0, y);
                    out.push(it);
                }
                anchors.append(&mut an);
                y += h;
                started = true;
            }
        }
    }
    (out, y, anchors)
}

/// Lays a table cell's content out at `width` (from `left`): its flow, finished.
fn cell_flow(
    ctx: &mut Ctx,
    d: &DocCtx,
    n: NodeId,
    s: &Style,
    left: f32,
    width: f32,
    fit_w: f32,
) -> Flow {
    let mut f = Flow::new(left, width, s.clone());
    f.fit_w = fit_w;
    f.width = width.max(1.0);
    ctx.table_depth += 1;
    walk(ctx, &mut f, d, n, s);
    flush(ctx, &mut f);
    ctx.table_depth -= 1;
    f
}

fn table(ctx: &mut Ctx, f: &mut Flow, d: &DocCtx, n: NodeId, s: &Style) {
    flush(ctx, f);
    f.space(s.margin[0]);
    let doc = d.doc;
    let cw = f.width;
    let style_of = |ctx: &Ctx, c: NodeId, parent: &Style| {
        let tag = doc.local(c).unwrap_or("");
        let decls = d.styles.matching(doc, c);
        compute(tag, doc.attrs(c), &decls, parent, cw, ctx.root_size)
    };
    // Rows of (cell, style, colspan).
    let mut rows: Vec<Vec<(NodeId, Style, usize)>> = Vec::new();
    let add_row =
        |ctx: &Ctx, tr: NodeId, rs: &Style, rows: &mut Vec<Vec<(NodeId, Style, usize)>>| {
            let mut row = Vec::new();
            for &c in doc.children(tr) {
                if doc.is(c, "td") || doc.is(c, "th") {
                    let cs = style_of(ctx, c, rs);
                    if cs.display == Display::None {
                        continue;
                    }
                    let span = doc
                        .attr(c, "colspan")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, MAX_COLUMNS);
                    row.push((c, cs, span));
                }
            }
            if !row.is_empty() && rows.len() < 10_000 {
                rows.push(row);
            }
        };
    for &c in doc.children(n) {
        match doc.local(c) {
            Some("tr") => {
                let rs = style_of(ctx, c, s);
                add_row(ctx, c, &rs, &mut rows);
            }
            Some("thead" | "tbody" | "tfoot") => {
                let gs = style_of(ctx, c, s);
                for &r in doc.children(c) {
                    if doc.is(r, "tr") {
                        let rs = style_of(ctx, r, &gs);
                        add_row(ctx, r, &rs, &mut rows);
                    }
                }
            }
            Some("caption") => {
                let cs = style_of(ctx, c, s);
                let cs = Style {
                    align: Align::Center,
                    ..cs
                };
                block(ctx, f, d, c, &cs);
            }
            _ => {}
        }
    }
    let ncols = rows
        .iter()
        .map(|r| r.iter().map(|c| c.2).sum::<usize>())
        .max()
        .unwrap_or(0)
        .min(MAX_COLUMNS);
    if ncols == 0 {
        f.space(s.margin[2]);
        return;
    }
    let border = s.border;
    let pad = if border { 4.0 } else { 3.0 };
    let mut min = vec![0.0f32; ncols];
    let mut max = vec![0.0f32; ncols];
    for row in &rows {
        let mut col = 0;
        for (c, cs, span) in row {
            if col >= ncols {
                break;
            }
            let lo = cell_flow(ctx, d, *c, cs, 0.0, 1.0, cw).max_w + 2.0 * pad;
            let hi = cell_flow(ctx, d, *c, cs, 0.0, 1e6, cw).max_w + 2.0 * pad;
            let span = (*span).min(ncols - col);
            for k in col..col + span {
                min[k] = min[k].max(lo / span as f32);
                max[k] = max[k].max(hi / span as f32);
            }
            col += span;
        }
    }
    let smin: f32 = min.iter().sum();
    let smax: f32 = max.iter().sum();
    let widths: Vec<f32> = if smax <= cw {
        max.clone()
    } else if smin >= cw {
        min.iter().map(|m| m * cw / smin.max(1.0)).collect()
    } else {
        let extra = cw - smin;
        let flex = (smax - smin).max(1e-3);
        min.iter()
            .zip(&max)
            .map(|(lo, hi)| lo + extra * (hi - lo) / flex)
            .collect()
    };
    let grey = [140, 140, 140, 255];
    for row in &rows {
        let mut col = 0;
        let mut x = 0.0;
        let mut cells = Vec::new();
        let mut row_h = 0.0f32;
        let mut anchors = Vec::new();
        for (c, cs, span) in row {
            if col >= ncols {
                break;
            }
            let span = (*span).min(ncols - col);
            let w: f32 = widths[col..col + span].iter().sum();
            let cf = cell_flow(
                ctx,
                d,
                *c,
                cs,
                f.left + x + pad,
                (w - 2.0 * pad).max(1.0),
                (w - 2.0 * pad).max(1.0),
            );
            let mut pending = cf.pending_anchors;
            let (items, h, mut an) = flatten(cf.atoms);
            anchors.append(&mut an);
            anchors.append(&mut pending);
            row_h = row_h.max(h + 2.0 * pad);
            cells.push((x, w, items, cs.border));
            x += w;
            col += span;
        }
        let mut items = Vec::new();
        for (x, w, its, cb) in cells {
            for mut it in its {
                it.shift(0.0, pad);
                items.push(it);
            }
            if border || cb {
                let (l, t) = (f.left + x, 0.0);
                items.push(Item::Rect {
                    x: l,
                    y: t,
                    w,
                    h: 0.75,
                    color: grey,
                });
                items.push(Item::Rect {
                    x: l,
                    y: t + row_h - 0.75,
                    w,
                    h: 0.75,
                    color: grey,
                });
                items.push(Item::Rect {
                    x: l,
                    y: t,
                    w: 0.75,
                    h: row_h,
                    color: grey,
                });
                items.push(Item::Rect {
                    x: l + w - 0.75,
                    y: t,
                    w: 0.75,
                    h: row_h,
                    color: grey,
                });
            }
        }
        f.max_w = f.max_w.max(f.left - f.origin + x);
        f.push_box(row_h, items, anchors, false);
    }
    f.space(s.margin[2]);
}

/// Stacks boxes onto pages.
struct Paginator<'a> {
    pages: Vec<PageData>,
    cur: Vec<Item>,
    y: f32,
    pend: f32,
    top: f32,
    bottom: f32,
    size: (f32, f32),
    spine: Option<usize>,
    anchors: &'a mut HashMap<(usize, String), usize>,
    started: bool,
}

impl Paginator<'_> {
    fn new_page(&mut self) {
        if !self.started {
            return;
        }
        self.pages.push(PageData {
            size: self.size,
            spine: self.spine,
            items: std::mem::take(&mut self.cur),
        });
        self.y = self.top;
        self.pend = 0.0;
        self.started = false;
    }

    fn place(&mut self, atoms: Vec<Atom>) {
        let limit = self.bottom;
        let room = self.bottom - self.top;
        let heights: Vec<(f32, bool)> = atoms
            .iter()
            .map(|a| match a {
                Atom::Box { h, keep, .. } => (*h, *keep),
                Atom::Space(h) => (*h, true),
                Atom::Break => (0.0, false),
            })
            .collect();
        let mut prev_keep = false;
        for (i, a) in atoms.into_iter().enumerate() {
            match a {
                Atom::Space(h) => {
                    if self.started {
                        self.pend = self.pend.max(h);
                    }
                }
                Atom::Break => self.new_page(),
                Atom::Box {
                    h,
                    items,
                    anchors,
                    keep,
                } => {
                    if keep && !prev_keep && self.started {
                        // The chain of boxes kept together, and the box after it.
                        let mut chain = self.pend + h;
                        let mut j = i + 1;
                        while j < heights.len() && heights[j].1 {
                            chain += heights[j].0;
                            j += 1;
                        }
                        if j < heights.len() {
                            chain += heights[j].0;
                        }
                        if self.y + chain > limit && chain <= room {
                            self.new_page();
                        }
                    }
                    if self.started && self.y + self.pend + h > limit {
                        self.new_page();
                    }
                    if self.started {
                        self.y += self.pend;
                    }
                    self.pend = 0.0;
                    for mut it in items {
                        it.shift(0.0, self.y);
                        self.cur.push(it);
                    }
                    let page = self.pages.len();
                    if let Some(sp) = self.spine {
                        for an in anchors {
                            self.anchors.entry((sp, an)).or_insert(page);
                        }
                    }
                    self.y += h;
                    self.started = true;
                    prev_keep = keep;
                }
            }
        }
    }
}

/// A page showing picture `res` fitted into `size` (inset by `margin`).
fn picture_page(
    res: u32,
    own: (u32, u32),
    size: (f32, f32),
    margin: f32,
    spine: Option<usize>,
) -> PageData {
    let (aw, ah) = (
        (size.0 - 2.0 * margin).max(1.0),
        (size.1 - 2.0 * margin).max(1.0),
    );
    let (iw, ih) = (own.0.max(1) as f32, own.1.max(1) as f32);
    let k = (aw / iw).min(ah / ih);
    let (w, h) = (iw * k, ih * k);
    PageData {
        size,
        spine,
        items: vec![Item::Image {
            res,
            x: (size.0 - w) / 2.0,
            y: (size.1 - h) / 2.0,
            w,
            h,
        }],
    }
}

fn styles_of(ctx: &mut Ctx, doc: &Doc, base: &str) -> Styles {
    let mut styles = Styles::default();
    let mut stack = vec![0];
    let mut sheets = 0;
    while let Some(n) = stack.pop() {
        match doc.local(n) {
            Some("link")
                if doc.attr(n, "rel").is_some_and(|r| {
                    r.to_ascii_lowercase()
                        .split_whitespace()
                        .any(|x| x == "stylesheet")
                }) =>
            {
                if let Some((path, _)) = doc.attr(n, "href").and_then(|h| resolve(base, h))
                    && let Some(css) = ctx.stylesheet(&path)
                    && sheets < 32
                {
                    sheets += 1;
                    let imports = styles.add(&css, dir_of(&path));
                    for im in imports.into_iter().take(8) {
                        if let Some((ip, _)) = resolve(dir_of(&path), &im)
                            && let Some(c) = ctx.stylesheet(&ip)
                        {
                            styles.add(&c, dir_of(&ip));
                        }
                    }
                }
            }
            Some("style") => {
                let t: String = doc
                    .children(n)
                    .iter()
                    .filter_map(|&c| match &doc.node(c).kind {
                        Kind::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect();
                styles.add(&t, base);
            }
            _ => {}
        }
        stack.extend(doc.children(n).iter().rev().copied());
    }
    let inner = ctx.inner;
    let read = |p: &str| inner.read(p, MAX_BINARY_PART);
    ctx.fonts.register(&styles.font_faces, &read);
    styles
}

pub(crate) fn paginate(
    inner: Arc<Inner>,
    setup: &PageSetup,
    decoder: Arc<dyn ImageDecoder>,
    cancel: &dyn Fn() -> bool,
) -> Result<Pages, Error> {
    let pkg = &inner.pkg;
    let fixed_book = pkg.metadata.layout == Layout::PrePaginated;
    let page = (setup.width.max(100.0), setup.height.max(100.0));
    let margin = setup.margin.clamp(0.0, page.0.min(page.1) / 4.0);
    let mut ctx = Ctx {
        inner: &inner,
        fonts: BookFonts::new(setup.system_fonts),
        lcx: LayoutContext::new(),
        font_table: Vec::new(),
        font_ids: HashMap::new(),
        images: Vec::new(),
        image_ids: HashMap::new(),
        decoder: decoder.as_ref(),
        cancel,
        cancelled: false,
        root_size: setup.font_size.clamp(6.0, 72.0),
        page_h: page.1 - 2.0 * margin,
        css: HashMap::new(),
        table_depth: 0,
        depth: 0,
        paragraphs: 0,
    };
    let mut anchors = HashMap::new();
    let mut pages: Vec<PageData> = Vec::new();
    let mut spine_first = vec![None; pkg.spine.len()];

    // The cover first, when the spine does not start with it.
    let cover = cover_path(&inner);
    let first_is_cover = pkg.spine.first().is_some_and(|s| {
        let m = &pkg.manifest[s.manifest];
        Some(&m.path) == pkg.cover_page.as_ref()
            || Some(&m.path) == cover.as_ref()
            || inner.read(&m.path, MAX_TEXT_PART).is_some_and(|b| {
                let doc = Doc::parse(&b);
                let body = doc.find(0, "body").unwrap_or(0);
                sole_picture(&doc, body).is_some() && first_image(&doc, dir_of(&m.path)) == cover
            })
    });
    if !first_is_cover
        && let Some(c) = &cover
        && let Some((res, own)) = ctx.image(c)
    {
        let size = if fixed_book {
            pkg.viewport.unwrap_or((own.0 as f32, own.1 as f32))
        } else {
            page
        };
        pages.push(picture_page(res, own, size, 0.0, None));
    }

    for (si, item) in pkg.spine.iter().enumerate() {
        if (cancel)() {
            return Err(Error::Cancelled);
        }
        let m = &pkg.manifest[item.manifest];
        let fixed = (fixed_book && !item.properties.contains("rendition:layout-reflowable"))
            || item.properties.contains("rendition:layout-pre-paginated");
        let start = pages.len();
        if m.media_type.starts_with("image/") {
            if let Some((res, own)) = ctx.image(&m.path) {
                let size = if fixed {
                    pkg.viewport.unwrap_or((own.0 as f32, own.1 as f32))
                } else {
                    page
                };
                pages.push(picture_page(res, own, size, 0.0, Some(si)));
            }
        } else if let Some(bytes) = inner.read(&m.path, MAX_TEXT_PART) {
            let doc = Doc::parse(&bytes);
            drop(bytes);
            let base = dir_of(&m.path).to_owned();
            let body = doc.find(0, "body").unwrap_or(0);
            let vp = doc
                .descendants(0, "meta")
                .into_iter()
                .find(|&n| doc.attr(n, "name") == Some("viewport"))
                .and_then(|n| doc.attr(n, "content"))
                .and_then(crate::package::parse_viewport);
            let size = if fixed {
                vp.or(pkg.viewport).unwrap_or(page)
            } else {
                page
            };
            let sole = sole_picture(&doc, body);
            let styles = styles_of(&mut ctx, &doc, &base);
            let d = DocCtx {
                doc: &doc,
                styles,
                base,
            };
            if let Some(pic) = sole
                && (fixed || si == 0 || pkg.cover_page.as_deref() == Some(m.path.as_str()))
                && let Some((res, own)) = picture_of(&mut ctx, &d, pic)
            {
                let size = if fixed && vp.is_none() && pkg.viewport.is_none() {
                    (own.0 as f32, own.1 as f32)
                } else {
                    size
                };
                pages.push(picture_page(res, own, size, 0.0, Some(si)));
            } else {
                let mg = if fixed { 0.0 } else { margin };
                ctx.page_h = size.1 - 2.0 * mg;
                let mut style = Style::root(ctx.root_size);
                // html and body styles.
                for el in [doc.find(0, "html"), doc.find(0, "body")]
                    .into_iter()
                    .flatten()
                {
                    let decls = d.styles.matching(&doc, el);
                    let tag = doc.local(el).unwrap_or("body").to_owned();
                    style = compute(&tag, doc.attrs(el), &decls, &style, size.0, ctx.root_size);
                    style.display = Display::Block;
                }
                let bl = style.margin[3] + style.padding[3];
                let bw = bl + style.margin[1] + style.padding[1];
                let (left, width) = if size.0 - 2.0 * mg - bw >= 100.0 {
                    (mg + bl, size.0 - 2.0 * mg - bw)
                } else {
                    (mg, size.0 - 2.0 * mg)
                };
                let mut f = Flow::new(left, width, style.clone());
                f.space(style.margin[0] + style.padding[0]);
                walk(&mut ctx, &mut f, &d, body, &style);
                flush(&mut ctx, &mut f);
                if ctx.cancelled {
                    return Err(Error::Cancelled);
                }
                let tail_anchors = std::mem::take(&mut f.pending_anchors);
                let mut pg = Paginator {
                    pages: Vec::new(),
                    cur: Vec::new(),
                    y: mg,
                    pend: 0.0,
                    top: mg,
                    bottom: if fixed { f32::INFINITY } else { size.1 - mg },
                    size,
                    spine: Some(si),
                    anchors: &mut anchors,
                    started: false,
                };
                let base_page = pages.len();
                pg.place(f.atoms);
                pg.new_page();
                let new = pg.pages;
                let n_new = new.len();
                pages.extend(new);
                fix_anchors(&mut anchors, si, base_page);
                if n_new > 0 {
                    for an in tail_anchors {
                        anchors.entry((si, an)).or_insert(pages.len() - 1);
                    }
                }
            }
        }
        if pages.len() > start {
            spine_first[si] = Some(start);
        }
        if pages.len() > MAX_PAGES {
            return Err(Error::TooLarge(format!("more than {MAX_PAGES} pages")));
        }
    }
    if pages.is_empty() {
        return Err(Error::NoPages);
    }
    // A document without pages opens at the next one that has some.
    let mut next = None;
    for v in spine_first.iter_mut().rev() {
        match v {
            Some(p) => next = Some(*p),
            None => *v = next,
        }
    }
    let toc_pages = pkg
        .toc
        .iter()
        .map(|t| {
            let sp = t.spine?;
            t.fragment
                .as_ref()
                .and_then(|f| anchors.get(&(sp, f.clone())).copied())
                .or(spine_first.get(sp).copied().flatten())
        })
        .collect();
    let Ctx {
        font_table,
        images,
        fonts,
        ..
    } = ctx;
    Ok(Pages {
        pages,
        fonts: font_table,
        _fonts_held: fonts.into_held(),
        images,
        anchors,
        spine_first,
        toc_pages,
        inner: inner.clone(),
        decoder,
        cache: Default::default(),
    })
}

/// Anchors of spine document `si` were recorded with page numbers counted within it
/// (the paginator starts at 0): shift them by `base`.
fn fix_anchors(anchors: &mut HashMap<(usize, String), usize>, si: usize, base: usize) {
    for ((sp, _), v) in anchors.iter_mut() {
        if *sp == si {
            *v += base;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbering() {
        assert_eq!(roman(1994, false), "mcmxciv");
        assert_eq!(alpha(28, true), "AB");
        assert_eq!(alpha(1, false), "a");
    }
}
