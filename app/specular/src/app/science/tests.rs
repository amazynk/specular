//! Science mode's pure parts: colours, numbers, the scale bar, slider hits, the grid and the
//! readout's sky lines, on synthetic FITS files built here.

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{Registry, Session};

use super::*;

/// A FITS file of `cards` (each `KEY = value`) and big-endian f32 `values`.
pub(super) fn fits(cards: &[(&str, &str)], values: &[f32]) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::new();
    let end = [("END", "")];
    for (k, v) in cards.iter().chain(&end) {
        let c = if v.is_empty() {
            (*k).to_owned()
        } else {
            format!("{k:<8}= {v:>20}")
        };
        h.extend(format!("{c:<80}").as_bytes());
    }
    h.resize(h.len().div_ceil(2880) * 2880, b' ');
    let mut d: Vec<u8> = values.iter().flat_map(|v| v.to_be_bytes()).collect();
    d.resize(d.len().div_ceil(2880) * 2880, 0);
    h.extend(d);
    h
}

pub(super) fn open(data: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec("synthetic.fits", data)))
        .expect("synthetic FITS opens")
}

/// A `w × h × planes` cube on a TAN projection near M 51, 1.8″ per pixel; the third axis
/// is `ctype3` (empty: colour, when there are three planes). Each plane is a sky with a
/// few Gaussian "stars" and a galaxy whose brightness changes from plane to plane.
pub(super) fn sky(w: usize, h: usize, planes: usize, ctype3: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(w * h * planes);
    let stars = [
        (0.2, 0.3, 900.0, 1.8),
        (0.75, 0.2, 400.0, 1.5),
        (0.6, 0.8, 1500.0, 2.2),
        (0.1, 0.85, 250.0, 1.3),
    ];
    for k in 0..planes {
        for j in 0..h {
            for i in 0..w {
                let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
                let (cx, cy) = (w as f32 * 0.45, h as f32 * 0.5);
                let r2 =
                    ((x - cx) / (w as f32 * 0.18)).powi(2) + ((y - cy) / (h as f32 * 0.12)).powi(2);
                let core = [1.0, 0.7, 0.4, 0.8, 0.6][k % 5];
                let disk = [0.35, 0.6, 1.0, 0.5, 0.8][k % 5];
                let r2c = r2 * 9.0;
                let mut s =
                    100.0 + 0.05 * x + 60.0 * disk * (-r2).exp() + 400.0 * core * (-r2c).exp();
                for (n, (sx, sy, a, sig)) in stars.into_iter().enumerate() {
                    let d2 = (x - sx * w as f32).powi(2) + (y - sy * h as f32).powi(2);
                    let tint = [1.0, 0.45, 0.15][(n + k) % 3];
                    s += a * tint * (-d2 / (2.0 * sig * sig)).exp();
                }
                // A deterministic speckle as noise.
                s += ((i * 7 + j * 13 + k * 5) % 11) as f32 - 5.0;
                v.push(s);
            }
        }
    }
    let (ws, hs, ns) = (w.to_string(), h.to_string(), planes.to_string());
    let (c1, c2) = (
        (w as f64 / 2.0 + 0.5).to_string(),
        (h as f64 / 2.0 + 0.5).to_string(),
    );
    let ct3 = format!("'{ctype3}'");
    let mut cards = vec![
        ("SIMPLE", "T"),
        ("BITPIX", "-32"),
        ("NAXIS", if planes > 1 { "3" } else { "2" }),
        ("NAXIS1", ws.as_str()),
        ("NAXIS2", hs.as_str()),
    ];
    if planes > 1 {
        cards.push(("NAXIS3", ns.as_str()));
    }
    cards.extend([
        ("CTYPE1", "'RA---TAN'"),
        ("CTYPE2", "'DEC--TAN'"),
        ("CRVAL1", "202.4696"),
        ("CRVAL2", "47.1952"),
        ("CRPIX1", c1.as_str()),
        ("CRPIX2", c2.as_str()),
        ("CDELT1", "-0.0005"),
        ("CDELT2", "0.0005"),
        ("RADESYS", "'FK5'"),
        ("EQUINOX", "2000.0"),
        ("BUNIT", "'ADU'"),
    ]);
    if planes > 1 && !ctype3.is_empty() {
        cards.extend([
            ("CTYPE3", ct3.as_str()),
            ("CRVAL3", "656.3"),
            ("CRPIX3", "1"),
            ("CDELT3", "0.5"),
            ("CUNIT3", "'nm'"),
        ]);
    }
    fits(&cards, &v)
}

/// Text without Fluent's isolation marks.
fn bare(s: &str) -> String {
    s.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn colours_per_channel_count() {
    assert_eq!(channel_colours(1), vec![[1.0; 3]]);
    assert_eq!(channel_colours(2), vec![[1.0, 0.0, 1.0], [0.0, 1.0, 0.0]]);
    assert_eq!(channel_colours(3)[2], [0.0, 0.0, 1.0]);
    assert_eq!(channel_colours(4).len(), 4);
    assert_eq!(channel_colours(9).len(), 9);
    assert_eq!(channel_colours(40).len(), MAX_CHANNELS);
}

#[test]
fn values_read_well() {
    assert_eq!(value_text(0.0), "0");
    assert_eq!(value_text(1234.0), "1234");
    assert_eq!(value_text(-3.0), "\u{2212}3");
    assert_eq!(value_text(0.5), "0.5");
    assert_eq!(value_text(12.3456), "12.35");
    assert_eq!(value_text(1.5e-5), "1.500e\u{2212}5");
    assert_eq!(value_text(f32::NAN), "NaN");
    assert_eq!(world_text(1_420_650_000.0), "1.42065e9");
    assert_eq!(world_text(657.3), "657.3");
    assert_eq!(world_text(-0.25), "\u{2212}0.25");
    assert_eq!(world_text(12_345.678_9), "12345.68");
}

#[test]
fn scene_classes_have_names() {
    let names: Vec<String> = (0..=11)
        .map(|n| class_name(ClassScheme::Sentinel2Scl, n).unwrap())
        .collect();
    assert_eq!(names[4], "Vegetation");
    assert!(names.iter().all(|n| !n.starts_with("s2-")), "{names:?}");
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 12);
    assert_eq!(class_name(ClassScheme::Sentinel2Scl, 12), None);
    for n in 0..=3 {
        assert!(class_name(ClassScheme::Sentinel2L1cMask, n).is_some());
    }
    assert_eq!(class_name(ClassScheme::Sentinel2L1cMask, 4), None);
}

#[test]
fn data_quality_bits() {
    assert_eq!(flag_bits(9.0).as_deref(), Some("0, 3"));
    assert_eq!(flag_bits(2_147_483_648.0).as_deref(), Some("31"));
    assert_eq!(flag_bits(1.0).as_deref(), Some("0"));
    assert_eq!(flag_bits(0.0), None);
    assert_eq!(flag_bits(-1.0), None);
    assert_eq!(flag_bits(2.5), None);
    assert_eq!(flag_bits(f32::NAN), None);
}

#[test]
fn scale_bar_picks_round_lengths() {
    // 1″ per pixel at one logical px per pixel: 120″ fits, so 2′.
    let b = scale_bar(1.0, 1.0).unwrap();
    assert_eq!(bare(&b.label), "2′");
    assert!((b.px - 120.0).abs() < 1e-3);
    // Zoomed in 8×: 15″ is 120 px.
    let b = scale_bar(1.0, 8.0).unwrap();
    assert_eq!(bare(&b.label), "15″");
    assert!((b.px - 120.0).abs() < 1e-3);
    // A wide field, 30″ per pixel: 1° is 120 px.
    let b = scale_bar(30.0, 1.0).unwrap();
    assert_eq!(bare(&b.label), "1°");
    // Sub-arcsecond.
    assert_eq!(bare(&scale_bar(0.01, 1.0).unwrap().label), "1″");
    assert!(scale_bar(0.0, 1.0).is_none());
    assert!(b.px <= SCALE_MAX_PX);
}

#[test]
fn linear_scale_bar_picks_round_lengths() {
    // 0.25 nm per pixel (in metres, as Velox writes it): 30 nm fits, so 20 nm over 80 px.
    let b = linear_scale_bar(2.5e-10, "m", 1.0).unwrap();
    assert_eq!(bare(&b.label), "20 nm");
    assert!((b.px - 80.0).abs() < 1e-3);
    // Zoomed in 64×: 0.47 nm fit, so 0.2 nm.
    let b = linear_scale_bar(0.25, "nm", 64.0).unwrap();
    assert_eq!(bare(&b.label), "0.2 nm");
    assert!(b.px <= SCALE_MAX_PX);
    // Micrometres and millimetres get their prefix.
    assert_eq!(
        bare(&linear_scale_bar(1.0, "µm", 1.0).unwrap().label),
        "100 µm"
    );
    assert_eq!(
        bare(&linear_scale_bar(0.1, "mm", 1.0).unwrap().label),
        "10 mm"
    );
    // A unit that is not a length stays as given.
    assert_eq!(
        bare(&linear_scale_bar(0.01, "1/nm", 1.0).unwrap().label),
        "1 1/nm"
    );
    assert!(linear_scale_bar(0.0, "nm", 1.0).is_none());
}

#[test]
fn unitless_scale_bar_has_no_trailing_space() {
    // A ROOT axis without a unit: the raw label (isolates included) is the number alone, so
    // "100 (layer)" gets one space, not two.
    let b = linear_scale_bar(1.0, "", 1.0).unwrap();
    assert_eq!(b.label, "100");
    let b = linear_scale_bar(1.0, "  ", 1.0).unwrap();
    assert_eq!(b.label, "100");
}

#[test]
fn slider_positions_follow_direction() {
    let r = Rect::new(10.0, 0.0, 100.0, 4.0);
    assert_eq!(slider_value(r, 10.0, 5, false), 0);
    assert_eq!(slider_value(r, 110.0, 5, false), 4);
    assert_eq!(slider_value(r, 60.0, 5, false), 2);
    assert_eq!(slider_value(r, 0.0, 5, false), 0);
    assert_eq!(slider_value(r, 10.0, 5, true), 4);
    assert_eq!(slider_value(r, 110.0, 5, true), 0);
    assert_eq!(slider_value(r, 50.0, 1, false), 0);
}

#[test]
fn sky_lines_sexagesimal_and_degrees() {
    let f = Frame::Equatorial {
        system: "FK5".into(),
        equinox: Some(2000.0),
    };
    let l: Vec<String> = sky_lines(&f, 202.4696, 47.1952)
        .iter()
        .map(|s| bare(s))
        .collect();
    assert_eq!(l.len(), 2);
    assert!(l[0].starts_with("RA 13:29:52.70"), "{}", l[0]);
    assert!(l[0].contains("+47:11:42.7"), "{}", l[0]);
    assert_eq!(l[1], "202.46960°  47.19520°".replace("  4", "  +4"));
    let l: Vec<String> = sky_lines(&Frame::Galactic, -10.0, -5.5)
        .iter()
        .map(|s| bare(s))
        .collect();
    assert_eq!(l, vec!["l 350.0000°  b \u{2212}5.5000°".to_owned()]);
}

#[test]
fn summary_of_a_cube_and_its_planes() {
    let s = open(sky(40, 30, 5, "WAVE"));
    let sum = s.science().expect("scientific").clone();
    assert_eq!((sum.width, sum.height), (40, 30));
    assert_eq!(sum.axes.len(), 1);
    assert_eq!(sum.axes[0].len, 5);
    assert_eq!(sum.channel_axis, None);
    assert_eq!(axis_name(&sum, 0), "WAVE");
    assert_eq!(axis_world(&sum, 0, 2).as_deref(), Some("657.3 nm"));
    let w = sum.wcs.as_ref().unwrap();
    assert_eq!(wcs_rest(&sum, w, &[3]), vec![3]);
    let p = s.nd_plane(&[4], &CancelToken::new()).unwrap().unwrap();
    assert_eq!((p.width, p.height), (40, 30));
    assert!((w.arcsec_per_pixel().unwrap() - 1.8).abs() < 1e-6);

    let mut sc = Science {
        summary: Some(Arc::new(sum)),
        index: vec![2],
        ..Default::default()
    };
    let r = sc.request().unwrap();
    assert_eq!(r.index, vec![2]);
    assert_eq!(r.composite, None);
    // The cube's planes have no names: toasts say "Channel n", not a bare number.
    assert_eq!(sc.channel_name(10), "11");
    assert_eq!(
        sc.channel_title(10).replace(['\u{2068}', '\u{2069}'], ""),
        "Channel 11"
    );
    // Compositing the cube's axis (no channel axis): all five planes.
    sc.composite = true;
    let r = sc.request().unwrap();
    assert_eq!(r.index, vec![0]);
    assert_eq!(r.composite, Some((0, 5)));
    assert!(sc.steppable().is_empty());
}

#[test]
fn colour_images_composite_by_default_look() {
    let s = open(sky(32, 24, 3, ""));
    let sum = s.science().unwrap().clone();
    assert_eq!(sum.channel_axis, Some(0));
    let mut sc = Science {
        summary: Some(Arc::new(sum)),
        index: vec![0],
        composite: true,
        ..Default::default()
    };
    assert_eq!(sc.channel_name(1), "G");
    assert_eq!(sc.channel_title(1), "G");
    let req = sc.request().unwrap();
    assert_eq!(req.composite, Some((0, 3)));
    sc.planes = (0..3u32)
        .map(|k| {
            let plane = s.nd_plane(&[k], &CancelToken::new()).unwrap().unwrap();
            let limits = Limits::ZScale.compute(&plane.values, plane.width as usize);
            SciencePlane {
                plane,
                limits,
                lut: Vec::new(),
                mid: 0.5,
            }
        })
        .collect();
    sc.hidden[1] = true;
    sc.stretch = Stretch::HistEq;
    let look = sc.look().unwrap();
    // No tables yet: linear until they come.
    assert_eq!(look.stretch, Stretch::Linear);
    assert_eq!(look.channels.len(), 3);
    assert!(look.channels[0].on && !look.channels[1].on && look.channels[2].on);
    assert_eq!(look.channels[2].colour, [0.0, 0.0, 1.0]);
    assert!(look.channels.iter().all(|c| c.hi > c.lo));
}

#[test]
fn grid_lines_cross_the_image_with_labels() {
    let s = open(sky(200, 150, 1, ""));
    let sum = s.science().unwrap();
    let w = sum.wcs.as_ref().unwrap();
    let lines = grid(w, 200, 150, &[]);
    assert!(lines.len() >= 4, "{lines:?}");
    let labels: Vec<&str> = lines
        .iter()
        .map(|l| l.label.as_str())
        .filter(|l| !l.is_empty())
        .collect();
    // 0.1° × 0.075° field: RA in seconds of time, Dec in arcminutes.
    assert!(labels.iter().any(|l| l.starts_with("13:29:")), "{labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("+47:")), "{labels:?}");
    // Every line passes through the image and its points lie near it.
    for l in &lines {
        assert!(l.pts.len() >= 2);
        assert!(
            l.pts
                .iter()
                .any(|&(x, y)| (0.0..=200.0).contains(&x) && (0.0..=150.0).contains(&y))
        );
    }
    // Lines of constant RA are about vertical here (north up, east left).
    let ra = lines
        .iter()
        .find(|l| l.label.starts_with("13:29:"))
        .unwrap();
    let (a, b) = (ra.pts[0], ra.pts[ra.pts.len() - 1]);
    assert!((a.0 - b.0).abs() < 0.2 * (a.1 - b.1).abs(), "{a:?} {b:?}");
}

#[test]
fn grid_round_steps() {
    // A 0.1° span: five seconds of time (0.021°) in RA, two arcminutes in Dec.
    assert!((grid_step(0.1, true) - 5.0 * 15.0 / 3600.0).abs() < 1e-9);
    assert!((grid_step(0.1, false) - 120.0 / 3600.0).abs() < 1e-9);
    assert!((grid_step(40.0, false) - 10.0).abs() < 1e-9);
}

#[test]
fn diffraction_rings_and_scatter_readout() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-xtal/tests/data/pilatus.cbf");
    let s = Registry::default()
        .open(Arc::new(Source::from_vec(
            "pilatus.cbf",
            std::fs::read(path).unwrap(),
        )))
        .unwrap();
    let sum = s.science().unwrap();
    let g = sum.diffraction.as_ref().unwrap();
    let lines = ring_lines(g, sum.width, sum.height);
    assert!(!lines.is_empty());
    let labels: Vec<String> = lines
        .iter()
        .filter(|l| !l.label.is_empty())
        .map(|l| bare(&l.label))
        .collect();
    assert!(
        !labels.is_empty() && labels.iter().all(|l| l.ends_with(" Å")),
        "{labels:?}"
    );
    // Each labelled ring once, and every ring touches the image.
    let mut unique = labels.clone();
    unique.dedup();
    assert_eq!(unique, labels);
    let (w, h) = (sum.width as f32, sum.height as f32);
    assert!(lines.iter().all(|l| l.pts.len() >= 2));
    assert!(lines.iter().any(|l| {
        l.pts
            .iter()
            .any(|&(x, y)| (0.0..=w).contains(&x) && (0.0..=h).contains(&y))
    }));
    // The readout agrees with the geometry, and reads infinite d at the beam centre.
    let at = g.at(10.5, 20.5).unwrap();
    let line = bare(&scatter_line(g, 10.5, 20.5).unwrap());
    assert!(
        line.starts_with(&format!("q {} Å⁻¹", value_text(at.q as f32))),
        "{line}"
    );
    assert!(
        line.ends_with(&format!("2θ {}°", value_text(at.two_theta as f32))),
        "{line}"
    );
    let (bx, by) = g.beam.unwrap();
    assert!(bare(&scatter_line(g, bx, by).unwrap()).contains("d ∞ Å"));
}

#[test]
fn power_spectrum_readout_in_spatial_frequency() {
    assert_eq!(significant(0.00468751, 4), "0.004688");
    assert_eq!(significant(142.7125, 4), "142.7");
    assert_eq!(significant(-0.125, 4), "\u{2212}0.125");
    let mut s = ScienceSummary {
        width: 64,
        height: 32,
        pixel: Some(dsci::PixelCalibration {
            x: dsci::Linear {
                offset: 0.0,
                step: 2.0,
            },
            y: dsci::Linear {
                offset: 0.0,
                step: 2.0,
            },
            unit_x: "nm".into(),
            unit_y: "nm".into(),
        }),
        ..Default::default()
    };
    // Bin 8 right of the centre of 64 px of 2 nm: 8 / 128 nm = 0.0625 / nm, period 16 nm.
    let lines: Vec<String> = fft_lines(&s, 40.5, 16.5).iter().map(|l| bare(l)).collect();
    assert_eq!(lines[0], "Frequency u 0.0625  v 0 1/nm");
    assert_eq!(lines[1], "Period 16 nm");
    let lines: Vec<String> = fft_lines(&s, 33.0, 16.0).iter().map(|l| bare(l)).collect();
    assert_eq!(lines[1], "Period 128 nm");
    // Periods in micrometres give frequencies per micrometre.
    s.pixel.as_mut().unwrap().x.step = 2000.0;
    s.pixel.as_mut().unwrap().y.step = 2000.0;
    let lines: Vec<String> = fft_lines(&s, 33.0, 16.0).iter().map(|l| bare(l)).collect();
    assert!(lines[0].starts_with("Frequency u 0.00781"), "{}", lines[0]);
    assert!(lines[0].ends_with("v 0 1/\u{b5}m"), "{}", lines[0]);
    assert_eq!(lines[1], "Period 128 \u{b5}m");
    // Without calibration: per pixel; the centre has no period.
    s.pixel = None;
    let lines: Vec<String> = fft_lines(&s, 48.0, 16.0).iter().map(|l| bare(l)).collect();
    assert_eq!(lines[0], "Frequency u 0.25  v 0 1/px");
    assert_eq!(lines[1], "Period 4 px");
    assert_eq!(fft_lines(&s, 32.0, 16.0).len(), 1);
}

/// "Next limits" goes on from the limits shown: from the file's own display range to the
/// first of the cycle, not to the one after the stored setting the file range hid.
#[test]
fn next_limits_after_a_file_range_starts_the_cycle() {
    let mut s = Science {
        limits: Limits::Percentile(99.0),
        ..Science::default()
    };
    s.shown = Some(Request {
        index: vec![0],
        composite: None,
        limits: s.limits,
        histeq: false,
        level: false,
        cfa: None,
        auto: false,
        ranges: (None, vec![(10.0, 200.0)]),
        fft: false,
        ion: None,
        thermal: None,
    });
    assert_eq!(s.next_limits(), Limits::ALL[0]);
    s.limits = s.next_limits();
    s.limits_chosen = true;
    assert_eq!(s.next_limits(), Limits::ALL[1], "then on through the cycle");
    // No file range: the one after the stored setting.
    s.shown = None;
    s.limits_chosen = false;
    s.limits = Limits::MinMax;
    assert_eq!(s.next_limits(), Limits::Percentile(99.5));
}
