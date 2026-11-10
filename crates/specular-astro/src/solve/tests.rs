use super::*;

fn dir(name: &str) -> PathBuf {
    let d = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("astro-solve-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A TAN solution as cards, one per line (how ASTAP's text output looks).
const CARDS: &[(&str, &str)] = &[
    ("CTYPE1", "'RA---TAN'"),
    ("CTYPE2", "'DEC--TAN'"),
    ("CRVAL1", "83.8221"),
    ("CRVAL2", "-5.3911"),
    ("CRPIX1", "320.5"),
    ("CRPIX2", "240.5"),
    ("CD1_1", "-4.1666E-4"),
    ("CD1_2", "1.0E-6"),
    ("CD2_1", "-1.0E-6"),
    ("CD2_2", "4.1666E-4"),
];

fn padded(cards: &[(&str, &str)]) -> Vec<u8> {
    let mut b = Vec::new();
    for (k, v) in cards {
        b.extend(format!("{k:<8}= {v:>20}").bytes());
        b.resize(b.len().div_ceil(80) * 80, b' ');
    }
    b.extend(format!("{:<80}", "END").bytes());
    b
}

fn check(h: &Header) {
    let w = solution_wcs(h, 640, 480).unwrap();
    let (ra, dec) = w.sky_at(320.0, 240.0, &[]).unwrap();
    assert!(
        (ra - 83.8221).abs() < 1e-6 && (dec + 5.3911).abs() < 1e-6,
        "{ra} {dec}"
    );
    assert!((w.arcsec_per_pixel().unwrap() - 1.5).abs() < 1e-3);
}

#[test]
fn astap_solution_from_wcs_and_ini() {
    let d = dir("astap");
    std::fs::write(d.join("solution.ini"), "PLTSOLVD=T\r\nCRVAL1=83.8221\r\n").unwrap();
    std::fs::write(d.join("solution.wcs"), padded(CARDS)).unwrap();
    check(&read_solution(Solver::Astap, &d).unwrap());
    // Without the .wcs, the .ini's keywords.
    std::fs::remove_file(d.join("solution.wcs")).unwrap();
    let ini: String = std::iter::once("PLTSOLVD=T".to_owned())
        .chain(CARDS.iter().map(|(k, v)| format!("{k}={v}")))
        .chain(["CMDLINE=astap -f x.fits".to_owned()])
        .map(|l| l + "\n")
        .collect();
    std::fs::write(d.join("solution.ini"), ini).unwrap();
    check(&read_solution(Solver::Astap, &d).unwrap());
    // Not solved, with ASTAP's reason.
    std::fs::write(
        d.join("solution.ini"),
        "PLTSOLVD=F\nERROR=Not enough stars\n",
    )
    .unwrap();
    assert_eq!(
        read_solution(Solver::Astap, &d),
        Err(Failure::NotSolved(Some("Not enough stars".into())))
    );
    let empty = dir("astap-empty");
    assert_eq!(read_solution(Solver::Astap, &empty), Err(Failure::NoOutput));
}

#[test]
fn astrometry_net_solution() {
    let d = dir("anet");
    assert_eq!(
        read_solution(Solver::AstrometryNet, &d),
        Err(Failure::NotSolved(None))
    );
    // One card per line, unpadded.
    let text: String = CARDS
        .iter()
        .map(|(k, v)| format!("{k:<8}= {v}\n"))
        .chain(["END\n".to_owned()])
        .collect();
    std::fs::write(d.join("solution.wcs"), text).unwrap();
    std::fs::write(d.join("solution.solved"), [1u8]).unwrap();
    check(&read_solution(Solver::AstrometryNet, &d).unwrap());
    std::fs::write(d.join("solution.wcs"), b"garbage without the end").unwrap();
    let h = read_solution(Solver::AstrometryNet, &d).unwrap();
    assert!(matches!(
        solution_wcs(&h, 10, 10),
        Err(Failure::Unreadable(_))
    ));
}

#[test]
fn sidecar_keeps_the_solution() {
    let h = header_file(&padded(CARDS)).unwrap();
    let side = sidecar(&h);
    assert_eq!(side.len() % 2880, 0);
    let back = header_file(&side).unwrap();
    assert_eq!(
        solution_wcs(&back, 640, 480).unwrap(),
        solution_wcs(&h, 640, 480).unwrap()
    );
}

#[test]
fn command_lines() {
    let img = Path::new("/data/m42.fits");
    let out = Path::new("/run/x");
    let hint = Hint {
        centre: Some((83.82, -5.39)),
        scale: Some(1.5),
        height: 480,
    };
    let s = |v: Vec<OsString>| {
        v.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(
        s(args(Solver::Astap, img, out, &hint)),
        "-f /data/m42.fits -o /run/x/solution -wcs -ra 5.588000 -spd 84.610000 -r 15.000000 \
         -fov 0.200000"
    );
    let a = s(args(Solver::AstrometryNet, img, out, &Hint::default()));
    assert!(a.starts_with("--no-plots --overwrite"));
    assert!(a.contains("--new-fits none"));
    assert!(a.ends_with("--dir /run/x --temp-dir /run/x --out solution /data/m42.fits"));
    assert!(!a.contains("--ra"));
}
