//! Geometry templates in the measuring tool (DESIGN.md §7, "Vet, orthopaedic and dental
//! geometry"): VHS, VLAS, Norberg angle, TPA, distraction index, femoral head, offset, leg
//! length, acetabular angle, Steiner and Downs.
//!
//! The tool asks for each point by name, in order; once the last one is placed the readouts
//! show with their units and the calibration's source, in the measurement list, CSV,
//! GeoJSON and figures. A placed point is dragged to move it and the values follow (one undo
//! step per drag). The numbers are `specular_measure::clinical`'s; nothing is detected or
//! graded here.

use specular_i18n::{FluentArgs, I18n, t};
use specular_measure::calib::{Kind as QKind, format_sig};
use specular_measure::clinical::{Landmark, Outcome, Readout, Template, Value};
use specular_measure::{Measurement, P};

use super::super::figure::Measured;
use super::{ItemView, length_with_unit};

/// A placed point is picked up within this distance (logical px).
pub(super) const HANDLE_HIT: f32 = 8.0;

fn tr(id: &str) -> String {
    I18n::global().tr(id)
}

/// The template's name (`geometry-<key>`).
pub(super) fn name(t: Template) -> String {
    tr(&format!("geometry-{}", t.key()))
}

/// A landmark's name (`geometry-pt-<key>`, with its number as `$n`).
pub(super) fn landmark(l: &Landmark) -> String {
    let id = format!("geometry-pt-{}", l.key);
    match l.n {
        Some(n) => {
            let mut args = FluentArgs::new();
            args.set("n", i64::from(n));
            I18n::global().tr_args(&id, &args)
        }
        None => tr(&id),
    }
}

/// What to place next: "VHS, point 3 of 11: short axis, first end".
pub(super) fn prompt(t: Template, placed: usize) -> String {
    let lms = t.landmarks();
    match lms.get(placed) {
        Some(l) => t!(
            "geometry-prompt",
            template = name(t),
            n = (placed + 1) as i64,
            total = lms.len() as i64,
            point = landmark(l)
        ),
        None => name(t),
    }
}

/// A number as the overlays write it: a real minus sign.
fn minus(s: String) -> String {
    s.replace('-', "\u{2212}")
}

/// A readout's value with its unit, and "extrapolated" when it ran past the marked
/// vertebrae.
pub(super) fn value(r: &Readout) -> String {
    let v = match r.value {
        Value::Quantity(q) if q.kind == QKind::Angle => {
            t!(
                "geometry-value-angle",
                value = minus(specular_i18n::fixed(q.value, 1))
            )
        }
        Value::Quantity(q) => length_with_unit(minus(format_sig(q.value, 4)), q.unit.symbol()),
        Value::Ratio(v) => minus(specular_i18n::fixed(v, 2)),
        Value::Vertebrae(v) => t!(
            "geometry-value-vertebrae",
            value = specular_i18n::fixed(v, 1)
        ),
    };
    if r.extrapolated {
        t!("geometry-extrapolated", value = v)
    } else {
        v
    }
}

/// "VHS: 10.4 v".
pub(super) fn line(r: &Readout) -> String {
    t!(
        "geometry-readout",
        name = tr(&format!("geometry-out-{}", r.key)),
        value = value(r)
    )
}

/// The list row's headline and the other readouts.
pub(super) fn lines(o: Option<&Outcome>) -> (String, Vec<String>) {
    match o.map(|o| o.readouts.as_slice()) {
        Some([first, rest @ ..]) => (line(first), rest.iter().map(line).collect()),
        _ => (t!("geometry-degenerate"), Vec::new()),
    }
}

/// A template's measurement as listed and drawn; `screen` maps stored pixels to logical px.
pub(super) fn item(m: &Measurement, screen: impl Fn(P) -> (f32, f32)) -> ItemView {
    let o = m.outcome();
    let (value, detail) = lines(o.as_ref());
    let map = |v: &[P]| v.iter().map(|p| screen(*p)).collect::<Vec<_>>();
    ItemView {
        title: t!(
            "measure-tool-item",
            n = i64::from(m.id),
            shape = m.template.map(name).unwrap_or_default()
        ),
        value,
        detail,
        badge: super::badge(m.cal.source, None),
        caveat: m.cal.source.is_approximate(),
        outline: Vec::new(),
        closed: false,
        handles: map(&m.shape.points()),
        guides: o.map_or_else(Vec::new, |o| o.guides.iter().map(|g| map(g)).collect()),
        plot: Vec::new(),
    }
}

/// A template's measurement in a figure: its construction lines (the first carries the
/// headline and the notes page's lines), then its points as small crosses.
pub(super) fn figure(m: &Measurement) -> Vec<Measured> {
    let Some(t) = m.template else {
        return Vec::new();
    };
    let o = m.outcome();
    let (head, rest) = lines(o.as_ref());
    let title = t!("measure-tool-item", n = i64::from(m.id), shape = name(t));
    let mut notes = vec![format!("{title}: {head}")];
    notes.extend(rest);
    let xy = |v: &[P]| v.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
    let mut out: Vec<Measured> = o
        .map(|o| o.guides)
        .unwrap_or_default()
        .iter()
        .map(|g| Measured {
            outline: xy(g),
            closed: false,
            label: String::new(),
            lines: Vec::new(),
        })
        .collect();
    out.extend(m.shape.points().iter().map(|p| Measured {
        outline: vec![(p.x, p.y)],
        closed: false,
        label: String::new(),
        lines: Vec::new(),
    }));
    if let Some(first) = out.first_mut() {
        first.label = head;
        first.lines = notes;
    }
    out
}

/// The placed point of `handles` (each measurement's id and its points on screen) nearest to
/// `p` within [`HANDLE_HIT`]: (id, point).
pub(super) fn handle_at(handles: &[(u32, Vec<(f32, f32)>)], p: (f32, f32)) -> Option<(u32, usize)> {
    let mut best: Option<(f32, (u32, usize))> = None;
    for (id, pts) in handles {
        for (i, q) in pts.iter().enumerate() {
            let d = (q.0 - p.0).hypot(q.1 - p.1);
            if d <= HANDLE_HIT && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, (*id, i)));
            }
        }
    }
    best.map(|(_, at)| at)
}

/// Moves point `i` of a geometry measurement to `q` (stored px); false when it has none.
pub(super) fn set_point(m: &mut Measurement, i: usize, q: P) -> bool {
    if m.template.is_none() {
        return false;
    }
    match &mut m.shape {
        specular_measure::Shape::Polyline(v) => match v.get_mut(i) {
            Some(pt) => {
                *pt = q;
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// The point `[` (back) or `]` (forward) goes to among `n`, from `cur` (the one under the
/// crosshair), wrapping; the first (or last) from none.
pub(super) fn step_point(n: usize, cur: Option<usize>, forward: bool) -> Option<usize> {
    if n == 0 {
        return None;
    }
    Some(match (cur, forward) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_measure::calib::{Calibration, Source, Unit};

    #[test]
    fn every_message_exists() {
        let i = I18n::global();
        for t in Template::ALL {
            let id = format!("geometry-{}", t.key());
            assert_ne!(i.tr(&id), id, "{id}");
            for l in t.landmarks() {
                let id = format!("geometry-pt-{}", l.key);
                assert!(!landmark(l).contains("geometry-pt"), "{id}");
            }
            // Readout names, from an outcome of each template.
            let pts: Vec<_> = (0..t.landmarks().len())
                .map(|k| {
                    let k = k as f64;
                    specular_measure::P::new(10.0 + k * 7.0 + (k * k) % 5.0, 20.0 + k * k * 3.0)
                })
                .collect();
            let cal = Calibration::spacing(0.2, 0.2, Unit::Mm, Source::DicomPixelSpacing).unwrap();
            if let Some(o) = specular_measure::clinical::compute(t, &pts, &cal) {
                for r in &o.readouts {
                    let id = format!("geometry-out-{}", r.key);
                    assert_ne!(i.tr(&id), id, "{id}");
                }
            }
        }
    }

    /// Landmark names are looked up by key at run time (`tr_args` with a made id): every
    /// numbered one, in every catalogue, writes its number and nothing unresolved.
    #[test]
    fn every_landmark_name_gets_its_number() {
        let mut checked = 0;
        for lang in specular_i18n::catalogue_locales() {
            let l = specular_i18n::Localizer::new(std::slice::from_ref(&lang), None);
            for t in Template::ALL {
                for lm in t.landmarks() {
                    let id = format!("geometry-pt-{}", lm.key);
                    let mut args = FluentArgs::new();
                    if let Some(n) = lm.n {
                        args.set("n", i64::from(n) + 40);
                    }
                    let s = l.tr_args(&id, &args);
                    assert!(!s.contains('{') && s != id, "{lang} {id}: {s}");
                    if let Some(n) = lm.n {
                        assert!(s.contains(&(n + 40).to_string()), "{lang} {id}: {s}");
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 19 * 40, "{checked}");
    }

    #[test]
    fn handles_pick_the_nearest() {
        let h = vec![(1, vec![(0.0, 0.0), (10.0, 0.0)]), (2, vec![(12.0, 0.0)])];
        assert_eq!(handle_at(&h, (11.5, 0.0)), Some((2, 0)));
        assert_eq!(handle_at(&h, (9.0, 1.0)), Some((1, 1)));
        assert_eq!(handle_at(&h, (40.0, 0.0)), None);
    }

    /// `[` and `]` wrap through the points; a keyboard move changes only the point held, and
    /// the readouts follow.
    #[test]
    fn keyboard_moves_one_point() {
        assert_eq!(step_point(3, None, true), Some(0));
        assert_eq!(step_point(3, None, false), Some(2));
        assert_eq!(step_point(3, Some(2), true), Some(0));
        assert_eq!(step_point(3, Some(0), false), Some(2));
        assert_eq!(step_point(0, None, true), None);
        let cal = Calibration::uncalibrated();
        let t = Template::Tpa;
        let pts: Vec<P> = (0..t.landmarks().len())
            .map(|i| P::new(10.0 + 7.0 * i as f64, 20.0 + 3.0 * (i * i) as f64))
            .collect();
        let mut m = Measurement::clinical(t, pts.clone(), cal);
        let before = m.outcome();
        assert!(set_point(&mut m, 1, P::new(40.0, 5.0)));
        let now = m.shape.points();
        assert_eq!(now[1], P::new(40.0, 5.0));
        for (i, (a, b)) in pts.iter().zip(&now).enumerate() {
            assert!(i == 1 || a == b);
        }
        assert_ne!(m.outcome(), before);
        assert!(!set_point(&mut m, 99, P::new(0.0, 0.0)));
        let mut line = Measurement::new(specular_measure::Shape::Line(pts[0], pts[1]), cal);
        assert!(!set_point(&mut line, 0, P::new(0.0, 0.0)), "not a template");
    }
}
