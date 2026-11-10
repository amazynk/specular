//! The validation suite: synthetic rasters with exact analytic answers.

use std::f64::consts::PI;

use specular_measure::calib::{CalibError, Calibration, Kind, Source, Unit};
use specular_measure::geom::{P, Shape};
use specular_measure::list::{List, Measurement};
use specular_measure::provenance::{self, Provenance};
use specular_measure::raster::{Samples, View};
use specular_measure::{cover, csv, geojson, imagej, profile, stats};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
}

macro_rules! assert_close {
    ($a:expr, $b:expr) => {
        assert_close!($a, $b, 1e-12)
    };
    ($a:expr, $b:expr, $tol:expr) => {{
        let (a, b): (f64, f64) = ($a, $b);
        assert!(
            close(a, b, $tol),
            "{} = {a} vs {} = {b}",
            stringify!($a),
            stringify!($b)
        );
    }};
}

/// A deterministic generator for the property checks.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    /// Sometimes a whole or half pixel, to hit the tie rules.
    fn coord(&mut self, lo: f64, hi: f64) -> f64 {
        let v = self.range(lo, hi);
        match (self.next() * 3.0) as u32 {
            0 => v.round(),
            1 => (v * 2.0).round() / 2.0,
            _ => v,
        }
    }
}

fn brute(shape: &Shape, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if shape.contains(P::new(f64::from(x) + 0.5, f64::from(y) + 0.5)) {
                v.push((x, y));
            }
        }
    }
    v
}

fn spanned(shape: &Shape, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    let mut last = (0u32, 0u32);
    cover::spans(shape, (0, 0, w, h), |y, a, b| {
        assert!(a < b && b <= w && y < h, "run {y}: {a}..{b}");
        assert!((y, a) >= last, "runs out of order");
        last = (y, b);
        v.extend((a..b).map(|x| (x, y)));
    });
    v
}

// --- Coverage ---

#[test]
fn rect_on_integer_bounds_covers_exactly_its_pixels() {
    let r = Shape::Rect {
        x0: 2.0,
        y0: 1.0,
        x1: 7.0,
        y1: 4.0,
    };
    assert_eq!(cover::count(&r, (0, 0, 20, 20)), 15);
    // Corners in any order.
    let r2 = Shape::Rect {
        x0: 7.0,
        y0: 4.0,
        x1: 2.0,
        y1: 1.0,
    };
    assert_eq!(spanned(&r, 20, 20), spanned(&r2, 20, 20));
}

#[test]
fn fractional_rect_counts_centres_half_open() {
    let r = |x0, x1| Shape::Rect {
        x0,
        y0: 0.0,
        x1,
        y1: 1.0,
    };
    // Centres 2.5, 3.5, 4.5, 5.5.
    assert_eq!(cover::count(&r(2.4, 5.6), (0, 0, 10, 1)), 4);
    assert_eq!(cover::count(&r(2.6, 5.4), (0, 0, 10, 1)), 2);
    // A centre on the left edge is in, on the right edge out.
    assert_eq!(cover::count(&r(2.5, 5.5), (0, 0, 10, 1)), 3);
}

#[test]
fn adjacent_rects_partition_the_image() {
    let a = Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 3.5,
        y1: 10.0,
    };
    let b = Shape::Rect {
        x0: 3.5,
        y0: 0.0,
        x1: 10.0,
        y1: 10.0,
    };
    let (pa, pb) = (spanned(&a, 10, 10), spanned(&b, 10, 10));
    assert_eq!(pa.len() + pb.len(), 100);
    assert!(pa.iter().all(|p| !pb.contains(p)));
}

#[test]
fn spans_match_the_point_test_for_every_shape() {
    let mut g = Lcg(7);
    let (w, h) = (48, 40);
    for _ in 0..400 {
        let shapes = [
            Shape::Ellipse {
                cx: g.coord(-5.0, 53.0),
                cy: g.coord(-5.0, 45.0),
                rx: g.coord(0.2, 30.0),
                ry: g.coord(0.2, 30.0),
            },
            Shape::Rect {
                x0: g.coord(-5.0, 53.0),
                y0: g.coord(-5.0, 45.0),
                x1: g.coord(-5.0, 53.0),
                y1: g.coord(-5.0, 45.0),
            },
            Shape::Polygon(
                (0..3 + (g.next() * 6.0) as usize)
                    .map(|_| P::new(g.coord(-5.0, 53.0), g.coord(-5.0, 45.0)))
                    .collect(),
            ),
            Shape::Freehand(
                (0..20)
                    .map(|_| P::new(g.coord(0.0, 48.0), g.coord(0.0, 40.0)))
                    .collect(),
            ),
            Shape::Point(P::new(g.coord(-1.0, 49.0), g.coord(-1.0, 41.0))),
        ];
        for s in &shapes {
            assert_eq!(spanned(s, w, h), brute(s, w, h), "{s:?}");
        }
    }
}

#[test]
fn clip_limits_runs() {
    let s = Shape::Ellipse {
        cx: 10.0,
        cy: 10.0,
        rx: 8.0,
        ry: 8.0,
    };
    let all = brute(&s, 30, 30);
    let mut clipped = Vec::new();
    cover::spans(&s, (5, 7, 12, 30), |y, a, b| {
        clipped.extend((a..b).map(|x| (x, y)))
    });
    let want: Vec<_> = all
        .into_iter()
        .filter(|&(x, y)| (5..12).contains(&x) && y >= 7)
        .collect();
    assert_eq!(clipped, want);
}

#[test]
fn axis_aligned_polygon_covers_like_the_rect() {
    let poly = Shape::Polygon(vec![
        P::new(2.5, 1.25),
        P::new(9.0, 1.25),
        P::new(9.0, 6.75),
        P::new(2.5, 6.75),
    ]);
    let rect = Shape::Rect {
        x0: 2.5,
        y0: 1.25,
        x1: 9.0,
        y1: 6.75,
    };
    assert_eq!(spanned(&poly, 12, 10), spanned(&rect, 12, 10));
}

#[test]
fn right_triangle_counts_and_area() {
    let t = Shape::Polygon(vec![P::new(0.0, 0.0), P::new(10.0, 0.0), P::new(0.0, 10.0)]);
    // Centre (i + ½, j + ½) is in when i + j + 1 < 10 (the hypotenuse is a right edge).
    assert_eq!(cover::count(&t, (0, 0, 20, 20)), 45);
    assert_eq!(t.area_px(), Some(50.0));
}

#[test]
fn self_intersecting_outline_alternates() {
    // Two overlapping squares drawn as one outline: the even-odd rule leaves the overlap out.
    let fig = Shape::Freehand(vec![
        P::new(0.0, 0.0),
        P::new(6.0, 0.0),
        P::new(6.0, 6.0),
        P::new(2.0, 6.0),
        P::new(2.0, 2.0),
        P::new(8.0, 2.0),
        P::new(8.0, 8.0),
        P::new(0.0, 8.0),
    ]);
    let px = brute(&fig, 10, 10);
    assert!(!px.contains(&(3, 3)), "the overlap");
    assert!(px.contains(&(0, 0)) && px.contains(&(7, 7)));
}

#[test]
fn circle_count_is_the_lattice_count() {
    // Centres within radius r of a pixel corner: Gauss's circle problem for N(r) with the
    // shifted lattice; compare with a direct count.
    let (cx, cy, r) = (50.0, 50.0, 20.0);
    let s = Shape::Ellipse {
        cx,
        cy,
        rx: r,
        ry: r,
    };
    let mut n = 0;
    for j in 0..100 {
        for i in 0..100 {
            let (dx, dy) = (f64::from(i) + 0.5 - cx, f64::from(j) + 0.5 - cy);
            if dx * dx + dy * dy <= r * r {
                n += 1;
            }
        }
    }
    assert_eq!(cover::count(&s, (0, 0, 100, 100)), n);
    assert!((n as f64 - PI * r * r).abs() < 2.0 * PI * r);
}

// --- Statistics ---

fn ramp_u16(w: u32, h: u32) -> Vec<u16> {
    (0..h).flat_map(|_| 0..w as u16).collect()
}

#[test]
fn constant_region_statistics() {
    let data = vec![7u8; 10 * 8];
    let v = View::new(10, 8, 1, Samples::U8(&data)).unwrap();
    let r = Shape::Rect {
        x0: 2.0,
        y0: 1.0,
        x1: 7.0,
        y1: 4.0,
    };
    let s = &stats::roi(&r, &v, 256).unwrap()[0];
    assert_eq!(s.count, 15);
    assert_eq!(
        (s.mean, s.sd, s.sd_sample, s.min, s.max, s.median, s.sum),
        (7.0, 0.0, 0.0, 7.0, 7.0, 7.0, 105.0)
    );
    assert_eq!(s.histogram.bins, vec![15]);
    assert_eq!(s.histogram.lo, 7.0);
}

#[test]
fn ramp_statistics_are_exact() {
    let data = ramp_u16(100, 6);
    let v = View::new(100, 6, 1, Samples::U16(&data)).unwrap();
    let r = Shape::Rect {
        x0: 10.0,
        y0: 0.0,
        x1: 20.0,
        y1: 5.0,
    };
    let s = &stats::roi(&r, &v, 256).unwrap()[0];
    // Values 10..=19, five of each.
    assert_eq!(s.count, 50);
    assert_eq!(s.sum, 725.0);
    assert_eq!(s.mean, 14.5);
    assert_eq!((s.min, s.max, s.median), (10.0, 19.0, 14.5));
    let pop = (99.0f64 / 12.0).sqrt(); // a uniform run of 10 integers
    assert_close!(s.sd, pop);
    assert_close!(s.sd_sample, (pop * pop * 50.0 / 49.0).sqrt());
    assert_eq!(s.histogram.bins, vec![5; 10]);
    assert_eq!((s.histogram.lo, s.histogram.width), (10.0, 1.0));

    // Wider than the bins: whole-number bins of width 2.
    let s = &stats::roi(&r, &v, 5).unwrap()[0];
    assert_eq!(s.histogram.bins, vec![10; 5]);
    assert_eq!(s.histogram.width, 2.0);
}

#[test]
fn float_data_matches_integer_data_and_skips_nan() {
    let ints = ramp_u16(100, 6);
    let mut floats: Vec<f32> = ints.iter().map(|&v| f32::from(v)).collect();
    let vi = View::new(100, 6, 1, Samples::U16(&ints)).unwrap();
    let e = Shape::Ellipse {
        cx: 40.0,
        cy: 3.0,
        rx: 25.5,
        ry: 2.7,
    };
    let a = stats::roi(&e, &vi, 256).unwrap().remove(0);
    let vf = View::new(100, 6, 1, Samples::F32(&floats)).unwrap();
    let b = stats::roi(&e, &vf, 256).unwrap().remove(0);
    assert_eq!(a.count, b.count);
    assert_close!(a.mean, b.mean);
    assert_close!(a.sd, b.sd, 1e-10);
    assert_eq!(
        (a.min, a.max, a.median, a.sum),
        (b.min, b.max, b.median, b.sum)
    );

    floats[3 * 100 + 40] = f32::NAN;
    let vf = View::new(100, 6, 1, Samples::F32(&floats)).unwrap();
    let c = stats::roi(&e, &vf, 256).unwrap().remove(0);
    assert_eq!((c.count, c.nan), (a.count - 1, 1));
    assert_close!(c.sum, a.sum - 40.0);
}

#[test]
fn interleaved_channels_and_strides() {
    // RGB with R = x, G = y, B = 200, rows padded to 40 samples.
    let (w, h) = (12u32, 9u32);
    let mut data = vec![0u8; 40 * h as usize];
    for y in 0..h {
        for x in 0..w {
            let i = y as usize * 40 + x as usize * 3;
            data[i..i + 3].copy_from_slice(&[x as u8, y as u8, 200]);
        }
    }
    let v = View::with_stride(w, h, 3, 40, Samples::U8(&data)).unwrap();
    let r = Shape::Rect {
        x0: 2.0,
        y0: 3.0,
        x1: 6.0,
        y1: 8.0,
    };
    let s = stats::roi(&r, &v, 256).unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].mean, 3.5);
    assert_eq!(s[1].mean, 5.0);
    assert_eq!((s[2].mean, s[2].sd), (200.0, 0.0));
}

#[test]
fn a_view_of_the_bounding_box_gives_the_same_statistics() {
    let (w, h) = (64u32, 48u32);
    let data: Vec<f64> = (0..w * h).map(|i| f64::from((i * 7919) % 1013)).collect();
    let full = View::new(w, h, 1, Samples::F64(&data)).unwrap();
    let shape = Shape::Polygon(vec![
        P::new(10.3, 5.0),
        P::new(40.0, 9.5),
        P::new(30.2, 33.7),
        P::new(12.0, 28.0),
    ]);
    let a = stats::roi(&shape, &full, 64).unwrap().remove(0);
    // Copy rows 4..35, columns 9..41 into a block placed at (9, 4).
    let (x0, y0, bw, bh) = (9u32, 4u32, 32u32, 31u32);
    let block: Vec<f64> = (y0..y0 + bh)
        .flat_map(|y| (x0..x0 + bw).map(move |x| (y, x)))
        .map(|(y, x)| data[(y * w + x) as usize])
        .collect();
    let part = View::new(bw, bh, 1, Samples::F64(&block))
        .unwrap()
        .at((x0, y0));
    let b = stats::roi(&shape, &part, 64).unwrap().remove(0);
    assert_eq!(a, b);
}

#[test]
fn callback_statistics_and_empty_regions() {
    let r = Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 4.0,
        y1: 1.0,
    };
    // Modality values with one pixel missing.
    let s = stats::roi_with(&r, 10, 10, 16, |x, _| {
        (x != 2).then_some(f64::from(x) * 10.0 - 1000.0)
    })
    .unwrap();
    assert_eq!(s.count, 3);
    assert_close!(s.mean, (-1000.0 - 990.0 - 970.0) / 3.0);
    assert_eq!(s.median, -990.0);
    // Outside the image, or a line: no statistics.
    let data = [1u8; 4];
    let v = View::new(2, 2, 1, Samples::U8(&data)).unwrap();
    let off = Shape::Rect {
        x0: 5.0,
        y0: 5.0,
        x1: 9.0,
        y1: 9.0,
    };
    assert!(stats::roi(&off, &v, 8).is_none());
    assert!(stats::roi(&Shape::Line(P::new(0.0, 0.0), P::new(2.0, 2.0)), &v, 8).is_none());
    assert!(View::new(3, 2, 1, Samples::U8(&data)).is_err());
}

// --- Calibration and geometry ---

#[test]
fn dicom_spacing_is_rows_then_columns() {
    let c = Calibration::dicom(0.5, 0.25, Source::DicomPixelSpacing).unwrap();
    let h = Shape::Line(P::new(0.0, 0.0), P::new(4.0, 0.0))
        .length(&c)
        .unwrap();
    let v = Shape::Line(P::new(0.0, 0.0), P::new(0.0, 4.0))
        .length(&c)
        .unwrap();
    assert_eq!((h.value, v.value), (1.0, 2.0));
    assert_eq!(
        (h.unit, h.source, h.kind),
        (Unit::Mm, Source::DicomPixelSpacing, Kind::Length)
    );
    let area = Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 10.0,
        y1: 10.0,
    }
    .area(&c)
    .unwrap();
    assert_eq!(area.value, 12.5);
    assert_eq!(area.to_string(), "12.50 mm²");
    let m = Calibration::dicom_magnified(0.2, 0.2, 1.25).unwrap();
    assert_close!(m.axis_spacing().0, 0.16);
    assert_eq!(m.source, Source::DicomMagnification);
}

#[test]
fn three_four_five_and_physical_angles() {
    let mm = Calibration::spacing(1.0, 1.0, Unit::Mm, Source::DicomPixelSpacing).unwrap();
    assert_eq!(
        Shape::Line(P::new(1.0, 1.0), P::new(4.0, 5.0))
            .length(&mm)
            .unwrap()
            .value,
        5.0
    );
    let poly = Shape::Polyline(vec![P::new(0.0, 0.0), P::new(3.0, 4.0), P::new(3.0, 10.0)]);
    assert_eq!(poly.length(&mm).unwrap().value, 11.0);

    let right = Shape::Angle {
        a: P::new(10.0, 0.0),
        vertex: P::new(0.0, 0.0),
        b: P::new(0.0, 7.0),
    };
    let aniso = Calibration::spacing(1.0, 2.0, Unit::Mm, Source::DicomPixelSpacing).unwrap();
    assert_close!(right.angle(&aniso).unwrap().value, 90.0);
    let diag = Shape::Angle {
        a: P::new(10.0, 0.0),
        vertex: P::new(0.0, 0.0),
        b: P::new(10.0, 10.0),
    };
    assert_close!(
        diag.angle(&Calibration::uncalibrated()).unwrap().value,
        45.0
    );
    assert_close!(diag.angle(&aniso).unwrap().value, 2f64.atan().to_degrees());
    let straight = Shape::Angle {
        a: P::new(-3.0, 0.0),
        vertex: P::new(0.0, 0.0),
        b: P::new(5.0, 0.0),
    };
    assert_eq!(straight.angle(&mm).unwrap().value, 180.0);
}

#[test]
fn uncalibrated_values_are_pixels() {
    let c = Calibration::uncalibrated();
    let q = Shape::Line(P::new(0.0, 0.0), P::new(6.0, 8.0))
        .length(&c)
        .unwrap();
    assert_eq!(
        (q.value, q.unit, q.source),
        (10.0, Unit::Px, Source::Uncalibrated)
    );
    assert_eq!(q.to_string(), "10.00 px");
    let e = Shape::Ellipse {
        cx: 0.0,
        cy: 0.0,
        rx: 3.0,
        ry: 2.0,
    };
    assert_close!(e.area(&c).unwrap().value, 6.0 * PI);
}

#[test]
fn resolution_tags_and_readable_units() {
    let dpi = Calibration::resolution(300.0, 300.0, Unit::Inch, Source::TiffResolution).unwrap();
    assert_eq!(dpi.unit, Unit::Mm);
    assert_close!(
        Shape::Line(P::new(0.0, 0.0), P::new(300.0, 0.0))
            .length(&dpi)
            .unwrap()
            .value,
        25.4
    );
    // 0.325 µm pixels given per centimetre.
    let micro = Calibration::resolution(
        1.0 / 0.325e-4,
        1.0 / 0.325e-4,
        Unit::Cm,
        Source::TiffResolution,
    )
    .unwrap();
    assert_eq!(micro.unit, Unit::Um);
    assert_close!(micro.axis_spacing().0, 0.325);
    let ome = Calibration::ome(0.5, 0.5, Unit::Um).unwrap();
    assert_eq!(ome.source, Source::OmePhysicalSize);
    assert_close!(ome.in_unit(Unit::Nm).unwrap().axis_spacing().1, 500.0);
}

#[test]
fn fits_wcs_with_rotation() {
    let c = Calibration::fits_cdelt(-1.0 / 3600.0, 1.0 / 3600.0, 30.0).unwrap();
    assert_eq!((c.unit, c.source), (Unit::Arcsec, Source::FitsWcs));
    let l = Shape::Line(P::new(0.0, 0.0), P::new(60.0, 80.0))
        .length(&c)
        .unwrap();
    assert_close!(l.value, 100.0);
    let a = Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 100.0,
        y1: 100.0,
    }
    .area(&c)
    .unwrap();
    assert_close!(a.value, 1e4);
    let sheared =
        Calibration::fits_cd([[1.0 / 3600.0, 0.5 / 3600.0], [0.0, 1.0 / 3600.0]]).unwrap();
    assert_close!(sheared.pixel_area(), 1.0);
}

#[test]
fn user_calibration_from_a_known_length() {
    let c = Calibration::from_known_length(P::new(0.0, 0.0), P::new(30.0, 40.0), 10.0, Unit::Mm)
        .unwrap();
    assert_eq!(c.source, Source::User);
    assert_close!(
        Shape::Line(P::new(0.0, 0.0), P::new(100.0, 0.0))
            .length(&c)
            .unwrap()
            .value,
        20.0
    );
    // Keeping known non-square pixels: rows twice as far apart.
    let base = Calibration::spacing(1.0, 2.0, Unit::Mm, Source::DicomImagerPixelSpacing).unwrap();
    let k = base
        .rescaled(P::new(0.0, 0.0), P::new(0.0, 10.0), 10.0, Unit::Mm)
        .unwrap();
    assert_close!(k.axis_spacing().0, 0.5);
    assert_close!(k.axis_spacing().1, 1.0);
    assert_eq!(
        Calibration::from_known_length(P::new(1.0, 1.0), P::new(1.0, 1.0), 5.0, Unit::Mm),
        Err(CalibError::ZeroLength)
    );
    assert_eq!(
        Calibration::from_known_length(P::new(0.0, 0.0), P::new(1.0, 1.0), -5.0, Unit::Mm),
        Err(CalibError::NotPositive)
    );
    assert_eq!(
        Calibration::from_known_length(P::new(0.0, 0.0), P::new(1.0, 1.0), 5.0, Unit::Px),
        Err(CalibError::PixelUnit)
    );
}

#[test]
fn every_value_carries_its_source() {
    for source in Source::ALL {
        let unit = if source == Source::Uncalibrated {
            Unit::Px
        } else {
            Unit::Mm
        };
        let c = Calibration::spacing(0.3, 0.3, unit, source).unwrap();
        let shapes = [
            Shape::Line(P::new(0.0, 0.0), P::new(1.0, 2.0)),
            Shape::Angle {
                a: P::new(1.0, 0.0),
                vertex: P::new(0.0, 0.0),
                b: P::new(0.0, 1.0),
            },
            Shape::Ellipse {
                cx: 2.0,
                cy: 2.0,
                rx: 1.0,
                ry: 1.0,
            },
        ];
        for s in &shapes {
            for q in [s.length(&c), s.angle(&c), s.area(&c), s.perimeter(&c)]
                .into_iter()
                .flatten()
            {
                assert_eq!(q.source, source);
            }
        }
        assert_eq!(Source::from_key(source.key()), Some(source));
    }
}

#[test]
fn perimeters() {
    let c = Calibration::spacing(0.5, 0.5, Unit::Mm, Source::User).unwrap();
    let circle = Shape::Ellipse {
        cx: 0.0,
        cy: 0.0,
        rx: 10.0,
        ry: 10.0,
    };
    assert_close!(circle.perimeter(&c).unwrap().value, 2.0 * PI * 5.0);
    // A 10 × 5 ellipse against a fine numeric integration.
    let e = Shape::Ellipse {
        cx: 0.0,
        cy: 0.0,
        rx: 10.0,
        ry: 5.0,
    };
    let n = 200_000;
    let numeric: f64 = (0..n)
        .map(|i| {
            let t = 2.0 * PI * (f64::from(i) + 0.5) / f64::from(n);
            (100.0 * t.sin().powi(2) + 25.0 * t.cos().powi(2)).sqrt() * 2.0 * PI / f64::from(n)
        })
        .sum();
    assert_close!(
        e.perimeter(&Calibration::uncalibrated()).unwrap().value,
        numeric,
        1e-9
    );
    // Non-square pixels: the 10 × 5 ellipse with rows twice as far is a 10 × 10 circle.
    let aniso = Calibration::spacing(1.0, 2.0, Unit::Mm, Source::User).unwrap();
    assert_close!(e.perimeter(&aniso).unwrap().value, 20.0 * PI);
    let r = Shape::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 4.0,
        y1: 3.0,
    };
    assert_close!(r.perimeter(&aniso).unwrap().value, 2.0 * (4.0 + 6.0));
}

// --- Profiles ---

#[test]
fn profile_along_a_ramp_is_linear_and_ends_on_the_end() {
    let (w, h) = (20u32, 10u32);
    // v = 2x + 3y at pixel centres (x, y the centre coordinates minus ½).
    let data: Vec<f64> = (0..h)
        .flat_map(|y| (0..w).map(move |x| 2.0 * f64::from(x) + 3.0 * f64::from(y)))
        .collect();
    let v = View::new(w, h, 1, Samples::F64(&data)).unwrap();
    let c = Calibration::spacing(0.5, 0.5, Unit::Mm, Source::User).unwrap();
    let path = [P::new(1.5, 2.5), P::new(11.8, 2.5)];
    let p = profile::along(&path, &v, &c, 1.0, 1);
    assert_eq!(p.len(), 12); // 0, 1, …, 10 and the end at 10.3
    assert_close!(*p.distance_px.last().unwrap(), 10.3);
    assert_close!(*p.distance.last().unwrap(), 5.15);
    for (i, pt) in p.points.iter().enumerate() {
        assert_close!(
            p.values[0][i],
            2.0 * (pt.x - 0.5) + 3.0 * (pt.y - 0.5),
            1e-12
        );
    }
    // A bent path, three wide: on a plane the perpendicular mean is the centre value.
    let bent = [P::new(2.5, 2.5), P::new(12.5, 7.5), P::new(4.5, 8.0)];
    let q = profile::along(&bent, &v, &c, 0.75, 3);
    for (i, pt) in q.points.iter().enumerate() {
        assert_close!(
            q.values[0][i],
            2.0 * (pt.x - 0.5) + 3.0 * (pt.y - 0.5),
            1e-9
        );
    }
    let total = (125.0f64).sqrt() + (64.25f64).sqrt();
    assert_close!(*q.distance_px.last().unwrap(), total);
    assert_close!(*q.distance.last().unwrap(), total * 0.5);
    // Outside the image: NaN.
    let out = profile::along(&[P::new(-5.0, 1.0), P::new(1.0, 1.0)], &v, &c, 1.0, 1);
    assert!(out.values[0][0].is_nan() && !out.values[0][6].is_nan());
}

#[test]
fn profile_through_frames() {
    let stack: Vec<Vec<u8>> = (0..5u8).map(|z| vec![z * 10; 16]).collect();
    let t = profile::through(0..5, |z| {
        (z != 3).then(|| {
            let v = View::new(4, 4, 1, Samples::U8(&stack[z as usize])).unwrap();
            profile::at(&v, P::new(2.0, 2.0)).unwrap()
        })
    });
    assert_eq!(t.frames, vec![0, 1, 2, 3, 4]);
    assert_eq!(t.values[0][..3], [0.0, 10.0, 20.0]);
    assert!(t.values[0][3].is_nan());
    let m = profile::through(0..5, |z| {
        let v = View::new(4, 4, 1, Samples::U8(&stack[z as usize])).unwrap();
        profile::roi_mean(&v, &Shape::rect(P::new(0.0, 0.0), P::new(2.0, 2.0)))
    });
    assert_eq!(m.values[0], vec![0.0, 10.0, 20.0, 30.0, 40.0]);
}

// --- The list ---

#[test]
fn list_undo() {
    let mut l = List::new();
    let c = Calibration::uncalibrated();
    let a = l.push(Measurement::new(Shape::Point(P::new(1.0, 1.0)), c));
    let b = l.push(Measurement::new(Shape::Point(P::new(2.0, 2.0)), c));
    assert_eq!((a, b), (1, 2));
    assert!(l.remove(a).is_some());
    assert_eq!(l.items()[0].id, b);
    l.clear();
    assert!(l.is_empty());
    assert!(l.undo()); // clear
    assert_eq!(l.len(), 1);
    assert!(l.undo()); // remove, back in place
    assert_eq!(
        l.items().iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let mut m = l.get(b).unwrap().clone();
    m.label = "renamed".into();
    assert!(l.replace(m));
    assert!(l.undo());
    assert_eq!(l.get(b).unwrap().label, "");
    let ids = l.extend([
        Measurement::new(Shape::Point(P::new(3.0, 3.0)), c),
        Measurement::new(Shape::Point(P::new(4.0, 4.0)), c),
    ]);
    assert_eq!(ids, vec![3, 4]);
    assert!(l.undo());
    assert_eq!(l.len(), 2);
    assert!(l.undo() && l.undo());
    assert!(l.is_empty() && !l.undo());
}

// --- Exchange ---

fn sample_list() -> Vec<Measurement> {
    let cal = Calibration::dicom(0.5, 0.25, Source::DicomPixelSpacing).unwrap();
    let shapes = [
        Shape::Point(P::new(3.25, 4.5)),
        Shape::Line(P::new(1.0, 2.0), P::new(30.0, 40.0)),
        Shape::Polyline(vec![
            P::new(0.5, 0.5),
            P::new(10.0, 3.0),
            P::new(12.0, 20.25),
        ]),
        Shape::Angle {
            a: P::new(10.0, 0.0),
            vertex: P::new(0.0, 0.0),
            b: P::new(5.0, 5.0),
        },
        Shape::Rect {
            x0: 2.0,
            y0: 3.0,
            x1: 12.0,
            y1: 9.0,
        },
        Shape::Rect {
            x0: 2.5,
            y0: 3.25,
            x1: 12.75,
            y1: 9.0,
        },
        Shape::Ellipse {
            cx: 20.0,
            cy: 20.0,
            rx: 5.0,
            ry: 3.0,
        },
        Shape::Ellipse {
            cx: 20.25,
            cy: 20.5,
            rx: 5.5,
            ry: 3.0,
        },
        Shape::Polygon(vec![P::new(1.0, 1.0), P::new(9.0, 2.0), P::new(5.0, 8.0)]),
        Shape::Freehand(vec![
            P::new(1.5, 1.0),
            P::new(9.0, 2.5),
            P::new(5.0, 8.0),
            P::new(2.0, 6.0),
        ]),
    ];
    let data: Vec<u8> = (0..64 * 64).map(|i| (i % 251) as u8).collect();
    let v = View::new(64, 64, 1, Samples::U8(&data)).unwrap();
    shapes
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let mut m = Measurement::new(s, cal);
            m.id = i as u32 + 1;
            m.label = match i {
                1 => "aorta, \"root\"".to_owned(),
                2 => "=SUM(A1)".to_owned(),
                3 => "זווית".to_owned(),
                _ => format!("m{i}"),
            };
            m.frame = (i % 3 == 0).then_some(i as u32);
            m.stats = stats::roi(&m.shape, &v, 256);
            m.channels = vec!["Gray".into()];
            m
        })
        .collect()
}

fn prov() -> Provenance {
    Provenance {
        software: "Specular 0.1.0".into(),
        file: "scan, 1.dcm".into(),
        decoder: Some("specular-dicom 0.1.0".into()),
        blake3: Some(provenance::hash_bytes(b"pixels")),
        size: Some((64, 64)),
        created: None,
    }
}

#[test]
fn csv_rows_quoting_and_provenance() {
    let items = sample_list();
    let text = csv::write(&items, &prov());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "# software: Specular 0.1.0");
    assert_eq!(lines[1], "# file: scan, 1.dcm");
    assert!(lines[3].starts_with("# blake3: ") && lines[3].len() == 10 + 64);
    assert_eq!(lines[5], csv::COLUMNS.join(","));
    assert_eq!(lines.len(), 6 + items.len());
    assert!(text.contains(",\"aorta, \"\"root\"\"\",line,"));
    assert!(text.contains(",'=SUM(A1),polyline,"));
    // Rectangle 2..12 × 3..9: 60 pixels, area 60 × 0.125 mm².
    let rect = lines.iter().find(|l| l.starts_with("5,")).unwrap();
    let cols: Vec<&str> = rect.split(',').collect();
    assert_eq!(cols[5], "60");
    assert_eq!(cols[15], "7.5");
    assert_eq!(cols[17], "mm");
    assert_eq!(cols[18], "dicom-pixel-spacing");
    assert_eq!(cols[19], "7.5");
    assert_eq!(cols[21], "2 3;12 9");
}

#[test]
fn geojson_round_trip() {
    let items = sample_list();
    let text = geojson::write(&items, &prov());
    // Valid JSON, QuPath's shape.
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], "FeatureCollection");
    let f = &v["features"][4];
    assert_eq!(f["properties"]["objectType"], "annotation");
    assert_eq!(f["geometry"]["type"], "Polygon");
    assert_eq!(f["geometry"]["coordinates"][0].as_array().unwrap().len(), 5);
    assert_eq!(f["properties"]["measurements"]["Area mm^2"], 7.5);
    let back = geojson::read(&text).unwrap();
    assert_eq!(back.skipped, 0);
    assert_eq!(back.provenance.unwrap(), prov());
    assert_eq!(back.items.len(), items.len());
    for (a, b) in items.iter().zip(&back.items) {
        assert_eq!(a.shape, b.shape);
        assert_eq!(a.label, b.label);
        assert_eq!(a.frame, b.frame);
        assert_eq!(a.cal, b.cal);
    }
}

#[test]
fn geojson_from_other_tools() {
    // QuPath's older array export, a closed ring with a hole, a line and a multipoint.
    let text = r#"[
      {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]],[[2,2],[3,2],[3,3],[2,2]]]},
       "properties":{"objectType":"annotation","name":"tumour"}},
      {"type":"Feature","geometry":{"type":"LineString","coordinates":[[1,1],[4,5]]},"properties":{}},
      {"type":"Feature","geometry":{"type":"MultiPoint","coordinates":[[1,2],[3,4]]}},
      {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[1,1]]]}},
      {"type":"Feature","geometry":{"type":"Circle"}}
    ]"#;
    let r = geojson::read(text).unwrap();
    assert_eq!(r.items.len(), 4);
    assert_eq!(r.items[0].label, "tumour");
    assert_eq!(r.items[0].shape.area_px(), Some(100.0));
    assert_eq!(
        r.items[1].shape,
        Shape::Line(P::new(1.0, 1.0), P::new(4.0, 5.0))
    );
    assert_eq!(r.items[3].shape, Shape::Point(P::new(3.0, 4.0)));
    assert_eq!((r.skipped, r.holes), (2, 1));
    let bare = geojson::read(r#"{"type":"Point","coordinates":[5.5,6.5]}"#).unwrap();
    assert_eq!(bare.items[0].shape, Shape::Point(P::new(5.5, 6.5)));
    assert!(geojson::read("[1,2").is_err());
    assert!(geojson::read("{\"a\":1}").is_err());
}

#[test]
fn imagej_round_trip_and_layout() {
    for m in sample_list() {
        let b = imagej::encode(&m).unwrap();
        assert_eq!(&b[0..4], b"Iout");
        assert_eq!(u16::from_be_bytes([b[4], b[5]]), 228);
        let back = imagej::decode(&b).unwrap();
        assert_eq!(back.shape, m.shape, "{}", m.label);
        assert_eq!(back.label, m.label);
        assert_eq!(back.frame, m.frame);
    }
    // An integer rectangle: type 1, bounds top, left, bottom, right, no sub-pixel option.
    let mut m = Measurement::new(
        Shape::Rect {
            x0: 2.0,
            y0: 3.0,
            x1: 12.0,
            y1: 9.0,
        },
        Calibration::uncalibrated(),
    );
    m.frame = Some(4);
    let b = imagej::encode(&m).unwrap();
    assert_eq!(b[6], 1);
    assert_eq!(&b[8..16], &[0, 3, 0, 2, 0, 9, 0, 12]);
    assert_eq!(&b[50..52], &[0, 0]);
    assert_eq!(i32::from_be_bytes(b[56..60].try_into().unwrap()), 5);
}

#[test]
fn imagej_reads_a_hand_made_integer_polygon() {
    // A traced polygon (type 8) at left 10, top 20 with three integer vertices, version 227,
    // no second header.
    let mut b = vec![0u8; 64 + 12];
    b[0..4].copy_from_slice(b"Iout");
    b[4..6].copy_from_slice(&227u16.to_be_bytes());
    b[6] = 8;
    b[8..16].copy_from_slice(&[0, 20, 0, 10, 0, 30, 0, 15]);
    b[16..18].copy_from_slice(&3u16.to_be_bytes());
    for (i, v) in [0i16, 5, 0, 0, 0, 10].into_iter().enumerate() {
        b[64 + 2 * i..66 + 2 * i].copy_from_slice(&v.to_be_bytes());
    }
    let m = imagej::decode(&b).unwrap();
    assert_eq!(
        m.shape,
        Shape::Polygon(vec![
            P::new(10.0, 20.0),
            P::new(15.0, 20.0),
            P::new(10.0, 30.0)
        ])
    );
    assert_eq!((m.label.as_str(), m.frame), ("", None));
    assert_eq!(imagej::decode(&b[..40]), Err(imagej::Error::NotRoi));
    b[6] = 6;
    assert_eq!(imagej::decode(&b), Err(imagej::Error::Unsupported(6)));
    let too_far = Measurement::new(
        Shape::Point(P::new(40000.0, 1.0)),
        Calibration::uncalibrated(),
    );
    assert_eq!(imagej::encode(&too_far), Err(imagej::Error::TooLarge));
}

#[test]
fn imagej_roi_sets() {
    let items = sample_list();
    let zip = imagej::write_zip(&items).unwrap();
    let (back, bad) = imagej::read_zip(&zip).unwrap();
    assert_eq!(bad, 0);
    assert_eq!(back.len(), items.len());
    for (a, b) in items.iter().zip(&back) {
        assert_eq!(a.shape, b.shape);
    }
    assert!(imagej::read_zip(b"not a zip").is_err());
}

#[test]
fn provenance_hashes() {
    assert_eq!(
        provenance::hash_bytes(b""),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("specular-measure");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hash.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 256) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    assert_eq!(
        provenance::hash_file(&path).unwrap(),
        provenance::hash_bytes(&data)
    );
}

/// The DICOM viewer's ROI statistics before they moved onto this crate (specular-dicom's
/// `calib::roi_stats` and `Roi`), kept as the oracle: every pixel of the bounding box whose
/// centre the shape contains, in one plain pass. (count, mean, population SD, min, max) and
/// the shape's area in pixels².
type Old = (u64, f64, f64, f64, f64);

fn dicom_viewer_stats(
    shape: &Shape,
    width: u32,
    height: u32,
    value: impl Fn(u32, u32) -> Option<f64>,
) -> (Option<Old>, f64) {
    let (x0, y0, x1, y1) = match *shape {
        Shape::Rect { x0, y0, x1, y1 } => (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)),
        Shape::Ellipse { cx, cy, rx, ry } => {
            (cx - rx.abs(), cy - ry.abs(), cx + rx.abs(), cy + ry.abs())
        }
        _ => unreachable!(),
    };
    let (contains, area): (Box<dyn Fn(f64, f64) -> bool>, f64) = match *shape {
        Shape::Ellipse { cx, cy, rx, ry } => (
            Box::new(move |x, y| {
                rx != 0.0 && ry != 0.0 && ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
            }),
            PI * rx.abs() * ry.abs(),
        ),
        _ => (
            Box::new(move |x, y| x >= x0 && x < x1 && y >= y0 && y < y1),
            (x1 - x0) * (y1 - y0),
        ),
    };
    let cx0 = (x0 - 0.5).ceil().max(0.0) as u32;
    let cy0 = (y0 - 0.5).ceil().max(0.0) as u32;
    let cx1 = ((x1 - 0.5).floor().min(f64::from(width) - 1.0)).max(-1.0);
    let cy1 = ((y1 - 0.5).floor().min(f64::from(height) - 1.0)).max(-1.0);
    if cx1 < 0.0 || cy1 < 0.0 {
        return (None, area);
    }
    let (mut n, mut sum, mut sum2) = (0u64, 0.0f64, 0.0f64);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for y in cy0..=cy1 as u32 {
        for x in cx0..=cx1 as u32 {
            if !contains(f64::from(x) + 0.5, f64::from(y) + 0.5) {
                continue;
            }
            let Some(v) = value(x, y) else { continue };
            n += 1;
            sum += v;
            sum2 += v * v;
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if n == 0 {
        return (None, area);
    }
    let mean = sum / n as f64;
    let var = (sum2 / n as f64 - mean * mean).max(0.0);
    (Some((n, mean, var.sqrt(), lo, hi)), area)
}

#[test]
fn agrees_with_the_dicom_viewer() {
    let (w, h) = (64u32, 48u32);
    let hu = |x: u32, y: u32| Some(f64::from((x * 37 + y * 101) % 400) - 1000.0);
    let mut g = Lcg(11);
    for _ in 0..200 {
        let (cx, cy) = (g.coord(-4.0, 68.0), g.coord(-4.0, 52.0));
        let (rx, ry) = (g.coord(0.3, 25.0), g.coord(0.3, 25.0));
        let shapes = [
            Shape::Ellipse { cx, cy, rx, ry },
            Shape::Rect {
                x0: cx,
                y0: cy,
                x1: cx + rx,
                y1: cy - ry,
            },
        ];
        for new in shapes {
            let (a, area) = dicom_viewer_stats(&new, w, h, hu);
            let b = stats::roi_with(&new, w, h, 64, hu);
            assert_close!(area, new.area_px().unwrap());
            match (a, b) {
                (None, None) => {}
                (Some((count, mean, sd, min, max)), Some(b)) => {
                    assert_eq!((count, min, max), (b.count, b.min, b.max), "{new:?}");
                    assert_close!(mean, b.mean, 1e-9);
                    assert_close!(sd, b.sd, 1e-6);
                }
                (a, b) => panic!("{new:?}: {a:?} vs {b:?}"),
            }
        }
    }
}
