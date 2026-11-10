//! The tree as text (for the info panel), the observation facts in Roman and JWST trees, and
//! an approximate TAN WCS from their simple pointing keywords.

use crate::yaml::{Node, Value, float_text, tag_name};

/// The longest value text kept.
const MAX_VALUE: usize = 200;
/// Scalar lists up to this long are shown inline.
const INLINE_SEQ: usize = 8;
/// Longest key path shown (`a.b[3].c`): keys may be megabytes long, and every leaf below a
/// key repeats it.
const MAX_PATH: usize = 512;

/// Flattens the tree into (dotted path, value text), at most `max` entries. Arrays read as
/// `ndarray float32 [4088, 4088]`; GWCS objects and transforms collapse to their tag name;
/// long scalar lists are cut with a count.
pub fn flatten(tree: &Value, max: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    flat(tree, String::new(), max, &mut out);
    out
}

fn cap(s: String) -> String {
    cap_to(s, MAX_VALUE)
}

fn cap_to(mut s: String, max: usize) -> String {
    if s.len() > max {
        s.truncate(floor_char(&s, max));
        s.push('…');
    }
    s
}

/// The largest char boundary of `s` at most `max`.
fn floor_char(s: &str, max: usize) -> usize {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn collapsed(v: &Value) -> Option<String> {
    let t = v.tag.as_deref()?;
    if v.tag_is("ndarray") {
        let dt = v
            .get("datatype")
            .and_then(Value::scalar_text)
            .unwrap_or_else(|| "?".into());
        let shape = v
            .get("shape")
            .and_then(Value::as_seq)
            .map(|s| {
                s.iter()
                    .filter_map(Value::scalar_text)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        return Some(format!("ndarray {dt} [{shape}]"));
    }
    if v.tag_is("quantity")
        && let Some(inner) = v.get("value")
    {
        let unit = v
            .get("unit")
            .and_then(Value::scalar_text)
            .unwrap_or_default();
        let inner_text = collapsed(inner).or_else(|| inner.scalar_text())?;
        return Some(format!("{inner_text} {unit}").trim_end().to_owned());
    }
    if t.contains("gwcs/") || t.contains("transform/") || t.contains("coordinates/") {
        return Some(format!("<{}>", tag_name(t)));
    }
    if v.tag_is("time")
        && let Some(s) = time_text(v)
    {
        return Some(s);
    }
    None
}

fn flat(v: &Value, path: String, max: usize, out: &mut Vec<(String, String)>) {
    if out.len() >= max {
        return;
    }
    if let Some(s) = collapsed(v) {
        out.push((path, cap(s)));
        return;
    }
    match &v.node {
        Node::Map(m) => {
            if m.is_empty() {
                out.push((path, "{}".into()));
                return;
            }
            for (k, c) in m {
                let k = &k[..floor_char(k, MAX_PATH)];
                let p = if path.is_empty() {
                    k.to_owned()
                } else {
                    cap_to(format!("{path}.{k}"), MAX_PATH)
                };
                flat(c, p, max, out);
            }
        }
        Node::Seq(items) => {
            let scalars = items.iter().all(|i| i.scalar_text().is_some());
            if scalars {
                let shown: Vec<String> = items
                    .iter()
                    .take(INLINE_SEQ)
                    .filter_map(Value::scalar_text)
                    .collect();
                let mut s = format!("[{}", shown.join(", "));
                if items.len() > INLINE_SEQ {
                    s.push_str(&format!(", … ({} items)", items.len()));
                }
                s.push(']');
                out.push((path, cap(s)));
            } else {
                for (i, c) in items.iter().enumerate() {
                    flat(c, cap_to(format!("{path}[{i}]"), MAX_PATH), max, out);
                }
            }
        }
        _ => {
            let s = v.scalar_text().unwrap_or_default();
            out.push((path, cap(s)));
        }
    }
}

/// A `time/time` node as text: the string form, or `value` of the mapping form (an MJD
/// number gets " MJD").
fn time_text(v: &Value) -> Option<String> {
    match &v.node {
        Node::Str(s) => Some(s.clone()),
        Node::Float(_) | Node::Int(_) => v.scalar_text(),
        Node::Map(_) => {
            let val = v.get("value")?;
            let text = match &val.node {
                Node::Str(s) => s.clone(),
                _ => val.scalar_text()?,
            };
            match v.get("format").and_then(Value::as_str) {
                Some(f) if f.eq_ignore_ascii_case("mjd") => Some(format!("{text} MJD")),
                _ => Some(text),
            }
        }
        _ => None,
    }
}

/// The observation facts of a Roman (`roman.meta`) or JWST (`meta`) tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub telescope: Option<String>,
    pub instrument: Option<String>,
    pub detector: Option<String>,
    /// Optical element (Roman) or filter (JWST).
    pub filter: Option<String>,
    pub start_time: Option<String>,
    /// Seconds.
    pub exposure_time: Option<f64>,
    pub target: Option<String>,
    /// The data model's tag name (`wfi_image`, ...).
    pub model: Option<String>,
}

/// Placeholders the data model makers write for unknown values.
fn placeholder_str(s: &str) -> bool {
    let t = s.trim();
    t.is_empty() || t == "?" || t == "-999999" || t.eq_ignore_ascii_case("none") || t == "N/A"
}

fn placeholder_num(f: f64) -> bool {
    !f.is_finite() || f == -999_999.0 || f == -999_999_999.0
}

fn text(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let s = match &v.node {
        Node::Str(s) => s.clone(),
        Node::Int(_) | Node::Float(_) => {
            if placeholder_num(v.as_f64()?) {
                return None;
            }
            v.scalar_text()?
        }
        _ => return None,
    };
    (!placeholder_str(&s)).then_some(s)
}

fn number(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    let v = if v.tag_is("quantity") {
        v.get("value")?
    } else {
        v
    };
    let f = match &v.node {
        Node::Str(s) => s.trim().parse().ok()?,
        _ => v.as_f64()?,
    };
    (!placeholder_num(f)).then_some(f)
}

/// The `meta` mapping of a Roman or JWST tree, and the model node holding it.
fn meta_node(tree: &Value) -> Option<(&Value, &Value)> {
    if let Some(r) = tree.get("roman")
        && let Some(m) = r.get("meta")
    {
        return Some((r, m));
    }
    tree.get("meta").map(|m| (tree, m))
}

/// The observation facts found in a Roman or JWST tree.
pub fn science_meta(tree: &Value) -> Meta {
    let Some((model, m)) = meta_node(tree) else {
        return Meta::default();
    };
    let instrument = m.get("instrument");
    let start = m
        .path(&["exposure", "start_time"])
        .and_then(|t| match &t.node {
            Node::Float(_) | Node::Int(_) => {
                number(Some(t)).map(|f| format!("{} MJD", float_text(f)))
            }
            _ => time_text(t).filter(|s| !placeholder_str(s)),
        });
    let exposure_time = number(m.path(&["exposure", "exposure_time"]))
        .or_else(|| number(m.path(&["exposure", "effective_exposure_time"])))
        .or_else(|| number(m.path(&["exposure", "integration_time"])));
    let target = text(m.path(&["target", "catalog_name"]))
        .or_else(|| text(m.path(&["target", "proposer_name"])))
        .or_else(|| text(m.path(&["target", "name"])));
    Meta {
        telescope: text(m.get("telescope")),
        instrument: text(instrument.and_then(|i| i.get("name"))).or_else(|| text(instrument)),
        detector: text(instrument.and_then(|i| i.get("detector"))),
        filter: text(instrument.and_then(|i| i.get("optical_element")))
            .or_else(|| text(instrument.and_then(|i| i.get("filter")))),
        start_time: start,
        exposure_time,
        target,
        model: model.tag.as_deref().map(|t| tag_name(t).to_owned()),
    }
}

fn fits_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// FITS WCS cards (keyword, value text) for a TAN projection approximating the pointing in the
/// tree's `wcsinfo` (Roman `roman.meta.wcsinfo`, JWST `meta.wcsinfo`), for an image
/// `width` × `height` in FITS pixel order. `None` when the tree has no usable pointing. The
/// distortion GWCS describes is ignored, hence approximate.
pub fn approx_wcs_cards(tree: &Value, width: u64, height: u64) -> Option<Vec<(String, String)>> {
    let (_, m) = meta_node(tree)?;
    let w = m.get("wcsinfo")?;
    let num = |k: &str| number(w.get(k));
    let mut cards = Vec::new();
    // A FITS-like wcsinfo (JWST): pass it through.
    if let (Some(c1), Some(c2)) = (text(w.get("ctype1")), text(w.get("ctype2")))
        && let (Some(v1), Some(v2)) = (num("crval1"), num("crval2"))
    {
        cards.push(("CTYPE1".to_owned(), fits_str(&c1)));
        cards.push(("CTYPE2".to_owned(), fits_str(&c2)));
        cards.push(("CRVAL1".to_owned(), float_text(v1)));
        cards.push(("CRVAL2".to_owned(), float_text(v2)));
        for k in [
            "crpix1", "crpix2", "cdelt1", "cdelt2", "pc1_1", "pc1_2", "pc2_1", "pc2_2",
        ] {
            if let Some(x) = num(k) {
                cards.push((k.to_ascii_uppercase(), float_text(x)));
            }
        }
        for k in ["cunit1", "cunit2", "radesys"] {
            if let Some(x) = text(w.get(k)) {
                cards.push((k.to_ascii_uppercase(), fits_str(&x)));
            }
        }
        if num("cdelt1").is_none() || num("crpix1").is_none() {
            return None;
        }
        return Some(cards);
    }
    // Roman (and JWST without FITS keywords): reference point and V3 position angle.
    let ra = num("ra_ref")?;
    let dec = num("dec_ref")?;
    if !(0.0..=360.0).contains(&ra) || !(-90.0..=90.0).contains(&dec) {
        return None;
    }
    let vparity = num("vparity").unwrap_or(-1.0).signum();
    let pa = match (num("roll_ref"), num("v3yangle")) {
        (Some(roll), Some(v3y)) => roll - vparity * v3y,
        _ => m
            .path(&["pointing", "pa_aperture"])
            .and_then(|p| number(Some(p)))?,
    };
    let telescope = text(m.get("telescope")).unwrap_or_default();
    let scale = num("scale")
        .or_else(|| (telescope.eq_ignore_ascii_case("roman")).then_some(0.11))
        .filter(|s| *s > 0.0 && *s < 3600.0)?;
    let cdelt = scale / 3600.0;
    let r = pa.to_radians();
    let (s, c) = r.sin_cos();
    cards.push(("CTYPE1".into(), fits_str("RA---TAN")));
    cards.push(("CTYPE2".into(), fits_str("DEC--TAN")));
    cards.push(("CRVAL1".into(), float_text(ra)));
    cards.push(("CRVAL2".into(), float_text(dec)));
    cards.push(("CRPIX1".into(), float_text((width as f64 + 1.0) / 2.0)));
    cards.push(("CRPIX2".into(), float_text((height as f64 + 1.0) / 2.0)));
    cards.push(("CDELT1".into(), float_text(cdelt)));
    cards.push(("CDELT2".into(), float_text(cdelt)));
    cards.push(("PC1_1".into(), float_text(vparity * c)));
    cards.push(("PC1_2".into(), float_text(s)));
    cards.push(("PC2_1".into(), float_text(-vparity * s)));
    cards.push(("PC2_2".into(), float_text(c)));
    cards.push(("CUNIT1".into(), fits_str("deg")));
    cards.push(("CUNIT2".into(), fits_str("deg")));
    Some(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::parse;

    const ROMAN: &str = "%YAML 1.1\n---\nroman: !<asdf://stsci.edu/datamodels/roman/tags/wfi_image-1.0.0>\n  meta:\n    telescope: ROMAN\n    instrument: {detector: WFI01, name: WFI, optical_element: F158}\n    exposure:\n      start_time: !time/time-1.1.0 2020-01-01T00:00:00.000\n      exposure_time: -999999\n      effective_exposure_time: 139.2\n    target: {proposer_name: '?', catalog_name: M31}\n    wcsinfo: {ra_ref: 10.5, dec_ref: 41.2, roll_ref: 30.0, v3yangle: -60.0, vparity: -1}\n    wcs: !<tag:stsci.edu:gwcs/wcs-1.2.0>\n      name: ''\n      steps: [1, 2]\n  data: !core/ndarray-1.0.0 {source: 0, datatype: float32, shape: [4088, 4088]}\n...\n";

    /// A megabyte-long key above a thousand leaves: each leaf's path used to repeat it.
    #[test]
    fn long_keys_are_cut() {
        let leaves: Vec<String> = (0..1000).map(|i| format!("k{i}: {i}")).collect();
        let y = format!("{}: {{{}}}\n", "x".repeat(1 << 20), leaves.join(", "));
        let t = parse(&y).unwrap();
        let f = flatten(&t, 4000);
        assert_eq!(f.len(), 1000);
        assert!(
            f.iter().all(|(p, _)| p.len() <= MAX_PATH + 3),
            "{}",
            f[0].0.len()
        );
    }

    #[test]
    fn roman_facts() {
        let t = parse(ROMAN).unwrap();
        let m = science_meta(&t);
        assert_eq!(m.telescope.as_deref(), Some("ROMAN"));
        assert_eq!(m.instrument.as_deref(), Some("WFI"));
        assert_eq!(m.detector.as_deref(), Some("WFI01"));
        assert_eq!(m.filter.as_deref(), Some("F158"));
        assert_eq!(m.start_time.as_deref(), Some("2020-01-01T00:00:00.000"));
        assert_eq!(m.exposure_time, Some(139.2));
        assert_eq!(m.target.as_deref(), Some("M31"));
        assert_eq!(m.model.as_deref(), Some("wfi_image"));
    }

    #[test]
    fn flattened() {
        let t = parse(ROMAN).unwrap();
        let f = flatten(&t, 100);
        let get = |k: &str| f.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str());
        assert_eq!(get("roman.meta.telescope"), Some("ROMAN"));
        assert_eq!(get("roman.meta.wcs"), Some("<wcs>"));
        assert_eq!(get("roman.data"), Some("ndarray float32 [4088, 4088]"));
        assert_eq!(
            get("roman.meta.exposure.start_time"),
            Some("2020-01-01T00:00:00.000")
        );
        assert_eq!(flatten(&t, 3).len(), 3);
    }

    #[test]
    fn roman_tan() {
        let t = parse(ROMAN).unwrap();
        let cards = approx_wcs_cards(&t, 4088, 4088).unwrap();
        let get = |k: &str| {
            cards
                .iter()
                .find(|(p, _)| p == k)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        assert_eq!(get("CTYPE1"), "'RA---TAN'");
        assert_eq!(get("CRVAL1"), "10.5");
        assert_eq!(get("CRPIX1"), "2044.5");
        // PA = 30 - (-1)(-60) = -30 degrees.
        let pc12: f64 = get("PC1_2").parse().unwrap();
        assert!((pc12 - (-30f64).to_radians().sin()).abs() < 1e-12);
        let pc11: f64 = get("PC1_1").parse().unwrap();
        assert!((pc11 + (-30f64).to_radians().cos()).abs() < 1e-12);
        let cd: f64 = get("CDELT1").parse().unwrap();
        assert!((cd - 0.11 / 3600.0).abs() < 1e-15);
    }

    #[test]
    fn no_wcs_without_pointing() {
        let t = parse("meta: {telescope: JWST, wcsinfo: {ra_ref: '?'}}\n").unwrap();
        assert!(approx_wcs_cards(&t, 10, 10).is_none());
        let t = parse("a: 1\n").unwrap();
        assert!(approx_wcs_cards(&t, 10, 10).is_none());
        assert_eq!(science_meta(&t), Meta::default());
    }

    #[test]
    fn jwst_fits_like() {
        let t = parse(
            "meta:\n  telescope: JWST\n  instrument: {name: NIRCAM, filter: F200W}\n  exposure: {start_time: 59800.5}\n  wcsinfo: {ctype1: RA---TAN, ctype2: DEC--TAN, crval1: 5.0, crval2: -3.0, crpix1: 10.0, crpix2: 12.0, cdelt1: 1.0e-05, cdelt2: 1.0e-05, pc1_1: 1.0, pc2_2: 1.0}\n",
        )
        .unwrap();
        let m = science_meta(&t);
        assert_eq!(m.filter.as_deref(), Some("F200W"));
        assert_eq!(m.start_time.as_deref(), Some("59800.5 MJD"));
        let cards = approx_wcs_cards(&t, 20, 20).unwrap();
        assert!(cards.contains(&("CRPIX2".into(), "12.0".into())));
        assert!(cards.contains(&("CTYPE2".into(), "'DEC--TAN'".into())));
    }
}
