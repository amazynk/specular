//! A measurement list as CSV (RFC 4180, comma-separated, CRLF-free `\n` lines).
//!
//! The provenance comes first as `# key: value` comment lines, then one header row and one
//! row per measurement and channel (a measurement without statistics has one row with the
//! channel and statistics columns empty). Numbers are written in full precision; `frame` is
//! 1-based; `unit` is the length unit (areas are in its square, angles in degrees);
//! `points` lists the defining points as `x y` pairs separated by `;` (two corners for a
//! rectangle or an ellipse's box).
//!
//! A geometry template's measurement ([`crate::clinical`]) has `shape` = the template's key
//! and one row per readout (`readout`, `readout_value`, `readout_unit`: `deg`, a length unit,
//! `ratio` or `vertebrae`; `points` lists all its points), then one row per landmark
//! (`readout` = the landmark's key, with its number after a `-` when it has one, and `points`
//! its position). Other measurements leave those three columns empty.

use std::fmt::Write as _;

use crate::list::Measurement;
use crate::provenance::Provenance;

pub const COLUMNS: [&str; 25] = [
    "id",
    "label",
    "shape",
    "frame",
    "channel",
    "count",
    "mean",
    "sd",
    "sd_sample",
    "min",
    "max",
    "sum",
    "median",
    "length",
    "perimeter",
    "area",
    "angle",
    "unit",
    "calibration",
    "pixel_area",
    "approximate",
    "points",
    "readout",
    "readout_value",
    "readout_unit",
];

/// Quotes a field when it holds a comma, quote or line break.
pub fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// A text field (a label): as [`field`], and one starting with `=`, `+`, `-`, `@`, a tab or a
/// carriage return gets a leading `'` so spreadsheets do not run it as a formula.
pub fn text(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        field(&format!("'{s}"))
    } else {
        field(s)
    }
}

fn num(v: Option<f64>) -> String {
    v.filter(|v| v.is_finite())
        .map_or_else(String::new, |v| v.to_string())
}

/// The list as CSV text.
pub fn write(items: &[Measurement], prov: &Provenance) -> String {
    let mut out = String::new();
    for (k, v) in prov.fields() {
        let _ = writeln!(out, "# {k}: {}", v.replace(['\n', '\r'], " "));
    }
    out.push_str(&COLUMNS.join(","));
    out.push('\n');
    for m in items {
        if m.template.is_some() {
            clinical_rows(&mut out, m);
            continue;
        }
        if m.probe.is_some() {
            probe_rows(&mut out, m);
            continue;
        }
        let points = m
            .shape
            .points()
            .iter()
            .map(|p| format!("{} {}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(";");
        let common = |channel: &str, s: Option<&crate::stats::ChannelStats>| {
            [
                m.id.to_string(),
                text(&m.label),
                m.shape.key().to_owned(),
                m.frame.map_or_else(String::new, |f| (f + 1).to_string()),
                text(channel),
                s.map_or_else(String::new, |s| s.count.to_string()),
                num(s.map(|s| s.mean)),
                num(s.map(|s| s.sd)),
                num(s.map(|s| s.sd_sample)),
                num(s.map(|s| s.min)),
                num(s.map(|s| s.max)),
                num(s.map(|s| s.sum)),
                num(s.map(|s| s.median)),
                num(m.length().map(|q| q.value)),
                num(m.perimeter().map(|q| q.value)),
                num(m.area().map(|q| q.value)),
                num(m.angle().map(|q| q.value)),
                m.cal.unit.key().to_owned(),
                m.cal.source.key().to_owned(),
                num(s.map(|s| s.count as f64 * m.cal.pixel_area())),
                if m.approximate { "1" } else { "0" }.to_owned(),
                field(&points),
                String::new(),
                String::new(),
                String::new(),
            ]
            .join(",")
        };
        match &m.stats {
            Some(stats) if !stats.is_empty() => {
                for (i, s) in stats.iter().enumerate() {
                    let name = m
                        .channels
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| (i + 1).to_string());
                    out.push_str(&common(&name, Some(s)));
                    out.push('\n');
                }
            }
            _ => {
                out.push_str(&common("", None));
                out.push('\n');
            }
        }
    }
    out
}

/// An inspection probe's rows ([`crate::inspect`]): one per readout, `shape` = the probe's
/// key, `points` its shape's points, or the readout's own point (a profile sample, a crack
/// cut). A probe that found nothing has one row with `readout` = `refused` and the reason
/// as its unit.
fn probe_rows(out: &mut String, m: &Measurement) {
    let Some(probe) = m.probe else { return };
    let pts = m.shape.points();
    let xy = |p: &crate::geom::P| format!("{} {}", p.x, p.y);
    let all = pts.iter().map(xy).collect::<Vec<_>>().join(";");
    let row = |out: &mut String, readout: &str, value: String, unit: &str, points: &str| {
        let mut cols = vec![
            m.id.to_string(),
            text(&m.label),
            probe.key().to_owned(),
            m.frame.map_or_else(String::new, |f| (f + 1).to_string()),
        ];
        cols.extend(std::iter::repeat_n(String::new(), 9));
        cols.extend([
            num(m.length().map(|q| q.value)),
            String::new(),
            num(m.area().map(|q| q.value)),
            String::new(),
            m.cal.unit.key().to_owned(),
            m.cal.source.key().to_owned(),
            String::new(),
            if m.approximate { "1" } else { "0" }.to_owned(),
            field(points),
            field(readout),
            value,
            unit.to_owned(),
        ]);
        out.push_str(&cols.join(","));
        out.push('\n');
    };
    let (a, b) = match pts[..] {
        [a, b, ..] => (a, b),
        [a] => (a, a),
        [] => Default::default(),
    };
    match &m.reading {
        Some(crate::inspect::Reading::Refused(r)) => {
            row(out, "refused", String::new(), r.key(), &all)
        }
        Some(r) => {
            for o in r.readouts(a, b, m.cal.unit.key()) {
                let at = o.at.as_ref().map_or_else(|| all.clone(), xy);
                row(out, &o.key, num(Some(o.value)), &o.unit, &at);
            }
        }
        None => row(out, "", String::new(), "", &all),
    }
}

/// A geometry template's rows: its readouts, then its landmarks.
fn clinical_rows(out: &mut String, m: &Measurement) {
    let Some(t) = m.template else { return };
    let pts = m.shape.points();
    let xy = |p: &crate::geom::P| format!("{} {}", p.x, p.y);
    let all = pts.iter().map(xy).collect::<Vec<_>>().join(";");
    let row = |out: &mut String, readout: &str, value: String, unit: &str, points: &str| {
        let mut cols = vec![
            m.id.to_string(),
            text(&m.label),
            t.key().to_owned(),
            m.frame.map_or_else(String::new, |f| (f + 1).to_string()),
        ];
        // channel … angle: empty.
        cols.extend(std::iter::repeat_n(String::new(), 13));
        cols.extend([
            m.cal.unit.key().to_owned(),
            m.cal.source.key().to_owned(),
            String::new(),
            if m.approximate { "1" } else { "0" }.to_owned(),
            field(points),
            field(readout),
            value,
            unit.to_owned(),
        ]);
        out.push_str(&cols.join(","));
        out.push('\n');
    };
    if let Some(o) = m.outcome() {
        for r in &o.readouts {
            let v = num(Some(r.value.number()));
            row(out, r.key, v, r.value.unit_key(), &all);
        }
    }
    for (l, p) in t.landmarks().iter().zip(&pts) {
        let key = match l.n {
            Some(n) => format!("{}-{n}", l.key),
            None => l.key.to_owned(),
        };
        row(out, &key, String::new(), "px", &xy(p));
    }
}
