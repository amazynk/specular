use crate::ctx::Ctx;
use specular_gfx::draw::kind;
use specular_gfx::{DrawList, Rect, Text};
use specular_theme::Palette;

const LONG: &str = "Rechercher dans les paramètres de l’application";

struct Drawn {
    /// Visible glyph boxes (rect ∩ clip), physical px.
    glyphs: Vec<[f32; 4]>,
    /// Caret boxes (accent-coloured rects, without anti-aliasing padding), physical px.
    carets: Vec<[f32; 4]>,
}

fn draw(scale: f32, rtl: bool, ts: f32, value: &str, preedit: &str) -> (Rect, Drawn) {
    let pal = Palette::dark();
    let mut dl = DrawList::new(scale);
    let mut text = Text::new();
    text.begin_frame(scale);
    let r = Rect::new(20.0, 30.0, 170.0, 32.0);
    {
        let mut cx = Ctx::new(
            &mut dl, &mut text, &pal, rtl, 400.0, 200.0, ts, 0.0, None, None,
        );
        cx.preedit = preedit.to_owned();
        cx.field(r, value, None, LONG, true, true);
    }
    let accent = pal.accent.to_linear_premul();
    let mut d = Drawn {
        glyphs: Vec::new(),
        carets: Vec::new(),
    };
    for p in &dl.prims {
        let [x, y, w, h] = p.rect;
        let [cx0, cy0, cx1, cy1] = p.clip;
        if p.params[2] == kind::GLYPH {
            let b = [x.max(cx0), y.max(cy0), (x + w).min(cx1), (y + h).min(cy1)];
            if b[2] > b[0] && b[3] > b[1] {
                d.glyphs.push(b);
            }
        } else if p.params[2] == kind::RECT && p.color == accent {
            let [x, y, w, h] = p.shape;
            d.carets.push([x, y, x + w, y + h]);
        }
    }
    (r, d)
}

#[test]
fn placeholder_stays_inside_and_clear_of_the_caret() {
    for scale in [1.0, 1.5, 2.0] {
        for rtl in [false, true] {
            for ts in [1.0, 2.0] {
                let (r, d) = draw(scale, rtl, ts, "", "");
                let case = format!("scale {scale} rtl {rtl} text scale {ts}");
                assert!(!d.glyphs.is_empty(), "{case}: no placeholder");
                let (fx0, fx1) = (r.x * scale, r.right() * scale);
                for g in &d.glyphs {
                    assert!(
                        g[0] >= fx0 - 0.5 && g[2] <= fx1 + 0.5,
                        "{case}: glyph {g:?} outside the field {fx0}..{fx1}"
                    );
                }
                assert_eq!(d.carets.len(), 1, "{case}: caret");
                let c = d.carets[0];
                assert!(
                    c[1] >= r.y * scale && c[3] <= r.bottom() * scale,
                    "{case}: caret height"
                );
                for g in &d.glyphs {
                    assert!(
                        c[2] <= g[0] + 0.01
                            || c[0] >= g[2] - 0.01
                            || c[3] <= g[1] + 0.01
                            || c[1] >= g[3] - 0.01,
                        "{case}: caret {c:?} overlaps glyph {g:?}"
                    );
                }
            }
        }
    }
}

/// A field sized with `Ctx::u` grows with the text size: its text and caret stay inside it
/// vertically (unclipped) at every text scale.
#[test]
fn scaled_fields_fit_their_text() {
    for ts in [0.8, 1.0, 1.5, 2.0] {
        let pal = Palette::dark();
        let mut dl = DrawList::new(1.0);
        let mut text = Text::new();
        text.begin_frame(1.0);
        let r = {
            let mut cx = Ctx::new(
                &mut dl, &mut text, &pal, false, 800.0, 600.0, ts, 0.0, None, None,
            );
            let r = Rect::new(20.0, 30.0, cx.u(240.0), cx.u(32.0));
            cx.field(r, "Hae kieliä", None, "", true, true);
            r
        };
        assert!(
            r.h >= 32.0 * ts.max(1.0) - 0.5,
            "text scale {ts}: height {}",
            r.h
        );
        let accent = pal.accent.to_linear_premul();
        let mut glyphs = 0;
        for p in &dl.prims {
            let [_, y, _, h] = if p.params[2] == kind::GLYPH {
                glyphs += 1;
                p.rect
            } else if p.params[2] == kind::RECT && p.color == accent {
                p.shape
            } else {
                continue;
            };
            assert!(
                y >= r.y - 0.5 && y + h <= r.bottom() + 0.5,
                "text scale {ts}: {y}..{} outside the field {}..{}",
                y + h,
                r.y,
                r.bottom()
            );
        }
        assert!(glyphs > 0, "text scale {ts}: no text");
    }
}

#[test]
fn text_or_composition_hides_the_placeholder() {
    let (_, empty) = draw(1.0, false, 1.0, "", "");
    for (value, preedit) in [("ab", ""), ("", "か")] {
        let (_, d) = draw(1.0, false, 1.0, value, preedit);
        assert!(
            !d.glyphs.is_empty() && d.glyphs.len() < empty.glyphs.len() / 4,
            "{value:?}/{preedit:?}: {} glyphs, placeholder had {}",
            d.glyphs.len(),
            empty.glyphs.len()
        );
    }
}
