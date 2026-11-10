//! Structure the renderer would recurse on without a bound: hayro parses PostScript calculator
//! (type 4) functions recursively, one stack frame per `{`, and builds stitching (type 3)
//! functions by recursing into their `/Functions` with no depth or cycle check. A document with
//! such a function is refused before rendering instead of overflowing the render stack.
//!
//! hayro also runs a form XObject's content again for every `Do` that draws it, a tiling
//! pattern's cell for every fill with it, a Type 3 glyph's procedure for every glyph and a soft
//! mask's group for every state setting it, with no budget: content that runs the next many
//! times makes exponential work, which used to run on in an abandoned render thread long after
//! the timeout. [`page_work`] counts what a page would run, and a page over [`MAX_PAGE_WORK`]
//! is refused before rendering.

use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::content::UntypedIter;
use hayro::hayro_syntax::object::{Array, Dict, Name, Object, ObjectIdentifier, Stream};
use hayro::hayro_syntax::page::{Page, Resources};
use std::collections::HashMap;

/// Deepest `{` nesting accepted in a calculator function (real ones nest a few levels).
const MAX_PROCEDURE_DEPTH: usize = 100;
/// Deepest stitching function accepted (real ones nest one or two levels).
const MAX_STITCH_DEPTH: usize = 32;
/// Most functions reachable from one stitching function.
const MAX_STITCH_NODES: usize = 4096;

/// Whether every function in the document is shallow enough for the renderer.
pub(crate) fn functions_bounded(pdf: &Pdf) -> bool {
    pdf.objects().into_iter().all(|o| match o {
        Object::Stream(s) => function_bounded(s.dict(), Some(&s)),
        Object::Dict(d) => function_bounded(&d, None),
        _ => true,
    })
}

fn function_bounded(dict: &Dict<'_>, stream: Option<&Stream<'_>>) -> bool {
    match dict.get::<i32>(b"FunctionType") {
        // A program that does not decode is not parsed at all.
        Some(4) => stream.is_none_or(|s| {
            s.decoded()
                .ok()
                .is_none_or(|p| procedure_depth(&p) <= MAX_PROCEDURE_DEPTH)
        }),
        Some(3) => stitching_bounded(dict),
        _ => true,
    }
}

/// Deepest `{` nesting in a calculator program (over-counts braces in comments, which only
/// makes the check stricter).
fn procedure_depth(program: &[u8]) -> usize {
    let (mut depth, mut max) = (0usize, 0usize);
    for &b in program {
        match b {
            b'{' => {
                depth += 1;
                max = max.max(depth);
            }
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// Walks a stitching function's tree (without recursion), refusing deep trees, cycles (which
/// are infinitely deep) and trees too large to build.
fn stitching_bounded(root: &Dict<'_>) -> bool {
    let mut stack = vec![(root.clone(), 0usize)];
    let mut nodes = 0usize;
    while let Some((dict, depth)) = stack.pop() {
        nodes += 1;
        if depth > MAX_STITCH_DEPTH || nodes > MAX_STITCH_NODES {
            return false;
        }
        if dict.get::<i32>(b"FunctionType") != Some(3) {
            continue;
        }
        let Some(functions) = dict.get::<Array<'_>>(b"Functions") else {
            continue;
        };
        for f in functions.iter::<Object<'_>>() {
            match f {
                Object::Dict(d) => stack.push((d, depth + 1)),
                Object::Stream(s) => stack.push((s.dict().clone(), depth + 1)),
                _ => {}
            }
            if stack.len() > MAX_STITCH_NODES {
                return false;
            }
        }
    }
    true
}

/// Most content-stream instructions one page may run, counting nested content once for every
/// time hayro runs it. Real pages run thousands to a few million; a form drawing the next one
/// ten times, twenty levels deep, runs 10²⁰.
pub(crate) const MAX_PAGE_WORK: u64 = 20_000_000;
/// Deepest nesting of forms, patterns, masks and glyphs followed (deeper counts as over the
/// budget; hayro stops forms at 50).
const MAX_FORM_DEPTH: usize = 64;

/// What nested content is run as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    /// A form XObject (also a soft mask's group and an annotation's appearance).
    Form,
    /// A tiling pattern's cell.
    Pattern,
    /// A Type 3 glyph's procedure.
    Glyph,
}

/// Nested content, the resources it resolves names in when it has none of its own, and the
/// paint it starts with.
type Key = (Kind, ObjectIdentifier, usize, Paint);

/// The identity of a resource dictionary (for the memo: content without resources of its own
/// counts per caller's resources).
fn res_id(res: &Resources<'_>) -> usize {
    let p = |d: &Dict<'_>| d.data().as_ptr() as usize;
    p(&res.x_objects)
        ^ p(&res.fonts).rotate_left(16)
        ^ p(&res.patterns).rotate_left(32)
        ^ p(&res.ext_g_states).rotate_left(48)
}

/// A Type 3 font: the glyph procedure of every code, and the resources the procedures use.
struct Type3<'a> {
    procs: Vec<Option<Stream<'a>>>,
    /// Every procedure (codes the encoding leaves out count as the costliest).
    all: Vec<Stream<'a>>,
    resources: Option<Resources<'a>>,
}

impl<'a> Type3<'a> {
    fn of(font: &Dict<'a>) -> Option<Type3<'a>> {
        if font.get::<Name<'_>>(b"Subtype").as_deref() != Some(b"Type3".as_slice()) {
            return None;
        }
        let procs_dict = font.get::<Dict<'a>>(b"CharProcs").unwrap_or_default();
        let mut procs = vec![None; 256];
        if let Some(diffs) = font
            .get::<Dict<'_>>(b"Encoding")
            .and_then(|e| e.get::<Array<'_>>(b"Differences"))
        {
            let mut code = 0usize;
            for o in diffs.iter::<Object<'_>>() {
                match o {
                    Object::Name(n) => {
                        if let Some(slot) = procs.get_mut(code) {
                            *slot = procs_dict.get::<Stream<'a>>(&n);
                        }
                        code = code.saturating_add(1);
                    }
                    o => {
                        if let Some(c) = o.into_i32() {
                            code = usize::try_from(c).unwrap_or(usize::MAX);
                        }
                    }
                }
            }
        }
        let all = procs_dict
            .keys()
            .filter_map(|k| procs_dict.get::<Stream<'a>>(&k))
            .collect();
        Some(Type3 {
            procs,
            all,
            resources: font.get::<Dict<'a>>(b"Resources").map(Resources::new),
        })
    }
}

/// The paint state the walk follows: tiling patterns run once per painting operator (and per
/// glyph), Type 3 fonts once per glyph.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct Paint {
    fill: u64,
    stroke: u64,
    /// The Type 3 font selected (by its dictionary's identity), if any.
    font: Option<usize>,
}

/// The walk of one page: memoised work of nested content, the nesting being walked, and the
/// Type 3 fonts seen.
#[derive(Default)]
struct Walk<'a> {
    memo: HashMap<Key, u64>,
    stack: Vec<Key>,
    fonts: HashMap<usize, Option<Type3<'a>>>,
    /// Of each Type 3 font: the costliest procedure.
    costliest: HashMap<(usize, usize, Paint), u64>,
    /// Instructions scanned: each is run at least once, so past [`MAX_PAGE_WORK`] the page is
    /// over the budget whatever the memo saved.
    scanned: u64,
}

/// The instructions page `page` runs (a lower bound once past [`MAX_PAGE_WORK`]): its content
/// and its annotations' appearances, with the content of every form XObject, tiling pattern
/// cell, soft mask and Type 3 glyph counted each time hayro runs it. Nested content is scanned
/// once per resources (memoised), so this costs one pass over the content the page can reach,
/// however often it is run: a DAG whose expansion is exponential is caught before hayro walks
/// it. Nested content that runs itself is over the budget: hayro follows such a cycle 50 forms
/// deep, and through patterns, masks and glyphs without a limit.
pub(crate) fn page_work(page: &Page<'_>) -> u64 {
    let mut walk = Walk::default();
    let res = page.resources();
    let mut total = page
        .page_stream()
        .map_or(0, |c| walk.content(c, res, Paint::default()));
    if let Some(annots) = page.raw().get::<Array<'_>>(b"Annots") {
        for annot in annots.iter::<Dict<'_>>() {
            if total > MAX_PAGE_WORK {
                break;
            }
            // Hidden annotations are not drawn.
            if annot.get::<u32>(b"F").unwrap_or(0) & 2 != 0 {
                continue;
            }
            if let Some(ap) = appearance(&annot) {
                let w = walk.nested(Kind::Form, &ap, None, res, Paint::default());
                total = total.saturating_add(w);
            }
        }
    }
    total
}

/// An annotation's normal appearance, as hayro picks it.
fn appearance<'a>(annot: &Dict<'a>) -> Option<Stream<'a>> {
    match annot.get::<Dict<'a>>(b"AP")?.get::<Object<'a>>(b"N")? {
        Object::Stream(s) => Some(s),
        Object::Dict(states) => annot
            .get::<Name<'_>>(b"AS")
            .and_then(|state| states.get::<Stream<'a>>(&state))
            .or_else(|| states.get::<Stream<'a>>(b"Off")),
        _ => None,
    }
}

impl<'a> Walk<'a> {
    /// Work of running `content` with `res`, starting with `paint`.
    fn content(&mut self, content: &[u8], res: &Resources<'a>, mut paint: Paint) -> u64 {
        let mut total = 0u64;
        let mut saved: Vec<Paint> = Vec::new();
        let mut shown: Vec<Vec<u8>> = Vec::new();
        let mut ops = UntypedIter::new(content);
        while let Some(ins) = ops.next() {
            total = total.saturating_add(1);
            self.scanned += 1;
            if self.scanned > MAX_PAGE_WORK {
                return u64::MAX;
            }
            let last = || ins.operands().last();
            let add = match ins.operator.as_ref() {
                b"q" => {
                    if saved.len() < 4096 {
                        saved.push(paint);
                    }
                    0
                }
                b"Q" => {
                    if let Some(p) = saved.pop() {
                        paint = p;
                    }
                    0
                }
                b"Do" => match last()
                    .and_then(|o| o.clone().into_name())
                    .and_then(|n| res.get_x_object(&n))
                {
                    Some(x) => match x.dict().get::<Name<'_>>(b"Subtype").as_deref() {
                        // A form starts with its caller's graphics state.
                        Some(b"Form") => self.nested(Kind::Form, &x, None, res, paint),
                        // A stencil mask is painted with the fill colour.
                        Some(b"Image") if x.dict().get::<bool>(b"ImageMask") == Some(true) => {
                            paint.fill
                        }
                        _ => 0,
                    },
                    None => 0,
                },
                op @ (b"scn" | b"SCN") => {
                    let cell = last()
                        .and_then(|o| o.clone().into_name())
                        .and_then(|n| res.get_pattern(&n))
                        .and_then(Object::into_stream)
                        .filter(|s| s.dict().get::<i32>(b"PatternType") == Some(1))
                        .map_or(0, |s| {
                            self.nested(Kind::Pattern, &s, None, res, Paint::default())
                        });
                    if op == b"scn" {
                        paint.fill = cell;
                    } else {
                        paint.stroke = cell;
                    }
                    0
                }
                b"g" | b"rg" | b"k" | b"cs" => {
                    paint.fill = 0;
                    0
                }
                b"G" | b"RG" | b"K" | b"CS" => {
                    paint.stroke = 0;
                    0
                }
                b"f" | b"F" | b"f*" => paint.fill,
                b"S" | b"s" => paint.stroke,
                b"B" | b"B*" | b"b" | b"b*" => paint.fill.saturating_add(paint.stroke),
                b"Tf" => {
                    paint.font = ins
                        .operands()
                        .next()
                        .and_then(|o| o.clone().into_name())
                        .and_then(|n| res.get_font(&n))
                        .and_then(|f| self.font(&f));
                    0
                }
                b"gs" => match last()
                    .and_then(|o| o.clone().into_name())
                    .and_then(|n| res.get_ext_g_state(&n))
                {
                    Some(gs) => {
                        if let Some(f) = gs.get::<Array<'_>>(b"Font") {
                            paint.font = f.iter::<Dict<'a>>().next().and_then(|f| self.font(&f));
                        }
                        // A soft mask is drawn once each time a state sets it.
                        gs.get::<Dict<'_>>(b"SMask")
                            .and_then(|m| m.get::<Stream<'a>>(b"G"))
                            .map_or(0, |g| {
                                self.nested(Kind::Form, &g, None, res, Paint::default())
                            })
                    }
                    None => 0,
                },
                op @ (b"Tj" | b"'" | b"\"" | b"TJ") => {
                    shown.clear();
                    let operand = if op == b"Tj" {
                        ins.operands().next()
                    } else {
                        last()
                    };
                    match operand {
                        Some(Object::String(s)) => shown.push(s.as_bytes().to_vec()),
                        // TJ: strings between kerning numbers.
                        Some(Object::Array(a)) => {
                            for o in a.iter::<Object<'_>>() {
                                if let Object::String(s) = o {
                                    shown.push(s.as_bytes().to_vec());
                                }
                            }
                        }
                        _ => {}
                    }
                    let mut w = 0u64;
                    for s in &shown {
                        for &code in s {
                            w = w.saturating_add(paint.fill);
                            if let Some(font) = paint.font {
                                w = w.saturating_add(self.glyph(font, code, res, paint));
                            }
                            if w > MAX_PAGE_WORK {
                                break;
                            }
                        }
                    }
                    w
                }
                _ => 0,
            };
            total = total.saturating_add(add);
            if total > MAX_PAGE_WORK {
                break;
            }
        }
        total
    }

    /// The Type 3 font `font` (by identity), if it is one.
    fn font(&mut self, font: &Dict<'a>) -> Option<usize> {
        let id = font.data().as_ptr() as usize;
        self.fonts
            .entry(id)
            .or_insert_with(|| Type3::of(font))
            .is_some()
            .then_some(id)
    }

    /// Work of drawing `code` in Type 3 font `font`, shown with resources `res` and `paint`
    /// (a glyph starts with the graphics state it is shown in).
    fn glyph(&mut self, font: usize, code: u8, res: &Resources<'a>, paint: Paint) -> u64 {
        let Some(Some(f)) = self.fonts.get(&font) else {
            return 0;
        };
        let own = f.resources.clone();
        match f.procs[usize::from(code)].clone() {
            Some(p) => self.nested(Kind::Glyph, &p, own.as_ref(), res, paint),
            None => {
                let key = (font, if own.is_some() { 0 } else { res_id(res) }, paint);
                if let Some(&w) = self.costliest.get(&key) {
                    return w;
                }
                let all = f.all.clone();
                let w = all
                    .iter()
                    .map(|p| self.nested(Kind::Glyph, p, own.as_ref(), res, paint))
                    .max()
                    .unwrap_or(0);
                self.costliest.insert(key, w);
                w
            }
        }
    }

    /// Work of running `stream` as `kind`, starting with `paint`: with its own resources, else
    /// `fallback` (a Type 3 font's), else its caller's `parent`.
    fn nested(
        &mut self,
        kind: Kind,
        stream: &Stream<'a>,
        fallback: Option<&Resources<'a>>,
        parent: &Resources<'a>,
        paint: Paint,
    ) -> u64 {
        let own = stream
            .dict()
            .get::<Dict<'a>>(b"Resources")
            .map(Resources::new);
        // A pattern without resources has none (it does not inherit its caller's).
        let inherits = own.is_none() && fallback.is_none() && kind != Kind::Pattern;
        let key = (
            kind,
            stream.obj_id(),
            if inherits { res_id(parent) } else { 0 },
            paint,
        );
        if let Some(&w) = self.memo.get(&key) {
            return w;
        }
        // Content that runs itself: see [`page_work`].
        if self.stack.contains(&key) || self.stack.len() >= MAX_FORM_DEPTH {
            return u64::MAX;
        }
        let Ok(content) = stream.decoded() else {
            return 0;
        };
        let empty = Resources::new(Dict::default());
        let res = match (&own, fallback) {
            (Some(r), _) => r.clone(),
            (None, Some(f)) => f.clone(),
            (None, None) if kind == Kind::Pattern => empty,
            (None, None) => parent.clone(),
        };
        self.stack.push(key);
        let w = self.content(&content, &res, paint);
        self.stack.pop();
        self.memo.insert(key, w);
        w
    }
}

#[cfg(test)]
mod tests {
    use super::procedure_depth;

    #[test]
    fn brace_depth() {
        assert_eq!(procedure_depth(b"{ 2 mul }"), 1);
        assert_eq!(procedure_depth(b"{ dup 0 gt { 1 } { 0 } ifelse }"), 2);
        assert_eq!(procedure_depth(b"} } {"), 1);
    }
}
