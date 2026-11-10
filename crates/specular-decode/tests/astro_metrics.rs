//! Star metrics and header facts of FITS subframes through the decoders.

use std::sync::Arc;

use specular_astro::synth::{Field, Profile, Psf, fits_u16};
use specular_core::{CancelToken, Source};
use specular_decode::{Registry, astro};

fn open(bytes: Vec<u8>) -> Arc<dyn specular_decode::Session> {
    Registry::new()
        .open(Arc::new(Source::from_vec("sub.fits", bytes)))
        .unwrap()
}

#[test]
fn a_mono_subframe_measures_with_its_header() {
    let psf = Psf::round(Profile::Moffat(3.0), 3.5);
    let f = Field::grid(900, 700, psf, 80, (800.0, 8000.0), 1200.0, 12.0, 31);
    let bytes = fits_u16(
        &f.render(),
        900,
        700,
        &[
            ("OBJECT", "'NGC 7000'"),
            ("FILTER", "'Ha      '"),
            ("IMAGETYP", "'Light Frame'"),
            ("EXPTIME", "300."),
            ("GAIN", "139"),
            ("CCD-TEMP", "-10.2"),
            ("DATE-OBS", "'2025-08-01T22:10:05.5'"),
            ("FOCALLEN", "400."),
            ("XPIXSZ", "3.76"),
        ],
    );
    let s = open(bytes);
    let h = astro::subframe_header(s.as_ref()).unwrap();
    assert_eq!(h.object.as_deref(), Some("NGC 7000"));
    assert_eq!(h.filter.as_deref(), Some("Ha"));
    assert_eq!(h.kind().as_deref(), Some("light"));
    assert_eq!(h.exposure, Some(300.0));
    assert_eq!(h.gain, Some(139.0));
    assert_eq!(h.ccd_temp, Some(-10.2));
    assert_eq!(h.date_obs.as_deref(), Some("2025-08-01T22:10:05.5"));
    let m = astro::measure(s.as_ref(), &CancelToken::new())
        .unwrap()
        .unwrap();
    assert_eq!(m.stars, 80);
    let fwhm = m.fwhm.unwrap();
    assert!((fwhm - 3.5).abs() < 0.1, "FWHM {fwhm}");
    let scale = 206.264_806 * 3.76 / 400.0;
    assert!((m.scale.unwrap() - scale as f32).abs() < 1e-4);
    assert!((m.fwhm_arcsec().unwrap() - 3.5 * scale as f32).abs() < 0.2);
    // The record kept with thumbnails reads back the same.
    assert_eq!(
        specular_astro::FrameMetrics::from_bytes(&m.to_bytes()),
        Some(m)
    );
}

#[test]
fn a_colour_camera_frame_is_measured_binned() {
    let psf = Psf::round(Profile::Gaussian, 6.0);
    let f = Field::grid(1000, 800, psf, 60, (2000.0, 9000.0), 800.0, 8.0, 32);
    let bytes = fits_u16(&f.render(), 1000, 800, &[("BAYERPAT", "'RGGB'")]);
    let s = open(bytes);
    let m = astro::measure(s.as_ref(), &CancelToken::new())
        .unwrap()
        .unwrap();
    assert_eq!(m.stars, 60);
    let fwhm = m.fwhm.unwrap();
    assert!((fwhm - 6.0).abs() < 0.3, "FWHM {fwhm}");
}

#[test]
fn other_images_have_no_subframe_facts() {
    // A FITS image with no keywords of interest.
    let s = open(fits_u16(&[0.0; 64], 8, 8, &[]));
    assert!(astro::subframe_header(s.as_ref()).is_none());
    // Measuring a blank frame finds nothing, without failing.
    let m = astro::measure(s.as_ref(), &CancelToken::new())
        .unwrap()
        .unwrap();
    assert_eq!(m.stars, 0);
}

/// Writes the stars measured in the oracle corpus (`tools/corpus/fetch-astrosub.sh`) and in
/// synthetic Moffat fields (written there as FITS too) as TSV for
/// `crates/specular-astro/gen/oracle.py`. Ignored; needs SPECULAR_ASTROSUB_CORPUS.
#[test]
#[ignore]
fn oracle_dump() {
    let Some(dir) = std::env::var_os("SPECULAR_ASTROSUB_CORPUS") else {
        eprintln!("SPECULAR_ASTROSUB_CORPUS not set; skipped");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    // Synthetic fields with a known truth.
    for (name, psf) in [
        (
            "synth-moffat25-fwhm3.fits",
            Psf::round(Profile::Moffat(2.5), 3.0),
        ),
        (
            "synth-moffat4-fwhm5.fits",
            Psf::round(Profile::Moffat(4.0), 5.0),
        ),
        ("synth-gauss-fwhm4.fits", Psf::round(Profile::Gaussian, 4.0)),
        (
            "synth-moffat3-elong.fits",
            Psf {
                profile: Profile::Moffat(3.0),
                fwhm_major: 5.0,
                fwhm_minor: 3.5,
                angle: 0.6,
            },
        ),
    ] {
        let f = Field::grid(1500, 1000, psf, 150, (500.0, 20000.0), 1500.0, 15.0, 77);
        std::fs::write(dir.join(name), fits_u16(&f.render(), 1500, 1000, &[])).unwrap();
    }
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "fits"))
        .collect();
    names.sort();
    for p in names {
        let s = Registry::new()
            .open(Arc::new(Source::open(&p).unwrap()))
            .unwrap();
        let t = std::time::Instant::now();
        let (m, stars) = astro::measure_stars(s.as_ref(), &CancelToken::new())
            .unwrap()
            .unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let mut out = String::from("x\ty\tamp\tbg\tfwhm\tmajor\tminor\tecc\tangle\tbeta\thfd\n");
        for st in &stars {
            out += &format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                st.x,
                st.y,
                st.amplitude,
                st.background,
                st.fwhm,
                st.fwhm_major,
                st.fwhm_minor,
                st.eccentricity,
                st.angle,
                st.beta,
                st.hfd
            );
        }
        std::fs::write(p.with_extension("stars.tsv"), out).unwrap();
        eprintln!(
            "{}: {} stars, {} measured, FWHM {:?}, HFD {:?}, ecc {:?}, {ms:.1} ms (with decode)",
            p.file_name().unwrap().to_string_lossy(),
            m.stars,
            m.measured,
            m.fwhm,
            m.hfd,
            m.eccentricity
        );
    }
}
