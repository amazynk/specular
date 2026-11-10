use std::collections::HashMap;

use super::*;
use specular_fits::Header;
use specular_fits::wcs::Wcs;

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/hips");

fn props(order: u32, width: u32) -> Properties {
    parse_properties(&format!(
        "# made up\nobs_title = Test survey\nhips_order = {order}\nhips_tile_width = {width}\n\
         hips_tile_format = jpeg png\nhips_frame = equatorial\ndataproduct_type = image\n"
    ))
    .unwrap()
}

/// The oracle's tile content: pixel (c, r) of tile n is (32c + 16, 32r + 16, n mod 251).
pub(crate) fn made_up_tile(n: u64, width: u32) -> Vec<u8> {
    let w = width as usize;
    let mut t = vec![0u8; w * w * 4];
    for r in 0..w {
        for c in 0..w {
            let i = (r * w + c) * 4;
            t[i..i + 4].copy_from_slice(&[
                (32 * c + 16) as u8,
                (32 * r + 16) as u8,
                (n % 251) as u8,
                255,
            ]);
        }
    }
    t
}

pub(crate) fn made_up_tiles(order: u32, width: u32) -> HashMap<(u32, u64), Vec<u8>> {
    (0..12u64 << (2 * order))
        .map(|n| ((order, n), made_up_tile(n, width)))
        .collect()
}

fn view_wcs() -> Wcs {
    let text = std::fs::read_to_string(format!("{DATA}/view.hdr")).unwrap();
    let pairs: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let h = Header::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")));
    Wcs::from_header(&h, &[48, 48]).unwrap()
}

/// Paints the oracle's view; `transpose`: with the tile pixels' rows and columns swapped.
fn paint_view(transpose: bool) -> Vec<u8> {
    let p = props(6, 8);
    let wcs = view_wcs();
    let sky = |x: usize, y: usize| wcs.sky_at(x as f64 + 0.5, y as f64 + 0.5, &[]);
    let mut tiles = HashMap::new();
    for y in 0..48 {
        for x in 0..48 {
            let (a, b) = sky(x, y).unwrap();
            let n = locate(6, 8, a, b).0;
            tiles.entry((6, n)).or_insert_with(|| made_up_tile(n, 8));
        }
    }
    if transpose {
        for t in tiles.values_mut() {
            let src = t.clone();
            for r in 0..8 {
                for c in 0..8 {
                    t[(r * 8 + c) * 4..(r * 8 + c) * 4 + 4]
                        .copy_from_slice(&src[(c * 8 + r) * 4..(c * 8 + r) * 4 + 4]);
                }
            }
        }
    }
    let mut out = vec![0u8; 48 * 48 * 4];
    let missing = paint(
        &mut out,
        48,
        48,
        Sky::Icrs,
        &p,
        6,
        |x, y| sky(x, y).map(|(a, b)| (a, b, Sky::Icrs)),
        |k, n| tiles.get(&(k, n)).map(Vec::as_slice),
    );
    assert!(missing.is_empty());
    out
}

/// Pixels that differ from the oracle's, and of those, the ones more than one tile pixel
/// away (another tile, or a column or row off by two or more).
fn oracle_mismatches(out: &[u8]) -> (usize, usize) {
    let text = std::fs::read_to_string(format!("{DATA}/view.tsv")).unwrap();
    let (mut bad, mut far) = (0, 0);
    for l in text.lines() {
        let v: Vec<i32> = l.split('\t').map(|s| s.parse().unwrap()).collect();
        let i = ((v[1] * 48 + v[0]) * 4) as usize;
        let got = [
            i32::from(out[i]),
            i32::from(out[i + 1]),
            i32::from(out[i + 2]),
        ];
        if got != [v[2], v[3], v[4]] {
            bad += 1;
            if got[2] != v[4] || (got[0] - v[2]).abs() > 32 || (got[1] - v[3]).abs() > 32 {
                far += 1;
            }
        }
    }
    (bad, far)
}

#[test]
fn painting_matches_corner_fit() {
    let (ours, far) = oracle_mismatches(&paint_view(false));
    let (transposed, _) = oracle_mismatches(&paint_view(true));
    eprintln!(
        "hips oracle: {ours} of 2304 differ ({far} by more than one); transposed {transposed}"
    );
    // 2304 pixels: the projective fit inside each tile moves a few boundary pixels by one.
    assert!(
        ours <= 2304 / 50 && far == 0,
        "{ours} of 2304 differ, {far} by more than one"
    );
    assert!(
        transposed > 2304 / 2,
        "the transposed rule differs in only {transposed}"
    );
}

#[test]
fn properties_and_paths() {
    let p = props(9, 512);
    assert_eq!(p.title, "Test survey");
    assert_eq!(p.format, TileFormat::Png);
    assert_eq!(p.frame, Sky::Icrs);
    assert_eq!(
        tile_path(5, 12_345, TileFormat::Jpeg),
        "Norder5/Dir10000/Npix12345.jpg"
    );
    assert_eq!(tile_path(3, 77, TileFormat::Png), "Norder3/Dir0/Npix77.png");
    // DSS2 colour at order 9 (×512): about 1 arcsecond per pixel.
    assert!((cell_size(18) * 3600.0 - 0.805).abs() < 0.01);
    assert_eq!(p.order_for(1.0), 0);
    assert_eq!(p.order_for(cell_size(14) * 1.01), 5);
    assert_eq!(p.order_for(1e-9), 9);
    for (text, want) in [
        (
            "hips_order = 3\ndataproduct_type = catalog\n",
            "the survey holds catalog data, not images",
        ),
        (
            "hips_order = 3\nhips_tile_format = fits\n",
            "its tiles are only in fits, which is not shown yet",
        ),
        (
            "hips_order = 3\nhips_frame = ecliptic\n",
            "its ecliptic coordinate frame is not supported",
        ),
        (
            "hips_tile_format = png\n",
            "its properties have no usable order",
        ),
        (
            "hips_order = 3\nhips_tile_width = 500\n",
            "its properties have no usable tile width",
        ),
    ] {
        assert_eq!(parse_properties(text).unwrap_err().to_string(), want);
    }
    // Old keywords.
    let old = parse_properties("maxOrder = 4\nformat = jpeg\ncoordsys = G\n").unwrap();
    assert_eq!(
        (old.max_order, old.format, old.frame),
        (4, TileFormat::Jpeg, Sky::Galactic)
    );
}

#[test]
fn coarser_tiles_stand_in() {
    let p = props(4, 8);
    // Only order 2 loaded.
    let tiles = made_up_tiles(2, 8);
    let mut out = vec![0u8; 4 * 4 * 4];
    let missing = paint(
        &mut out,
        4,
        4,
        Sky::Galactic,
        &p,
        4,
        |x, y| (x != 0).then_some((10.0 * x as f64, 5.0 * y as f64, Sky::Galactic)),
        |k, n| tiles.get(&(k, n)).map(Vec::as_slice),
    );
    assert!(!missing.is_empty());
    let mut sorted = missing.clone();
    sorted.dedup();
    assert_eq!(sorted, missing);
    // Painted from order 2 except the first column, left clear.
    for y in 0..4 {
        assert_eq!(out[y * 16 + 3], 0);
        assert_eq!(out[y * 16 + 4 + 3], 255);
    }
}

#[test]
fn locate_agrees_with_healpix() {
    // The tile index is the order-k cell; the pixel's cell at k + m is inside it.
    for &(l, b) in &[(0.0, 0.0), (45.0, 41.8), (200.0, -60.0), (359.9, 89.0)] {
        let (n, c, r) = locate(3, 8, l, b);
        let theta = (90.0f64 - b).to_radians();
        let phi = f64::to_radians(l);
        assert_eq!(n, ang2pix_nest(8, theta, phi));
        let fine = ang2pix_nest(64, theta, phi);
        assert_eq!(fine >> 6, n);
        assert!(c < 8 && r < 8);
    }
}

/// Timing (run by hand, `--ignored --nocapture`): a 960 × 540 painting of a TAN view.
#[test]
#[ignore]
fn paint_timing() {
    let p = parse_properties("hips_order = 9\nhips_tile_width = 512\nhips_tile_format = jpeg\n")
        .unwrap();
    let h = Header::from_pairs([
        ("CTYPE1", "'RA---TAN'", ""),
        ("CTYPE2", "'DEC--TAN'", ""),
        ("CRVAL1", "202.47", ""),
        ("CRVAL2", "47.2", ""),
        ("CRPIX1", "480.5", ""),
        ("CRPIX2", "270.5", ""),
        ("CDELT1", "-0.0005", ""),
        ("CDELT2", "0.0005", ""),
    ]);
    let wcs = Wcs::from_header(&h, &[960, 540]).unwrap();
    let order = p.order_for(0.0005);
    let sky = |x: usize, y: usize| wcs.sky_at(x as f64 + 0.5, y as f64 + 0.5, &[]);
    let mut tiles = HashMap::new();
    for y in (0..540).step_by(4) {
        for x in (0..960).step_by(4) {
            let (a, b) = sky(x, y).unwrap();
            let n = locate(order, 512, a, b).0;
            tiles
                .entry((order, n))
                .or_insert_with(|| made_up_tile(n, 512));
        }
    }
    let mut out = vec![0u8; 960 * 540 * 4];
    let t = std::time::Instant::now();
    let runs = 5;
    for _ in 0..runs {
        paint(
            &mut out,
            960,
            540,
            Sky::Icrs,
            &p,
            order,
            |x, y| sky(x, y).map(|(a, b)| (a, b, Sky::Icrs)),
            |k, n| tiles.get(&(k, n)).map(Vec::as_slice),
        );
    }
    eprintln!(
        "hips paint 960x540, order {order}, {} tiles: {:.1} ms",
        tiles.len(),
        t.elapsed().as_secs_f64() * 1000.0 / f64::from(runs)
    );
}
