//! Aperio's ImageDescription: a first line naming the library, a line describing the level
//! ("46000x32914 [0,100 46000x32914] (256x256) JPEG/RGB Q=30"), then `key = value` pairs
//! separated by `|`.

use crate::{Props, Vendor};

/// Whether a description is Aperio's.
pub(crate) fn is_aperio(desc: &str) -> bool {
    desc.starts_with("Aperio")
}

/// The `key = value` pairs, in order.
pub(crate) fn pairs(desc: &str) -> Vec<(String, String)> {
    desc.split('|')
        .skip(1)
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_owned(), v.trim().to_owned()))
        })
        .collect()
}

/// The slide facts from the first IFD's description.
pub(crate) fn props(desc: &str) -> Props {
    let mut p = Props::new(Vendor::Aperio);
    let kv = pairs(desc);
    let get = |k: &str| {
        kv.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    };
    let num = |k: &str| {
        get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    p.mpp = num("MPP").map(|m| (m, m));
    p.objective = num("AppMag");
    p.scanner = get("ScanScope ID").map(str::to_owned);
    p.date = match (get("Date"), get("Time")) {
        (Some(d), Some(t)) => Some(format!("{d} {t}")),
        (Some(d), None) => Some(d.to_owned()),
        _ => None,
    };
    p.name = get("Filename").map(str::to_owned);
    // The library line and the level line come first.
    let mut lines = desc.split('|').next().unwrap_or("").lines();
    if let Some(lib) = lines.next() {
        p.properties
            .push(("Library".to_owned(), lib.trim().to_owned()));
    }
    p.properties.extend(kv);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_pairs() {
        let d = "Aperio Image Library v10.0.50\r\n46920x33014 [0,100 46000x32914] (256x256) JPEG/RGB Q=30|AppMag = 20|StripeWidth = 2040|ScanScope ID = CPAPERIOCS|Filename = CMU-1|Date = 12/29/09|Time = 09:59:15|User = b414003d|MPP = 0.4990|Left = 25.691574";
        assert!(is_aperio(d));
        let p = props(d);
        assert_eq!(p.mpp, Some((0.499, 0.499)));
        assert_eq!(p.objective, Some(20.0));
        assert_eq!(p.scanner.as_deref(), Some("CPAPERIOCS"));
        assert_eq!(p.date.as_deref(), Some("12/29/09 09:59:15"));
        assert_eq!(p.properties[0].1, "Aperio Image Library v10.0.50");
    }
}
