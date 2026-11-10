//! Compares a parsed file with what nptdms read (`gen/make_samples.py`).

use serde_json::Value;
use specular_tdms::{Channel, Error, Prop, parse};

#[derive(Default, Debug)]
pub struct Tally {
    pub files: usize,
    pub channels: usize,
    pub values: usize,
    pub mismatches: usize,
    pub notes: Vec<String>,
}

impl Tally {
    fn miss(&mut self, what: String) {
        self.mismatches += 1;
        if self.notes.len() < 40 {
            self.notes.push(what);
        }
    }
}

fn same(a: f64, b: &Value) -> bool {
    match b.as_f64() {
        None if b.as_str() == Some("inf") => a == f64::INFINITY,
        None if b.as_str() == Some("-inf") => a == f64::NEG_INFINITY,
        None => a.is_nan(),
        Some(b) => a == b || (a - b).abs() <= 1e-12 * b.abs(),
    }
}

fn prop_matches(p: &Prop, e: &Value) -> bool {
    let Some((tag, v)) = e.as_object().and_then(|o| o.iter().next()) else {
        return false;
    };
    match (tag.as_str(), p) {
        ("i", Prop::I64(x)) => v.as_i64() == Some(*x),
        ("i", Prop::U64(x)) => v.as_u64() == Some(*x),
        ("f", Prop::F64(x)) => same(*x, v),
        ("s", Prop::Str(s)) => v.as_str() == Some(s),
        ("b", Prop::Bool(b)) => v.as_bool() == Some(*b),
        ("t", Prop::Time { seconds, fraction }) => {
            v[0].as_i64() == Some(*seconds) && v[1].as_u64() == Some(*fraction)
        }
        ("c", Prop::Complex(re, im)) => same(*re, &v[0]) && same(*im, &v[1]),
        _ => false,
    }
}

fn props(t: &mut Tally, what: &str, ours: &[(String, Prop)], e: &Value) {
    let Some(e) = e.as_object() else {
        return t.miss(format!("{what}: no properties expected"));
    };
    if e.len() != ours.len() {
        t.miss(format!(
            "{what}: {} properties, nptdms {}",
            ours.len(),
            e.len()
        ));
    }
    for (k, v) in e {
        match ours.iter().find(|(n, _)| n == k) {
            Some((_, p)) if prop_matches(p, v) => {}
            p => t.miss(format!("{what}: property {k} = {p:?}, nptdms {v}")),
        }
    }
}

fn values(t: &mut Tally, what: &str, got: &[f64], e: &Value) {
    let Some(e) = e.as_array() else {
        return t.miss(format!("{what}: bad expectation"));
    };
    if e.len() != got.len() {
        return t.miss(format!("{what}: {} values, nptdms {}", got.len(), e.len()));
    }
    t.values += e.len();
    let bad = got.iter().zip(e).filter(|(a, b)| !same(**a, b)).count();
    if bad > 0 {
        let first = got
            .iter()
            .zip(e)
            .position(|(a, b)| !same(*a, b))
            .unwrap_or(0);
        t.miss(format!(
            "{what}: {bad} values differ, first at {first}: {} vs {}",
            got[first], e[first]
        ));
    }
}

fn channel(t: &mut Tally, data: &[u8], what: &str, c: &Channel, e: &Value) {
    t.channels += 1;
    props(t, what, &c.properties, &e["props"]);
    if e["len"].as_u64() != Some(c.len) {
        t.miss(format!("{what}: length {}, nptdms {}", c.len, e["len"]));
        return;
    }
    let n = c.len as usize;
    let mut buf = vec![0f64; n];
    if e["refused"].as_bool() == Some(true) {
        if !matches!(c.read(data, 0, &mut buf), Err(Error::Unsupported(_))) {
            t.miss(format!("{what}: not refused"));
        }
        return;
    }
    if let Some(s) = e["strings"].as_array() {
        match c.strings(data, usize::MAX) {
            Ok(got)
                if got.len() == s.len()
                    && got.iter().zip(s).all(|(a, b)| b.as_str() == Some(a)) =>
            {
                t.values += s.len();
            }
            r => t.miss(format!("{what}: strings {r:?}, nptdms {s:?}")),
        }
        return;
    }
    if !e["values"].is_null() {
        match c.read(data, 0, &mut buf) {
            Ok(()) => values(t, &format!("{what} values"), &buf, &e["values"]),
            Err(err) => t.miss(format!("{what}: {err}")),
        }
    }
    if !e["raw"].is_null() {
        match c.read_raw(data, 0, &mut buf) {
            Ok(()) => values(t, &format!("{what} raw"), &buf, &e["raw"]),
            Err(err) => t.miss(format!("{what}: {err}")),
        }
    }
    if !e["re"].is_null() {
        let mut im = vec![0f64; n];
        match c.read_complex(data, 0, &mut buf, &mut im) {
            Ok(()) => {
                values(t, &format!("{what} re"), &buf, &e["re"]);
                values(t, &format!("{what} im"), &im, &e["im"]);
            }
            Err(err) => t.miss(format!("{what}: {err}")),
        }
    }
    if let Some(s) = e["scalers"].as_object() {
        for (id, v) in s {
            let id: u32 = id.parse().unwrap_or(u32::MAX);
            match c.read_scaler(id, data, 0, &mut buf) {
                Ok(()) => values(t, &format!("{what} scaler {id}"), &buf, v),
                Err(err) => t.miss(format!("{what} scaler {id}: {err}")),
            }
        }
    }
}

pub fn compare(t: &mut Tally, name: &str, data: &[u8], e: &Value) {
    let f = match parse(data) {
        Ok(f) => f,
        Err(err) => return t.miss(format!("{name}: {err}")),
    };
    t.files += 1;
    props(t, &format!("{name} /"), &f.properties, &e["props"]);
    let eg = e["groups"].as_array().cloned().unwrap_or_default();
    // nptdms lists only groups with a group object or channels; ours may hold an empty one.
    let ours: Vec<_> = f
        .groups
        .iter()
        .filter(|g| !g.channels.is_empty() || eg.iter().any(|x| x["name"] == g.name.as_str()))
        .collect();
    if ours.len() != eg.len() {
        return t.miss(format!(
            "{name}: {} groups, nptdms {}",
            ours.len(),
            eg.len()
        ));
    }
    for (g, ge) in ours.iter().zip(&eg) {
        let gw = format!("{name} {}", g.name);
        if ge["name"].as_str() != Some(&g.name) {
            t.miss(format!("{gw}: nptdms names it {}", ge["name"]));
            continue;
        }
        props(t, &gw, &g.properties, &ge["props"]);
        let ce = ge["channels"].as_array().cloned().unwrap_or_default();
        if ce.len() != g.channels.len() {
            t.miss(format!(
                "{gw}: {} channels, nptdms {}",
                g.channels.len(),
                ce.len()
            ));
            continue;
        }
        for (c, e) in g.channels.iter().zip(&ce) {
            let cw = format!("{gw}/{}", c.name);
            if e["name"].as_str() != Some(&c.name) {
                t.miss(format!("{cw}: nptdms names it {}", e["name"]));
                continue;
            }
            channel(t, data, &cw, c, e);
        }
    }
}
