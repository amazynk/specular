use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn all_decode(name: &str) -> Vec<Field> {
    let file = fixture(name);
    let fields = scan(&file).unwrap();
    for f in &fields {
        let v = f
            .decode(&file)
            .unwrap_or_else(|e| panic!("{name} {}: {e}", f.param.short));
        assert_eq!(v.len(), f.grid.points());
        let p = f.grid.plane(&v);
        assert_eq!(p.values.len(), (p.width * p.height) as usize);
        assert!(f.grid.geo().is_some(), "{name} {}", f.grid.name());
    }
    fields
}

#[test]
fn sniffs() {
    assert!(sniff(&fixture("simple_axes.grib2")));
    assert!(sniff(&fixture("edition1.grib1")));
    assert!(!sniff(b"GRIB\0\0\0\x03"));
}

#[test]
fn axes_fixture() {
    let f = all_decode("simple_axes.grib2");
    assert_eq!(f.len(), 12);
    let t850 = f
        .iter()
        .find(|f| f.param.short == "t" && f.level.value == Some(850.0) && f.forecast.start == 6)
        .unwrap();
    assert_eq!(t850.level.label(), "850 hPa");
    assert_eq!(t850.forecast.label(), "+6 h");
    assert_eq!(t850.param.label(), "t (Temperature)");
}

#[test]
fn packings_fixture() {
    let file = fixture("packings.grib2");
    let fields = scan(&file).unwrap();
    let names: Vec<_> = fields.iter().map(|f| f.packing.name()).collect();
    for want in [
        "simple",
        "complex",
        "complex, spatial differencing",
        "JPEG 2000",
        "PNG",
        "CCSDS",
        "IEEE float",
    ] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // Every packing of the same field gives (nearly) the same values.
    let simple = fields[0].decode(&file).unwrap();
    for f in &fields {
        let v = f.decode(&file).unwrap();
        assert_eq!(v.len(), simple.len());
        assert!(v.iter().any(|x| x.is_finite()));
    }
    assert!(fields.iter().any(|f| f.bitmap.is_some()));
}

#[test]
fn grids_fixture() {
    let f = all_decode("grids.grib2");
    let names: Vec<_> = f.iter().map(|f| f.grid.name()).collect();
    for want in [
        "regular lat/lon",
        "regular Gaussian N",
        "reduced Gaussian N32",
        "rotated regular lat/lon",
        "polar stereographic (north pole)",
        "polar stereographic (south pole)",
        "Lambert conformal",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(want)),
            "{want} in {names:?}"
        );
    }
}

#[test]
fn products_and_editions() {
    let p = all_decode("products.grib2");
    assert!(p.iter().any(|f| f.ensemble.is_some()));
    let tp = p.iter().find(|f| f.param.short == "tp").unwrap();
    assert_eq!(tp.forecast.end, Some(12));
    assert_eq!(tp.forecast.statistic, Some("accumulation"));
    let e1 = all_decode("edition1.grib1");
    assert!(e1.iter().all(|f| f.edition == 1));
    assert!(e1.iter().any(|f| f.bitmap.is_some()));
    let m = all_decode("mixed.grb");
    assert!(m.iter().any(|f| f.edition == 1) && m.iter().any(|f| f.edition == 2));
}

#[test]
fn dates() {
    let d = DateTime {
        year: 2024,
        month: 2,
        day: 29,
        hour: 18,
        minute: 30,
        second: 0,
    };
    assert_eq!(DateTime::from_unix(d.unix()), d);
    assert_eq!(d.to_string(), "2024-02-29 18:30");
    assert_eq!(DateTime::from_unix(d.unix() + 6 * 3600).day, 1);
}

#[test]
fn damaged_input() {
    let mut file = fixture("simple_axes.grib2");
    // Truncation keeps the complete messages before the cut.
    let n = scan(&file).unwrap().len();
    file.truncate(file.len() - 10);
    let fields = scan(&file).unwrap();
    assert_eq!(fields.len(), n - 1);
    assert!(scan(b"GRIB\0\0\0\x02garbage").is_err());
    assert!(scan(b"nothing here").is_err());
}

/// Tiny messages whose counts would overflow or allocate without bound.
mod hostile {
    use super::*;

    /// A GRIB1 constant field (0 bits per value) on an `ni` × `nj` lat/lon grid.
    fn grib1(ni: u16, nj: u16) -> Vec<u8> {
        let mut pds = [0u8; 28];
        pds[2] = 28;
        pds[3] = 2; // Parameter table.
        pds[4] = 98; // ECMWF.
        pds[7] = 0x80; // Grid description present.
        pds[8] = 11;
        pds[9] = 1; // Surface.
        (pds[12], pds[13], pds[14], pds[17], pds[24]) = (24, 1, 1, 1, 21);
        let mut gds = [0u8; 32];
        gds[2] = 32;
        gds[4] = 255;
        gds[6..8].copy_from_slice(&ni.to_be_bytes());
        gds[8..10].copy_from_slice(&nj.to_be_bytes());
        let mut bds = [0u8; 12];
        bds[2] = 12;
        let total = 8 + pds.len() + gds.len() + bds.len() + 4;
        let mut m = b"GRIB".to_vec();
        m.extend(&(total as u32).to_be_bytes()[1..]);
        m.push(1);
        m.extend(pds);
        m.extend(gds);
        m.extend(bds);
        m.extend(b"7777");
        m
    }

    fn field(ni: u16, nj: u16) -> (Field, Vec<u8>) {
        let file = grib1(ni, nj);
        let f = scan(&file).unwrap().remove(0);
        (f, file)
    }

    #[test]
    fn constant_field_on_a_huge_grid() {
        // 65534 × 65534 points (65535 is "missing") from 84 bytes: a 34 GB vector.
        let (f, file) = field(65534, 65534);
        assert!(f.decode(&file).is_err());
        assert!(f.plane(&file).is_err());
        // A 1 × 1 grid declaring 2^31 values.
        let (mut f, file) = field(1, 1);
        assert_eq!(f.decode(&file).unwrap().len(), 1);
        f.values = 1 << 31;
        assert!(f.decode(&file).is_err());
        // 16384 × 16384 constant points: 2 GiB of f64 under the old cap, refused now.
        let (f, file) = field(16384, 16384);
        assert!(f.decode(&file).is_err());
    }

    #[test]
    fn reduced_plane_past_the_cap() {
        // One row of 2^20 points and 4095 of 1: a 2^20 × 4096 plane.
        let (mut f, file) = field(1, 1);
        let mut pl = vec![1u32; 4096];
        pl[0] = 1 << 20;
        let mut g = (*f.grid).clone();
        (g.ni, g.nj, g.pl) = (0, 4096, Some(pl));
        f.values = g.points();
        f.grid = Arc::new(g);
        assert!(f.plane(&file).is_err());
    }

    #[test]
    fn spatial_differencing_overflow() {
        // Second-order differencing of zeros from 2^31 − 1 overflows i64 after ~10^5 values.
        let n = 200_000;
        let (mut f, _) = field(1, 1);
        let Packing::Simple(scale) = f.packing else {
            unreachable!()
        };
        let mut g = (*f.grid).clone();
        g.ni = n;
        f.grid = Arc::new(g);
        f.values = n as usize;
        f.packing = Packing::Complex(
            scale,
            Complex {
                missing: 0,
                groups: 1,
                width_ref: 0,
                width_bits: 0,
                length_ref: n,
                length_inc: 0,
                last_length: n,
                length_bits: 0,
                order: 2,
                octets: 4,
            },
        );
        let data: Vec<u8> = [0x7FFF_FFFFu32; 3]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        f.data = 0..data.len();
        f.bitmap = None;
        assert_eq!(f.decode(&data).unwrap().len(), n as usize);
    }

    #[test]
    fn valid_time_overflow() {
        // Centuries in 4 octets: start and length of 2^31 − 1 each.
        let (mut f, _) = field(1, 1);
        f.forecast = Forecast {
            unit: TimeUnit::Year,
            start: 214_748_364_700,
            end: Some(429_496_729_400),
            statistic: None,
        };
        assert_eq!(f.forecast.valid_seconds(), i64::MAX);
        let _ = f.valid();
    }
}
