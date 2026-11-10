//! Units as the UI's language writes them (Hebrew writes the everyday units with its own
//! abbreviations): the tools' texts, and how a Hebrew label reads on screen, checked on the
//! shaper's clusters and on the drawn pixels (not by eye).

use specular_gfx::{Align, Text, TextStyle};
use specular_i18n::{I18n, langid, strip_isolates};

use super::measure_tool::length_with_unit;
use super::science::{area_text, linear_scale_bar};

fn he<R>(f: impl FnOnce() -> R) -> R {
    I18n::with_thread_language(langid!("he"), f)
}

fn plain(s: String) -> String {
    strip_isolates(&s).replace('\u{200f}', "")
}

/// The measuring tools, scale bars and areas write centimetres, metres and kilometres the
/// Hebrew way in Hebrew, µm and Å in Latin, and the SI symbols in English.
#[test]
fn tools_write_hebrew_units_in_hebrew() {
    he(|| {
        assert_eq!(plain(length_with_unit("12.5".into(), "mm")), "12.5 מ״מ");
        assert_eq!(plain(length_with_unit("12.5".into(), "cm")), "12.5 ס״מ");
        assert_eq!(plain(length_with_unit("12.5".into(), "m")), "12.5 מ׳");
        assert_eq!(plain(length_with_unit("12.5".into(), "km")), "12.5 ק״מ");
        assert_eq!(plain(length_with_unit("12.5".into(), "ft")), "12.5 רגל");
        assert_eq!(plain(length_with_unit("12.5".into(), "mi")), "12.5 מייל");
        assert_eq!(plain(length_with_unit("12.5".into(), "µm")), "12.5 µm");
        assert_eq!(plain(length_with_unit("12.5".into(), "\u{c5}")), "12.5 Å");
        // A georeferenced image's scale bar: metres, then kilometres.
        let bar = linear_scale_bar(1.0, "m", 1.0).unwrap();
        assert!(plain(bar.label.clone()).ends_with(" מ׳"), "{}", bar.label);
        let bar = linear_scale_bar(100.0, "m", 1.0).unwrap();
        assert!(plain(bar.label.clone()).ends_with(" ק״מ"), "{}", bar.label);
        // Microscopy stays in µm.
        let bar = linear_scale_bar(1e-6, "m", 1.0).unwrap();
        assert!(plain(bar.label.clone()).ends_with(" µm"), "{}", bar.label);
        assert!(area_text(2.0, "mm").ends_with(" ממ״ר"));
        assert!(area_text(2.0, "µm").ends_with(" µm²"));
        // The Info panel's own catalogue.
        let l = specular_metadata::Localizer::new("he".parse().unwrap());
        assert_eq!(l.unit("km"), "ק״מ");
        assert_eq!(l.unit("µm"), "µm");
    });
    assert_eq!(plain(length_with_unit("12.5".into(), "cm")), "12.5 cm");
    assert!(plain(linear_scale_bar(100.0, "m", 1.0).unwrap().label).ends_with(" km"));
}

/// A Hebrew length reads number first, unit second: from the right, so on screen the
/// number is right of the unit. Checked on the shaper's clusters (the caret before the
/// number is right of the caret before the unit, and the visual order is "unit number"),
/// and on the pixels: drawn right-aligned, the number alone covers the label's right end
/// and leaves the unit's width of ink to its left.
#[test]
fn hebrew_lengths_read_from_the_right() {
    let labels: Vec<(String, &str)> = he(|| {
        ["mm", "cm", "m", "km", "ha", "km²", "m²", "ft", "mi", "ac"]
            .into_iter()
            .map(|u| {
                let sym = specular_i18n::unit_symbol(u);
                let s = specular_i18n::t!("map-length", value = "1250", unit = sym);
                (s, u)
            })
            .chain([
                (length_with_unit("60.0".into(), "mm"), "mm"),
                (length_with_unit("60.0".into(), "cm"), "cm"),
            ])
            .collect()
    });
    let mut text = Text::bundled_only();
    text.begin_frame(1.0);
    let st = TextStyle::new(16.0);
    for (label, unit) in &labels {
        let shown = plain(label.clone());
        let num_at = shown.find(|c: char| c.is_ascii_digit()).unwrap();
        let num_end = shown[num_at..]
            .find(' ')
            .map_or(shown.len(), |i| num_at + i);
        let number = &shown[num_at..num_end];
        let sym = shown[num_end..].trim();
        assert!(
            sym.chars().any(specular_i18n::is_rtl_letter),
            "{unit}: {shown:?}"
        );
        // The shaper: one right-to-left line, the number on the right of the unit.
        let b = text.layout(label, st, None, Align::Start);
        assert!(b.rtl, "{unit}: {shown:?}");
        let n = label.find(number).unwrap();
        let u = label.find(sym).unwrap();
        assert!(b.caret_x(n) > b.caret_x(u), "{unit}: {shown:?}");
        let visual = text.visual_order(label);
        let reversed: String = sym.chars().rev().collect();
        assert!(
            visual.trim().starts_with(&reversed) && visual.trim().ends_with(number),
            "{unit}: {visual:?}"
        );
    }
    // The pixels.
    let Some(mut h) = specular_shots::Harness::new() else {
        return;
    };
    let size = (240.0, 40.0);
    let ink = |h: &mut specular_shots::Harness, s: &str| -> (u32, u32) {
        let s = s.to_owned();
        let shot = h.render(
            size,
            1.0,
            |dl| {
                dl.rect(
                    specular_gfx::Rect::new(0.0, 0.0, size.0, size.1),
                    0.0,
                    specular_theme::Color::hex(0xffffff),
                );
            },
            |dl, text| {
                let b = text.layout(&s, TextStyle::new(16.0), None, Align::Start);
                text.draw(
                    dl,
                    &b,
                    size.0 - 10.0 - b.width,
                    10.0,
                    specular_theme::Color::hex(0x000000),
                );
            },
        );
        let w = shot.width as usize;
        let cols: Vec<u32> = (0..shot.width)
            .filter(|&x| {
                (0..shot.height as usize).any(|y| shot.rgb[(y * w + x as usize) * 3] < 128)
            })
            .collect();
        (*cols.first().unwrap(), *cols.last().unwrap())
    };
    for (label, unit) in &labels {
        let shown = plain(label.clone());
        let num_at = shown.find(|c: char| c.is_ascii_digit()).unwrap();
        let num_end = shown[num_at..]
            .find(' ')
            .map_or(shown.len(), |i| num_at + i);
        let all = ink(&mut h, label);
        let number = ink(&mut h, &shown[num_at..num_end]);
        let sym = ink(&mut h, shown[num_end..].trim());
        // The number ends where the label does, on the right; the unit's ink is left of it.
        assert!(all.1.abs_diff(number.1) <= 1, "{unit}: {all:?} {number:?}");
        let unit_w = sym.1 - sym.0;
        assert!(
            number.0 >= all.0 + unit_w,
            "{unit}: label {all:?}, number {number:?}, unit {unit_w} px"
        );
    }
}
