//! The focal range and widest aperture a lens's name states: "EF-S17-85mm f/4-5.6 IS USM",
//! "FE 24-70mm F2.8 GM II", "XF18-55mmF2.8-4 R LM OIS", "LUMIX G VARIO 12-60/F3.5-5.6",
//! "AF-S DX Zoom-Nikkor 18-55mm 1:3.5-5.6G"; through a teleconverter the name adds
//! ("EF100-400mm f/4.5-5.6L IS II USM +1.4x III", "FE 70-200mm F2.8 GM OSS + 1.4X
//! Teleconverter"), scaled by its factor.

/// What a lens name states; either part may be missing.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct NameSpec {
    /// Shortest and longest focal length, in mm.
    pub focal: Option<(f64, f64)>,
    /// Widest aperture at the shortest and at the longest focal length (equal for a constant
    /// aperture), as f-numbers.
    pub aperture: Option<(f64, f64)>,
}

/// A decimal number at the start of `s`, and the bytes it takes.
fn number(s: &[u8]) -> Option<(f64, usize)> {
    let int = s.iter().take_while(|c| c.is_ascii_digit()).count();
    if int == 0 || int > 4 {
        return None;
    }
    let mut n = int;
    if s.get(n) == Some(&b'.') {
        let frac = s[n + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if frac > 0 {
            n += 1 + frac;
        }
    }
    let v = std::str::from_utf8(&s[..n]).ok()?.parse().ok()?;
    Some((v, n))
}

/// A number or a range ("4", "3.5-5.6", "3.5–5.6") at the start of `s`, and the bytes it
/// takes.
fn range(s: &[u8]) -> Option<((f64, f64), usize)> {
    let (a, n) = number(s)?;
    let rest = &s[n..];
    let dash = if rest.starts_with(b"-") {
        1
    } else if rest.starts_with("–".as_bytes()) {
        "–".len()
    } else {
        0
    };
    match (dash > 0).then(|| number(&rest[dash..])).flatten() {
        Some((b, m)) if b >= a => Some(((a, b), n + dash + m)),
        _ => Some(((a, a), n)),
    }
}

/// The f-numbers a scaled aperture is given as: the third stops.
const THIRD_STOPS: [f64; 37] = [
    1.0, 1.1, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, 2.5, 2.8, 3.2, 3.5, 4.0, 4.5, 5.0, 5.6, 6.3, 7.1, 8.0,
    9.0, 10.0, 11.0, 13.0, 14.0, 16.0, 18.0, 20.0, 22.0, 25.0, 29.0, 32.0, 36.0, 40.0, 45.0, 51.0,
    57.0, 64.0,
];

/// The factor of the teleconverter `s` (what follows a lens name's "+") names: the first
/// number followed by "x" or "X" and not by a letter or digit ("1.4x", "1.4X Teleconverter",
/// "Z TELECONVERTER TC-2.0x", "XF1.4X TC WR").
fn converter(s: &[u8]) -> Option<f64> {
    let mut i = 0;
    while i < s.len() {
        let digit_start = s[i].is_ascii_digit() && (i == 0 || !s[i - 1].is_ascii_digit());
        if digit_start && let Some((v, n)) = number(&s[i..]) {
            let x = s.get(i + n).is_some_and(|c| c.eq_ignore_ascii_case(&b'x'));
            let ends = s.get(i + n + 1).is_none_or(|c| !c.is_ascii_alphanumeric());
            if x && ends {
                return Some(v);
            }
            i += n;
            continue;
        }
        i += 1;
    }
    None
}

/// The spec of lens name `name`; through a teleconverter ("<lens> + 1.4x"), the lens's
/// focal range and apertures times its factor, the apertures as the nearest third stops.
pub(crate) fn name_spec(name: &str) -> NameSpec {
    let Some((lens, tc)) = name.split_once('+') else {
        return lens_spec(name);
    };
    let k = match converter(tc.as_bytes()) {
        Some(k) if (1.1..=3.0).contains(&k) => k,
        Some(_) => return NameSpec::default(),
        // A converter of no factor read ("+ TC-14E"); else no converter at all.
        None if tc.to_ascii_lowercase().contains("tc")
            || tc.to_ascii_lowercase().contains("tele") =>
        {
            return NameSpec::default();
        }
        None => return lens_spec(name),
    };
    let spec = lens_spec(lens);
    let stop = |f: f64| {
        let f = f * k;
        THIRD_STOPS
            .into_iter()
            .min_by(|a, b| (a / f).ln().abs().total_cmp(&(b / f).ln().abs()))
            .unwrap_or(f)
    };
    NameSpec {
        focal: spec.focal.map(|(a, b)| (a * k, b * k)),
        aperture: spec.aperture.map(|(a, b)| (stop(a), stop(b))),
    }
}

/// The spec of lens name `name`, a teleconverter aside. The aperture follows "f/", "F/", "f" or "F" (not within a
/// word, but after "mm") or "1:"; the focal range is the one before "mm", or else the one
/// just before the aperture ("12-60/F3.5-5.6").
fn lens_spec(name: &str) -> NameSpec {
    let b = name.as_bytes();
    let mut spec = NameSpec::default();
    // Where the aperture's marker starts.
    let mut marker = None;
    let mut i = 0;
    while i < b.len() {
        let before = i.checked_sub(1).map(|p| b[p]);
        let word_start = before.is_none_or(|c| !c.is_ascii_alphanumeric())
            || (i >= 2 && b[i - 2..i].eq_ignore_ascii_case(b"mm"));
        let skip = match b[i] {
            b'f' | b'F' if word_start => {
                if b.get(i + 1) == Some(&b'/') {
                    2
                } else {
                    1
                }
            }
            b'1' if word_start && b.get(i + 1) == Some(&b':') => 2,
            _ => 0,
        };
        if skip > 0
            && let Some(((x, y), _)) = range(&b[i + skip..])
            && (0.7..=64.0).contains(&x)
            && (x..=64.0).contains(&y)
        {
            spec.aperture = Some((x, y));
            marker = Some(i);
            break;
        }
        i += 1;
    }
    // "<range>mm" (or "<range> mm"), else "<range>/" or "<range> " just before the marker.
    let plausible = |(x, y): (f64, f64)| (1.0..=5000.0).contains(&x) && (x..=5000.0).contains(&y);
    let mut i = 0;
    while i < b.len() {
        // A number's start, not its fraction's.
        let digit_start = b[i].is_ascii_digit()
            && (i == 0 || !b[i - 1].is_ascii_digit())
            && !(i >= 2 && b[i - 1] == b'.' && b[i - 2].is_ascii_digit());
        if digit_start && let Some((r, n)) = range(&b[i..]) {
            let after = &b[i + n..];
            let mm = after.starts_with(b"mm") || after.starts_with(b" mm");
            let at_marker = marker.is_some_and(|m| m == i + n + 1 && b"/ ".contains(&b[i + n]));
            if (mm || at_marker) && plausible(r) {
                spec.focal = Some(r);
                break;
            }
            i += n;
            continue;
        }
        i += 1;
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let s = |f: (f64, f64), a: (f64, f64)| NameSpec {
            focal: Some(f),
            aperture: Some(a),
        };
        for (name, want) in [
            ("EF-S17-85mm f/4-5.6 IS USM", s((17.0, 85.0), (4.0, 5.6))),
            ("EF70-200mm f/2.8L IS II USM", s((70.0, 200.0), (2.8, 2.8))),
            ("RF24-105mm F4 L IS USM", s((24.0, 105.0), (4.0, 4.0))),
            ("FE 24-70mm F2.8 GM II", s((24.0, 70.0), (2.8, 2.8))),
            ("E PZ 16-50mm F3.5-5.6 OSS", s((16.0, 50.0), (3.5, 5.6))),
            ("XF18-55mmF2.8-4 R LM OIS", s((18.0, 55.0), (2.8, 4.0))),
            ("LUMIX G VARIO 12-60/F3.5-5.6", s((12.0, 60.0), (3.5, 5.6))),
            ("LEICA DG 12-60/F2.8-4.0", s((12.0, 60.0), (2.8, 4.0))),
            ("LEICA DG 12-35/F2.8", s((12.0, 35.0), (2.8, 2.8))),
            ("LUMIX S 24-105/F4", s((24.0, 105.0), (4.0, 4.0))),
            ("LUMIX G 25/F1.7", s((25.0, 25.0), (1.7, 1.7))),
            ("LEICA DG SUMMILUX 15/F1.7", s((15.0, 15.0), (1.7, 1.7))),
            ("LUMIX S 1.8/50", NameSpec::default()),
            (
                "LUMIX G X VARIO PZ 14-42/F3.5-5.6",
                s((14.0, 42.0), (3.5, 5.6)),
            ),
            ("OLYMPUS M.7-14mm F2.8", s((7.0, 14.0), (2.8, 2.8))),
            (
                "AF-S DX Zoom-Nikkor 18-55mm 1:3.5-5.6G",
                s((18.0, 55.0), (3.5, 5.6)),
            ),
            ("OLYMPUS M.12-40mm F2.8", s((12.0, 40.0), (2.8, 2.8))),
            ("EF50mm f/1.8 STM", s((50.0, 50.0), (1.8, 1.8))),
            ("NIKKOR Z 24-120mm f/4 S", s((24.0, 120.0), (4.0, 4.0))),
            ("28-75mm F/2.8 Di III RXD", s((28.0, 75.0), (2.8, 2.8))),
            (
                "Sigma 18-35mm f/1.8 DC HSM | Art 013",
                s((18.0, 35.0), (1.8, 1.8)),
            ),
            ("EF-S18-55mm f/3.5–5.6 IS STM", s((18.0, 55.0), (3.5, 5.6))),
        ] {
            assert_eq!(name_spec(name), want, "{name}");
        }
        // Only part, or nothing.
        let n = name_spec("EF-S18-55mm");
        assert_eq!((n.focal, n.aperture), (Some((18.0, 55.0)), None));
        let n = name_spec("FE 4/24-105 G OSS");
        assert_eq!(n.aperture, None);
        for name in ["", "Lens", "FEF", "F", "f/", "1:", "100 mm", "AF 0-0mm f/0"] {
            let n = name_spec(name);
            assert!(n.aperture.is_none() || n.focal.is_none(), "{name}: {n:?}");
        }
        // A reversed range is not one.
        assert_eq!(name_spec("70-24mm f/5.6-4").aperture, Some((5.6, 5.6)));
    }

    #[test]
    fn teleconverters() {
        for (name, focal, aperture) in [
            (
                "Canon EF 100-400mm f/4.5-5.6L IS II USM + 1.4x",
                (140.0, 560.0),
                (6.3, 8.0),
            ),
            (
                "EF100-400mm f/4.5-5.6L IS II USM +1.4x III",
                (140.0, 560.0),
                (6.3, 8.0),
            ),
            (
                "FE 70-200mm F2.8 GM OSS + 1.4X Teleconverter",
                (98.0, 280.0),
                (4.0, 4.0),
            ),
            (
                "FE 70-200mm F2.8 GM OSS + 2X Teleconverter",
                (140.0, 400.0),
                (5.6, 5.6),
            ),
            (
                "NIKKOR Z 100-400mm f/4.5-5.6 VR S + Z TELECONVERTER TC-2.0x",
                (200.0, 800.0),
                (9.0, 11.0),
            ),
            (
                "XF100-400mmF4.5-5.6 R LM OIS WR + XF1.4X TC WR",
                (140.0, 560.0),
                (6.3, 8.0),
            ),
            ("EF300mm f/2.8L IS USM + 1.7x", (510.0, 510.0), (5.0, 5.0)),
        ] {
            let got = name_spec(name);
            let near = |a: Option<(f64, f64)>, b: (f64, f64)| {
                a.is_some_and(|a| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9)
            };
            assert!(
                near(got.focal, focal) && near(got.aperture, aperture),
                "{name}: {got:?}"
            );
        }
        // A converter of no factor read, or none plausible, states nothing.
        for name in [
            "AF-S NIKKOR 200-500mm f/5.6E ED VR + TC-14E",
            "EF 70-200mm f/2.8 + 8x",
            "FE 70-200mm F2.8 GM OSS + Teleconverter",
        ] {
            assert_eq!(name_spec(name), NameSpec::default(), "{name}");
        }
        // A "+" naming no converter is just part of the name.
        let n = name_spec("70-200mm F2.8 + hood");
        assert_eq!(
            (n.focal, n.aperture),
            (Some((70.0, 200.0)), Some((2.8, 2.8)))
        );
    }
}
