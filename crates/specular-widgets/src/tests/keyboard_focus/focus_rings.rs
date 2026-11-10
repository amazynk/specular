use super::*;

/// A dialog's default button is the filled one, with no outline; the keyboard focus is
/// an outline set off from its button by a gap, on either button and in both themes.
#[test]
fn default_button_is_not_a_focus_ring() {
    let mut text = Text::new();
    with_image(|st| {
        for pal in [Palette::dark(), Palette::light()] {
            let mut c = chrome(900.0, false);
            c.confirm_trash("beach.jpg".into());
            let dl = frame(&mut c, &mut text, st, &pal);
            let ok = hit(&c, &Target::ConfirmOk);
            let cancel = hit(&c, &Target::ConfirmCancel);
            assert!(outlines_around(&dl, ok).is_empty(), "{:?}", pal.mode);
            assert!(outlines_around(&dl, cancel).is_empty(), "{:?}", pal.mode);
            for (t, r) in [(Target::ConfirmCancel, cancel), (Target::ConfirmOk, ok)] {
                tab(&mut c, st);
                assert_eq!(c.modal_focus.as_ref(), Some(&t));
                let dl = frame(&mut c, &mut text, st, &pal);
                let rings = outlines_around(&dl, r);
                assert_eq!(rings.len(), 1, "{t:?} {:?}: {rings:?}", pal.mode);
                let [x, y, w, h, bw] = rings[0];
                assert!(bw >= 2.0, "ring width {bw}");
                // At least 2 px of the sheet between the ring's inner edge and the button.
                for gap in [
                    r.x - (x + bw),
                    r.y - (y + bw),
                    x + w - bw - r.right(),
                    y + h - bw - r.bottom(),
                ] {
                    assert!(gap >= 1.99, "{t:?}: gap {gap}");
                }
                let other = if t == Target::ConfirmOk { cancel } else { ok };
                assert!(outlines_around(&dl, other).is_empty());
            }
        }
    });
}

/// Every dialog's Close (About, the shortcuts, Settings, the hardware report, the
/// insights) takes the push buttons' focus ring, set off from it by a gap of the sheet,
/// and no ring inside it (`Ctx::close_button`); left to right and right to left, text
/// scale 1 and 2, both themes.
#[test]
fn dialog_close_buttons_have_the_offset_ring() {
    let mut text = Text::new();
    with_image(|st| {
        for (rtl, scale, pal) in [
            (false, 1.0, Palette::dark()),
            (true, 1.0, Palette::light()),
            (false, 2.0, Palette::light()),
            (true, 2.0, Palette::dark()),
        ] {
            for m in [
                crate::Modal::About,
                crate::Modal::CheatSheet,
                crate::Modal::Settings,
                crate::Modal::HwReport,
                crate::Modal::Insights,
            ] {
                let case = format!("{m:?} rtl {rtl} scale {scale} {:?}", pal.mode);
                let mut c = chrome(900.0, rtl);
                c.text_scale = scale;
                match m {
                    crate::Modal::HwReport => c.show_report(crate::report::sample()),
                    crate::Modal::Insights => c.show_insights(content()),
                    crate::Modal::Settings => c.open_settings_at("viewing"),
                    m => c.open_modal(m),
                }
                frame(&mut c, &mut text, st, &pal);
                let r = hit(&c, &Target::ModalClose);
                // Unfocused: no outline at all.
                let dl = frame(&mut c, &mut text, st, &pal);
                assert!(outlines_around(&dl, r).is_empty(), "{case}");
                c.modal_focus = Some(Target::ModalClose);
                let dl = frame(&mut c, &mut text, st, &pal);
                assert_eq!(c.modal_focus, Some(Target::ModalClose), "{case}");
                let rings = outlines_around(&dl, r);
                assert_eq!(rings.len(), 1, "{case}: {rings:?}");
                let [x, y, w, h, bw] = rings[0];
                assert!(bw >= 2.0, "{case}: ring width {bw}");
                for gap in [
                    r.x - (x + bw),
                    r.y - (y + bw),
                    x + w - bw - r.right(),
                    y + h - bw - r.bottom(),
                ] {
                    assert!(gap >= 1.99, "{case}: gap {gap}");
                }
                // Nor the icon buttons' inner ring.
                let inner = dl
                    .prims
                    .iter()
                    .filter(|p| {
                        let [px, py, pw, ph] = p.shape;
                        p.params[2] == specular_gfx::draw::kind::RECT
                            && p.params[0] > 0.0
                            && p.color[3] == 0.0
                            && p.color2[3] > 0.0
                            && px >= r.x - 0.01
                            && py >= r.y - 0.01
                            && px + pw <= r.right() + 0.01
                            && py + ph <= r.bottom() + 0.01
                    })
                    .count();
                assert_eq!(inner, 0, "{case}");
            }
        }
    });
}

/// The focus ring keeps 3:1 against the sheets it is drawn on, which is also what fills
/// a button ring's gap, in both themes (WCAG 2.4.11 / 1.4.11).
#[test]
fn focus_ring_contrast() {
    let ratio = |a: specular_theme::Color, b: specular_theme::Color| {
        let (la, lb) = (a.luminance(), b.luminance());
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    };
    for pal in [Palette::dark(), Palette::light()] {
        for bg in [pal.surface, pal.surface_raised] {
            let r = ratio(pal.focus_ring, bg);
            assert!(r >= 3.0, "{:?}: {r:.2}", pal.mode);
        }
    }
}

/// Text and focus rings of the newer views (Develop's panel, its labels
/// over the picture and reason tooltips, the sound page) meet WCAG AA in both themes:
/// 4.5:1 for text, 3:1 for the ring, computed from the palette on what each is drawn on.
/// The labels over the picture sit on the translucent glass: checked over white and black.
#[test]
fn new_views_meet_aa() {
    use specular_theme::Color;
    let ratio = |a: Color, b: Color| {
        let (la, lb) = (a.luminance(), b.luminance());
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    };
    let over = |top: Color, bottom: Color| bottom.lerp(top.with_alpha(1.0), top.a).with_alpha(1.0);
    for p in [Palette::dark(), Palette::light()] {
        let text = [
            // Develop: card titles, values, history (muted: the steps that can be redone).
            ("text on card", p.text, p.surface_raised),
            ("muted on card", p.text_muted, p.surface_raised),
            ("muted on panel", p.text_muted, p.sidebar),
            ("choice value on card", p.accent, p.surface_raised),
            (
                "reason tooltip",
                p.tooltip_text,
                over(p.tooltip_bg, Color::WHITE),
            ),
            (
                "label on glass over white",
                p.text,
                over(p.glass, Color::WHITE),
            ),
            (
                "label on glass over black",
                p.text,
                over(p.glass, Color::BLACK),
            ),
            // The sound page: panels on the canvas, chips on the canvas or the accent tint.
            ("sound text", p.text, p.surface),
            ("sound muted", p.text_muted, p.surface),
            ("sound chip off", p.text_muted, p.canvas),
            ("sound chip on", p.text, over(p.accent_soft, p.canvas)),
        ];
        for (what, fg, bg) in text {
            let r = ratio(fg, bg);
            assert!(r >= 4.5, "{:?} {what}: {r:.2}", p.mode);
        }
        // Every palette colour used for text, on every surface, plain and hovered (the
        // accent value on a hovered card was 4.1:1 in light and 3.5:1 in dark).
        let surfaces = [
            ("canvas", p.canvas),
            ("surface", p.surface),
            ("card", p.surface_raised),
            ("panel", p.sidebar),
        ];
        let inks = [
            ("text", p.text),
            ("muted", p.text_muted),
            ("accent", p.accent),
            ("success", p.success),
            ("warning", p.warning),
            ("danger", p.danger),
        ];
        for (page, base) in surfaces {
            for (state, bg) in [("", base), (" hovered", over(p.surface_hover, base))] {
                for (ink, fg) in inks {
                    let r = ratio(fg, bg);
                    assert!(r >= 4.5, "{:?} {ink} on {page}{state}: {r:.2}", p.mode);
                }
            }
            // Text on the accent band (the chosen row, an on chip).
            for (ink, fg) in [("text", p.text), ("muted", p.text_muted)] {
                let r = ratio(fg, over(p.accent_soft, base));
                assert!(
                    r >= 4.5,
                    "{:?} {ink} on the accent band on {page}: {r:.2}",
                    p.mode
                );
            }
            // A pressed row or button (the press replaces the hover and chosen
            // tints): its label at 4.5:1; every ink as a mark (an active button's accent
            // icon, a status dot) at 3:1.
            let pressed = over(p.surface_pressed, base);
            let r = ratio(p.text, pressed);
            assert!(r >= 4.5, "{:?} text on {page} pressed: {r:.2}", p.mode);
            for (ink, fg) in inks {
                let r = ratio(fg, pressed);
                assert!(
                    r >= 3.0,
                    "{:?} {ink} mark on {page} pressed: {r:.2}",
                    p.mode
                );
            }
            // Selected text in a text field: the text on the selection tint.
            let r = ratio(p.text, over(p.selection, base));
            assert!(
                r >= 4.5,
                "{:?} text on the selection on {page}: {r:.2}",
                p.mode
            );
        }
        // A switch that is on: the white knob on the filled track, plain and hovered (was
        // the accent text colour, 2.4:1 in dark).
        for (state, track) in [
            ("", p.accent_fill),
            (" hovered", over(p.surface_hover, p.accent_fill)),
        ] {
            let r = ratio(Color::WHITE, track);
            assert!(r >= 3.0, "{:?} knob on the track{state}: {r:.2}", p.mode);
        }
        // Button labels (white in both themes) on the filled default and destructive
        // buttons: plain, hovered (lightened 12 % towards the label, or drawn at 85 % over
        // the page).
        for (what, fill) in [("accent", p.accent_fill), ("danger", p.danger_fill)] {
            let mut backs = vec![("plain", fill), ("hovered", fill.lerp(p.accent_text, 0.12))];
            backs.push(("hovered towards text", fill.lerp(p.text, 0.12)));
            for (page, base) in surfaces {
                backs.push((page, over(fill.with_alpha(0.85), base)));
            }
            for (state, bg) in backs {
                let r = ratio(p.accent_text, bg);
                assert!(
                    r >= 4.5,
                    "{:?} label on {what} fill {state}: {r:.2}",
                    p.mode
                );
            }
        }
        assert_eq!(p.accent_text, Color::WHITE, "button labels stay white");
        for (what, bg) in [
            ("panel", p.sidebar),
            ("card", p.surface_raised),
            ("sound page", p.canvas),
            ("lyrics", p.surface),
        ] {
            let r = ratio(p.focus_ring, bg);
            assert!(r >= 3.0, "{:?} ring on {what}: {r:.2}", p.mode);
        }
    }
}

/// The quiet card switch (Develop's cards, the equaliser's on switch) still reads
/// as on or off. On: a track filled with the muted ink (the text ink hovered), the knob in
/// the card colour at the end. Off: an outline in the faint ink (text hovered) with a knob
/// of the same ink at the start. Every mark at 3:1 on the card (the switch has its own
/// hit area, so the header's hover tint is never under it), and the plain on and off inks
/// apart from each other too.
#[test]
fn the_quiet_switch_reads_on_and_off() {
    use specular_theme::Color;
    let ratio = |a: Color, b: Color| {
        let (la, lb) = (a.luminance(), b.luminance());
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    };
    for p in [Palette::dark(), Palette::light()] {
        let m = p.mode;
        let card = p.surface_raised;
        for (state, on_ink, off_ink) in [
            ("", p.text_muted, p.text_faint),
            (" hovered", p.text, p.text),
        ] {
            let r = ratio(on_ink, card);
            assert!(r >= 3.0, "{m:?} on track on the card{state}: {r:.2}");
            let r = ratio(card, on_ink);
            assert!(r >= 3.0, "{m:?} on knob on its track{state}: {r:.2}");
            let r = ratio(off_ink, card);
            assert!(r >= 3.0, "{m:?} off outline and knob{state}: {r:.2}");
        }
        let r = ratio(p.text_muted, p.text_faint);
        assert!(r >= 1.5, "{m:?} on ink vs off ink: {r:.2}");
    }
}
