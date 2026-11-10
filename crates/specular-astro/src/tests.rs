use std::time::Instant;

use crate::synth::{Field, Profile, Psf};
use crate::{
    FrameMetrics, Image, Metric, Options, SubframeHeader, measure, measure_stars, suggest,
};

fn run(f: &Field) -> (FrameMetrics, Vec<crate::Star>) {
    let v = f.render();
    let img = Image {
        values: &v,
        width: f.width,
        height: f.height,
    };
    measure_stars(&img, &Options::default())
}

fn within(got: f32, want: f64, tol: f64, what: &str) {
    let rel = (f64::from(got) - want).abs() / want;
    assert!(
        rel <= tol,
        "{what}: got {got}, want {want} (off {:.2} %, allowed {:.1} %)",
        rel * 100.0,
        tol * 100.0
    );
}

#[test]
fn gaussian_fwhm_hfd_and_count() {
    for fwhm in [2.5, 3.5, 5.0, 8.0] {
        let psf = Psf::round(Profile::Gaussian, fwhm);
        let f = Field::grid(1200, 900, psf, 150, (400.0, 4000.0), 1000.0, 10.0, 7);
        let (m, _) = run(&f);
        assert_eq!(m.stars, 150, "count at FWHM {fwhm}");
        within(m.fwhm.unwrap(), fwhm, 0.03, &format!("FWHM {fwhm}"));
        within(
            m.hfd.unwrap(),
            psf.hfd(),
            0.05,
            &format!("HFD at FWHM {fwhm}"),
        );
        assert!(m.eccentricity.unwrap() < 0.2, "round: {:?}", m.eccentricity);
        within(m.background, 1000.0, 0.002, "background");
        within(m.noise, 10.0, 0.05, "noise");
    }
}

#[test]
fn moffat_fwhm_and_beta() {
    for beta in [2.5, 4.0] {
        for fwhm in [3.0, 5.0] {
            let psf = Psf::round(Profile::Moffat(beta), fwhm);
            let f = Field::grid(1200, 900, psf, 120, (500.0, 5000.0), 800.0, 8.0, 11);
            let (m, stars) = run(&f);
            assert_eq!(m.stars, 120, "count, β {beta} FWHM {fwhm}");
            within(
                m.fwhm.unwrap(),
                fwhm,
                0.03,
                &format!("Moffat β {beta} FWHM {fwhm}"),
            );
            let mut b: Vec<f32> = stars.iter().map(|s| s.beta).collect();
            b.sort_by(f32::total_cmp);
            within(b[b.len() / 2], beta, 0.15, "β");
            // The aperture cuts the far wings off: the HFD is a little under the whole
            // profile's.
            within(
                m.hfd.unwrap(),
                psf.hfd(),
                0.12,
                &format!("Moffat HFD β {beta}"),
            );
        }
    }
}

#[test]
fn elongated_stars_give_eccentricity_and_angle() {
    for (major, minor, angle) in [(5.0, 4.0, 0.3), (6.0, 3.0, -1.0), (4.0, 3.6, 1.2)] {
        let psf = Psf {
            profile: Profile::Moffat(3.5),
            fwhm_major: major,
            fwhm_minor: minor,
            angle,
        };
        let f = Field::grid(1000, 800, psf, 100, (600.0, 6000.0), 500.0, 6.0, 3);
        let (m, stars) = run(&f);
        within(m.fwhm.unwrap(), psf.fwhm(), 0.03, "mean FWHM");
        within(
            m.eccentricity.unwrap(),
            psf.eccentricity(),
            0.06,
            "eccentricity",
        );
        let s = stars.iter().max_by(|a, b| a.snr.total_cmp(&b.snr)).unwrap();
        within(s.fwhm_major, major, 0.03, "major");
        within(s.fwhm_minor, minor, 0.03, "minor");
        let d = (f64::from(s.angle) - angle).rem_euclid(std::f64::consts::PI);
        let d = d.min(std::f64::consts::PI - d);
        assert!(d < 0.05, "angle {} want {angle}", s.angle);
    }
}

#[test]
fn positions_are_found() {
    let psf = Psf::round(Profile::Gaussian, 3.0);
    let f = Field::grid(600, 400, psf, 40, (1000.0, 3000.0), 200.0, 5.0, 5);
    let (_, stars) = run(&f);
    assert_eq!(stars.len(), 40);
    for t in &f.stars {
        let near = stars
            .iter()
            .map(|s| (f64::from(s.x) - t.x).hypot(f64::from(s.y) - t.y))
            .fold(f64::INFINITY, f64::min);
        assert!(near < 0.05, "star at {:.2},{:.2}: nearest {near}", t.x, t.y);
    }
}

#[test]
fn hot_pixels_and_gradients_are_not_stars() {
    let psf = Psf::round(Profile::Gaussian, 4.0);
    let mut f = Field::grid(800, 600, psf, 60, (500.0, 3000.0), 1000.0, 10.0, 9);
    f.slope = (0.4, -0.2);
    let mut v = f.render();
    let mut rng = crate::synth::Rng::new(99);
    for _ in 0..300 {
        let x = 20 + (rng.unit() * 760.0) as usize;
        let y = 20 + (rng.unit() * 560.0) as usize;
        v[y * 800 + x] += 5000.0;
    }
    let img = Image {
        values: &v,
        width: 800,
        height: 600,
    };
    let m = measure(&img, &Options::default());
    // A hot pixel landing on a star's core can hide it, or on its bright wing count once
    // more; of the 300, at most two may.
    assert!((58..=62).contains(&m.stars), "{} stars", m.stars);
    within(m.fwhm.unwrap(), 4.0, 0.03, "FWHM with a gradient");
}

#[test]
fn noise_alone_has_no_stars() {
    let psf = Psf::round(Profile::Gaussian, 3.0);
    let f = Field::grid(1000, 1000, psf, 0, (1.0, 1.0), 300.0, 12.0, 2);
    let (m, _) = run(&f);
    assert_eq!(m.stars, 0);
    assert_eq!(m.fwhm, None);
    within(m.noise, 12.0, 0.05, "noise");
}

/// A simulated frame without noise (its sky exactly zero) still has its stars found and
/// measured; an empty one has none.
#[test]
fn noise_free_frames_are_measured() {
    let psf = Psf::round(Profile::Moffat(3.0), 3.5);
    let f = Field::grid(800, 600, psf, 100, (500.0, 5000.0), 0.0, 0.0, 5);
    let (m, _) = run(&f);
    assert_eq!(m.stars, 100);
    within(m.fwhm.unwrap(), 3.5, 0.03, "FWHM");
    let blank = Field::grid(400, 300, psf, 0, (1.0, 1.0), 0.0, 0.0, 5);
    let (m, _) = run(&blank);
    assert_eq!(m.stars, 0);
}

#[test]
fn saturated_stars_are_counted_not_measured() {
    let psf = Psf::round(Profile::Gaussian, 4.0);
    let mut f = Field::grid(800, 600, psf, 50, (500.0, 2000.0), 100.0, 5.0, 4);
    // Five stars far brighter, clipped flat at the sensor's top.
    for s in f.stars.iter_mut().step_by(10) {
        s.peak = 40000.0;
    }
    let mut v = f.render();
    for x in v.iter_mut() {
        *x = x.min(20000.0);
    }
    let img = Image {
        values: &v,
        width: 800,
        height: 600,
    };
    let (m, stars) = measure_stars(&img, &Options::default());
    assert_eq!(m.stars, 50);
    assert_eq!(stars.len(), 45, "five saturated left unmeasured");
    within(m.fwhm.unwrap(), 4.0, 0.03, "FWHM");
    // A known saturation level does the same.
    let opts = Options {
        saturation: Some(19000.0),
        ..Options::default()
    };
    assert_eq!(measure_stars(&img, &opts).1.len(), 45);
}

#[test]
fn worse_seeing_and_clouds_move_the_metrics() {
    let psf = Psf::round(Profile::Moffat(3.0), 3.0);
    let good = run(&Field::grid(
        800,
        600,
        psf,
        80,
        (500.0, 4000.0),
        500.0,
        8.0,
        1,
    ))
    .0;
    let blurred = run(&Field::grid(
        800,
        600,
        Psf::round(Profile::Moffat(3.0), 4.5),
        80,
        (330.0, 2700.0),
        500.0,
        8.0,
        1,
    ))
    .0;
    // Thin cloud: a third of the light, a brighter sky.
    let cloud = run(&Field::grid(
        800,
        600,
        psf,
        80,
        (170.0, 1300.0),
        900.0,
        11.0,
        1,
    ))
    .0;
    assert!(blurred.hfd.unwrap() > good.hfd.unwrap() * 1.3);
    assert!(cloud.snr.unwrap() < good.snr.unwrap());
    assert!(cloud.stars <= good.stars);
}

#[test]
fn metrics_round_trip() {
    let m = FrameMetrics {
        stars: 812,
        measured: 600,
        hfd: Some(3.25),
        fwhm: Some(2.9),
        eccentricity: None,
        background: 1203.5,
        noise: 11.25,
        snr: Some(1.4),
        scale: Some(1.21),
    };
    assert_eq!(FrameMetrics::from_bytes(&m.to_bytes()), Some(m));
    assert_eq!(FrameMetrics::from_bytes(&[2, 0, 0]), None);
    assert!((m.fwhm_arcsec().unwrap() - 2.9 * 1.21).abs() < 1e-5);
}

#[test]
fn colour_frames_measure_binned() {
    let psf = Psf::round(Profile::Gaussian, 6.0);
    let f = Field::grid(1000, 800, psf, 60, (800.0, 4000.0), 400.0, 6.0, 8);
    let v = f.render();
    let (b, w, h) = crate::bin2(&Image {
        values: &v,
        width: 1000,
        height: 800,
    });
    let m = measure(
        &Image {
            values: &b,
            width: w,
            height: h,
        },
        &Options::default(),
    )
    .scaled(2.0);
    // Binning adds a pixel's width in quadrature: sqrt(6² + 2²·(2.3548²/12)·…) ≈ 6.15.
    within(m.fwhm.unwrap(), 6.0, 0.04, "binned FWHM");
}

#[test]
fn auto_reject_picks_the_outliers() {
    let mut hfd: Vec<Option<f32>> = (0..30)
        .map(|i| Some(3.0 + 0.05 * ((i * 7 % 11) as f32 - 5.0) / 5.0))
        .collect();
    hfd[4] = Some(4.5);
    hfd[17] = Some(3.9);
    hfd[20] = None;
    let s = suggest(&hfd, Metric::Hfd, 3.0).unwrap();
    assert_eq!(s.frames, vec![4, 17]);
    // Raising k keeps only the worst; a better value is never suggested.
    let s = suggest(&hfd, Metric::Hfd, 25.0).unwrap();
    assert_eq!(s.frames, vec![4]);
    hfd[2] = Some(1.0);
    assert!(!suggest(&hfd, Metric::Hfd, 3.0).unwrap().frames.contains(&2));
    // Star counts: fewer is worse.
    let mut stars: Vec<Option<f32>> = (0..20)
        .map(|i| Some(800.0 + (i % 5) as f32 * 10.0))
        .collect();
    stars[3] = Some(200.0);
    stars[9] = Some(2000.0);
    assert_eq!(suggest(&stars, Metric::Stars, 2.0).unwrap().frames, vec![3]);
    assert!(suggest(&[Some(1.0), None], Metric::Hfd, 2.0).is_none());
    // All equal but one.
    let mut flat = vec![Some(2.0f32); 10];
    flat[6] = Some(5.0);
    assert_eq!(suggest(&flat, Metric::Fwhm, 2.0).unwrap().frames, vec![6]);
}

#[test]
fn header_keywords_and_scale() {
    let cards = [
        ("OBJECT", "M 31"),
        ("FILTER", "Ha"),
        ("IMAGETYP", "Light Frame"),
        ("EXPTIME", "300."),
        ("GAIN", "100"),
        ("CCD-TEMP", "-9.8"),
        ("DATE-OBS", "2025-10-12T21:33:01.123"),
        ("FOCALLEN", "530"),
        ("XPIXSZ", "3.76"),
        ("XBINNING", "1"),
    ];
    let get = |k: &str| cards.iter().find(|c| c.0 == k).map(|c| c.1.to_owned());
    let h = SubframeHeader::from_keywords(get);
    assert_eq!(h.object.as_deref(), Some("M 31"));
    assert_eq!(h.filter.as_deref(), Some("Ha"));
    assert_eq!(h.kind().as_deref(), Some("light"));
    assert_eq!(h.exposure, Some(300.0));
    assert_eq!(h.gain, Some(100.0));
    assert_eq!(h.ccd_temp, Some(-9.8));
    assert!((h.optics_scale().unwrap() - 1.4633).abs() < 1e-3);
    let cd = [
        ("CD1_1", "-2.5E-4"),
        ("CD1_2", "1.0E-5"),
        ("CD2_1", "1.0E-5"),
        ("CD2_2", "2.5E-4"),
    ];
    let s = crate::header::wcs_scale(|k| cd.iter().find(|c| c.0 == k).map(|c| c.1.to_owned()));
    assert!((s.unwrap() - 0.9007).abs() < 1e-3, "{s:?}");
    for (t, want) in [
        ("Dark Frame", "dark"),
        ("FLAT", "flat"),
        ("Bias Frame", "bias"),
        ("DARKFLAT", "darkflat"),
        ("Light", "light"),
    ] {
        let h = SubframeHeader {
            frame_type: Some(t.into()),
            ..Default::default()
        };
        assert_eq!(h.kind().as_deref(), Some(want));
    }
}

/// A 24 MP frame (6000 × 4000) with 3000 stars: the budget is a few tens of ms on the CPU.
#[test]
fn timing_24mp() {
    let psf = Psf::round(Profile::Moffat(3.0), 3.2);
    let f = Field::grid(6000, 4000, psf, 3000, (200.0, 20000.0), 1500.0, 15.0, 21);
    let v = f.render();
    let img = Image {
        values: &v,
        width: 6000,
        height: 4000,
    };
    let opts = Options::default();
    let _ = measure(&img, &opts);
    let mut best = f64::INFINITY;
    let mut m = FrameMetrics::default();
    for _ in 0..3 {
        let t = Instant::now();
        m = measure(&img, &opts);
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    eprintln!(
        "24 MP: {best:.1} ms on {} threads, {} stars, {} measured, FWHM {:?}",
        rayon::current_num_threads(),
        m.stars,
        m.measured,
        m.fwhm
    );
    // A noise bump on a bright star's wing can count once in a while.
    assert!((2997..=3003).contains(&m.stars), "{} stars", m.stars);
    within(m.fwhm.unwrap(), 3.2, 0.03, "FWHM");
    // Generous for a loaded machine; the report gives the measured time.
    assert!(best < 1500.0, "{best} ms");
}
