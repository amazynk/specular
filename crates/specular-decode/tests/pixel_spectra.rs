//! The spectrum under the cursor of hyperspectral cubes: an ENVI cube with band wavelengths
//! (written to the target's scratch directory) and a FITS cube with a spectral third axis
//! (in memory). Values are checked against what the files store.

use std::path::Path;
use std::sync::Arc;

use specular_core::Source;
use specular_decode::Registry;
use specular_decode::science::SpectrumX;

const W: usize = 3;
const H: usize = 2;
const B: usize = 5;

fn value(x: usize, y: usize, b: usize) -> i16 {
    (1000 + 100 * b + 10 * y + x) as i16
}

#[test]
fn envi_cube_spectrum_against_wavelengths() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pixel-spectra");
    std::fs::create_dir_all(&dir).unwrap();
    // BSQ, little-endian 16-bit integers, wavelengths in micrometres.
    let mut data = Vec::new();
    for b in 0..B {
        for y in 0..H {
            for x in 0..W {
                data.extend(value(x, y, b).to_le_bytes());
            }
        }
    }
    std::fs::write(dir.join("cube.img"), &data).unwrap();
    let hdr = format!(
        "ENVI\nsamples = {W}\nlines = {H}\nbands = {B}\nheader offset = 0\nfile type = ENVI Standard\n\
         data type = 2\ninterleave = bsq\nbyte order = 0\nwavelength units = Micrometers\n\
         wavelength = {{0.4, 0.5, 0.6, 0.7, 0.8}}\n"
    );
    std::fs::write(dir.join("cube.hdr"), hdr).unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::open(dir.join("cube.img")).unwrap()))
        .unwrap();
    let sp = s.science().unwrap().spectra.clone().expect("spectra");
    assert_eq!(sp.x_axis(), ("Wavelength".into(), "nm".into()));
    assert_eq!(sp.channels(), B);
    for y in 0..H {
        for x in 0..W {
            let spec = sp.at(x as u32, y as u32, &[]).unwrap();
            let want: Vec<f32> = (0..B).map(|b| f32::from(value(x, y, b))).collect();
            assert_eq!(spec.y, want, "({x}, {y})");
            assert!(!spec.sticks);
            assert_eq!(spec.x_range(), Some((400.0, 800.0)));
            assert!(matches!(spec.x, SpectrumX::Values(_)));
        }
    }
    assert!(sp.at(W as u32, 0, &[]).is_none());
}

fn card(k: &str, v: &str) -> String {
    format!("{k:<8}= {v:>20}")
}

fn fits(cards: &[String], data: &[u8]) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::new();
    for c in cards.iter().chain(std::iter::once(&"END".to_string())) {
        h.extend(format!("{c:<80}").as_bytes());
    }
    h.resize(h.len().div_ceil(2880) * 2880, b' ');
    let mut d = data.to_vec();
    d.resize(d.len().div_ceil(2880) * 2880, 0);
    h.extend(d);
    h
}

#[test]
fn fits_cube_spectrum_against_wcs() {
    // NAXIS1 × NAXIS2 × NAXIS3 big-endian 16-bit integers; FITS rows are bottom-up.
    let mut data = Vec::new();
    for b in 0..B {
        for row in 0..H {
            for x in 0..W {
                data.extend(value(x, row, b).to_be_bytes());
            }
        }
    }
    let cards = [
        card("SIMPLE", "T"),
        card("BITPIX", "16"),
        card("NAXIS", "3"),
        card("NAXIS1", &W.to_string()),
        card("NAXIS2", &H.to_string()),
        card("NAXIS3", &B.to_string()),
        card("CTYPE3", "'WAVE'"),
        card("CUNIT3", "'Angstrom'"),
        card("CRPIX3", "1"),
        card("CRVAL3", "4000"),
        card("CDELT3", "2.5"),
    ];
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("ifu.fits", fits(&cards, &data))))
        .unwrap();
    let sp = s.science().unwrap().spectra.clone().expect("spectra");
    let (name, unit) = sp.x_axis();
    assert_eq!(name, "Wavelength");
    assert_eq!(unit, "Angstrom");
    assert_eq!(sp.channels(), B);
    for y in 0..H {
        for x in 0..W {
            let spec = sp.at(x as u32, y as u32, &[]).unwrap();
            // Displayed row y is FITS row H-1-y.
            let want: Vec<f32> = (0..B).map(|b| f32::from(value(x, H - 1 - y, b))).collect();
            assert_eq!(spec.y, want, "({x}, {y})");
            let (a, z) = spec.x_range().unwrap();
            assert!(
                (a - 4000.0).abs() < 1e-6 && (z - 4010.0).abs() < 1e-6,
                "{a} {z}"
            );
        }
    }
    // A cube whose third axis is not spectral has no pixel spectra.
    let mut plain = cards.to_vec();
    plain[6] = card("CTYPE3", "'STOKES'");
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("p.fits", fits(&plain, &data))))
        .unwrap();
    assert!(s.science().unwrap().spectra.is_none());
}
