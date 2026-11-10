//! MGRS (and USNG, which writes the same references with spaces) on WGS 84: the UTM zone and
//! latitude band, the 100 km square's two letters, then easting and northing within it,
//! truncated to the precision asked for. Outside 80° S..84° N the polar UPS letters A, B, Y
//! and Z are used.

use crate::utm::{BANDS, Utm, band, band_south, central_meridian, from_utm, to_utm_in, utm_zone};

/// Column letters of the 100 km squares by zone set (zone − 1) mod 3.
const COLS: [&[u8; 8]; 3] = [b"ABCDEFGH", b"JKLMNPQR", b"STUVWXYZ"];
/// Row letters, repeating every 2 000 km (offset by 5 in even zones).
const ROWS: &[u8; 20] = b"ABCDEFGHJKLMNPQRSTUV";
/// UPS columns west and east of the pole's meridian, and rows (no I, O).
const UPS_WEST: &[u8; 12] = b"JKLPQRSTUXYZ";
const UPS_EAST: &[u8; 12] = b"ABCFGHJKLPQR";
const UPS_ROWS: &[u8; 24] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// UPS band, false easting of its columns and false northing of its rows.
fn ups_origin(letter: u8) -> Option<(f64, f64)> {
    Some(match letter {
        b'A' => (800_000.0, 800_000.0),
        b'B' => (2_000_000.0, 800_000.0),
        b'Y' => (800_000.0, 1_300_000.0),
        b'Z' => (2_000_000.0, 1_300_000.0),
        _ => return None,
    })
}

fn digits_of(e: f64, n: f64, digits: u8) -> String {
    if digits == 0 {
        return String::new();
    }
    let scale = 10f64.powi(5 - i32::from(digits));
    let w = usize::from(digits);
    let e = ((e.rem_euclid(100_000.0)) / scale).floor() as u64;
    let n = ((n.rem_euclid(100_000.0)) / scale).floor() as u64;
    format!(" {e:0w$} {n:0w$}")
}

/// The MGRS reference of a point, `digits` (0..=5) per axis: "32TMT 63936 47407".
pub fn to_mgrs(lat: f64, lon: f64, digits: u8) -> Option<String> {
    if !(lat.is_finite() && lon.is_finite()) || lat.abs() > 90.0 || digits > 5 {
        return None;
    }
    if let (Some(zone), Some(b)) = (utm_zone(lat, lon), band(lat)) {
        let u = to_utm_in(lat, lon, zone, lat >= 0.0)?;
        let col = (u.easting / 100_000.0).floor() as i64 - 1;
        let set = COLS[usize::from((zone - 1) % 3)];
        let c = *set.get(usize::try_from(col).ok()?)?;
        let mut row = (u.northing / 100_000.0).floor() as i64;
        if zone.is_multiple_of(2) {
            row += 5;
        }
        let r = ROWS[row.rem_euclid(20) as usize];
        return Some(format!(
            "{zone}{b}{}{}{}",
            c as char,
            r as char,
            digits_of(u.easting, u.northing, digits)
        ));
    }
    // UPS.
    let north = lat > 0.0;
    let u = to_utm_in(lat, lon, 0, north)?;
    let west = u.easting < 2_000_000.0;
    let letter = match (north, west) {
        (true, true) => b'Y',
        (true, false) => b'Z',
        (false, true) => b'A',
        (false, false) => b'B',
    };
    let (fe, fnorth) = ups_origin(letter)?;
    let ci = ((u.easting - fe) / 100_000.0).floor() as i64;
    let ri = ((u.northing - fnorth) / 100_000.0).floor() as i64;
    let cols = if west { UPS_WEST } else { UPS_EAST };
    let c = *cols.get(usize::try_from(ci).ok()?)?;
    let r = *UPS_ROWS.get(usize::try_from(ri).ok()?)?;
    Some(format!(
        "{}{}{}{}",
        letter as char,
        c as char,
        r as char,
        digits_of(u.easting, u.northing, digits)
    ))
}

/// The centre of the cell an MGRS or USNG reference names: (latitude, longitude). Spaces
/// and case do not matter; the digits must split evenly.
pub fn from_mgrs(s: &str) -> Option<(f64, f64)> {
    let t: Vec<u8> = s
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let nd = t.iter().take_while(|b| b.is_ascii_digit()).count();
    if nd > 2 {
        return None;
    }
    let rest = &t[nd..];
    let letters = 3;
    if rest.len() < letters || !rest[..letters].iter().all(u8::is_ascii_alphabetic) {
        return None;
    }
    let digs = &rest[letters..];
    if !digs.len().is_multiple_of(2) || digs.len() > 10 || !digs.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let half = digs.len() / 2;
    let scale = 10f64.powi(5 - half as i32);
    let num = |d: &[u8]| -> f64 {
        d.iter()
            .fold(0.0, |acc, b| acc * 10.0 + f64::from(b - b'0'))
    };
    let (de, dn) = (num(&digs[..half]) * scale, num(&digs[half..]) * scale);
    let centre = scale / 2.0;
    let (bl, cl, rl) = (rest[0], rest[1], rest[2]);
    if nd == 0 {
        // UPS.
        let (fe, fnorth) = ups_origin(bl)?;
        let west = matches!(bl, b'A' | b'Y');
        let cols = if west { UPS_WEST } else { UPS_EAST };
        let ci = cols.iter().position(|&c| c == cl)?;
        let ri = UPS_ROWS.iter().position(|&r| r == rl)?;
        let u = Utm {
            zone: 0,
            north: matches!(bl, b'Y' | b'Z'),
            easting: fe + ci as f64 * 100_000.0 + de + centre,
            northing: fnorth + ri as f64 * 100_000.0 + dn + centre,
        };
        return from_utm(&u);
    }
    let zone: u8 = std::str::from_utf8(&t[..nd]).ok()?.parse().ok()?;
    if !(1..=60).contains(&zone) || !BANDS.contains(&bl) {
        return None;
    }
    let set = COLS[usize::from((zone - 1) % 3)];
    let ci = set.iter().position(|&c| c == cl)?;
    let mut ri = ROWS.iter().position(|&r| r == rl)? as i64;
    if zone.is_multiple_of(2) {
        ri -= 5;
    }
    let ri = ri.rem_euclid(20);
    let north = bl >= b'N';
    let easting = (ci as f64 + 1.0) * 100_000.0 + de + centre;
    let n_mod = ri as f64 * 100_000.0 + dn + centre;
    // The band's lowest northing in the zone (on the central meridian in the north, at the
    // widest zone edge in the south), less a margin: the 2 000 km cycle above it is the one.
    let south_edge = band_south(bl as char)?;
    let cm = central_meridian(zone);
    let mut min_n = f64::INFINITY;
    for off in [-6.0, -3.0, 0.0, 3.0, 6.0] {
        if let Some(u) = to_utm_in(south_edge, cm + off, zone, north) {
            min_n = min_n.min(u.northing);
        }
    }
    let base = min_n - 1_000.0;
    let k = ((base - n_mod) / 2_000_000.0).ceil();
    let northing = n_mod + k.max(0.0) * 2_000_000.0;
    from_utm(&Utm {
        zone,
        north,
        easting,
        northing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_references() {
        assert_eq!(
            to_mgrs(47.376887, 8.541694, 5).as_deref(),
            Some("32TMT 65402 47149")
        );
        assert_eq!(to_mgrs(47.376887, 8.541694, 0).as_deref(), Some("32TMT"));
        assert_eq!(to_mgrs(-89.5, 10.0, 5).as_deref(), Some("BAN 09639 54668"));
        assert_eq!(to_mgrs(85.0, -20.0, 5).as_deref(), Some("YYB 10022 78040"));
        assert_eq!(
            to_mgrs(20.0, -157.9, 5).as_deref(),
            Some("4QFH 15071 11859")
        );
        let (la, lo) = from_mgrs("32tmt6540247149").unwrap();
        assert!((la - 47.376887).abs() < 1e-5 && (lo - 8.541694).abs() < 1e-5);
        assert!(from_mgrs("32TMT654024714").is_none());
        assert!(from_mgrs("61TMT").is_none());
    }
}
