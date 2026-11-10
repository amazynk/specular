//! The name of a LAS file's coordinate reference system: from its OGC WKT VLR (the first
//! quoted name), or from its GeoTIFF keys (a citation, else the EPSG code).

use crate::las::Vlr;

const PROJECTION: &str = "LASF_Projection";

/// The CRS name, if the file says one.
pub(crate) fn name(vlrs: &[Vlr]) -> Option<String> {
    let of = |id: u16| {
        vlrs.iter()
            .find(|v| v.user == PROJECTION && v.id == id)
            .map(|v| v.data)
    };
    if let Some(w) = of(2112).and_then(wkt_name) {
        return Some(w);
    }
    geokeys(of(34735)?, of(34737).unwrap_or(&[]))
}

/// The first quoted string of a WKT text (the outermost CRS's name).
pub(crate) fn wkt_name(data: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(data);
    let open = s.find('"')?;
    let rest = &s[open + 1..];
    let close = rest.find('"')?;
    let name = rest[..close].trim();
    (!name.is_empty() && name.len() <= 256).then(|| name.to_string())
}

fn geokeys(dir: &[u8], ascii: &[u8]) -> Option<String> {
    let words: Vec<u16> = dir
        .chunks_exact(2)
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
        .collect();
    let n = usize::from(*words.get(3)?);
    let mut epsg_proj = None;
    let mut epsg_geo = None;
    let mut epsg_vert = None;
    let mut citation = None;
    let mut citation_gt = None;
    for k in 0..n {
        let Some(e) = words.get(4 + 4 * k..8 + 4 * k) else {
            break;
        };
        let (key, loc, count, value) = (e[0], e[1], usize::from(e[2]), e[3]);
        let text = || -> Option<String> {
            if loc != 34737 {
                return None;
            }
            let start = usize::from(value);
            let b = ascii.get(start..start + count)?;
            let s = String::from_utf8_lossy(b);
            let s = s.trim_matches(|c: char| c == '|' || c == '\0' || c.is_whitespace());
            // Some writers join several names with '|': the first is the CRS's.
            let s = s.split('|').next().unwrap_or("").trim();
            (!s.is_empty()).then(|| s.to_string())
        };
        let code = (loc == 0 && value != 0 && value != 32767).then_some(value);
        match key {
            3072 => epsg_proj = code,
            2048 => epsg_geo = code,
            4096 => epsg_vert = code,
            3073 => citation = text(),
            1026 => citation_gt = text(),
            _ => {}
        }
    }
    let epsg = epsg_proj.or(epsg_geo);
    let mut name = match (citation.or(citation_gt), epsg) {
        (Some(c), Some(e)) => format!("{c} (EPSG:{e})"),
        (Some(c), None) => c,
        (None, Some(e)) => format!("EPSG:{e}"),
        (None, None) => return None,
    };
    if let Some(v) = epsg_vert {
        name.push_str(&format!(" + EPSG:{v}"));
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wkt_first_name() {
        let w = br#"PROJCS["NAD83 / UTM zone 15N",GEOGCS["NAD83"]]"#;
        assert_eq!(wkt_name(w).as_deref(), Some("NAD83 / UTM zone 15N"));
        assert_eq!(wkt_name(b"nothing"), None);
    }

    #[test]
    fn geokey_citation_and_code() {
        let mut dir = vec![1u16, 1, 0, 3];
        dir.extend([3072, 0, 1, 26915]);
        dir.extend([3073, 34737, 21, 0]);
        dir.extend([4096, 0, 1, 5703]);
        let dir: Vec<u8> = dir.iter().flat_map(|w| w.to_le_bytes()).collect();
        let ascii = b"NAD83 / UTM zone 15N|";
        assert_eq!(
            geokeys(&dir, ascii).as_deref(),
            Some("NAD83 / UTM zone 15N (EPSG:26915) + EPSG:5703")
        );
        // Truncated directories end the list.
        assert_eq!(geokeys(&dir[..10], ascii), None);
    }
}
