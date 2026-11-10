//! Unit tests on files built in memory.

use super::*;

/// One header (cards `KEY = value`) padded to blocks.
fn header(cards: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in cards {
        let card = if v.is_empty() {
            format!("{k:<80}")
        } else {
            format!("{k:<8}= {v:>20}")
        };
        out.extend_from_slice(format!("{card:<80}").as_bytes());
    }
    out.extend_from_slice(format!("{:<80}", "END").as_bytes());
    while !out.len().is_multiple_of(header::BLOCK) {
        out.push(b' ');
    }
    out
}

fn pad(mut d: Vec<u8>) -> Vec<u8> {
    while !d.len().is_multiple_of(header::BLOCK) {
        d.push(0);
    }
    d
}

fn image(first: bool, bitpix: i32, axes: &[usize], extra: &[(&str, &str)], data: &[u8]) -> Vec<u8> {
    let mut cards: Vec<(String, String)> = Vec::new();
    if first {
        cards.push(("SIMPLE".into(), "T".into()));
    } else {
        cards.push(("XTENSION".into(), "'IMAGE   '".into()));
    }
    cards.push(("BITPIX".into(), bitpix.to_string()));
    cards.push(("NAXIS".into(), axes.len().to_string()));
    for (i, a) in axes.iter().enumerate() {
        cards.push((format!("NAXIS{}", i + 1), a.to_string()));
    }
    if !first {
        cards.push(("PCOUNT".into(), "0".into()));
        cards.push(("GCOUNT".into(), "1".into()));
    }
    for (k, v) in extra {
        cards.push(((*k).into(), (*v).into()));
    }
    let refs: Vec<(&str, &str)> = cards
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out = header(&refs);
    out.extend(pad(data.to_vec()));
    out
}

fn bytes_i16(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

#[test]
fn every_bitpix() {
    // 3 × 2 image, FITS rows bottom-up: values 0..6; display puts row 2 (3, 4, 5) first.
    let expect = [3.0, 4.0, 5.0, 0.0, 1.0, 2.0];
    let cases: Vec<(i32, Vec<u8>)> = vec![
        (8, (0u8..6).collect()),
        (16, bytes_i16(&[0, 1, 2, 3, 4, 5])),
        (32, (0i32..6).flat_map(|x| x.to_be_bytes()).collect()),
        (64, (0i64..6).flat_map(|x| x.to_be_bytes()).collect()),
        (-32, (0..6).flat_map(|x| (x as f32).to_be_bytes()).collect()),
        (-64, (0..6).flat_map(|x| (x as f64).to_be_bytes()).collect()),
    ];
    for (bitpix, data) in cases {
        let f = image(true, bitpix, &[3, 2], &[], &data);
        let fits = parse(&f).unwrap();
        let p = read_plane(&f, &fits.hdus[0], 0).unwrap();
        assert_eq!((p.width, p.height), (3, 2));
        assert_eq!(p.values, expect, "BITPIX {bitpix}");
        assert!(!p.truncated);
    }
}

#[test]
fn scaling_and_blank() {
    let data = bytes_i16(&[-32768, -1, 0, 32767]);
    let f = image(
        true,
        16,
        &[4, 1],
        &[("BZERO", "32768"), ("BLANK", "-1")],
        &data,
    );
    let fits = parse(&f).unwrap();
    let img = fits.hdus[0].image.as_ref().unwrap();
    assert_eq!(img.coding(), "16-bit unsigned");
    let p = read_plane(&f, &fits.hdus[0], 0).unwrap();
    assert_eq!(p.values[0], 0.0);
    assert!(p.values[1].is_nan());
    assert_eq!(&p.values[2..], &[32768.0, 65535.0]);

    let f = image(
        true,
        8,
        &[2, 1],
        &[("BSCALE", "0.5"), ("BZERO", "-1")],
        &[2, 4],
    );
    let p = read_plane(&f, &parse(&f).unwrap().hdus[0], 0).unwrap();
    assert_eq!(p.values, [0.0, 1.0]);
}

#[test]
fn cube_planes() {
    // 2 × 1 × 3 × 2 hypercube: planes are indexed axis 3 fastest.
    let data: Vec<u8> = (0..12).collect();
    let f = image(true, 8, &[2, 1, 3, 2], &[], &data);
    let fits = parse(&f).unwrap();
    let hdu = &fits.hdus[0];
    let img = hdu.image.as_ref().unwrap();
    assert_eq!(img.planes(), 6);
    assert_eq!(img.plane_index(&[2, 1]), 5);
    let p = read_plane(&f, hdu, img.plane_index(&[1, 1])).unwrap();
    assert_eq!(p.values, [8.0, 9.0]);
}

#[test]
fn cube_spectrum() {
    // 2 × 2 × 3 cube of 16-bit values with BZERO: value = 10·plane + stored index.
    let mut v = Vec::new();
    for k in 0..3i16 {
        v.extend((0..4).map(|i| 10 * k + i));
    }
    let f = image(true, 16, &[2, 2, 3], &[("BZERO", "100")], &bytes_i16(&v));
    let fits = parse(&f).unwrap();
    let hdu = &fits.hdus[0];
    // Displayed row 0 is FITS row 1 (stored indices 2, 3).
    assert_eq!(read_spectrum(&f, hdu, 1, 0).unwrap(), [103.0, 113.0, 123.0]);
    assert_eq!(read_spectrum(&f, hdu, 0, 1).unwrap(), [100.0, 110.0, 120.0]);
    for k in 0..3 {
        let p = read_plane(&f, hdu, k).unwrap();
        assert_eq!(p.values[1], read_spectrum(&f, hdu, 1, 0).unwrap()[k]);
    }
    assert!(read_spectrum(&f, hdu, 2, 0).is_err());
    // A cut-off file reads NaN past its end.
    let cut = &f[..2880 + 8 + 2];
    let s = read_spectrum(cut, hdu, 1, 0).unwrap();
    assert_eq!(s[0], 103.0);
    assert!(s[1].is_nan() && s[2].is_nan());
}

#[test]
fn extensions() {
    let mut f = image(true, 8, &[], &[("EXTEND", "T")], &[]);
    f.extend(image(
        false,
        -32,
        &[2, 2],
        &[("EXTNAME", "'SCI'"), ("EXTVER", "1")],
        &[0; 16],
    ));
    f.extend(image(false, 16, &[1, 1], &[("EXTNAME", "'DQ'")], &[0, 7]));
    let fits = parse(&f).unwrap();
    assert_eq!(fits.hdus.len(), 3);
    assert!(fits.hdus[0].image.is_none());
    let names: Vec<_> = fits.images().map(|h| h.name()).collect();
    assert_eq!(names, [Some("SCI".into()), Some("DQ".into())]);
    let p = read_plane(&f, &fits.hdus[2], 0).unwrap();
    assert_eq!(p.values, [7.0]);
}

#[test]
fn truncated_and_garbage() {
    let f = image(true, -32, &[4, 4], &[], &[0x3f, 0x80, 0, 0]);
    let cut = &f[..header::BLOCK + 8];
    let fits = parse(cut).unwrap();
    let p = read_plane(cut, &fits.hdus[0], 0).unwrap();
    assert!(p.truncated);
    // The first two FITS values are the start of the bottom display row.
    assert_eq!(p.values[12], 1.0);
    assert!(p.values[0].is_nan());
    assert_eq!(parse(b"not fits"), Err(Error::NotFits));
    assert!(parse(&f[..100]).is_err());
}

#[test]
fn huge_axes_rejected() {
    let f = image(true, 8, &[1 << 20, 1 << 20], &[], &[]);
    let fits = parse(&f).unwrap();
    assert!(fits.hdus[0].image.is_none());
}

/// Tiny files whose header fields would overflow, allocate or loop without bound.
mod hostile {
    use super::*;
    use crate::table::{Datum, Table};

    /// An empty primary HDU and a binary table with `cards` (after `NAXIS = 2`).
    fn bintable(cards: &[(&str, &str)], data: &[u8]) -> Vec<u8> {
        let mut f = image(true, 8, &[], &[], &[]);
        let mut c = vec![("XTENSION", "'BINTABLE'"), ("BITPIX", "8"), ("NAXIS", "2")];
        c.extend_from_slice(cards);
        c.extend([("PCOUNT", "0"), ("GCOUNT", "1")]);
        f.extend(header(&c));
        f.extend(pad(data.to_vec()));
        f
    }

    fn hdr(cards: &[(&str, &str)]) -> Header {
        Header::from_pairs(cards.iter().map(|(k, v)| (*k, *v, "")))
    }

    /// 2^55 rows in the header (the most a 16-byte row allows), one in the file.
    const HUGE_ROWS: &str = "36028797018963968";

    #[test]
    fn spectrum_of_a_huge_cube() {
        // A 1 × 1 × 2^40 cube: the spectrum would be a 4 TB vector.
        let f = image(true, 8, &[1, 1, 1 << 40], &[], &[0; 8]);
        let fits = parse(&f).unwrap();
        assert!(fits.hdus[0].image.is_some());
        assert!(read_spectrum(&f, &fits.hdus[0], 0, 0).is_err());
    }

    #[test]
    fn compressed_column_widths_overflow() {
        // The first overflows repeat × 4, the second the running row offset.
        for tform2 in ["'9223372036854775807J'", "'4611686018427387903J'"] {
            let h = hdr(&[
                ("ZBITPIX", "16"),
                ("ZNAXIS", "2"),
                ("ZNAXIS1", "1"),
                ("ZNAXIS2", "1"),
                ("ZCMPTYPE", "'RICE_1'"),
                ("TFIELDS", "3"),
                ("TTYPE1", "'COMPRESSED_DATA'"),
                ("TFORM1", "'1PB'"),
                ("TFORM2", tform2),
                ("TFORM3", "'1J'"),
            ]);
            assert!(tiled::image_desc(&h).is_err(), "{tform2}");
        }
    }

    #[test]
    fn tzero_past_i64() {
        let t = Table::from_header(
            &hdr(&[
                ("NAXIS1", "8"),
                ("NAXIS2", "1"),
                ("TFIELDS", "1"),
                ("TFORM1", "'K'"),
                ("TZERO1", "10"),
            ]),
            0,
            8,
            false,
        )
        .unwrap();
        let d = i64::MAX.to_be_bytes();
        assert!(matches!(t.cell(&d, 0, 0)[..], [Datum::Float(_)]));
        let t = Table::from_header(
            &hdr(&[
                ("NAXIS1", "20"),
                ("NAXIS2", "1"),
                ("TFIELDS", "1"),
                ("TFORM1", "'I20'"),
                ("TBCOL1", "1"),
                ("TZERO1", "10"),
            ]),
            0,
            20,
            true,
        )
        .unwrap();
        let d = format!("{:>20}", i64::MAX);
        assert!(matches!(t.cell(d.as_bytes(), 0, 0)[..], [Datum::Float(_)]));
    }

    #[test]
    fn catalogue_rows_past_the_end() {
        let mut row = Vec::new();
        row.extend(10.0f64.to_be_bytes());
        row.extend(20.0f64.to_be_bytes());
        let f = bintable(
            &[
                ("NAXIS1", "16"),
                ("NAXIS2", HUGE_ROWS),
                ("TFIELDS", "2"),
                ("TTYPE1", "'RA'"),
                ("TFORM1", "'D'"),
                ("TTYPE2", "'DEC'"),
                ("TFORM2", "'D'"),
            ],
            &row,
        );
        let c = catalogue::parse(&f).unwrap();
        assert_eq!(c.lng[0], 10.0);
    }

    #[test]
    fn votable_zero_width_fields() {
        let xml = br#"<?xml version="1.0"?>
<VOTABLE version="1.4"><RESOURCE><TABLE name="t">
<FIELD name="ra" datatype="double" arraysize="0" ucd="pos.eq.ra;meta.main"/>
<FIELD name="dec" datatype="double" arraysize="0" ucd="pos.eq.dec;meta.main"/>
<DATA><BINARY><STREAM encoding="base64">AAAA</STREAM></BINARY></DATA>
</TABLE></RESOURCE></VOTABLE>"#;
        let _ = catalogue::parse(xml);
    }

    #[test]
    fn events_rows_past_the_end() {
        let mut row = Vec::new();
        row.extend(1.0f32.to_be_bytes());
        row.extend(2.0f32.to_be_bytes());
        row.extend(100i32.to_be_bytes());
        let f = bintable(
            &[
                ("NAXIS1", "12"),
                ("NAXIS2", HUGE_ROWS),
                ("TFIELDS", "3"),
                ("EXTNAME", "'EVENTS'"),
                ("TTYPE1", "'X'"),
                ("TFORM1", "'E'"),
                ("TTYPE2", "'Y'"),
                ("TFORM2", "'E'"),
                ("TTYPE3", "'PI'"),
                ("TFORM3", "'J'"),
            ],
            &row,
        );
        let fits = parse(&f).unwrap();
        let ev = events::Events::from_hdu(&f, &fits.hdus[1])
            .unwrap()
            .unwrap();
        assert!(ev.count < 1000);
        for band in 0..ev.bands.len() {
            ev.bin(&f, 0, band, &|| false).unwrap();
        }
    }

    #[test]
    fn healpix_rows_past_the_end() {
        let mut row = Vec::new();
        row.extend(4u64.to_be_bytes());
        row.extend(0.5f64.to_be_bytes());
        // 2^31 rows: a 48 GB index and 2^31 lookups before.
        let f = bintable(
            &[
                ("NAXIS1", "16"),
                ("NAXIS2", "2147483648"),
                ("TFIELDS", "2"),
                ("PIXTYPE", "'HEALPIX'"),
                ("ORDERING", "'NUNIQ'"),
                ("TTYPE1", "'UNIQ'"),
                ("TFORM1", "'K'"),
                ("TTYPE2", "'PROBDENSITY'"),
                ("TFORM2", "'D'"),
            ],
            &row,
        );
        let fits = parse(&f).unwrap();
        let mut map = healpix::Map::from_hdu(&fits.hdus[1]).unwrap().unwrap();
        map.index(&f).unwrap();
        // The block padding past the row reads as UNIQ 0, which has no credible regions.
        let _ = map.credible(&f, 0, &[0.5]);
    }
}

/// Planes of tile-compressed cubes whose tiles span several planes.
mod deep_tiles {
    use super::*;

    fn value(x: usize, y: usize, z: usize) -> i16 {
        ((x * 7 + y * 13 + z * 31) % 30_000) as i16
    }

    /// A NOCOMPRESS tile-compressed `w × h × d` cube of [`value`]s in tiles `t`.
    fn cube(w: usize, h: usize, d: usize, t: [usize; 3]) -> Vec<u8> {
        let n = [w.div_ceil(t[0]), h.div_ceil(t[1]), d.div_ceil(t[2])];
        let mut rows = Vec::new();
        let mut heap = Vec::new();
        for tz in 0..n[2] {
            for ty in 0..n[1] {
                for tx in 0..n[0] {
                    let start = heap.len();
                    for z in tz * t[2]..((tz + 1) * t[2]).min(d) {
                        for y in ty * t[1]..((ty + 1) * t[1]).min(h) {
                            for x in tx * t[0]..((tx + 1) * t[0]).min(w) {
                                heap.extend_from_slice(&value(x, y, z).to_be_bytes());
                            }
                        }
                    }
                    rows.extend_from_slice(&((heap.len() - start) as i32).to_be_bytes());
                    rows.extend_from_slice(&(start as i32).to_be_bytes());
                }
            }
        }
        let nrows = n[0] * n[1] * n[2];
        let mut f = image(true, 8, &[], &[], &[]);
        let cards: Vec<(String, String)> = [
            ("XTENSION", "'BINTABLE'".to_owned()),
            ("BITPIX", "8".into()),
            ("NAXIS", "2".into()),
            ("NAXIS1", "8".into()),
            ("NAXIS2", nrows.to_string()),
            ("PCOUNT", heap.len().to_string()),
            ("GCOUNT", "1".into()),
            ("TFIELDS", "1".into()),
            ("TTYPE1", "'COMPRESSED_DATA'".into()),
            ("TFORM1", "'1PB'".into()),
            ("ZIMAGE", "T".into()),
            ("ZBITPIX", "16".into()),
            ("ZNAXIS", "3".into()),
            ("ZNAXIS1", w.to_string()),
            ("ZNAXIS2", h.to_string()),
            ("ZNAXIS3", d.to_string()),
            ("ZTILE1", t[0].to_string()),
            ("ZTILE2", t[1].to_string()),
            ("ZTILE3", t[2].to_string()),
            ("ZCMPTYPE", "'NOCOMPRESS'".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let refs: Vec<(&str, &str)> = cards
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        f.extend(header(&refs));
        rows.extend(heap);
        f.extend(pad(rows));
        f
    }

    fn check(f: &[u8], hdu: &Hdu, z: usize) {
        let img = hdu.image.as_ref().unwrap();
        let (w, h) = (img.width(), img.height());
        let p = read_plane(f, hdu, z).unwrap();
        assert!(!p.truncated);
        for (x, y) in [
            (0, 0),
            (w - 1, 0),
            (0, h - 1),
            (w - 1, h - 1),
            (w / 2, h / 3),
        ] {
            assert_eq!(
                p.values[(h - 1 - y) * w + x],
                f32::from(value(x, y, z)),
                "{x} {y} {z}"
            );
        }
    }

    #[test]
    fn planes_of_deep_tiles_in_any_order() {
        // Edge tiles on every axis; slabs revisited out of order.
        let f = cube(5, 7, 11, [2, 3, 4]);
        let fits = parse(&f).unwrap();
        let hdu = &fits.hdus[1];
        for z in [10, 0, 3, 4, 9, 8, 1, 5, 2, 7, 6, 10, 0] {
            check(&f, hdu, z);
        }
    }

    /// One 64 × 64 × 512 tile is decoded whole (8 MiB of f32) for any of its planes: refused
    /// under a 4 MiB memory budget, read under 64 MiB. A plane of 20000 × 20000 with no data
    /// (1.6 GB of f32) is refused under 64 MiB before it is allocated.
    #[test]
    fn planes_within_memory_budget() {
        use specular_core::budget::with_limit;
        let f = cube(64, 64, 512, [64, 64, 512]);
        let fits = parse(&f).unwrap();
        let hdu = &fits.hdus[1];
        let e = with_limit(4 << 20, || read_plane(&f, hdu, 3)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed >= 64 * 64 * 513 * 4 && n.needed > n.free, "{n:?}");
        with_limit(64 << 20, || check(&f, hdu, 3));

        let f = image(true, 8, &[20_000, 20_000], &[], &[]);
        let fits = parse(&f).unwrap();
        let e = with_limit(64 << 20, || read_plane(&f, &fits.hdus[0], 0)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed >= 1_600_000_000 && n.needed > n.free, "{n:?}");
    }

    #[test]
    fn hostile_every_plane_of_one_deep_tile() {
        // One 64 × 64 × 4096 tile: decoding it whole for each of its planes was 4096 × 16M
        // samples.
        let (w, h, d) = (64, 64, 4096);
        let f = cube(w, h, d, [w, h, d]);
        let fits = parse(&f).unwrap();
        let hdu = &fits.hdus[1];
        let t = std::time::Instant::now();
        for z in 0..d {
            check(&f, hdu, z);
        }
        let el = t.elapsed();
        assert!(el < std::time::Duration::from_secs(5), "{el:?}");
    }
}
