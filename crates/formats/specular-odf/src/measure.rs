//! Text measurement with the drawing stack's fonts, for what the layout leaves to the
//! reader: list labels aligned to their label width, and text fitted to its frame.

use std::sync::Arc;

use specular_drawpage::Run;
use specular_vector::fonts::{Face, Fonts};
use specular_vector::scene::FontSpec;

fn face(family: &str, bold: bool, italic: bool) -> Arc<Face> {
    let fonts = Fonts::shared();
    let r = fonts.resolve(&FontSpec::default(), Some(family), bold, italic);
    if r.substituted {
        let lower = family.to_ascii_lowercase();
        let generic = if lower.contains("mono") || lower.contains("courier") {
            "Courier New"
        } else if lower.contains("serif") && !lower.contains("sans") || lower.contains("times") {
            "Times New Roman"
        } else {
            "Arial"
        };
        return fonts
            .resolve(&FontSpec::default(), Some(generic), bold, italic)
            .face;
    }
    r.face
}

/// The advance width of `run`'s text on one line, points.
pub fn width(run: &Run) -> f64 {
    let f = face(&run.family, run.bold, run.italic);
    let k = run.size / f64::from(f.units_per_em.max(1.0));
    run.text
        .chars()
        .take(100_000)
        .map(|c| {
            let c = if c == '\u{a0}' || c == '\t' { ' ' } else { c };
            f.glyph(c)
                .map_or(f64::from(f.units_per_em) * 0.5, |g| f64::from(g.advance))
                * k
                + run.letter_spacing
        })
        .sum()
}

/// The natural line height (ascent − descent) of `run`'s font at its size, points.
pub fn line_height(run: &Run) -> f64 {
    let f = face(&run.family, run.bold, run.italic);
    (f64::from(f.ascent) - f64::from(f.descent)) * run.size / f64::from(f.units_per_em.max(1.0))
}
