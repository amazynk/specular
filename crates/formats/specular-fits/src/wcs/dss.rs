//! Digitized Sky Survey plate solutions (`PLTRAH`… / `PPO1..6` / `AMDX1..13`, `AMDY1..13`),
//! as STScI's DSS cutouts carry them, read as wcslib does: they win over any `CTYPE` / `CD`
//! cards beside them.
//!
//! A pixel (1-based, in the cutout whose corner is plate pixel `CNPIX1/2`) is first put on
//! the plate in millimetres from its centre,
//!
//! ```text
//! x = (PPO3 − XPIXELSZ · (CNPIX1 + px − 0.5)) / 1000
//! y = (YPIXELSZ · (CNPIX2 + py − 0.5) − PPO6) / 1000
//! ```
//!
//! then to standard coordinates in arcseconds by the plate polynomial (r² = x² + y²),
//!
//! ```text
//! ξ = A1 x + A2 y + A3 + A4 x² + A5 xy + A6 y² + A7 r² + A8 x³ + A9 x²y + A10 xy² + A11 y³
//!     + A12 x r² + A13 x r⁴
//! η = B1 y + B2 x + B3 + B4 y² + B5 xy + B6 x² + B7 r² + B8 y³ + B9 xy² + B10 x²y + B11 x³
//!     + B12 y r² + B13 y r⁴
//! ```
//!
//! and deprojected gnomonically about the plate centre. That is a TPV system: the
//! millimetres are the linear step (scaled by `PLTSCALE` so the polynomial stays close to
//! the identity), the polynomial TPV's, the plate centre `CRVAL`. The header is rewritten
//! so and read as TPV. The magnitude and colour terms (`AMDX14..20`) are not positional and
//! are left out, as wcslib does.

use crate::header::{Card, Header, Value};

/// TPV term numbers (longitude axis, variables x, y; the latitude axis swaps them) of
/// x^i y^j for i + j ≤ 5.
fn term(i: u32, j: u32) -> usize {
    let d = i + j;
    // Degree d starts at TPV term: 0, 1, 4, 7, 12, 17 (r terms 3, 11 skipped).
    let start = [0, 1, 4, 7, 12, 17][d as usize];
    start + j as usize
}

/// Adds the plate polynomial `a` (`AMDX1..13` or `AMDY1..13`, in that axis' own variable
/// order: u its own coordinate, v the other) to TPV coefficients.
fn add(c: &mut [f64; 40], a: &[f64; 13], scale: f64) {
    let mut put = |i: u32, j: u32, v: f64| {
        // u = k·mm: a degree-d monomial in millimetres is the monomial in u over k^d; arcsec
        // to degrees.
        c[term(i, j)] += v / 3600.0 / scale.powi((i + j) as i32);
    };
    put(1, 0, a[0]);
    put(0, 1, a[1]);
    put(0, 0, a[2]);
    put(2, 0, a[3]);
    put(1, 1, a[4]);
    put(0, 2, a[5]);
    put(2, 0, a[6]);
    put(0, 2, a[6]);
    put(3, 0, a[7]);
    // A9 x²y / B9 xy²: for the latitude axis (u = y) xy² is u²v, the same slot.
    put(2, 1, a[8]);
    put(1, 2, a[9]);
    put(0, 3, a[10]);
    // u r² = u³ + uv²; u r⁴ = u⁵ + 2u³v² + uv⁴.
    put(3, 0, a[11]);
    put(1, 2, a[11]);
    put(5, 0, a[12]);
    put(3, 2, 2.0 * a[12]);
    put(1, 4, a[12]);
}

fn is_wcs_key(k: &str) -> bool {
    let digits_after = |p: &str| {
        k.strip_prefix(p)
            .is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit() || b == b'_'))
    };
    [
        "CTYPE", "CRVAL", "CRPIX", "CDELT", "CUNIT", "CROTA", "CD", "PC", "PV", "AMDX", "AMDY",
    ]
    .iter()
    .any(|p| digits_after(p))
        || matches!(
            k,
            "LONPOLE" | "LATPOLE" | "A_ORDER" | "B_ORDER" | "AP_ORDER" | "BP_ORDER"
        )
        || ["A_", "B_", "AP_", "BP_"].iter().any(|p| {
            k.strip_prefix(p)
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        })
}

/// The header rewritten as TPV when it holds a DSS plate solution; `None` otherwise.
pub(super) fn to_tpv(h: &Header) -> Option<Header> {
    let f = |k: &str| h.f64(k).filter(|v| v.is_finite());
    let ra = 15.0
        * (f("PLTRAH")? + f("PLTRAM").unwrap_or(0.0) / 60.0 + f("PLTRAS").unwrap_or(0.0) / 3600.0);
    let sign = if h
        .str("PLTDECSN")
        .is_some_and(|s| s.trim_start().starts_with('-'))
    {
        -1.0
    } else {
        1.0
    };
    let dec = sign
        * (f("PLTDECD")?
            + f("PLTDECM").unwrap_or(0.0) / 60.0
            + f("PLTDECS").unwrap_or(0.0) / 3600.0);
    let (ppo3, ppo6) = (f("PPO3")?, f("PPO6")?);
    let (xsz, ysz) = (f("XPIXELSZ")?, f("YPIXELSZ")?);
    let (cx, cy) = (f("CNPIX1").unwrap_or(0.0), f("CNPIX2").unwrap_or(0.0));
    let scale = f("PLTSCALE")? / 3600.0;
    if xsz <= 0.0 || ysz <= 0.0 || scale <= 0.0 {
        return None;
    }
    let mut amd = [[0.0; 13]; 2];
    for (a, axis) in amd.iter_mut().zip(["AMDX", "AMDY"]) {
        for (k, v) in a.iter_mut().enumerate() {
            *v = f(&format!("{axis}{}", k + 1)).unwrap_or(0.0);
        }
    }
    if amd[0][0] == 0.0 && amd[0][1] == 0.0 {
        return None;
    }
    let mut c = [[0.0; 40]; 2];
    add(&mut c[0], &amd[0], scale);
    add(&mut c[1], &amd[1], scale);
    let mut cards: Vec<Card> = h
        .cards
        .iter()
        .filter(|c| !is_wcs_key(&c.key))
        .cloned()
        .collect();
    let mut put = |key: String, value: Value| {
        cards.push(Card {
            key,
            value: Some(value),
            comment: String::new(),
        });
    };
    put("CTYPE1".into(), Value::Str("RA---TPV".into()));
    put("CTYPE2".into(), Value::Str("DEC--TPV".into()));
    put("CRVAL1".into(), Value::Float(ra));
    put("CRVAL2".into(), Value::Float(dec));
    // x_mm = −XPIXELSZ/1000 · (px − (PPO3/XPIXELSZ − CNPIX1 + 0.5)), and y alike.
    put("CRPIX1".into(), Value::Float(ppo3 / xsz - cx + 0.5));
    put("CRPIX2".into(), Value::Float(ppo6 / ysz - cy + 0.5));
    put("CD1_1".into(), Value::Float(-xsz / 1000.0 * scale));
    put("CD1_2".into(), Value::Float(0.0));
    put("CD2_1".into(), Value::Float(0.0));
    put("CD2_2".into(), Value::Float(ysz / 1000.0 * scale));
    for (axis, coeffs) in c.iter().enumerate() {
        for (k, &v) in coeffs.iter().enumerate() {
            if v != 0.0 {
                put(format!("PV{}_{k}", axis + 1), Value::Float(v));
            }
        }
    }
    Some(Header { cards })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_numbers_follow_tpv() {
        assert_eq!([term(0, 0), term(1, 0), term(0, 1)], [0, 1, 2]);
        assert_eq!([term(2, 0), term(1, 1), term(0, 2)], [4, 5, 6]);
        assert_eq!([term(3, 0), term(0, 3)], [7, 10]);
        assert_eq!(
            [term(4, 0), term(5, 0), term(3, 2), term(1, 4)],
            [12, 17, 19, 21]
        );
    }

    #[test]
    fn wcs_keys() {
        for k in [
            "CTYPE1", "CD1_2", "PC001001", "PV2_17", "AMDX13", "A_ORDER", "A_0_2", "CROTA2",
        ] {
            assert!(is_wcs_key(k), "{k}");
        }
        for k in [
            "CDSEC",
            "PCOUNT",
            "PLTSCALE",
            "AMPSEC",
            "OBJECT",
            "A_SOMETHING",
        ] {
            assert!(!is_wcs_key(k), "{k}");
        }
    }
}
