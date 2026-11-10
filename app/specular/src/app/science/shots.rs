//! Screenshot tests of science mode: a synthetic FITS sky (generated here) read as planes
//! and drawn by the image renderer through a stretch — a colour composite, a cube's plane in
//! asinh, log, square root and histogram equalisation — with the readout, the plane
//! sliders, the scale bar and the coordinate grid, dark and light at 1× and 1.5×, and right
//! to left in Hebrew. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular science::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::Orientation;
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};

use super::tests::{fits, open, sky};
use super::*;
use crate::app::medical::draw_readout;

const SIZE: (f32, f32) = (480.0, 300.0);
const W: u32 = 160;
const H: u32 = 120;

/// The science panels as a shot drew them: the readout, the sliders (panel and tracks), the
/// scale bar.
#[derive(Default)]
struct Drawn {
    readout: Option<Rect>,
    sliders: Option<(Rect, Vec<(usize, Rect)>)>,
    scale: Option<Rect>,
}

/// The screen-reader audit of the panels `d` drew.
fn audit(
    name: &str,
    rtl: bool,
    lines: &[String],
    sliders: &[SliderRow],
    bar: Option<&ScaleBar>,
    d: &Drawn,
) {
    super::super::a11y_audit::audit_screen(name, SIZE, rtl, |t| {
        a11y(
            t,
            d.readout.map(|r| (lines, r)),
            d.sliders
                .as_ref()
                .map(|(p, tracks)| (*p, sliders, tracks.as_slice())),
            bar.zip(d.scale),
            1.0,
        );
    });
}

/// The view of the whole image filling most of a `size` (physical px) target.
fn view_of(size: (u32, u32)) -> View {
    View::fixed(
        (W, H),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        },
        size.1 as f32 / H as f32,
        (W as f32 / 2.0, H as f32 / 2.0),
    )
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    /// Planes in the file (3 and no axis type: colour).
    planes: usize,
    ctype3: &'static str,
    stretch: Stretch,
    limits: Limits,
    index: u32,
    composite: bool,
    hidden: [bool; MAX_CHANNELS],
    grid: bool,
    dark: bool,
    rtl: bool,
    scale: f32,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, c: &Case) {
    let s = open(sky(W as usize, H as usize, c.planes, c.ctype3));
    let summary = Arc::new(s.science().expect("scientific").clone());
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![c.index; summary.axes.len()],
        composite: c.composite,
        hidden: c.hidden,
        stretch: c.stretch,
        limits: c.limits,
        grid: c.grid,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let positions: Vec<Vec<u32>> = match req.composite {
        Some((axis, n)) => (0..n)
            .map(|k| {
                let mut i = req.index.clone();
                i[axis] = k;
                i
            })
            .collect(),
        None => vec![req.index.clone()],
    };
    sc.planes = positions
        .iter()
        .map(|p| {
            let plane = s.nd_plane(p, &CancelToken::new()).unwrap().unwrap();
            let limits = c.limits.compute(&plane.values, plane.width as usize);
            let lut = match limits {
                Some((lo, hi)) if req.histeq => nd::histeq_lut(&plane.values, lo, hi),
                _ => Vec::new(),
            };
            SciencePlane {
                plane,
                limits,
                lut,
                mid: 0.5,
            }
        })
        .collect();
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    let values: Vec<&[f32]> = sc
        .planes
        .iter()
        .map(|p| p.plane.values.as_slice())
        .collect();
    renderer.set_value_channels(&h.gpu, &mut img, &values);
    renderer.set_science(&h.gpu, &mut img, Some(look));

    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let view = view_of(size_px);
    let pal = if c.dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let w = summary.wcs.as_ref().unwrap();
    let rest = wcs_rest(&summary, w, &sc.index);
    let to_screen = view.stored_from_screen().invert();
    let image_rect = view.image_screen_rect();
    let grid_lines: Vec<GridLine> = if c.grid {
        grid(w, W, H, &rest)
            .into_iter()
            .map(|g| GridLine {
                pts: g
                    .pts
                    .iter()
                    .map(|&p| {
                        let q = to_screen.apply(p);
                        (q.0 / c.scale, q.1 / c.scale)
                    })
                    .collect(),
                label: g.label,
            })
            .collect()
    } else {
        Vec::new()
    };
    let bar = scale_bar(
        w.arcsec_per_pixel().unwrap(),
        f64::from(view.scale() / c.scale),
    );
    let comp = c.composite.then(|| sc.composite_axis()).flatten();
    let sliders: Vec<SliderRow> = (0..summary.axes.len())
        .filter(|&a| summary.axes[a].len >= 2 && Some(a) != comp)
        .map(|a| SliderRow {
            axis: a,
            name: axis_name(&summary, a),
            world: axis_world(&summary, a, sc.index[a]),
            index: sc.index[a],
            len: summary.axes[a].len,
            selected: true,
        })
        .collect();
    // The readout as at a star's centre.
    let (px, py) = (0.6 * W as f32, 0.8 * H as f32);
    let mut lines = Vec::new();
    let vals: Vec<f32> = sc
        .planes
        .iter()
        .filter_map(|p| p.plane.value(px as u32, py as u32))
        .collect();
    if vals.len() == 1 {
        lines.push(t!(
            "science-value-unit",
            value = value_text(vals[0]),
            unit = "ADU"
        ));
    } else {
        let text = vals
            .iter()
            .enumerate()
            .map(|(k, v)| format!("{} {}", sc.channel_name(k), value_text(*v)))
            .collect::<Vec<_>>()
            .join("  ");
        lines.push(t!("science-values", values = text));
    }
    let (l, b) = w.sky_at(f64::from(px), f64::from(py), &rest).unwrap();
    lines.extend(sky_lines(&w.celestial.as_ref().unwrap().frame, l, b));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(c.stretch),
        limits = limits_name(c.limits)
    ));
    let rtl = c.rtl;
    let mut d = Drawn::default();
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            if !grid_lines.is_empty() {
                let r = image_rect;
                let clip = Rect::new(r.x / c.scale, r.y / c.scale, r.w / c.scale, r.h / c.scale);
                draw_grid(dl, text, &grid_lines, clip, 1.0, GRID_INK);
            }
            d.readout = Some(draw_readout(dl, text, &lines, area, rtl, 1.0));
            d.sliders = (!sliders.is_empty())
                .then(|| draw_sliders(dl, text, &pal, &sliders, area, rtl, 1.0));
            if let Some(bar) = &bar {
                let panel = d.sliders.as_ref().map(|s| s.0);
                d.scale = Some(draw_scale(dl, text, &pal, bar, area, panel, rtl, 1.0));
            }
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    audit(c.name, rtl, &lines, &sliders, bar.as_ref(), &d);
}

#[test]
fn science_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-science",
    );
    let base = Case {
        name: "",
        planes: 3,
        ctype3: "",
        stretch: Stretch::Linear,
        limits: Limits::ZScale,
        index: 0,
        composite: true,
        hidden: [false; MAX_CHANNELS],
        grid: false,
        dark: true,
        rtl: false,
        scale: 1.0,
    };
    let cases = [
        Case {
            name: "science-rgb-grid-dark",
            grid: true,
            ..base
        },
        Case {
            name: "science-rgb-hidden-green-light",
            stretch: Stretch::Sqrt,
            hidden: std::array::from_fn(|k| k == 1),
            dark: false,
            scale: 1.5,
            ..base
        },
        Case {
            name: "science-cube-asinh-light",
            planes: 5,
            ctype3: "WAVE",
            stretch: Stretch::Asinh,
            limits: Limits::MinMax,
            index: 2,
            composite: false,
            dark: false,
            ..base
        },
        Case {
            name: "science-cube-log-dark",
            planes: 5,
            ctype3: "WAVE",
            stretch: Stretch::Log,
            limits: Limits::Percentile(99.5),
            index: 4,
            composite: false,
            grid: true,
            scale: 1.5,
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "science-cube-histeq-he",
            planes: 5,
            ctype3: "WAVE",
            stretch: Stretch::HistEq,
            index: 1,
            composite: false,
            grid: true,
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &c);
    });
    goldens.finish();
}

/// A microscopy case: one of the images in `tests/data/science` (written by
/// `crates/formats/specular-em/gen/make_samples.py shots`).
#[derive(Clone, Copy)]
struct Micro {
    name: &'static str,
    file: &'static str,
    index: u32,
    level: bool,
    /// The plane's power spectrum instead of the plane.
    fft: bool,
    limits: Limits,
    dark: bool,
    rtl: bool,
    scale: f32,
    /// A colour table file the user imported (`scitiff/…`).
    user_lut: Option<&'static str>,
}

/// A science test file: `scitiff/…` from specular-tiff's fixtures, `micro/…` from
/// specular-micro's (Leica LIF, Nikon ND2), else the app's own.
fn data_path(file: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(f) = file.strip_prefix("micro/") {
        return root
            .join("../../crates/formats/specular-micro/tests/data")
            .join(f);
    }
    match file.strip_prefix("scitiff/") {
        Some(f) => root
            .join("../../crates/formats/specular-tiff/tests/data/scitiff")
            .join(f),
        None => root.join("tests/data/science").join(file),
    }
}

fn open_data(file: &str) -> Arc<dyn specular_decode::Session> {
    let p = data_path(file);
    let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(file, bytes)))
        .expect("microscopy image opens")
}

fn shoot_micro(h: &mut Harness, goldens: &mut Goldens, c: &Micro) {
    let s = open_data(c.file);
    let summary = Arc::new(s.science().expect("scientific").clone());
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![c.index; summary.axes.len()],
        stretch: Stretch::Linear,
        limits: c.limits,
        level: c.level,
        fft: c.fft,
        user_lut: c
            .user_lut
            .map(|f| dsci::read_lut(&std::fs::read(data_path(f)).unwrap()).expect("colour table")),
        ..Default::default()
    };
    let req = sc.request().unwrap();
    assert_eq!(req.level, c.level, "{}", c.name);
    assert_eq!(req.fft, c.fft, "{}", c.name);
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let plane = transform(plane, &req).unwrap();
    // The limits as the app finds them: the file's display range, else `c.limits`.
    sc.planes = prepare(plane, &req, &req.index);
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));

    let view_of = |size: (u32, u32)| {
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            size.1 as f32 / ht as f32,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let view = view_of(size_px);
    let pal = if c.dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let cal = summary.pixel.as_ref().expect("pixel calibration");
    let bar = linear_scale_bar(
        cal.x.step.abs(),
        &cal.unit_x,
        f64::from(view.scale() / c.scale),
    )
    .filter(|_| !c.fft);
    assert!(bar.is_some() || c.fft, "{}", c.name);
    let sliders: Vec<SliderRow> = (0..summary.axes.len())
        .filter(|&a| summary.axes[a].len >= 2)
        .map(|a| SliderRow {
            axis: a,
            name: axis_name(&summary, a),
            world: axis_world(&summary, a, sc.index[a]),
            index: sc.index[a],
            len: summary.axes[a].len,
            selected: true,
        })
        .collect();
    let (px, py) = ((0.6 * w as f32).floor(), (0.4 * ht as f32).floor());
    let v = value_text(sc.planes[0].plane.value(px as u32, py as u32).unwrap());
    let mut lines = Vec::new();
    if c.fft {
        lines.push(t!("science-fft-power", value = v));
        lines.extend(fft_lines(&summary, px, py));
        lines.push(t!("science-fft"));
    } else {
        lines.push(match summary.unit_at(&sc.index) {
            Some(u) => t!("science-value-unit", value = v, unit = u.to_owned()),
            None => t!("science-value", value = v),
        });
        lines.push(t!(
            "science-position",
            x = quantity_text(cal.x.at(f64::from(px)), &cal.unit_x),
            y = quantity_text(cal.y.at(f64::from(py)), &cal.unit_y)
        ));
        let limits = if req.file_range(&req.index).is_some() {
            t!("science-limits-file")
        } else {
            limits_name(c.limits)
        };
        lines.push(t!(
            "science-look",
            stretch = stretch_name(Stretch::Linear),
            limits = limits
        ));
    }
    if req.level {
        lines.push(t!("science-levelled"));
    }
    let rtl = c.rtl;
    let mut d = Drawn::default();
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            d.readout = Some(draw_readout(dl, text, &lines, area, rtl, 1.0));
            d.sliders = (!sliders.is_empty())
                .then(|| draw_sliders(dl, text, &pal, &sliders, area, rtl, 1.0));
            if let Some(bar) = &bar {
                let panel = d.sliders.as_ref().map(|s| s.0);
                d.scale = Some(draw_scale(dl, text, &pal, bar, area, panel, rtl, 1.0));
            }
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    audit(c.name, rtl, &lines, &sliders, bar.as_ref(), &d);
}

/// Electron and scanning probe microscopy: a DigitalMicrograph micrograph, a section of an
/// MRC particle stack, and an AFM height map with plane levelling (also right to left);
/// light microscopy: an ImageJ hyperstack's channel through its own LUT and display range,
/// and another through a colour table file the user imported.
#[test]
fn microscopy_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-microscopy",
    );
    let base = Micro {
        name: "",
        file: "",
        index: 0,
        level: false,
        fft: false,
        limits: Limits::ZScale,
        dark: true,
        rtl: false,
        scale: 1.0,
        user_lut: None,
    };
    let cases = [
        Micro {
            name: "science-dm-particles-dark",
            file: "particles.dm3",
            ..base
        },
        Micro {
            name: "science-mrc-stack-light",
            file: "stack.mrcs",
            index: 1,
            limits: Limits::MinMax,
            dark: false,
            scale: 1.5,
            ..base
        },
        Micro {
            name: "science-afm-levelled-dark",
            file: "height.ibw",
            level: true,
            limits: Limits::Percentile(99.5),
            ..base
        },
        // The levelled height map's power spectrum, with frequency and period readout.
        Micro {
            name: "science-afm-fft-dark",
            file: "height.ibw",
            level: true,
            fft: true,
            limits: Limits::MinMax,
            ..base
        },
        Micro {
            name: "science-imagej-lut-range-dark",
            file: "scitiff/hyperstack.tif",
            ..base
        },
        Micro {
            name: "science-imagej-user-lut-light",
            file: "scitiff/hyperstack.tif",
            index: 1,
            dark: false,
            scale: 1.5,
            user_lut: Some("scitiff/text.lut"),
            ..base
        },
    ];
    for c in &cases {
        shoot_micro(&mut h, &mut goldens, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Micro {
            name: "science-afm-levelled-he",
            file: "height.ibw",
            level: true,
            limits: Limits::Percentile(99.5),
            rtl: true,
            ..base
        };
        shoot_micro(&mut h, &mut goldens, &c);
    });
    goldens.finish();
}

/// An elevation model (a Float32 GeoTIFF in UTM zone 33N with a no-data corner, generated
/// here) in science mode with a false-colour map: the no-data corner shows the canvas, the
/// readout gives the height and the map position in metres and in longitude and latitude,
/// and the scale bar is in metres. `shade`: the hillshade on, over the false colours
/// (`colour`) or alone.
#[allow(clippy::too_many_arguments)]
fn shoot_dem(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    dark: bool,
    rtl: bool,
    scale: f32,
    shade: bool,
    colour: bool,
) {
    use specular_decode::geo::specular_gis::fixtures::{self, GeoGrid};
    let (w, ht) = (240usize, 150usize);
    let vals = fixtures::dem_values(w, ht, true);
    let grid = GeoGrid {
        epsg: 32633,
        origin: (500_000.0, 5_000_000.0),
        pixel: (30.0, 30.0),
    };
    let tif = fixtures::geotiff_f32(&vals, w, ht, grid, Some(-9999.0), 128);
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec("dem.tif", tif)))
        .expect("opens");
    let summary = Arc::new(s.science().expect("values").clone());
    let geo = summary.geo.clone().expect("georeferenced");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        stretch: Stretch::Linear,
        limits: Limits::MinMax,
        hillshade: shade,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let limits = Limits::MinMax.compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            (size.0 as f32 / w as f32).min(size.1 as f32 / ht as f32),
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let view = view_of(size_px);
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    // UTM metres: 30 m a pixel east, 30 m south down the rows.
    if shade {
        let hs = sc
            .hillshade_now()
            .expect("an elevation model suits the hillshade");
        assert!(
            (hs.step[0] - 30.0).abs() < 1e-3 && (hs.step[1] + 30.0).abs() < 1e-3,
            "{hs:?}"
        );
        assert_eq!((hs.azimuth, hs.altitude), (315.0, 45.0));
    }
    let bar = super::super::geo::geo_scale_bar(
        &geo,
        f64::from(w) / 2.0,
        f64::from(ht) / 2.0,
        f64::from(view.scale() / scale),
    );
    assert!(
        bar.as_ref().is_some_and(|b| b.label.contains("m\u{2069}")),
        "{bar:?}"
    );
    let (px, py) = ((0.6 * w as f32).floor(), (0.4 * ht as f32).floor());
    let v = value_text(sc.planes[0].plane.value(px as u32, py as u32).unwrap());
    let mut lines = vec![match summary.unit_at(&sc.index) {
        Some(u) => t!("science-value-unit", value = v, unit = u.to_owned()),
        None => t!("science-value", value = v),
    }];
    lines.extend(super::super::geo::geo_position_lines(
        &geo,
        f64::from(px),
        f64::from(py),
    ));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(Limits::MinMax)
    ));
    if let Some(hs) = sc.hillshade_now() {
        lines.push(t!(
            "science-hillshaded",
            azimuth = hs.azimuth.to_string(),
            altitude = hs.altitude.to_string()
        ));
    }
    let mut d = Drawn::default();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions {
                false_colour: colour.then_some(specular_inspect::ColourMap::Batlow),
                ..RenderOptions::default()
            };
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            d.readout = Some(draw_readout(dl, text, &lines, area, rtl, 1.0));
            if let Some(bar) = &bar {
                d.scale = Some(draw_scale(dl, text, &pal, bar, area, None, rtl, 1.0));
            }
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
    audit(name, rtl, &lines, &[], bar.as_ref(), &d);
}

/// GeoTIFF elevation models: a false-colour DEM with its no-data corner, map and lon/lat
/// readout and metre scale bar, dark at 1×, light at 1.5×, and right to left in Arabic; the
/// hillshade alone (dark, 1×) and over the false colours (light, 1.5×).
#[test]
fn dem_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gis-dem",
    );
    shoot_dem(
        &mut h,
        &mut goldens,
        "gis-dem-batlow-dark",
        true,
        false,
        1.0,
        false,
        true,
    );
    shoot_dem(
        &mut h,
        &mut goldens,
        "gis-dem-batlow-light",
        false,
        false,
        1.5,
        false,
        true,
    );
    shoot_dem(
        &mut h,
        &mut goldens,
        "gis-dem-hillshade-dark",
        true,
        false,
        1.0,
        true,
        false,
    );
    shoot_dem(
        &mut h,
        &mut goldens,
        "gis-dem-hillshade-batlow-light",
        false,
        false,
        1.5,
        true,
        true,
    );
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("ar"), || {
        shoot_dem(
            &mut h,
            &mut goldens,
            "gis-dem-readout-ar",
            true,
            true,
            1.0,
            false,
            true,
        );
    });
    goldens.finish();
}

/// The colour sky's red, green and blue planes as a one-shot colour camera records them: one
/// sample per pixel through an RGGB filter array (displayed orientation).
fn mosaic() -> Vec<f32> {
    let s = open(sky(W as usize, H as usize, 3, ""));
    let planes: Vec<Arc<NdPlane>> = (0..3)
        .map(|k| {
            s.nd_plane(&[0, 0, k], &CancelToken::new())
                .unwrap()
                .unwrap()
        })
        .collect();
    let w = W as usize;
    (0..W as usize * H as usize)
        .map(|i| planes[Cfa::RGGB.at(i % w, i / w)].values[i])
        .collect()
}

/// A one-plane FITS of the displayed `values` (stored bottom row first) on a `code`
/// projection, with `BAYERPAT` when `bayer`.
fn raw_fits(values: &[f32], code: &str, bayer: bool) -> Vec<u8> {
    let w = W as usize;
    let stored: Vec<f32> = values.chunks(w).rev().flatten().copied().collect();
    let (ws, hs) = (W.to_string(), H.to_string());
    let (c1, c2) = (
        (W as f64 / 2.0 + 0.5).to_string(),
        (H as f64 / 2.0 + 0.5).to_string(),
    );
    let (t1, t2) = (format!("'RA---{code}'"), format!("'DEC--{code}'"));
    let mut cards = vec![
        ("SIMPLE", "T"),
        ("BITPIX", "-32"),
        ("NAXIS", "2"),
        ("NAXIS1", ws.as_str()),
        ("NAXIS2", hs.as_str()),
        ("CTYPE1", t1.as_str()),
        ("CTYPE2", t2.as_str()),
        ("CRVAL1", "202.4696"),
        ("CRVAL2", "47.1952"),
        ("CRPIX1", c1.as_str()),
        ("CRPIX2", c2.as_str()),
        ("CDELT1", "-0.0005"),
        ("CDELT2", "0.0005"),
        ("BUNIT", "'ADU'"),
    ];
    if bayer {
        cards.extend([("BAYERPAT", "'RGGB'"), ("ROWORDER", "'BOTTOM-UP'")]);
    }
    fits(&cards, &stored)
}

/// A one-plane FITS as science mode shows it, debayered when its header names a Bayer
/// pattern: the readout (the three colours, the sky or why there is none, the automatic
/// stretch, the pattern) and the scale bar or, without usable world coordinates, the reason.
fn shoot_raw(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    file: Vec<u8>,
    dark: bool,
    rtl: bool,
) {
    let s = open(file);
    let summary = Arc::new(s.science().expect("scientific").clone());
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    sc.planes = prepare(plane.clone(), &req, &req.index);
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    match req.cfa {
        Some(cfa) => renderer.set_value_cfa(&h.gpu, &mut img, &plane.values, cfa),
        None => renderer.set_value_channels(&h.gpu, &mut img, &[plane.values.as_slice()]),
    }
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let w = summary.wcs.as_ref().expect("a WCS");
    let view = view_of((SIZE.0 as u32, SIZE.1 as u32));
    let bar = match &w.unsupported {
        Some(why) => Some(ScaleBar {
            px: 0.0,
            label: wcs_unsupported(why),
        }),
        None => scale_bar(w.arcsec_per_pixel().unwrap(), f64::from(view.scale())),
    };
    let (px, py) = (0.6 * W as f32, 0.8 * H as f32);
    let mut lines = Vec::new();
    match req.cfa {
        Some(cfa) => {
            let mut v = [0.0; 3];
            cfa.debayer_row(
                &plane.values,
                W as usize,
                H as usize,
                py as usize,
                |x, rgb| {
                    if x == px as usize {
                        v = rgb;
                    }
                },
            );
            let text = v
                .iter()
                .zip(["R", "G", "B"])
                .map(|(v, n)| format!("{n} {}", value_text(*v)))
                .collect::<Vec<_>>()
                .join("  ");
            lines.push(t!("science-values", values = text));
        }
        None => {
            let v = plane.value(px as u32, py as u32).unwrap();
            lines.push(t!(
                "science-value-unit",
                value = value_text(v),
                unit = "ADU"
            ));
        }
    }
    match (&w.celestial, &w.unsupported) {
        (_, Some(why)) => lines.push(wcs_unsupported(why)),
        (Some(c), None) => {
            let (l, b) = w.sky_at(f64::from(px), f64::from(py), &[]).unwrap();
            lines.extend(sky_lines(&c.frame, l, b));
        }
        (None, None) => {}
    }
    match sc.stretch_now() {
        Stretch::Auto => lines.push(stretch_name(Stretch::Auto)),
        st => lines.push(t!(
            "science-look",
            stretch = stretch_name(st),
            limits = limits_name(sc.limits)
        )),
    }
    if let Some(c) = sc.cfa() {
        lines.push(t!("science-debayer-on", pattern = c.name()));
    }
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            draw_readout(dl, text, &lines, area, rtl, 1.0);
            if let Some(bar) = &bar {
                draw_scale(dl, text, &pal, bar, area, None, rtl, 1.0);
            }
        },
    );
    goldens.check(&format!("{name}-1x"), &shot);
}

/// Colour camera raw frames: a FITS frame with `BAYERPAT` debayered on the GPU with the
/// automatic stretch, a SER video's Bayer frame as its session decodes it, and a FITS
/// header whose projection is not supported (the reason in place of the sky position and
/// the scale bar, also right to left).
#[test]
fn raw_frames_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-raw-frames",
    );
    let raw = mosaic();
    let bayer = raw_fits(&raw, "TAN", true);
    assert_eq!(open(bayer.clone()).science().unwrap().cfa, Some(Cfa::RGGB));
    shoot_raw(
        &mut h,
        &mut goldens,
        "science-debayer-dark",
        bayer,
        true,
        false,
    );

    let tnx = raw_fits(&raw, "TNX", false);
    shoot_raw(
        &mut h,
        &mut goldens,
        "science-wcs-unsupported-light",
        tnx.clone(),
        false,
        false,
    );
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shoot_raw(
            &mut h,
            &mut goldens,
            "science-wcs-unsupported-he",
            tnx,
            true,
            true,
        );
    });

    // The same mosaic as an 8-bit SER video frame: its session demosaics it.
    let hi = raw.iter().copied().fold(f32::MIN, f32::max);
    let frame: Vec<u8> = raw.iter().map(|v| (v / hi * 255.0).round() as u8).collect();
    let header = specular_ser::Header {
        colour_id: 8,
        width: W,
        height: H,
        depth: 8,
        ..Default::default()
    };
    let ser = specular_ser::write(&header, &[frame.clone(), frame.clone()], None);
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec("planet.ser", ser)))
        .expect("SER opens");
    assert_eq!(s.debayer(), Some(Cfa::RGGB));
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &CancelToken::new(),
        )
        .unwrap();
    // The session's demosaic is the same bilinear one, in the same phase: each colour, put
    // through its own automatic stretch, rises with the demosaiced value.
    let samples: Vec<f32> = frame.iter().map(|&v| f32::from(v)).collect();
    let p = &r.planes[0];
    let mut pairs: [Vec<(f32, u8)>; 3] = Default::default();
    for y in 0..H as usize {
        Cfa::RGGB.debayer_row(&samples, W as usize, H as usize, y, |x, rgb| {
            for (c, want) in rgb.into_iter().enumerate() {
                pairs[c].push((want, p.data[y * p.stride + x * 4 + c]));
            }
        });
    }
    for (c, mut v) in pairs.into_iter().enumerate() {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        let worst = v
            .windows(2)
            .map(|w| i32::from(w[0].1) - i32::from(w[1].1))
            .max();
        assert!(
            worst.unwrap_or(0) <= 1,
            "colour {c} out of order by {worst:?}"
        );
    }
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    renderer.upload_overview(&h.gpu, &mut img, &r);
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |_, _| {},
    );
    goldens.check("ser-bayer-frame-1x", &shot);
    goldens.finish();
}

/// A seismic 2-D line (`int16_line.sgy` from specular-segy's fixtures) in science mode:
/// through the red-white-blue table at the default gain (±99th percentile), whole; and
/// zoomed until the traces are far enough apart to draw as wiggles with their positive
/// lobes filled (variable area), with the trace, CDP and time readout.
fn shoot_seismic(h: &mut Harness, goldens: &mut Goldens, name: &str, zoom: f32, scale: f32) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-segy/tests/data/int16_line.sgy");
    let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(
            "int16_line.sgy",
            bytes,
        )))
        .expect("SEG-Y opens");
    let summary = Arc::new(s.science().expect("values").clone());
    let seismic = summary.seismic.clone().expect("seismic");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        stretch: Stretch::Linear,
        ..Default::default()
    };
    let gain = sc.limits_now();
    assert_eq!(gain, Limits::SEISMIC);
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let limits = gain.compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    // Zoomed: centred a third of the way across, near the top.
    let centre = if zoom > 1.0 {
        (w as f32 / 3.0, ht as f32 / 4.0)
    } else {
        (w as f32 / 2.0, ht as f32 / 2.0)
    };
    let view_of = |size: (u32, u32)| {
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom * (size.0 as f32 / w as f32).min(size.1 as f32 / ht as f32),
            centre,
        )
    };
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let view = view_of(size_px);
    let from = view.stored_from_screen();
    let a = from.apply((0.0, 0.0));
    let b = from.apply((size_px.0 as f32, size_px.1 as f32));
    let to_screen = from.invert();
    let clip = limits.map_or(0.0, |(lo, hi)| lo.abs().max(hi.abs()));
    let wiggle = wiggles(
        &sc.planes[0].plane,
        clip,
        (a.0, a.1, b.0, b.1),
        view.scale() / scale,
        |q| {
            let s = to_screen.apply(q);
            (s.0 / scale, s.1 / scale)
        },
    );
    assert_eq!(wiggle.is_some(), zoom > 1.0, "wiggles only when zoomed in");
    let (px, py) = if zoom > 1.0 {
        (centre.0.floor(), centre.1.floor())
    } else {
        ((0.6 * w as f32).floor(), (0.4 * ht as f32).floor())
    };
    let v = value_text(sc.planes[0].plane.value(px as u32, py as u32).unwrap());
    let mut lines = vec![t!("science-value", value = v)];
    lines.extend(seismic_lines(&seismic, &sc.index, px as u32, py as u32));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(gain)
    ));
    let image_rect = view.image_screen_rect();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            if let Some(wg) = &wiggle {
                let r = image_rect;
                let clip = Rect::new(r.x / scale, r.y / scale, r.w / scale, r.h / scale);
                draw_wiggles(dl, wg, clip);
            }
            draw_readout(dl, text, &lines, area, false, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Seismic sections: the colour image of a whole line, and wiggles with variable area when
/// zoomed in (at 1.5×).
#[test]
fn seismic_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-seismic",
    );
    shoot_seismic(&mut h, &mut goldens, "seismic-line-colour", 1.0, 1.0);
    shoot_seismic(&mut h, &mut goldens, "seismic-line-wiggle", 4.0, 1.5);
    goldens.finish();
}

/// Annotations an ImageScope user drew on the eight-channel spots (`micro/blobs_v3.nd2`, 0.5 µm
/// pixels): a polygon, an ellipse, a rectangle, an arrow and a ruler in one layer, a point and
/// an open outline in another.
const SPOT_ANNOTATIONS: &str = r#"<Annotations MicronsPerPixel="0.5">
<Annotation Id="1" Name="Tumor" LineColor="41215"><Regions>
<Region Id="1" Type="0" Text="tumour"><Vertices><Vertex X="43" Y="18"/><Vertex X="40" Y="23.2"/><Vertex X="34" Y="23.2"/><Vertex X="31" Y="18"/><Vertex X="34" Y="12.8"/><Vertex X="40" Y="12.8"/></Vertices></Region>
<Region Id="2" Type="2" Text="nucleus"><Vertices><Vertex X="19" Y="3"/><Vertex X="29" Y="13"/></Vertices></Region>
<Region Id="3" Type="1" Text="ROI"><Vertices><Vertex X="5" Y="12"/><Vertex X="17" Y="12"/><Vertex X="17" Y="24"/><Vertex X="5" Y="24"/></Vertices></Region>
<Region Id="4" Type="3" Text=""><Vertices><Vertex X="18" Y="33"/><Vertex X="26" Y="29.5"/></Vertices></Region>
<Region Id="5" Type="4" Text="d"><Vertices><Vertex X="29" Y="33"/><Vertex X="45" Y="33"/></Vertices></Region>
</Regions></Annotation>
<Annotation Id="2" Name="Stroma" LineColor="7929680"><Regions>
<Region Id="6" Type="0" Text="mitosis"><Vertices><Vertex X="33.2" Y="25.1"/></Vertices></Region>
<Region Id="7" Type="0" Text=""><Vertices><Vertex X="2" Y="2"/><Vertex X="6" Y="3"/><Vertex X="9" Y="2.5"/><Vertex X="13" Y="4"/><Vertex X="15" Y="7"/><Vertex X="12" Y="9"/><Vertex X="6" Y="8"/></Vertices></Region>
</Regions></Annotation>
</Annotations>"#;

/// An eight-channel composite (more than the four channels a texture holds), some channels
/// hidden, and slide annotations drawn over it.
fn shoot_vendor(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    hidden: &[usize],
    annotations: bool,
    dark: bool,
    scale: f32,
) {
    let s = open_data("micro/blobs_v3.nd2");
    let summary = Arc::new(s.science().expect("scientific").clone());
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        composite: true,
        ..Default::default()
    };
    for &k in hidden {
        sc.hidden[k] = true;
    }
    let req = sc.request().unwrap();
    let (axis, n) = req.composite.expect("a channel composite");
    assert_eq!(n, 8);
    sc.planes = (0..n)
        .flat_map(|k| {
            let mut i = req.index.clone();
            i[axis] = k;
            let plane = s.nd_plane(&i, &CancelToken::new()).unwrap().unwrap();
            prepare(plane, &req, &i)
        })
        .collect();
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    let values: Vec<&[f32]> = sc
        .planes
        .iter()
        .map(|p| p.plane.values.as_slice())
        .collect();
    renderer.set_value_channels(&h.gpu, &mut img, &values);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            size.1 as f32 / ht as f32,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let view = view_of(size_px);
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let cal = summary.pixel.clone().expect("pixel calibration");
    let bar = linear_scale_bar(
        cal.x.step.abs(),
        &cal.unit_x,
        f64::from(view.scale() / scale),
    );
    // The readout at the first channel's spot.
    let (px, py) = (37u32, 18u32);
    let shown: Vec<String> = sc
        .planes
        .iter()
        .enumerate()
        .filter(|(k, _)| !sc.hidden[*k])
        .filter_map(|(k, p)| {
            Some(format!(
                "{} {}",
                sc.channel_name(k),
                value_text(p.plane.value(px, py)?)
            ))
        })
        .collect();
    let mut lines: Vec<String> = shown
        .chunks(4)
        .map(|c| t!("science-values", values = c.join("  ")))
        .collect();
    lines.extend([t!(
        "science-position",
        x = quantity_text(cal.x.at(f64::from(px)), &cal.unit_x),
        y = quantity_text(cal.y.at(f64::from(py)), &cal.unit_y)
    )]);
    let shapes = if annotations {
        let set = dsci::annot::parse(SPOT_ANNOTATIONS.as_bytes(), None).unwrap();
        assert_eq!(set.items.len(), 7);
        let to_screen = view.stored_from_screen().invert();
        crate::app::annotations::place(
            &set,
            |p| {
                let q = to_screen.apply(p);
                (q.0 / scale, q.1 / scale)
            },
            Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
            |a| Some(quantity_text(a.length(cal.x.step, cal.y.step), &cal.unit_x)),
        )
    } else {
        Vec::new()
    };
    let image_rect = view.image_screen_rect();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let r = image_rect;
            let clip = Rect::new(r.x / scale, r.y / scale, r.w / scale, r.h / scale);
            crate::app::annotations::draw(dl, text, &shapes, clip, 1.0);
            if !annotations {
                draw_readout(dl, text, &lines, area, false, 1.0);
            }
            if let Some(bar) = &bar {
                draw_scale(dl, text, &pal, bar, area, None, false, 1.0);
            }
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Nikon ND2 with eight channels in their own colours (two textures' worth), with half the
/// channels hidden, and with an Aperio annotation layer over it.
#[test]
fn vendor_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-vendor",
    );
    shoot_vendor(
        &mut h,
        &mut goldens,
        "vendor-nd2-eight-channels-dark",
        &[],
        false,
        true,
        1.0,
    );
    shoot_vendor(
        &mut h,
        &mut goldens,
        "vendor-nd2-four-hidden-light",
        &[1, 3, 5, 7],
        false,
        false,
        1.5,
    );
    shoot_vendor(
        &mut h,
        &mut goldens,
        "slide-annotations-dark",
        &[],
        true,
        true,
        1.0,
    );
    goldens.finish();
}

/// A sky case: one of specular-fits' table fixtures (`crates/formats/specular-fits/gen/
/// table_fixtures.py`), shown at `index`, with the coordinate grid, and a catalogue over it.
#[derive(Clone, Copy)]
struct SkyCase {
    name: &'static str,
    file: &'static str,
    index: &'static [u32],
    limits: Limits,
    colour: Option<specular_inspect::ColourMap>,
    grid: bool,
    catalogue: Option<&'static str>,
}

fn table_fixture(file: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-fits/tests/data/tables")
        .join(file);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn shoot_sky(h: &mut Harness, goldens: &mut Goldens, c: &SkyCase) {
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(
            c.file,
            table_fixture(c.file),
        )))
        .expect("opens");
    let summary = Arc::new(s.science().expect("scientific").clone());
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: c.index.to_vec(),
        limits: c.limits,
        limits_chosen: true,
        grid: c.grid,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    sc.planes = prepare(plane, &req, &req.index);
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));

    let view_of = |size: (u32, u32)| {
        let fit = (size.0 as f32 / w as f32).min(size.1 as f32 / ht as f32);
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            fit,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let size_px = (SIZE.0.round() as u32, SIZE.1.round() as u32);
    let view = view_of(size_px);
    let pal = Palette::dark();
    let wcs = summary.wcs.as_ref().expect("sky coordinates");
    let rest = wcs_rest(&summary, wcs, &sc.index);
    let to_screen = view.stored_from_screen().invert();
    let place = |lines: Vec<GridLine>| -> Vec<GridLine> {
        lines
            .into_iter()
            .map(|g| GridLine {
                pts: g.pts.iter().map(|&p| to_screen.apply(p)).collect(),
                label: g.label,
            })
            .collect()
    };
    let grid_lines = if c.grid {
        place(grid(wcs, w, ht, &rest))
    } else {
        Vec::new()
    };
    let contours = match &summary.credible {
        Some(cr) if cr.index == sc.index => place(sky::credible_lines(cr, &sc.planes[0].plane)),
        _ => Vec::new(),
    };
    let cat = c
        .catalogue
        .map(|f| specular_decode::science::catalogue_of(&table_fixture(f)).expect("a catalogue"));
    let markers: Vec<sky::Marker> = cat
        .as_ref()
        .map(|cat| {
            let (m, on) = sky::place(cat, 0, wcs, w, ht, &rest);
            assert!(on > 0, "{}: no sources on the image", c.name);
            m.into_iter()
                .map(|m| sky::Marker {
                    at: to_screen.apply(m.at),
                    ..m
                })
                .collect()
        })
        .unwrap_or_default();
    // The tooltip of the source nearest the centre.
    let tip = cat.as_ref().and_then(|cat| {
        let mid = (SIZE.0 / 2.0, SIZE.1 / 2.0);
        let d = |m: &sky::Marker| (m.at.0 - mid.0).hypot(m.at.1 - mid.1);
        let k = (0..markers.len()).min_by(|&a, &b| d(&markers[a]).total_cmp(&d(&markers[b])))?;
        let m = markers[k];
        let tip = sky::Tip {
            at: m.at,
            lines: sky::tip_lines(c.catalogue.unwrap_or_default(), cat, m.row as usize),
        };
        Some((k, tip))
    });
    let sliders: Vec<SliderRow> = (0..summary.axes.len())
        .filter(|&a| summary.axes[a].len >= 2)
        .map(|a| SliderRow {
            axis: a,
            name: axis_name(&summary, a),
            world: axis_world(&summary, a, sc.index[a]),
            index: sc.index[a],
            len: summary.axes[a].len,
            selected: a == 0,
        })
        .collect();
    // The readout at the brightest pixel.
    let p = &sc.planes[0].plane;
    let k = (0..p.values.len())
        .filter(|&k| p.values[k].is_finite())
        .max_by(|&a, &b| p.values[a].total_cmp(&p.values[b]))
        .unwrap();
    let (px, py) = ((k % w as usize) as f32 + 0.5, (k / w as usize) as f32 + 0.5);
    let v = value_text(p.values[k]);
    let mut lines = vec![match summary.unit_at(&sc.index) {
        Some(u) => t!("science-value-unit", value = v, unit = u.to_owned()),
        None => t!("science-value", value = v),
    }];
    let (l, b) = wcs.sky_at(f64::from(px), f64::from(py), &rest).unwrap();
    lines.extend(sky_lines(&wcs.celestial.as_ref().unwrap().frame, l, b));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(c.limits)
    ));
    let colour = c.colour;
    let mut d = Drawn::default();
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions {
                false_colour: colour,
                ..RenderOptions::default()
            };
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let r = view.image_screen_rect();
            let clip =
                Rect::new(r.x, r.y, r.w, r.h).intersect(&Rect::new(0.0, 0.0, SIZE.0, SIZE.1));
            draw_grid(dl, text, &grid_lines, clip, 1.0, GRID_INK);
            draw_grid(dl, text, &contours, clip, 1.0, CONTOUR_INK);
            draw_markers(dl, &markers, clip, tip.as_ref().map(|t| t.0));
            d.readout = Some(draw_readout(dl, text, &lines, area, false, 1.0));
            if !sliders.is_empty() {
                d.sliders = Some(draw_sliders(dl, text, &pal, &sliders, area, false, 1.0));
            }
            if let Some((_, t)) = &tip {
                draw_tip(dl, text, t, area, 1.0);
            }
        },
    );
    goldens.check(&format!("{}-1x", c.name), &shot);
    audit(c.name, false, &lines, &sliders, None, &d);
}

/// FITS tables on the sky: a HEALPix map as a Mollweide all-sky image with the graticule, a
/// gravitational-wave sky map's 50 % and 90 % credible regions, an X-ray event list binned
/// into an image, and a source catalogue over it with one source's fields.
#[test]
fn sky_tables_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-fits-tables",
    );
    let cases = [
        SkyCase {
            name: "healpix-mollweide-grid-dark",
            file: "nest16.fits",
            index: &[0],
            limits: Limits::MinMax,
            colour: Some(specular_inspect::ColourMap::Viridis),
            grid: true,
            catalogue: None,
        },
        SkyCase {
            name: "gw-credible-regions-dark",
            file: "flatprob.fits",
            index: &[0],
            limits: Limits::MinMax,
            colour: Some(specular_inspect::ColourMap::Lajolla),
            grid: true,
            catalogue: None,
        },
        SkyCase {
            name: "xray-events-binned-dark",
            file: "chandra.fits",
            index: &[1, 3],
            limits: Limits::MinMax,
            colour: None,
            grid: true,
            catalogue: None,
        },
        SkyCase {
            name: "catalogue-overlay-dark",
            file: "chandra.fits",
            index: &[2, 3],
            limits: Limits::MinMax,
            colour: None,
            grid: false,
            catalogue: Some("cat.vot"),
        },
    ];
    for c in &cases {
        shoot_sky(&mut h, &mut goldens, c);
    }
    goldens.finish();
}

/// A mass spectrometry image (imzML from specular-imzml's fixtures) in science mode through
/// viridis: an ion image at the base peak of a pixel (profile spectra, drawn as a curve) or
/// the total ion current with the m/z prompt open (centroided spectra, drawn as sticks), each
/// with the spectrum of the pixel under the pointer above the readout.
fn shoot_imzml(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    file: &str,
    ion: bool,
    dark: bool,
    scale: f32,
) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-imzml/tests/data")
        .join(file);
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("imzML opens");
    let summary = Arc::new(s.science().expect("values").clone());
    let ms = summary.mass_spec.clone().expect("mass spectrometry image");
    let (w, ht) = (summary.width, summary.height);
    let index = vec![0; summary.axes.len()];
    // The pixel with a spectrum nearest the centre.
    let (px, py) = (0..ht)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| ms.has_spectrum(x, y, &index))
        .min_by_key(|&(x, y)| x.abs_diff(w / 2).pow(2) + y.abs_diff(ht / 2).pow(2))
        .expect("a spectrum");
    let window = ion.then(|| dsci::IonWindow {
        mz: ms.base_peak(px, py, &index).expect("a peak"),
        tolerance: dsci::IonTolerance::Da(0.5),
    });
    let mut sc = Science {
        summary: Some(summary.clone()),
        index,
        stretch: Stretch::Linear,
        ion: window,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    assert_eq!(req.ion, window);
    let cancel = CancelToken::new();
    let plane = match window {
        Some(win) => ms.ion_plane(win, &req.index, &cancel),
        None => ms.tic_plane(&req.index, &cancel),
    }
    .unwrap();
    let limits = sc.limits_now().compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    // The image at the start edge, mostly clear of the spectrum and the readout.
    let view_of = |size: (u32, u32)| {
        let zoom = (0.4 * size.0 as f32 / w as f32).min(0.8 * size.1 as f32 / ht as f32);
        let margin = 12.0 * size.0 as f32 / SIZE.0;
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            ((size.0 as f32 / 2.0 - margin) / zoom, ht as f32 / 2.0),
        )
    };
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let mut lines = Vec::new();
    if !ion {
        let text = trimmed(ms.base_peak(px, py, &sc.index).unwrap(), 4);
        lines.push(t!("science-ion-prompt", text = format!("{text} 5 pp")));
        lines.push(t!(
            "science-ion-prompt-hint",
            tolerance = ion::tolerance_text(ion::DEFAULT_TOLERANCE)
        ));
    }
    let v = value_text(sc.planes[0].plane.value(px, py).unwrap());
    lines.push(t!("science-value", value = v));
    lines.extend(ion::spot_lines(&ms, &sc.index, px, py));
    let cal = summary.pixel.as_ref().expect("pixel size");
    lines.push(t!(
        "science-position",
        x = quantity_text(cal.x.at(f64::from(px)), &cal.unit_x),
        y = quantity_text(cal.y.at(f64::from(py)), &cal.unit_y)
    ));
    lines.push(match window {
        Some(w) => t!("science-ion-window", window = ion::window_text(w)),
        None => t!("science-ion-tic"),
    });
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(sc.limits_now())
    ));
    let spectra = summary.spectra.as_ref().expect("spectra");
    let (axis, unit) = spectra.x_axis();
    let plot = SpectrumPlot {
        at: (px, py),
        index: sc.index.clone(),
        spectrum: spectra.at(px, py, &sc.index).expect("a spectrum"),
        unit: if unit.trim().is_empty() { axis } else { unit },
    };
    assert_eq!(
        plot.spectrum.sticks, !ion,
        "centroided fixture draws sticks"
    );
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let below = draw_readout(dl, text, &lines, area, false, 1.0);
            draw_spectrum(dl, text, &pal, &plot, area, Some(below), false, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Mass spectrometry images: an ion image with its profile spectrum (dark, 1× and 1.5×) and
/// the total ion current with the m/z prompt and a centroided spectrum (light).
#[test]
fn mass_spec_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-mass-spec",
    );
    for scale in [1.0, 1.5] {
        let (g, n) = (&mut goldens, "imzml-ion-profile");
        shoot_imzml(&mut h, g, n, "continuous_f32.imzML", true, true, scale);
    }
    let (g, n) = (&mut goldens, "imzml-tic-prompt");
    shoot_imzml(&mut h, g, n, "processed_f64_f32.imzML", false, false, 1.0);
    goldens.finish();
}

/// A thermal still (FLIR radiometric JPEG from specular-fmt-thermal's fixtures) in science
/// mode through the camera's palette, from its coldest to its hottest pixel, with the
/// temperature under the pointer; with an isotherm (and the object parameters prompt open);
/// or its visible photo (the file's second page).
fn shoot_thermal(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    unit: TemperatureUnit,
    isotherm: bool,
    scale: f32,
) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-fmt-thermal/tests/data/rjpeg.jpg");
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("R-JPEG opens");
    let summary = Arc::new(s.science().expect("values").clone());
    let th = summary.thermal.clone().expect("thermal");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        unit_chosen: Some(unit),
        ..Default::default()
    };
    assert_eq!(sc.limits_now(), Limits::MinMax);
    let req = sc.request().unwrap();
    let (cond, u) = req.thermal.expect("temperatures asked for");
    assert_eq!(u, unit);
    let plane = th.plane(&cond, unit, &CancelToken::new()).unwrap();
    let limits = sc.limits_now().compute(&plane.values, plane.width as usize);
    // The pixel under the pointer: the hottest.
    let hot = plane
        .values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i as u32)
        .unwrap();
    let (px, py) = (hot % w, hot / w);
    if isotherm {
        let mut sorted = plane.values.clone();
        sorted.sort_by(f32::total_cmp);
        let median = unit.to_celsius(f64::from(sorted[sorted.len() / 2]));
        sc.isotherm = Some((true, median));
        sc.thermal_prompt = Some(thermal::Prompt {
            field: thermal::Field::Reflected,
            emissivity: "0.9".into(),
            reflected: "7".into(),
        });
    }
    sc.shown = Some(req);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    assert_eq!(look.isotherm.is_some(), isotherm);
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        let zoom = (0.55 * size.0 as f32 / w as f32).min(0.9 * size.1 as f32 / ht as f32);
        let margin = 12.0 * size.0 as f32 / SIZE.0;
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            ((size.0 as f32 / 2.0 - margin) / zoom, ht as f32 / 2.0),
        )
    };
    let v = value_text(sc.planes[0].plane.value(px, py).unwrap());
    let mut lines = vec![t!("science-value-unit", value = v, unit = unit.symbol())];
    lines.extend(sc.thermal_lines());
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(sc.limits_now())
    ));
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), false, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Thermal stills: the temperatures through the camera palette with the readout (°C, 1×
/// and 1.5×), an isotherm above the median with the parameters prompt (°F), and the
/// visible photo.
#[test]
fn thermal_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-thermal",
    );
    let c = TemperatureUnit::Celsius;
    for scale in [1.0, 1.5] {
        shoot_thermal(&mut h, &mut goldens, "thermal-readout", c, false, scale);
    }
    let f = TemperatureUnit::Fahrenheit;
    shoot_thermal(&mut h, &mut goldens, "thermal-isotherm", f, true, 1.0);
    // The visible photo: the second page, an ordinary picture.
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-fmt-thermal/tests/data/rjpeg.jpg");
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("R-JPEG opens");
    let v = s.open_page(1).expect("visible photo");
    assert_eq!(
        v.page_label(),
        Some(specular_decode::PageLabel::Thermal { visible: true })
    );
    let (w, ht) = (v.info().width, v.info().height);
    let r = v
        .decode(
            &specular_decode::DecodeRequest {
                scale: 1,
                region: None,
                parallel: false,
                prepare_regions: false,
            },
            &CancelToken::new(),
        )
        .unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.upload_overview(&h.gpu, &mut img, &r);
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = View::fixed(
                (w, ht),
                Orientation::NORMAL,
                PxRect {
                    x: 0.0,
                    y: 0.0,
                    w: size.0 as f32,
                    h: size.1 as f32,
                },
                size.1 as f32 / ht as f32,
                (w as f32 / 2.0, ht as f32 / 2.0),
            );
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |_, _| {},
    );
    goldens.check("thermal-visible-1x", &shot);
    goldens.finish();
}

/// A CERN ROOT 2-D histogram (from specular-root's fixtures) in science mode through viridis,
/// y up, at the file's limits: the readout of the bin at a pixel (axis titles, ranges, bin
/// numbers), the under- and overflow note, the resampling note for variable bins, and the
/// scale bar along x named by its axis.
fn shoot_root(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    (file, object): (&str, &str),
    stretch: Stretch,
    dark: bool,
) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-root/tests/data")
        .join(file);
    let first = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("ROOT opens");
    let s = (0..first.page_count())
        .map(|i| first.open_page(i).expect("page"))
        .find(|s| {
            matches!(s.page_label(),
                Some(specular_decode::PageLabel::Object { name, .. }) if &*name == object)
        })
        .expect("the object");
    let summary = Arc::new(s.science().expect("values").clone());
    let root = summary.root.clone().expect("ROOT facts");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        stretch,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let (lo, hi) = summary.display_ranges[0];
    sc.planes = vec![SciencePlane {
        plane,
        limits: Some((lo as f32, hi as f32)),
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        let zoom = (0.8 * size.0 as f32 / w as f32).min(0.8 * size.1 as f32 / ht as f32);
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    // A pixel right of and above the centre.
    let (px, py) = (w * 3 / 5, ht * 2 / 5);
    let mut lines = vec![t!(
        "science-value",
        value = value_text(sc.planes[0].plane.value(px, py).unwrap())
    )];
    lines.extend(root_lines(&root, px, py, &sc.index));
    lines.extend(root_notes(&root));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(stretch),
        limits = t!("science-limits-file")
    ));
    let zoom = view_of((SIZE.0 as u32, SIZE.1 as u32)).scale();
    let bar = root_scale_bar(&root, f64::from(zoom)).expect("a scale bar");
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            draw_readout(dl, text, &lines, area, false, 1.0);
            draw_scale(dl, text, &pal, &bar, area, None, false, 1.0);
        },
    );
    goldens.check(&format!("{name}-1x"), &shot);
}

/// ROOT histograms: a TH2F at linear and log colour scales (with under- and overflow), and
/// a TH2D with variable bins on both axes, resampled.
#[test]
fn root_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-root",
    );
    let g = &mut goldens;
    let th2 = ("types.root", "h2f");
    shoot_root(
        &mut h,
        g,
        "root-th2-linear-dark",
        th2,
        Stretch::Linear,
        true,
    );
    shoot_root(&mut h, g, "root-th2-log-light", th2, Stretch::Log, false);
    let v = ("variable.root", "v2xy");
    shoot_root(&mut h, g, "root-variable-dark", v, Stretch::Linear, true);
    goldens.finish();
}

/// A MODIS-like HDF-EOS2 grid (sinusoidal, packed int16 with a fill value): the first
/// dataset in the default stretch with the readout's lon/lat, under the chrome's page
/// stepper (datasets are pages) and the info panel's HDF4 and HDF-EOS groups.
fn shoot_hdf4(h: &mut Harness, goldens: &mut Goldens, dark: bool, scale: f32, scroll: f32) {
    const SIZE: (f32, f32) = (760.0, 480.0);
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-hdf4/tests/data/eos_grid.hdf");
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("HDF4 opens");
    assert!(s.page_count() >= 3, "datasets are pages");
    let summary = Arc::new(s.science().expect("values").clone());
    let geo = summary.geo.clone().expect("a georeferenced grid");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let limits = sc.limits_now().compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    // The image on the start side, clear of the info panel.
    let view_of = |size: (u32, u32)| {
        let zoom = (0.33 * size.0 as f32 / w as f32).min(0.4 * size.1 as f32 / ht as f32);
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            // The image point at the target's centre: the image's centre lands at 24 %
            // across and a third down.
            (
                w as f32 / 2.0 + 0.26 * size.0 as f32 / zoom,
                ht as f32 / 2.0 + 0.17 * size.1 as f32 / zoom,
            ),
        )
    };
    // A pixel holding data near the centre.
    let (px, py) = (0..ht)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| sc.planes[0].plane.value(x, y).is_some_and(f32::is_finite))
        .min_by_key(|&(x, y)| x.abs_diff(w / 2).pow(2) + y.abs_diff(ht / 2).pow(2))
        .expect("data");
    let v = value_text(sc.planes[0].plane.value(px, py).unwrap());
    let mut lines = vec![match summary.unit_at(&sc.index).map(str::trim) {
        Some(u) => t!("science-value-unit", value = v, unit = u.to_owned()),
        None => t!("science-value", value = v),
    }];
    let geo_lines = crate::app::geo::geo_position_lines(&geo, f64::from(px), f64::from(py));
    assert!(!geo_lines.is_empty(), "lon/lat in the readout");
    lines.extend(geo_lines);
    lines.push(t!(
        "science-look",
        stretch = stretch_name(sc.stretch),
        limits = limits_name(sc.limits_now())
    ));
    let loc = crate::loader::meta_localizer(specular_i18n::I18n::global().language());
    let (report, _) = crate::loader::metadata_report(s.as_ref(), &loc);
    for id in ["hdf4", "hdf_eos"] {
        assert!(
            report.summary.iter().any(|g| g.key == id),
            "info group {id}"
        );
    }
    let page_label = s
        .page_label()
        .as_ref()
        .and_then(crate::app::pages::label_text);
    let settings = specular_settings::Settings::default();
    let store = specular_settings::SettingsStore::in_memory();
    let hud = specular_widgets::HudStats::default();
    let keymap = specular_input::Keymap::default();
    let st = specular_widgets::ViewerState {
        actions: specular_input::ActionState {
            image: specular_input::ImageState::Ready,
            count: 1,
            pages: s.page_count(),
            ..specular_input::ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 1,
        zoom: 1.0,
        at_fit: false,
        fullscreen: false,
        file_name: Some("eos_grid.hdf"),
        dims: Some((w, ht)),
        loading: false,
        error: None,
        truncated: false,
        metadata: Some(&report),
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: specular_widgets::Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: Some(specular_widgets::PageBadge {
            index: 0,
            count: s.page_count(),
            label: page_label.as_deref(),
        }),
    };
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let mut c = specular_widgets::Chrome::new();
    c.reduce_motion = true;
    c.set_size(SIZE.0, SIZE.1);
    c.poke(1.0);
    assert!(c.handle_action(Action::ToggleInfo));
    c.scroll_settings(scroll);
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            let mut warm = specular_gfx::DrawList::new(scale);
            c.draw(&mut warm, text, &st, &pal, 1.0);
            c.draw(dl, text, &st, &pal, 1.0);
            // Beside the info panel, above the page stepper.
            draw_readout(
                dl,
                text,
                &lines,
                (0.0, 0.0, 400.0, SIZE.1 - 110.0),
                false,
                1.0,
            );
        },
    );
    let theme = if dark { "dark" } else { "light" };
    goldens.check(&format!("hdf4-eos-grid-{theme}-{scale}x"), &shot);
}

/// HDF4: a MODIS-like HDF-EOS2 grid, dark at 1× and light at 1.5×.
#[test]
fn hdf4_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-hdf4",
    );
    // Scrolled to the HDF4 and HDF-EOS groups, and at the top (the grid's geospatial group).
    shoot_hdf4(&mut h, &mut goldens, true, 1.0, 440.0);
    shoot_hdf4(&mut h, &mut goldens, false, 1.5, 0.0);
    goldens.finish();
}

/// An unstructured mesh (specular-ugrid's fixtures) in science mode through its colour
/// table: the page whose values sit at `location`, with the mesh edges, or the coastlines
/// and graticule, over it, and the readout a little off the centre.
#[allow(clippy::too_many_arguments)]
fn shoot_ugrid(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    file: &str,
    location: dsci::MeshLocation,
    wireframe: bool,
    coast: bool,
    scale: f32,
) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-ugrid/tests/data")
        .join(file);
    let session = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::open(&p).expect("fixture")))
        .expect("UGRID opens");
    let s = (0..session.page_count())
        .map(|k| session.open_page(k).unwrap())
        .find(|s| {
            s.science()
                .and_then(|sc| sc.mesh.as_ref())
                .is_some_and(|m| m.facts().location == Some(location))
        })
        .expect("a page with values there");
    let summary = Arc::new(s.science().expect("values").clone());
    let mesh = summary.mesh.clone().expect("a mesh");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: vec![0; summary.axes.len()],
        stretch: Stretch::Linear,
        limits: Limits::MinMax,
        ..Default::default()
    };
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let limits = Limits::MinMax.compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            0.92 * (size.0 as f32 / w as f32).min(size.1 as f32 / ht as f32),
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let view = view_of(size_px);
    let to_screen = view.stored_from_screen().invert();
    let place = |q: (f32, f32)| {
        let s = to_screen.apply(q);
        (s.0 / scale, s.1 / scale)
    };
    let all = [0.0, 0.0, f64::from(w), f64::from(ht)];
    let edges: Vec<[(f32, f32); 2]> = if wireframe {
        let e = mesh.edges(all, MAX_MESH_EDGES).expect("few enough edges");
        assert_eq!(e.len() as u64, mesh.facts().edges);
        e.iter()
            .map(|e| [place((e[0], e[1])), place((e[2], e[3]))])
            .collect()
    } else {
        Vec::new()
    };
    let grid: Vec<GridLine> = if coast {
        let g = summary.geo.as_ref().expect("lon/lat");
        coast::geo_grid(g, w, ht)
            .into_iter()
            .map(|l| GridLine {
                pts: l.pts.into_iter().map(place).collect(),
                label: l.label,
            })
            .collect()
    } else {
        Vec::new()
    };
    // The readout a little off the image's centre, on the mesh.
    let (px, py) = (0.55 * f64::from(w), 0.45 * f64::from(ht));
    let mut lines = mesh_lines(mesh.as_ref(), px, py, &sc.index, summary.unit_at(&sc.index));
    assert_eq!(lines.len(), 2, "{lines:?}");
    if let Some(g) = &summary.geo {
        lines.extend(super::super::geo::geo_position_lines(g, px, py));
    } else if let Some(c) = &summary.pixel {
        lines.push(t!(
            "science-position",
            x = quantity_text(c.x.at(px.floor()), &c.unit_x),
            y = quantity_text(c.y.at(py.floor()), &c.unit_y)
        ));
    }
    lines.push(t!(
        "science-look",
        stretch = stretch_name(Stretch::Linear),
        limits = limits_name(Limits::MinMax)
    ));
    let image_rect = view.image_screen_rect();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let r = image_rect;
            let clip = Rect::new(r.x / scale, r.y / scale, r.w / scale, r.h / scale);
            draw_edges(dl, &edges, clip);
            draw_grid(dl, text, &grid, clip, 1.0, GRID_INK);
            draw_readout(dl, text, &lines, area, false, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Unstructured meshes: face values (triangles, quads and a pentagon) flat with the
/// wireframe, and node values interpolated across a global mesh under the coastlines and
/// graticule (at 1.5×).
#[test]
fn ugrid_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-ugrid",
    );
    let (face, node) = (dsci::MeshLocation::Face, dsci::MeshLocation::Node);
    let (g, n) = (&mut goldens, "ugrid-faces-wireframe");
    shoot_ugrid(&mut h, g, n, "mixed_fill_start1.nc", face, true, false, 1.0);
    let (g, n) = (&mut goldens, "ugrid-nodes-coast");
    shoot_ugrid(&mut h, g, n, "global_node.nc", node, false, true, 1.5);
    goldens.finish();
}

/// The geodata readout on three maps: NAD27 / UTM zone 17N (shifted to WGS 84), a datum
/// whose shift is not known (named after the coordinates), and a projection not supported
/// here (Krovak: said so, no numbers), in dark at 1× and in Hebrew light at 1×.
#[test]
fn geo_readout_matches_goldens() {
    use specular_decode::geo::specular_gis::fixtures::{self, GeoGrid};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gis-readout",
    );
    let open_grid = |epsg: u16, origin: (f64, f64), pixel: f64| {
        let vals = vec![1.0f32; 16 * 16];
        let grid = GeoGrid {
            epsg,
            origin,
            pixel: (pixel, pixel),
        };
        let tif = fixtures::geotiff_f32(&vals, 16, 16, grid, None, 16);
        let s = specular_decode::Registry::default()
            .open(Arc::new(specular_core::Source::from_vec("map.tif", tif)))
            .expect("opens");
        assert_eq!(s.info().format, specular_decode::Format::GeoTiff);
        s.geo().expect("georeferenced")
    };
    let maps = [
        open_grid(26717, (630_000.0, 4_830_000.0), 30.0),
        // Monte Mario (Rome): longitudes from Rome's meridian, no Helmert to WGS 84 in the
        // dataset.
        open_grid(4806, (0.0, 42.0), 0.01),
        open_grid(5514, (-740_000.0, -1_040_000.0), 10.0),
    ];
    for (name, dark, lang) in [
        ("gis-readout-dark-1x", true, specular_i18n::langid!("en-US")),
        (
            "gis-readout-he-light-1x",
            false,
            specular_i18n::langid!("he"),
        ),
    ] {
        specular_i18n::I18n::with_thread_language(lang, || {
            let rtl = !dark;
            let pal = if dark {
                Palette::dark()
            } else {
                Palette::light()
            };
            let mut lines = Vec::new();
            for g in &maps {
                if let Some(c) = &g.crs {
                    lines.push(c.name.clone());
                }
                lines.extend(super::super::geo::geo_position_lines(g, 8.0, 8.0));
            }
            assert!(
                lines.iter().any(|l| l == &t!("geo-unsupported")),
                "{lines:?}"
            );
            let shot = h.render(
                SIZE,
                1.0,
                |dl| {
                    dl.rect(
                        specular_gfx::Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                        0.0,
                        pal.canvas,
                    )
                },
                |dl, text| {
                    draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), rtl, 1.0);
                },
            );
            goldens.check(name, &shot);
        });
    }
    goldens.finish();
}

/// A synthetic FCS dataset: three populations of 60 000 events in FSC-A / SSC-A and a
/// fluorescence pair with spillover, from a fixed seed.
fn fcs_fixture() -> Vec<u8> {
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut uniform = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut normal = move || {
        let (a, b) = (uniform().max(1e-12), uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    };
    let pops = [
        ((60_000.0, 30_000.0), (8_000.0, 5_000.0), (50.0, 200.0)),
        ((120_000.0, 70_000.0), (12_000.0, 9_000.0), (8_000.0, 300.0)),
        (
            (90_000.0, 160_000.0),
            (10_000.0, 14_000.0),
            (400.0, 20_000.0),
        ),
    ];
    let mut data = Vec::new();
    for i in 0..60_000 {
        let ((fx, sx), (fw, sw), (a, b)) = pops[i % 3];
        let fsc = fx + fw * normal();
        let ssc = sx + sw * normal();
        let f1 = a * (1.0 + 0.3 * normal()).abs() + 30.0 * normal();
        let f2 = b * (1.0 + 0.3 * normal()).abs() + 30.0 * normal() + 0.15 * f1;
        for v in [fsc, ssc, f1, f2] {
            data.extend_from_slice(&(v as f32).to_le_bytes());
        }
    }
    let text: Vec<(&str, String)> = [
        ("$BYTEORD", "1,2,3,4"),
        ("$DATATYPE", "F"),
        ("$PAR", "4"),
        ("$TOT", "60000"),
        ("$P1N", "FSC-A"),
        ("$P1B", "32"),
        ("$P1R", "262144"),
        ("$P2N", "SSC-A"),
        ("$P2B", "32"),
        ("$P2R", "262144"),
        ("$P3N", "FITC-A"),
        ("$P3S", "CD3"),
        ("$P3B", "32"),
        ("$P3R", "262144"),
        ("$P4N", "PE-A"),
        ("$P4S", "CD19"),
        ("$P4B", "32"),
        ("$P4R", "262144"),
        ("$SPILLOVER", "2,FITC-A,PE-A,1,0.15,0.02,1"),
    ]
    .iter()
    .map(|(k, v)| (*k, (*v).to_owned()))
    .collect();
    specular_fcs::write_dataset(&text, &data)
}

/// A flow cytometry plot (synthetic FCS) in science mode's default look (log stretch over
/// the plane's least to greatest count, viridis), its axis ticks on the bottom and left
/// edges, the readout at a bin and the plane sliders.
fn shoot_fcs(h: &mut Harness, goldens: &mut Goldens, name: &str, index: &[u32], scale: f32) {
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(
            "cells.fcs",
            fcs_fixture(),
        )))
        .expect("FCS opens");
    let summary = Arc::new(s.science().expect("values").clone());
    let cy = summary.cytometry.clone().expect("cytometry facts");
    let (w, ht) = (summary.width, summary.height);
    let mut sc = Science {
        summary: Some(summary.clone()),
        index: index.to_vec(),
        ..Default::default()
    };
    assert_eq!(sc.stretch_now(), Stretch::Log);
    assert_eq!(sc.limits_now(), Limits::MinMax);
    let req = sc.request().unwrap();
    let plane = s
        .nd_plane(&req.index, &CancelToken::new())
        .unwrap()
        .unwrap();
    let limits = Limits::MinMax.compute(&plane.values, plane.width as usize);
    sc.planes = vec![SciencePlane {
        plane,
        limits,
        lut: Vec::new(),
        mid: 0.5,
    }];
    let look = sc.look().unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[sc.planes[0].plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let view_of = |size: (u32, u32)| {
        // The plot at the left, clear of the readout.
        let zoom = 0.9 * size.1 as f32 / ht as f32;
        let cx = (size.0 as f32 / 2.0 - 0.05 * size.1 as f32) / zoom;
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            (cx, ht as f32 / 2.0),
        )
    };
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let view = view_of(size_px);
    let to_screen = view.stored_from_screen().invert();
    let ticks: Vec<GridLine> = cytometry_ticks(&cy, &sc.index)
        .into_iter()
        .map(|g| GridLine {
            pts: g
                .pts
                .iter()
                .map(|&p| {
                    let q = to_screen.apply(p);
                    (q.0 / scale, q.1 / scale)
                })
                .collect(),
            label: g.label,
        })
        .collect();
    assert!(ticks.len() >= 4, "{} ticks", ticks.len());
    // The readout at the fullest bin.
    let at = sc.planes[0]
        .plane
        .values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i as u32);
    let (px, py) = (at % w, at / w);
    let count = sc.planes[0].plane.value(px, py);
    let mut lines = cytometry_lines(&cy, &sc.index, f64::from(px), f64::from(py), count);
    lines.extend(cytometry_notes(&cy, &sc.index));
    lines.push(t!(
        "science-look",
        stretch = stretch_name(sc.stretch_now()),
        limits = limits_name(sc.limits_now())
    ));
    let image_rect = view.image_screen_rect();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let r = image_rect;
            let clip = Rect::new(r.x / scale, r.y / scale, r.w / scale, r.h / scale);
            draw_grid(dl, text, &ticks, clip, 1.0, GRID_INK);
            draw_readout(dl, text, &lines, area, false, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// A Hamamatsu VMS (specular-wsi's synthetic 2 × 2 set of JPEG files with an optimisation
/// file and map) whole, from its pyramid, with the scale bar of its microns per pixel.
fn shoot_vms(h: &mut Harness, goldens: &mut Goldens, name: &str, dark: bool) {
    let dir = std::env::temp_dir().join(format!("specular-vms-shot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (set, _, _) = specular_wsi::fixtures::vms_set(true);
    let names = specular_wsi::vms::files(&set[0]).unwrap();
    std::fs::write(dir.join("s.vms"), &set[0]).unwrap();
    for (n, d) in names.iter().zip(&set[1..]) {
        std::fs::write(dir.join(n), d).unwrap();
    }
    let s = specular_decode::Registry::default()
        .open(Arc::new(
            specular_core::Source::open(dir.join("s.vms")).unwrap(),
        ))
        .expect("VMS opens");
    assert_eq!(s.info().format, specular_decode::Format::Vms);
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &CancelToken::new(),
        )
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let summary = s.science().expect("slide facts").clone();
    let (w, ht) = (r.width, r.height);
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.upload_overview(&h.gpu, &mut img, &r);
    let view_of = |size: (u32, u32)| {
        let zoom = (0.85 * size.0 as f32 / w as f32).min(0.85 * size.1 as f32 / ht as f32);
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let cal = summary.pixel.clone().expect("microns per pixel");
    let zoom = view_of((SIZE.0 as u32, SIZE.1 as u32)).scale();
    let bar = linear_scale_bar(cal.x.step.abs(), &cal.unit_x, f64::from(zoom)).expect("a bar");
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(size);
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            draw_scale(
                dl,
                text,
                &pal,
                &bar,
                (0.0, 0.0, SIZE.0, SIZE.1),
                None,
                false,
                1.0,
            );
        },
    );
    goldens.check(&format!("{name}-1x"), &shot);
}

/// FCS density plots (FSC/SSC at 1x; compensated FITC/PE on logicle at 1.5x;
/// the FITC histogram) and a Hamamatsu VMS slide.
#[test]
fn micro2_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-micro2",
    );
    let g = &mut goldens;
    shoot_fcs(&mut h, g, "fcs-fsc-ssc", &[0, 1, 0, 0, 0], 1.0);
    shoot_fcs(&mut h, g, "fcs-fitc-pe-logicle", &[2, 3, 0, 0, 0], 1.5);
    shoot_fcs(&mut h, g, "fcs-fitc-histogram", &[2, 4, 0, 0, 0], 1.0);
    shoot_vms(&mut h, g, "vms-slide-dark", true);
    goldens.finish();
}
