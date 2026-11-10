//! Comparison of our reading of a face with ttf-parser's (the oracle).

#![allow(dead_code)]

use specular_font::{Face, PathEl, Permission};

/// Mismatches by kind, with the first few described.
#[derive(Default, Debug)]
pub struct Tally {
    pub checked: usize,
    pub mismatches: usize,
    pub notes: Vec<String>,
}

impl Tally {
    fn bad(&mut self, what: String) {
        self.mismatches += 1;
        if self.notes.len() < 20 {
            self.notes.push(what);
        }
    }
    pub fn check_pub(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.check(ok, what)
    }
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.checked += 1;
        if !ok {
            self.bad(what());
        }
    }
}

#[derive(Default)]
struct Rec(Vec<PathEl>);

impl ttf_parser::OutlineBuilder for Rec {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(PathEl::MoveTo(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(PathEl::LineTo(x, y));
    }
    fn quad_to(&mut self, a: f32, b: f32, x: f32, y: f32) {
        self.0.push(PathEl::QuadTo(a, b, x, y));
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.0.push(PathEl::CubicTo(a, b, c, d, x, y));
    }
    fn close(&mut self) {
        self.0.push(PathEl::Close);
    }
}

fn coords(e: &PathEl) -> (u8, Vec<f32>) {
    match *e {
        PathEl::MoveTo(x, y) => (0, vec![x, y]),
        PathEl::LineTo(x, y) => (1, vec![x, y]),
        PathEl::QuadTo(a, b, x, y) => (2, vec![a, b, x, y]),
        PathEl::CubicTo(a, b, c, d, x, y) => (3, vec![a, b, c, d, x, y]),
        PathEl::Close => (4, vec![]),
    }
}

pub fn same_path(a: &[PathEl], b: &[PathEl]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(p, q)| {
            let (k1, v1) = coords(p);
            let (k2, v2) = coords(q);
            k1 == k2 && v1.iter().zip(&v2).all(|(x, y)| (x - y).abs() <= 1e-3)
        })
}

/// The oracle's outline of `gid`.
pub fn oracle_outline(f: &ttf_parser::Face<'_>, gid: u16) -> Option<Vec<PathEl>> {
    let mut r = Rec::default();
    f.outline_glyph(ttf_parser::GlyphId(gid), &mut r)?;
    (!r.0.is_empty()).then_some(r.0)
}

fn name_rank(p: ttf_parser::PlatformId, e: u16, l: u16) -> Option<u8> {
    use ttf_parser::PlatformId::*;
    Some(match (p, e) {
        (Windows, 0 | 1 | 10) if l == 0x0409 => 0,
        (Windows, 0 | 1 | 10) if l & 0xFF == 0x09 => 1,
        (Windows, 0 | 1 | 10) => 2,
        (Unicode, _) => 3,
        _ => return None,
    })
}

fn oracle_name(f: &ttf_parser::Face<'_>, id: u16) -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for n in f.names() {
        if n.name_id != id {
            continue;
        }
        let Some(k) = name_rank(n.platform_id, n.encoding_id, n.language_id) else {
            continue;
        };
        if best.as_ref().is_some_and(|b| b.0 <= k) {
            continue;
        }
        if let Some(s) = n.to_string() {
            best = Some((k, s));
        }
    }
    best.map(|b| b.1)
}

/// Compares every fact the oracle reads, and every outline.
pub fn compare(face: &Face, label: &str, t: &mut Tally) {
    let data = face.sfnt();
    let f = match ttf_parser::Face::parse(&data, face.index()) {
        Ok(f) => f,
        Err(e) => {
            t.bad(format!("{label}: oracle cannot parse: {e}"));
            return;
        }
    };
    t.check(f.number_of_glyphs() == face.glyph_count, || {
        format!(
            "{label}: glyphs {} vs {}",
            face.glyph_count,
            f.number_of_glyphs()
        )
    });
    t.check(f.units_per_em() == face.metrics.units_per_em, || {
        format!("{label}: upem")
    });
    let n = &face.names;
    for (id, ours) in [
        (0, &n.copyright),
        (1, &n.family),
        (2, &n.subfamily),
        (4, &n.full_name),
        (5, &n.version),
        (6, &n.postscript),
        (8, &n.manufacturer),
        (9, &n.designer),
        (11, &n.vendor_url),
        (12, &n.designer_url),
        (13, &n.licence),
        (14, &n.licence_url),
        (16, &n.typographic_family),
        (17, &n.typographic_subfamily),
        (19, &n.sample_text),
    ] {
        if let Some(o) = oracle_name(&f, id) {
            t.check(&o == ours, || {
                format!("{label}: name {id}: {ours:?} vs {o:?}")
            });
        }
    }
    // The character map both ways: ours against the oracle, and every Unicode code point the
    // oracle's subtables list.
    for &(c, g) in face.chars() {
        let o = char::from_u32(c)
            .and_then(|ch| f.glyph_index(ch))
            .map(|g| g.0);
        t.check(o == Some(g), || {
            format!("{label}: cmap U+{c:04X}: {g} vs {o:?}")
        });
    }
    if let Some(cmap) = f.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            let mut cps = Vec::new();
            sub.codepoints(|c| cps.push(c));
            for c in cps {
                let Some(g) = sub
                    .glyph_index(c)
                    .filter(|g| g.0 != 0 && g.0 < face.glyph_count)
                else {
                    continue;
                };
                let ours = char::from_u32(c).and_then(|ch| face.glyph_index(ch));
                if f.glyph_index(char::from_u32(c).unwrap_or('\0')) != Some(g) {
                    continue;
                }
                t.check(ours == Some(g.0), || format!("{label}: missed U+{c:04X}"));
            }
        }
    }
    if let Some(os2) = f.tables().os2 {
        let perm = match os2.permissions() {
            Some(ttf_parser::Permissions::Installable) => Some(Permission::Installable),
            Some(ttf_parser::Permissions::Restricted) => Some(Permission::Restricted),
            Some(ttf_parser::Permissions::PreviewAndPrint) => Some(Permission::PreviewPrint),
            Some(ttf_parser::Permissions::Editable) => Some(Permission::Editable),
            None => None,
        };
        let e = face.embedding;
        if perm.is_some() {
            t.check(e.map(|e| e.permission) == perm, || {
                format!("{label}: fsType")
            });
        }
        if let Some(e) = e {
            t.check(e.no_subsetting == !os2.is_subsetting_allowed(), || {
                format!("{label}: subsetting")
            });
            t.check(e.bitmap_only == !os2.is_outline_embedding_allowed(), || {
                format!("{label}: bitmap only")
            });
        }
        t.check(os2.weight().to_number() == face.weight, || {
            format!("{label}: weight")
        });
    }
    let axes: Vec<_> = f.variation_axes().into_iter().collect();
    t.check(axes.len() == face.axes.len(), || {
        format!("{label}: axis count")
    });
    for (a, b) in axes.iter().zip(&face.axes) {
        t.check(
            a.tag.to_bytes() == b.tag
                && a.min_value == b.min
                && a.def_value == b.default
                && a.max_value == b.max
                && a.hidden == b.hidden,
            || format!("{label}: axis {}", b.tag_str()),
        );
    }
    let o = face.outliner();
    for g in 0..face.glyph_count {
        t.check(
            f.glyph_hor_advance(ttf_parser::GlyphId(g)) == face.advance(g),
            || format!("{label}: advance {g}"),
        );
        if let Some(name) = f.glyph_name(ttf_parser::GlyphId(g)) {
            let ours = face.glyph_name(g);
            t.check(ours.as_deref() == Some(name), || {
                format!("{label}: glyph name {g}: {ours:?} vs {name}")
            });
        }
        let ours = o.outline(g).map(|o| o.path);
        let mut theirs = oracle_outline(&f, g);
        // ttf-parser leaves a CFF2 glyph's last contour open (CFF2 has no endchar).
        if face.outlines == specular_font::OutlineKind::Cff2
            && let Some(p) = theirs.as_mut()
            && p.last() != Some(&PathEl::Close)
        {
            p.push(PathEl::Close);
        }
        let ok = match (&ours, &theirs) {
            (Some(a), Some(b)) => same_path(a, b),
            (None, None) => true,
            _ => false,
        };
        t.check(ok, || {
            format!(
                "{label}: outline {g}: {} vs {} commands",
                ours.as_ref().map_or(0, |p| p.len()),
                theirs.as_ref().map_or(0, |p| p.len())
            )
        });
    }
}
