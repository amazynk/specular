//! Map furniture and small panels: the scale bar (metric or imperial, round lengths), the
//! north arrow (when north is not up), the overview map (the world's coastlines around the
//! map, its extent and the part on screen), the Go to coordinate field, the identify panel
//! and the measuring tool's result.

use specular_decode::geo::specular_gis::coastline::coastlines;
use specular_geodesy::coords::{CoordFormat, format};
use specular_geodesy::units::{self, Units, format_value};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_BODY, FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

use super::super::science::ScaleBar;

/// The longest scale bar, logical px.
pub(super) const SCALE_MAX_PX: f64 = 140.0;
/// The overview map's size, logical px.
pub(super) const OVERVIEW_W: f32 = 200.0;
pub(super) const OVERVIEW_H: f32 = 120.0;
/// The north arrow's diameter, logical px.
pub(super) const NORTH_D: f32 = 40.0;
/// North further than this from straight up (degrees) shows the arrow.
pub(super) const NORTH_SLACK: f64 = 0.5;

/// A position as the readout writes it in `f` ("Lat, lon 47.376887, 8.541694",
/// "UTM 32T 463936 5247407").
pub(super) fn position_line(lat: f64, lon: f64, f: CoordFormat) -> Option<String> {
    let p = format(lat, lon, f)?;
    Some(t!("map-position", format = f.id(), position = p))
}

/// The scale bar for `metres_per_px` ground metres per logical px.
pub(super) fn scale_bar(metres_per_px: f64, u: Units) -> Option<ScaleBar> {
    if !(metres_per_px.is_finite() && metres_per_px > 0.0) {
        return None;
    }
    let c = units::scale_bar(metres_per_px * SCALE_MAX_PX, u)?;
    let px = (c.metres / metres_per_px) as f32;
    (px >= 8.0).then(|| ScaleBar {
        px,
        label: t!(
            "map-length",
            value = format_value(c.value),
            unit = specular_i18n::unit_symbol(c.unit.symbol())
        ),
    })
}

/// Draws the north arrow pointing at `angle` (radians clockwise from screen up) in the
/// top-end corner of `area`, below `below` (the overview when it shows).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_north(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    angle: f64,
    area: (f32, f32, f32, f32),
    below: Option<Rect>,
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, ay, aw, _) = area;
    let m = TOOLBAR_MARGIN;
    let d = NORTH_D * ts;
    let x = if rtl { ax + m } else { ax + aw - m - d };
    let y = below.map_or(ay + m, |b| b.y + b.h + 8.0 * ts);
    let r = Rect::new(x, y, d, d);
    dl.glass(r, d / 2.0, pal.glass, pal.glass_border, 1.0);
    let (cx, cy) = r.center();
    let (s, c) = (angle.sin() as f32, angle.cos() as f32);
    let at = |along: f32, across: f32| (cx + along * s + across * c, cy - along * c + across * s);
    let len = d * 0.36;
    let tip = at(len * 0.45, 0.0);
    let tail = at(-len, 0.0);
    let (l, rr) = (at(-len * 0.05, -len * 0.32), at(-len * 0.05, len * 0.32));
    dl.line(tail, tip, 2.0 * ts, pal.text_muted);
    dl.line(l, tip, 2.0 * ts, pal.danger);
    dl.line(rr, tip, 2.0 * ts, pal.danger);
    let n = text.layout(
        "N",
        TextStyle::new(FONT_SMALL * 0.9 * ts).weight(700.0),
        None,
        Align::Start,
    );
    let (nx, ny) = at(len * 0.9, 0.0);
    text.draw(dl, &n, nx - n.width / 2.0, ny - n.height / 2.0, pal.text);
    r
}

/// What the overview map shows: a window in degrees (lon0, lat0, lon1, lat1) and, inside
/// it, the map's extent and the part on screen as rings of (lon, lat).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Overview {
    pub window: [f64; 4],
    pub extent: Vec<(f64, f64)>,
    pub visible: Vec<(f64, f64)>,
}

/// The window around a ring of (lon, lat): its bounds widened eight times (at least 20°
/// across, at most the world), keeping the screen's aspect of the panel.
pub(super) fn window_around(ring: &[(f64, f64)]) -> Option<[f64; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(lon, lat) in ring {
        x0 = x0.min(lon);
        x1 = x1.max(lon);
        y0 = y0.min(lat);
        y1 = y1.max(lat);
    }
    if x0 > x1 {
        return None;
    }
    let aspect = f64::from(OVERVIEW_W / OVERVIEW_H);
    let mut w = ((x1 - x0) * 8.0).clamp(20.0, 360.0);
    let mut h = ((y1 - y0) * 8.0).max(w / aspect).min(180.0);
    w = w.max(h * aspect).min(360.0);
    h = h.min(w / aspect).min(180.0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let cy = cy.clamp(-90.0 + h / 2.0, 90.0 - h / 2.0);
    Some([cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0])
}

/// Draws the overview map in the top-end corner of `area`.
pub(super) fn draw_overview(
    dl: &mut DrawList,
    pal: &Palette,
    o: &Overview,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, ay, aw, _) = area;
    let m = TOOLBAR_MARGIN;
    let (w, h) = (OVERVIEW_W * ts, OVERVIEW_H * ts);
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let r = Rect::new(x, ay + m, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let inner = r.inset(4.0 * ts);
    let [wx0, wy0, wx1, wy1] = o.window;
    let to = |lon: f64, lat: f64| {
        (
            inner.x + ((lon - wx0) / (wx1 - wx0)) as f32 * inner.w,
            inner.y + ((wy1 - lat) / (wy1 - wy0)) as f32 * inner.h,
        )
    };
    dl.push_clip(inner);
    let land = pal.text_muted.with_alpha(0.7);
    for line in coastlines() {
        let mut prev: Option<(f32, f32)> = None;
        for p in line {
            let (lon, lat) = (f64::from(p[0]), f64::from(p[1]));
            let inside =
                lon >= wx0 - 10.0 && lon <= wx1 + 10.0 && lat >= wy0 - 10.0 && lat <= wy1 + 10.0;
            let q = to(lon, lat);
            if let (Some(a), true) = (prev, inside) {
                dl.line(a, q, ts, land);
            }
            prev = inside.then_some(q);
        }
    }
    let ring = |pts: &[(f64, f64)], dl: &mut DrawList, c: Color, wd: f32| {
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            dl.line(to(a.0, a.1), to(b.0, b.1), wd, c);
        }
    };
    ring(&o.extent, dl, pal.text, ts);
    if !o.visible.is_empty() {
        // The part on screen: a box, and a dot when it is too small to see.
        ring(&o.visible, dl, pal.accent, 2.0 * ts);
        let (sx, sy) = o
            .visible
            .iter()
            .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        let n = o.visible.len() as f64;
        dl.circle(to(sx / n, sy / n), 2.5 * ts, pal.accent);
    }
    dl.pop_clip();
    r
}

/// The Go to coordinate field.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct GoTo {
    pub text: String,
    /// Why the last Enter did not go anywhere.
    pub error: Option<String>,
}

impl GoTo {
    /// Adds pasted text: line breaks and tabs (a coordinate copied from a table or two
    /// lines) become single spaces; leading and trailing space is dropped.
    pub(super) fn paste(&mut self, s: &str) {
        let words: Vec<&str> = s
            .split(|c: char| c.is_whitespace() || c.is_control())
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return;
        }
        if !self.text.is_empty() && !self.text.ends_with(' ') && s.starts_with(char::is_whitespace)
        {
            self.text.push(' ');
        }
        self.text.push_str(&words.join(" "));
        self.error = None;
    }
}

/// Draws a small panel of `lines` at the top centre of `area` (`y` below the top margin),
/// the first line strong. With `field` (its text and an input method's composition), the
/// first line is a text field holding it; returns the panel and where the field's input
/// method window opens.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_box(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    title: &str,
    field: Option<(&str, &str)>,
    lines: &[String],
    area: (f32, f32, f32, f32),
    y: f32,
    rtl: bool,
    ts: f32,
) -> (Rect, Option<Rect>) {
    let (ax, _, aw, _) = area;
    let st = TextStyle::new(FONT_SMALL * ts);
    let tb = text.layout(title, st.weight(600.0), None, Align::Start);
    let blocks: Vec<_> = lines
        .iter()
        .map(|l| text.layout(l, st.tabular(), Some(aw * 0.8), Align::Start))
        .collect();
    let field_h = (FONT_BODY * 2.0 * ts).round();
    let field_w = (320.0 * ts).min(aw * 0.8);
    let w = blocks
        .iter()
        .map(|b| b.width)
        .chain([tb.width, if field.is_some() { field_w } else { 0.0 }])
        .fold(0.0, f32::max)
        + 2.0 * PAD;
    let h = 2.0 * PAD
        + tb.height
        + 4.0 * ts
        + field.map_or(0.0, |_| field_h + 4.0 * ts)
        + blocks.iter().map(|b| b.height + 2.0 * ts).sum::<f32>();
    let x = ax + (aw - w) / 2.0;
    let r = Rect::new(x, y, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let line_x = |bw: f32| if rtl { x + w - PAD - bw } else { x + PAD };
    let mut ty = y + PAD;
    text.draw(dl, &tb, line_x(tb.width), ty, pal.text);
    ty += tb.height + 4.0 * ts;
    let mut ime = None;
    if let Some((value, preedit)) = field {
        let f = Rect::new(x + PAD, ty, w - 2.0 * PAD, field_h);
        dl.rect_border(f, RADIUS_SM, pal.surface, pal.accent, 1.5 * ts);
        // Coordinates read left to right in every language.
        let st = TextStyle::new(FONT_BODY * ts).tabular();
        ime = Some(draw_field_text(
            dl,
            text,
            pal,
            (value, preedit),
            st,
            f.inset_xy(8.0 * ts, 0.0),
            (false, 5.0 * ts, ts),
            pal.text,
        ));
        ty += field_h + 4.0 * ts;
    }
    for b in &blocks {
        text.draw(dl, b, line_x(b.width), ty, pal.text_muted);
        ty += b.height + 2.0 * ts;
    }
    (r, ime)
}

/// Draws a one-line field's text in `room` with the caret after it (the fields on maps
/// type at the end): an input method's composition (`value.1`) after the text, underlined,
/// the caret after the composition, kept in view by the shared field helper. `start` is
/// whether text that fits starts at the right (`start.0`) and the caret's inset from the
/// field's top and bottom (`start.1`) and the scale (`start.2`). Returns where the input
/// method's window opens.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_field_text(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    (value, preedit): (&str, &str),
    st: TextStyle,
    room: Rect,
    (start_right, inset, ts): (bool, f32, f32),
    ink: Color,
) -> Rect {
    use specular_widgets::line_edit::{compose, draw_preedit_underline, field_x0, ime_area};
    let (shown, range) = compose(value, value.len(), preedit);
    let b = text.layout(&shown, st, None, Align::Start);
    let cx = b.caret_x(range.end);
    let x0 = field_x0(&b, cx, room, start_right || b.rtl);
    let ty = room.y + (room.h - b.height) / 2.0;
    dl.push_clip(room.inset_xy(-2.0, 0.0));
    text.draw(dl, &b, x0, ty, ink);
    if !range.is_empty() {
        draw_preedit_underline(dl, &b, x0, range.clone(), ty + b.height, pal.text_muted);
    }
    dl.pop_clip();
    let x = (x0 + cx).clamp(room.x - 2.0, room.right() + 2.0);
    let (top, bottom) = (room.y + inset, room.bottom() - inset);
    dl.line((x, top), (x, bottom), ts, pal.text);
    let caret = Rect::new(x - 0.75, top, 1.5, bottom - top);
    if range.is_empty() {
        caret
    } else {
        ime_area(&b, x0, range, caret.y, caret.h, room)
    }
}

/// Draws the identify panel (what was clicked: a feature's attributes, a cell's values) at
/// the end side of `area`, from `y`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_identify(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    title: &str,
    rows: &[(String, String)],
    more: usize,
    area: (f32, f32, f32, f32),
    y: f32,
    rtl: bool,
    ts: f32,
) -> Rect {
    let (ax, _, aw, _) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let w = (260.0 * ts).min(aw - 2.0 * m);
    let tb = text.layout(title, st.weight(600.0), Some(w - 2.0 * PAD), Align::Start);
    let key_w = w * 0.42;
    let laid: Vec<_> = rows
        .iter()
        .map(|(k, v)| {
            (
                text.layout(k, st, Some(key_w - 6.0 * ts), Align::Start),
                text.layout(v, st.tabular(), Some(w - 2.0 * PAD - key_w), Align::Start),
            )
        })
        .collect();
    let more_b = (more > 0).then(|| {
        text.layout(
            &t!("map-identify-more", count = more as i64),
            st,
            None,
            Align::Start,
        )
    });
    let h = 2.0 * PAD
        + tb.height
        + 6.0 * ts
        + laid
            .iter()
            .map(|(k, v)| k.height.max(v.height) + 3.0 * ts)
            .sum::<f32>()
        + more_b.as_ref().map_or(0.0, |b| b.height + 2.0 * ts);
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let r = Rect::new(x, y, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let start = |bw: f32, from: f32, span: f32| if rtl { from + span - bw } else { from };
    let mut ty = y + PAD;
    text.draw(
        dl,
        &tb,
        start(tb.width, x + PAD, w - 2.0 * PAD),
        ty,
        pal.text,
    );
    ty += tb.height + 6.0 * ts;
    for (k, v) in &laid {
        let (kx, vx) = if rtl {
            (x + w - PAD - k.width, x + PAD)
        } else {
            (x + PAD, x + PAD + key_w)
        };
        text.draw(dl, k, kx, ty, pal.text_muted);
        text.draw(dl, v, vx, ty, pal.text);
        ty += k.height.max(v.height) + 3.0 * ts;
    }
    if let Some(b) = &more_b {
        text.draw(
            dl,
            b,
            start(b.width, x + PAD, w - 2.0 * PAD),
            ty,
            pal.text_muted,
        );
    }
    r
}

#[cfg(test)]
mod tests {
    use specular_gfx::{Align, Rect, TextStyle};

    /// A map field draws an input method's composition after its text, the caret after the
    /// composition and the candidate window over it; Hebrew text in a right-to-left field
    /// keeps its start on the right, the composition and caret on its left.
    #[test]
    fn fields_draw_the_composition_at_the_caret() {
        let Some(mut h) = specular_shots::Harness::new() else {
            return;
        };
        let pal = specular_theme::Palette::dark();
        let room = Rect::new(20.0, 10.0, 240.0, 24.0);
        let st = TextStyle::new(14.0);
        let mut got = Vec::new();
        let cases = [
            ("47.37, 8.54", "東京", false),
            ("47.37, 8.54", "", false),
            ("\u{5e9}\u{5dc}\u{5d5}\u{5dd}", "\u{5e2}\u{5d5}", true),
        ];
        h.render(
            (300.0, 50.0),
            1.0,
            |_| {},
            |dl, text| {
                for (value, pre, rtl) in cases {
                    let ime = super::draw_field_text(
                        dl,
                        text,
                        &pal,
                        (value, pre),
                        st,
                        room,
                        (rtl, 4.0, 1.0),
                        pal.text,
                    );
                    let alone = text.layout(value, st, None, Align::Start);
                    let both = text.layout(&format!("{value}{pre}"), st, None, Align::Start);
                    got.push((ime, alone.width, both.width));
                }
            },
        );
        // Latin: the composition right of the text, the window over it.
        let (ime, w, wb) = got[0];
        assert!((ime.x - (room.x + w)).abs() < 2.0, "{ime:?} after {w}");
        assert!((ime.right() - (room.x + wb)).abs() < 3.0, "{ime:?} to {wb}");
        // No composition: the window at the caret after the text.
        let (ime, w, _) = got[1];
        assert!((ime.x + 0.75 - (room.x + w)).abs() < 1.0, "{ime:?} {w}");
        assert!(ime.w < 2.0);
        // Hebrew, right to left: the text from the field's right end, the composition on
        // its left, the window there.
        let (ime, w, wb) = got[2];
        assert!(ime.right() <= room.right() - w + 2.0, "{ime:?} left of {w}");
        assert!((ime.x - (room.right() - wb)).abs() < 3.0, "{ime:?} at {wb}");
    }

    #[test]
    fn go_to_paste_joins_lines() {
        let mut g = super::GoTo {
            error: Some("x".into()),
            ..Default::default()
        };
        g.paste("  51.5074\t\r\n-0.1278\n");
        assert_eq!(g.text, "51.5074 -0.1278");
        assert_eq!(g.error, None);
        g.paste("\n\t ");
        assert_eq!(g.text, "51.5074 -0.1278");
        let mut g = super::GoTo {
            text: "51.5".into(),
            ..Default::default()
        };
        g.paste(" N");
        assert_eq!(g.text, "51.5 N");
    }

    use super::*;

    #[test]
    fn scale_bars_are_round_lengths() {
        // 10 m per px: 1400 m fit, so 1 km of 100 px.
        let b = scale_bar(10.0, Units::Metric).unwrap();
        assert!((b.px - 100.0).abs() < 1e-3, "{}", b.px);
        assert!(
            b.label.contains('1') && b.label.contains("km"),
            "{}",
            b.label
        );
        let b = scale_bar(10.0, Units::Imperial).unwrap();
        assert!(
            b.label.contains("mi") || b.label.contains("ft"),
            "{}",
            b.label
        );
        assert!(b.px <= SCALE_MAX_PX as f32);
        // 0.01 m per px: 1 m.
        let b = scale_bar(0.01, Units::Metric).unwrap();
        assert!(b.label.contains("m"), "{}", b.label);
        assert!(scale_bar(0.0, Units::Metric).is_none());
    }

    #[test]
    fn overview_window_holds_the_map() {
        let ring = [(8.0, 47.0), (9.0, 47.0), (9.0, 48.0), (8.0, 48.0)];
        let w = window_around(&ring).unwrap();
        assert!(
            w[0] < 8.0 && w[2] > 9.0 && w[1] < 47.0 && w[3] > 48.0,
            "{w:?}"
        );
        let aspect = (w[2] - w[0]) / (w[3] - w[1]);
        assert!((aspect - f64::from(OVERVIEW_W / OVERVIEW_H)).abs() < 1e-6);
        let world = window_around(&[(-180.0, -85.0), (180.0, 85.0)]).unwrap();
        assert!(world[2] - world[0] <= 360.0 && world[3] - world[1] <= 180.0);
        assert!(world[1] >= -90.0 && world[3] <= 90.0);
    }

    #[test]
    fn readout_lines_by_format() {
        let strip = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let l = strip(position_line(47.376887, 8.541694, CoordFormat::Decimal).unwrap());
        assert!(l.contains("47.376887, 8.541694"), "{l}");
        let l = strip(position_line(47.376887, 8.541694, CoordFormat::Utm).unwrap());
        assert!(l.starts_with("UTM 32T"), "{l}");
        let l = strip(position_line(47.376887, 8.541694, CoordFormat::Mgrs).unwrap());
        assert!(l.starts_with("MGRS 32TMT"), "{l}");
        let l = strip(position_line(47.376887, 8.541694, CoordFormat::Dms).unwrap());
        assert!(l.contains("47°22′36.79″N"), "{l}");
    }
}
