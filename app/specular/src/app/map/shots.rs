//! Screenshot tests of the map view: a parcels Shapefile in UTM zone 33N (generated here,
//! with its `.shx`, `.dbf` and `.prj`) with a GPX track in degrees added over it, drawn by
//! vello from the map's own scene with the layer panel's styles; the layer panel, the
//! attribute table with a selection, the readout with the scale bar, north arrow and
//! overview map, and an elevation profile. Dark at 1×, and Hebrew light (right to left).
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular map::shots`, then review the
//! rewritten PNGs.

use std::collections::BTreeSet;
use std::sync::Arc;

use specular_core::Orientation;
use specular_decode::geo::GeoRef;
use specular_decode::geo::specular_gis::fixtures;
use specular_geodesy::coords::CoordFormat;
use specular_geodesy::seam::GeoTransform;
use specular_gfx::Rect;
use specular_render::vector::VectorRenderer;
use specular_render::{PxRect, View};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use super::layers::{self, Coords, MapLayer};
use super::seam::GeoRefTransform;
use super::{furniture, measure, table, view};

const SIZE: (f32, f32) = (560.0, 460.0);

/// The parcels map: its georeference and stored size.
fn parcels() -> (Arc<GeoRef>, (u32, u32)) {
    let dir = std::env::temp_dir().join(format!("specular-map-shots-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let feats = fixtures::parcels(6, 100.0, 4, (500_000.0, 5_000_000.0));
    let p = fixtures::shapefile_polygons(&feats, fixtures::UTM33N_WKT);
    std::fs::write(dir.join("parcels.shp"), &p.shp).unwrap();
    std::fs::write(dir.join("parcels.shx"), &p.shx).unwrap();
    std::fs::write(dir.join("parcels.dbf"), &p.dbf).unwrap();
    std::fs::write(dir.join("parcels.prj"), &p.prj).unwrap();
    let s = specular_decode::Registry::default()
        .open_path(&dir.join("parcels.shp"))
        .expect("opens");
    let _ = std::fs::remove_dir_all(&dir);
    let g = s.geo().expect("georeferenced");
    (g, (s.info().width, s.info().height))
}

/// A GPX track (lon, lat) through the parcels: stored positions taken back to degrees.
fn track(geo: &dyn GeoTransform, size: (u32, u32)) -> Vec<MapLayer> {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let mut gpx = String::from(
        r#"<?xml version="1.0"?><gpx version="1.1" creator="t" xmlns="http://www.topografix.com/GPX/1/1"><trk><name>Morning walk</name><trkseg>"#,
    );
    for i in 0..=24 {
        let t = f64::from(i) / 24.0;
        let x = w * (0.1 + 0.8 * t);
        let y = h * (0.75 - 0.5 * t + 0.12 * (t * 9.0).sin());
        let (lon, lat) = geo.to_lonlat(x, y).unwrap();
        gpx.push_str(&format!(r#"<trkpt lat="{lat:.8}" lon="{lon:.8}"/>"#));
    }
    gpx.push_str("</trkseg></trk></gpx>");
    let ds =
        Arc::new(specular_decode::geo::specular_gis::gpx::read(gpx.as_bytes()).expect("a track"));
    layers::layers_of(&ds, Coords::LonLat, None, Some("walk.gpx".into()), 1)
}

struct Fixture {
    g: Arc<GeoRef>,
    geo: GeoRefTransform,
    size: (u32, u32),
    layers: Vec<MapLayer>,
    scene: Arc<specular_decode::drawing::specular_vector::prep::Prepared>,
}

fn fixture() -> Fixture {
    let (g, size) = parcels();
    let geo = GeoRefTransform(g.clone());
    let inv = g.to_map.inverse().unwrap();
    let data = g.features.as_ref().unwrap().dataset.clone();
    let mut ls = layers::layers_of(&data, Coords::Stored(inv), None, None, 0);
    let mut walk = track(&geo, size);
    walk[0].user_colour = Some([230, 60, 30]);
    walk[0].line_px = Some(2.5);
    ls.extend(walk);
    let scene = super::prepare_scene(&ls, &geo);
    Fixture {
        g,
        geo,
        size,
        layers: ls,
        scene,
    }
}

/// The view of the whole map at `size` (physical px), turned by `turn` quarter turns.
fn view_of(f: &Fixture, size: (u32, u32), orient: Orientation) -> View {
    let vp = PxRect {
        x: 0.0,
        y: 0.0,
        w: size.0 as f32,
        h: size.1 as f32,
    };
    let (w, h) = f.size;
    let (dw, dh) = if orient.swaps_axes() { (h, w) } else { (w, h) };
    let s = (size.0 as f32 / dw as f32).min(size.1 as f32 / dh as f32) * 0.95;
    View::fixed((w, h), orient, vp, s, (dw as f32 / 2.0, dh as f32 / 2.0))
}

/// Draws the map's layers as the frame does.
#[allow(clippy::too_many_arguments)]
fn render_layers(
    f: &Fixture,
    renderer: &mut Option<VectorRenderer>,
    gpu: &specular_gfx::Gpu,
    enc: &mut specular_gfx::wgpu::CommandEncoder,
    target: &specular_gfx::wgpu::TextureView,
    size: (u32, u32),
    view: &View,
    dark: bool,
    dpi: f64,
) {
    if renderer.is_none() {
        *renderer = Some(VectorRenderer::new(gpu).expect("vello"));
    }
    let mut map = super::Map {
        layers: f.layers.clone(),
        scene: Some((0, f.scene.clone())),
        ..Default::default()
    };
    let frame = view::MapFrame {
        id: 1,
        key: 0,
        dark,
        paper: true,
    };
    view::render(
        &mut map,
        renderer.as_mut().unwrap(),
        gpu,
        enc,
        target,
        size,
        view,
        &frame,
        dpi,
    );
}

/// Screen (logical px) of a stored position under `view`.
fn screen(view: &View, s: (f64, f64), scale: f32) -> (f32, f32) {
    let m = view.screen_from_stored_f64();
    let k = f64::from(scale);
    (
        ((m[0] * s.0 + m[1] * s.1 + m[2]) / k) as f32,
        ((m[3] * s.0 + m[4] * s.1 + m[5]) / k) as f32,
    )
}

/// The layer panel as the app builds it, the track chosen with its controls focused.
fn panel(f: &Fixture) -> layers::Panel {
    let n = f.layers.len();
    let rows = (0..n)
        .rev()
        .map(|i| {
            let l = &f.layers[i];
            layers::Row {
                index: i,
                name: l.shown_name(),
                colour: l.ink(),
                visible: l.visible,
                opacity: l.opacity,
                features: l.feature_count(),
                kind: l.kinds().first().copied(),
                added: l.added.is_some(),
                chosen: i == n - 1,
                focused: false,
            }
        })
        .collect();
    layers::Panel {
        rows,
        controls: view::controls_for(&f.layers[n - 1]),
        focused_control: Some(layers::Control::Width),
        confirm: None,
        scroll: 0,
        adding: false,
    }
}

#[derive(Clone, Copy)]
enum Case {
    Layers,
    Table,
    Readout,
    Profile,
}

#[allow(clippy::too_many_arguments)]
fn shoot(
    h: &mut Harness,
    goldens: &mut Goldens,
    renderer: &mut Option<VectorRenderer>,
    f: &Fixture,
    case: Case,
    name: &str,
    dark: bool,
    rtl: bool,
) {
    let scale = 1.0f32;
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let orient = match case {
        Case::Readout => Orientation::NORMAL.rotated(1),
        _ => Orientation::NORMAL,
    };
    let px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let v = view_of(f, px, orient);
    let area = (0.0, 0.0, SIZE.0, SIZE.1);
    let (w, hh) = (f64::from(f.size.0), f64::from(f.size.1));
    // The parcels' layer and its table: sorted by name, descending, one parcel selected.
    let parcels = &f.layers[0];
    let src = &parcels.data.layers[parcels.layer];
    let selected: BTreeSet<usize> = [7].into();
    let mut t = table::Table::new(0);
    t.col = 1;
    t.sort_by(1);
    t.sort_by(1);
    t.refresh(src, &selected);
    t.reveal(7, 6);
    // The selected parcel's outline on screen.
    let mut outline = Vec::new();
    if let Some(specular_decode::geo::specular_gis::Geometry::Polygon(r)) =
        &src.features[7].geometry
    {
        for p in &r[0] {
            let s = parcels.coords.to_stored(*p, &f.geo).unwrap();
            outline.push(screen(&v, s, scale));
        }
    }
    // A profile along the track over hills (heights made up from the position).
    let pts: Vec<(f64, f64)> = [0.15, 0.5, 0.85]
        .iter()
        .map(|&t| {
            let (lon, lat) = f.geo.to_lonlat(w * t, hh * (0.8 - 0.6 * t)).unwrap();
            (lat, lon)
        })
        .collect();
    let profile = measure::elevation(&pts, |lat, lon| {
        let (x, y) = f.geo.from_lonlat(lon, lat)?;
        let (u, s) = (x / w, y / hh);
        Some(420.0 + 300.0 * (1.0 - s) + 90.0 * (u * 11.0).sin() * (s * 7.0).cos())
    });
    let path: Vec<(f32, f32)> = measure::densify(&pts, false, 10.0)
        .into_iter()
        .map(|(lat, lon)| screen(&v, f.geo.from_lonlat(lon, lat).unwrap(), scale))
        .collect();
    let corners: Vec<(f32, f32)> = pts
        .iter()
        .map(|&(lat, lon)| screen(&v, f.geo.from_lonlat(lon, lat).unwrap(), scale))
        .collect();
    // The readout at the middle of a parcel, in DMS, then UTM; the scale bar at the centre.
    let at = (w * 0.42, hh * 0.38);
    let (lon, lat) = f.geo.to_lonlat(at.0, at.1).unwrap();
    // Made when drawn, in the shot's language.
    let lines = || {
        let mut lines = Vec::new();
        lines.extend(super::super::geo::geo_map_line(&f.g, at.0, at.1));
        lines.extend(furniture::position_line(lat, lon, CoordFormat::Dms));
        lines.extend(furniture::position_line(lat, lon, CoordFormat::Mgrs));
        lines.push(specular_i18n::t!(
            "geo-feature",
            name = "Parcel 14".to_owned()
        ));
        lines
    };
    let mpp = {
        let c = (px.0 as f32 / 2.0, px.1 as f32 / 2.0);
        let a = v.stored_from_screen().apply((c.0 - 50.0, c.1));
        let b = v.stored_from_screen().apply((c.0 + 50.0, c.1));
        let (la, lb) = (
            f.geo.to_lonlat(f64::from(a.0), f64::from(a.1)).unwrap(),
            f.geo.to_lonlat(f64::from(b.0), f64::from(b.1)).unwrap(),
        );
        specular_geodesy::geodesic::inverse(la.1, la.0, lb.1, lb.0).s12 / 100.0
    };
    let bar = furniture::scale_bar(mpp, specular_geodesy::units::Units::Metric).unwrap();
    // North on screen: a quarter turn puts it at the left (and the map's own convergence).
    let north = {
        let c = (w / 2.0, hh / 2.0);
        let (lon, lat) = f.geo.to_lonlat(c.0, c.1).unwrap();
        let q = f.geo.from_lonlat(lon, lat + 0.0005).unwrap();
        let (a, b) = (screen(&v, c, scale), screen(&v, q, scale));
        f64::from(b.0 - a.0).atan2(-f64::from(b.1 - a.1))
    };
    let overview = {
        let ring: Vec<(f64, f64)> = [(0.0, 0.0), (w, 0.0), (w, hh), (0.0, hh)]
            .iter()
            .map(|&s| f.geo.to_lonlat(s.0, s.1).unwrap())
            .collect();
        furniture::Overview {
            window: furniture::window_around(&ring).unwrap(),
            visible: ring.clone(),
            extent: ring,
        }
    };
    let panel = panel(f);
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let v = view_of(f, size, orient);
            render_layers(
                f,
                renderer,
                gpu,
                enc,
                target,
                size,
                &v,
                dark,
                f64::from(scale),
            );
        },
        |dl, text| match case {
            Case::Layers => {
                layers::draw_panel(dl, text, &pal, &panel, area, rtl, 1.0);
            }
            Case::Table => {
                let hl = table::highlight(&pal);
                for w2 in outline.windows(2) {
                    dl.line(w2[0], w2[1], 4.0, hl.with_alpha(0.35));
                    dl.line(w2[0], w2[1], 1.6, hl);
                }
                let tv = table::TableView {
                    table: &t,
                    layer: src,
                    name: parcels.shown_name(),
                    selected: &selected,
                    focused: true,
                    preedit: "",
                };
                table::draw(dl, text, &pal, &tv, area, rtl, 1.0);
            }
            Case::Readout => {
                crate::app::medical::draw_readout(dl, text, &lines(), area, rtl, 1.0);
                super::super::science::draw_scale(dl, text, &pal, &bar, area, None, rtl, 1.0);
                let o = furniture::draw_overview(dl, &pal, &overview, area, rtl, 1.0);
                furniture::draw_north(dl, text, &pal, north, area, Some(o), rtl, 1.0);
            }
            Case::Profile => {
                let ink = specular_theme::Color::rgba(1.0, 0.85, 0.1, 1.0);
                for w2 in path.windows(2) {
                    dl.line(
                        w2[0],
                        w2[1],
                        3.5,
                        specular_theme::Color::rgba(0.0, 0.0, 0.0, 0.55),
                    );
                    dl.line(w2[0], w2[1], 2.0, ink);
                }
                for p in &corners {
                    dl.circle(*p, 3.0, ink);
                }
                let cursor = Some(profile.samples.len() * 2 / 3);
                measure::draw_chart(
                    dl,
                    text,
                    &pal,
                    &profile,
                    cursor,
                    specular_geodesy::units::Units::Metric,
                    area,
                    None,
                    rtl,
                    1.0,
                );
            }
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

#[test]
fn map_view_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-map-view",
    );
    let f = fixture();
    // The scene: every parcel and the track, each layer's items in turn.
    assert_eq!(f.layers.len(), 2);
    assert_eq!(
        f.layers[1].added.as_deref(),
        Some(std::path::Path::new("walk.gpx"))
    );
    let mut renderer = None;
    for (case, name) in [
        (Case::Layers, "map-layers"),
        (Case::Table, "map-table"),
        (Case::Readout, "map-readout"),
        (Case::Profile, "map-profile"),
    ] {
        shoot(
            &mut h,
            &mut goldens,
            &mut renderer,
            &f,
            case,
            &format!("{name}-dark"),
            true,
            false,
        );
        specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            shoot(
                &mut h,
                &mut goldens,
                &mut renderer,
                &f,
                case,
                &format!("{name}-he-light"),
                false,
                true,
            );
        });
    }
    goldens.finish();
}

/// An elevation model (a Float32 GeoTIFF in UTM zone 33N with a no-data corner, generated
/// here) with its slope (dark, left to right) or aspect (Hebrew light, right to left)
/// colours from the GPU, contour lines with their labels, the legend and the readout's
/// terrain line.
fn shoot_terrain(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    kind: specular_render::Relief,
    dark: bool,
    rtl: bool,
) {
    use specular_decode::geo::specular_gis::fixtures::{self as fx, GeoGrid};
    use specular_render::{ChannelLook, Hillshade, ImageRenderer, ScienceLook};
    let (w, ht) = (240usize, 150usize);
    let vals = fx::dem_values(w, ht, true);
    let grid = GeoGrid {
        epsg: 32633,
        origin: (500_000.0, 5_000_000.0),
        pixel: (30.0, 30.0),
    };
    let tif = fx::geotiff_f32(&vals, w, ht, grid, Some(-9999.0), 128);
    let s = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec("dem.tif", tif)))
        .expect("opens");
    let summary = s.science().expect("values").clone();
    let geo = GeoRefTransform(summary.geo.clone().expect("georeferenced"));
    let plane = s
        .nd_plane(
            &vec![0; summary.axes.len()],
            &specular_core::CancelToken::new(),
        )
        .unwrap()
        .unwrap();
    let (lo, hi) = plane
        .values
        .iter()
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let step = super::terrain::ground_steps(&geo, w as f64 / 2.0, ht as f64 / 2.0).unwrap();
    assert!(
        (step[0] - 30.0).abs() < 0.5 && (step[1] + 30.0).abs() < 0.5,
        "{step:?}"
    );
    let look = ScienceLook {
        stretch: specular_core::nd::Stretch::Linear,
        channels: vec![ChannelLook {
            lo,
            hi,
            colour: [1.0; 3],
            on: true,
            mid: 0.5,
        }],
        luts: Vec::new(),
        colour_table: None,
        hillshade: Some(Hillshade {
            step: [step[0] as f32, step[1] as f32],
            kind,
            ..Hillshade::default()
        }),
        isotherm: None,
    };
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w as u32, ht as u32, None);
    renderer.set_value_channels(&h.gpu, &mut img, &[plane.values.as_slice()]);
    renderer.set_science(&h.gpu, &mut img, Some(look));
    let fit = |size: (u32, u32)| {
        View::fixed(
            (w as u32, ht as u32),
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
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let plane = Arc::new(plane);
    let contours = super::terrain::trace(&plane, (w as f64, ht as f64), "m").expect("contours");
    let view = fit((SIZE.0 as u32, SIZE.1 as u32));
    let cv = super::terrain::view_of(
        &contours,
        |p| screen(&view, p, 1.0),
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
    );
    assert!(!cv.labels.is_empty());
    let legend = super::terrain::relief_legend(kind).expect("a legend");
    let d = super::terrain::dem(&plane, (w as f64, ht as f64), step, 1.0);
    let (px, py) = (w as f64 * 0.6, ht as f64 * 0.4);
    let mut lines: Vec<String> = Vec::new();
    lines.extend(furniture::position_line(
        geo.to_lonlat(px, py).unwrap().1,
        geo.to_lonlat(px, py).unwrap().0,
        CoordFormat::Decimal,
    ));
    lines.extend(super::terrain::slope_line(
        &d,
        (w as f64, ht as f64),
        px,
        py,
    ));
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let v = fit(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &v,
                &specular_render::RenderOptions::default(),
            );
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            super::terrain::draw_contours(
                dl,
                text,
                &pal,
                &cv,
                Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                1.0,
            );
            super::terrain::draw_legend(dl, text, &pal, &legend, area, 12.0, rtl, 1.0);
            crate::app::medical::draw_readout(dl, text, &lines, area, rtl, 1.0);
        },
    );
    goldens.check(&format!("{name}-1x"), &shot);
}

#[test]
fn terrain_tools_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-map-terrain",
    );
    shoot_terrain(
        &mut h,
        &mut goldens,
        "map-slope-dark",
        specular_render::Relief::Slope,
        true,
        false,
    );
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        shoot_terrain(
            &mut h,
            &mut goldens,
            "map-aspect-he-light",
            specular_render::Relief::Aspect,
            false,
            true,
        );
    });
    goldens.finish();
}

#[derive(Clone, Copy)]
enum Tool {
    Time,
    Services,
    Geotag,
    Export,
}

/// The tools' panels over the parcels map: the time slider on a track with its position
/// marked and photos placed along it (with camera directions); the services panel asking
/// before the first request; the geotagging preview; the export panel.
#[allow(clippy::too_many_arguments)]
fn shoot_tool(
    h: &mut Harness,
    goldens: &mut Goldens,
    renderer: &mut Option<VectorRenderer>,
    f: &Fixture,
    tool: Tool,
    name: &str,
    dark: bool,
    rtl: bool,
) {
    use specular_geodesy::track::{Fix, Timeline};
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let px = (SIZE.0 as u32, SIZE.1 as u32);
    let v = view_of(f, px, Orientation::NORMAL);
    let area = (0.0, 0.0, SIZE.0, SIZE.1);
    let (w, hh) = (f64::from(f.size.0), f64::from(f.size.1));
    // A walk across the parcels, an hour from 08:00 UTC.
    let t0 = specular_geodesy::time::parse("2024-06-03T08:00:00Z")
        .unwrap()
        .secs;
    let fixes: Vec<Fix> = (0..=24)
        .map(|i| {
            let t = f64::from(i) / 24.0;
            let (lon, lat) = f
                .geo
                .to_lonlat(w * (0.1 + 0.8 * t), hh * (0.75 - 0.5 * t))
                .unwrap();
            Fix {
                t: t0 + 3600.0 * t,
                lat,
                lon,
                ele: Some(400.0 + 50.0 * t),
            }
        })
        .collect();
    let track = Arc::new(Timeline::new(fixes));
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let v = view_of(f, size, Orientation::NORMAL);
            render_layers(f, renderer, gpu, enc, target, size, &v, dark, 1.0);
        },
        |dl, text| match tool {
            Tool::Time => {
                let mut bar = super::timebar::TimeBar::track(track.clone());
                bar.pos = t0 + 1500.0;
                let (lat, lon) = bar.track_at().unwrap();
                let p = screen(&v, f.geo.from_lonlat(lon, lat).unwrap(), 1.0);
                dl.circle(p, 8.0, pal.accent.with_alpha(0.35));
                dl.circle(p, 4.5, pal.accent);
                for (k, dir) in [(0.2, Some(30.0)), (0.55, Some(200.0)), (0.8, None)] {
                    let (lat, lon) = track.at(t0 + 3600.0 * k).unwrap().0;
                    let q = screen(&v, f.geo.from_lonlat(lon, lat).unwrap(), 1.0);
                    super::view::draw_photo_pin(dl, q, dir, 1.0);
                }
                super::timebar::draw(dl, text, &pal, &bar, true, area, None, rtl, 1.0);
            }
            Tool::Services => {
                let mut s = super::services::Services {
                    servers: Some(vec![super::services::Server {
                        id: "s1".into(),
                        name: "Swiss geodata".into(),
                        url: "https://wms.geo.example/ows".into(),
                        kind: Some(specular_map::ogc::Kind::Wms),
                    }]),
                    ..Default::default()
                };
                s.go(super::services::Page::List);
                s.go(super::services::Page::Consent {
                    host: "wms.geo.example".into(),
                    then: super::services::Pending::Describe(0),
                });
                let view = s.view(true).unwrap();
                super::services::draw(dl, text, &pal, &view, "", area, 12.0, rtl, 1.0);
            }
            Tool::Geotag => {
                use super::photos::{Outcome, Planned};
                let plan: Vec<Planned> = [
                    (
                        "IMG_0101.JPG",
                        Outcome::Write {
                            lat: 45.1234,
                            lon: 15.2345,
                            ele: Some(420.0),
                            fix: t0,
                        },
                    ),
                    (
                        "IMG_0102.JPG",
                        Outcome::Write {
                            lat: 45.1301,
                            lon: 15.2402,
                            ele: None,
                            fix: t0,
                        },
                    ),
                    ("IMG_0103.CR3", Outcome::HasPosition),
                    ("IMG_0104.JPG", Outcome::Outside(5400.0)),
                    ("scan.png", Outcome::NoTime),
                ]
                .into_iter()
                .map(|(n, o)| Planned {
                    path: n.into(),
                    sidecar: (format!("{n}.xmp").into(), false),
                    taken: Some(t0),
                    outcome: o,
                })
                .collect();
                let g = super::tools::Geotag {
                    plan: Arc::new(plan),
                    scroll: 0,
                    writing: false,
                };
                let m = super::Map {
                    t: super::tools::Tools {
                        offset: 3600.0,
                        scan: Some(Arc::new(Vec::new())),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let view = super::view::geotag_view(&m, &g, true);
                super::services::draw(dl, text, &pal, &view, "", area, 12.0, rtl, 1.0);
            }
            Tool::Export => {
                let p = super::export::Panel {
                    print: false,
                    format: super::export::Format::Pdf,
                    scale: 25_000.0,
                    dpi: 300,
                    paper: specular_print::layout::Paper::A4,
                    row: 1,
                    status: None,
                };
                let l = super::export::layout(&p, (520.0, 400.0), 10.0).ok();
                let view = p.view(l, true);
                super::services::draw(dl, text, &pal, &view, "", area, 12.0, rtl, 1.0);
            }
        },
    );
    goldens.check(&format!("{name}-1x"), &shot);
}

#[test]
fn tool_panels_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-map-tools",
    );
    let f = fixture();
    let mut renderer = None;
    for (tool, name) in [
        (Tool::Time, "map-time"),
        (Tool::Services, "map-services"),
        (Tool::Geotag, "map-geotag"),
        (Tool::Export, "map-export"),
    ] {
        shoot_tool(
            &mut h,
            &mut goldens,
            &mut renderer,
            &f,
            tool,
            &format!("{name}-dark"),
            true,
            false,
        );
        specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            shoot_tool(
                &mut h,
                &mut goldens,
                &mut renderer,
                &f,
                tool,
                &format!("{name}-he-light"),
                false,
                true,
            );
        });
    }
    goldens.finish();
}

/// The export's furniture strip: title, scale and CRS, scale bar, north arrow, legend and
/// attribution, as it is printed (white paper, dark ink), left to right and mirrored.
#[test]
fn export_furniture_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-map-export",
    );
    // Built in the shot's language, so the mirrored strip is in Hebrew.
    let furniture = || super::export::Furniture {
        title: "parcels".into(),
        subtitle: specular_i18n::t!(
            "map-export-subtitle",
            scale = super::export::scale_text(25_000.0),
            crs = "WGS 84 / UTM zone 33N".to_owned()
        ),
        bar: super::export::bar_for(
            25_000.0 * 0.0254 / 96.0,
            540.0,
            specular_geodesy::units::Units::Metric,
        ),
        north: Some(0.12),
        legend: vec![
            ([230, 60, 30], "walk.gpx".into()),
            ([40, 110, 200], "parcels".into()),
        ],
        ramp: super::terrain::relief_legend(specular_render::Relief::Slope),
        attribution: specular_i18n::t!(
            "map-export-data",
            sources = "parcels · walk.gpx".to_owned()
        ),
    };
    for (name, rtl) in [("map-export-strip", false), ("map-export-strip-he", true)] {
        let run = |h: &mut Harness| {
            h.render(
                (SIZE.0, 100.0),
                1.0,
                |dl| {
                    dl.rect(
                        Rect::new(0.0, 0.0, SIZE.0, 100.0),
                        0.0,
                        specular_theme::Color::hex(0xffffff),
                    )
                },
                |dl, text| {
                    super::export::draw_furniture(
                        dl,
                        text,
                        &furniture(),
                        Rect::new(0.0, 4.0, SIZE.0, 92.0),
                        rtl,
                    );
                },
            )
        };
        let shot = if rtl {
            specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || run(&mut h))
        } else {
            run(&mut h)
        };
        goldens.check(&format!("{name}-1x"), &shot);
    }
    goldens.finish();
}

/// The export renders through the viewer's renderers in tiles: the parcels map (vector
/// layers) at 1:2,500 as an A4 PDF page and as a PNG; the seams between small tiles leave
/// no trace (the same pixels as one tile); the furniture strip has ink; the files are a
/// PDF and a PNG that records its resolution. Prints the time taken.
#[test]
fn export_renders_in_tiles() {
    use super::export::{self, Format, Panel};
    let Some(h) = Harness::new() else { return };
    let f = fixture();
    let mut vectors = VectorRenderer::new(&h.gpu).ok();
    let mut images = specular_render::ImageRenderer::new(&h.gpu, 32 << 20);
    let mut map = super::Map {
        layers: f.layers.clone(),
        scene: Some((0, f.scene.clone())),
        vector: true,
        ..Default::default()
    };
    // Ground metres per stored pixel at the centre.
    let (w, hh) = (f64::from(f.size.0), f64::from(f.size.1));
    let [ex, ny] = super::terrain::ground_steps(&f.geo, w / 2.0, hh / 2.0).unwrap();
    let m_stored = (ex.abs() + ny.abs()) / 2.0;
    for (format, dpi) in [(Format::Png, 150), (Format::Pdf, 100)] {
        let p = Panel {
            print: false,
            format,
            scale: 2500.0,
            dpi,
            paper: specular_print::layout::Paper::A4,
            row: 0,
            status: None,
        };
        let l = export::layout(&p, (w, hh), m_stored).unwrap();
        let furniture = export::Furniture {
            title: "parcels".into(),
            subtitle: export::scale_text(p.scale),
            bar: export::bar_for(
                p.scale * 0.0254 / 96.0,
                l.strip.2 as f32 * 96.0 / dpi as f32,
                specular_geodesy::units::Units::Metric,
            ),
            north: Some(0.0),
            legend: f
                .layers
                .iter()
                .map(|ly| (ly.ink(), ly.shown_name()))
                .collect(),
            ramp: None,
            attribution: "Data: parcels".into(),
        };
        let scene = export::Scene {
            stored: f.size,
            orient: Orientation::NORMAL,
            centre: (w as f32 / 2.0, hh as f32 / 2.0),
            img: None,
            opts: specular_render::RenderOptions::default(),
            frame: Some(view::MapFrame {
                id: 1,
                key: 0,
                dark: false,
                paper: true,
            }),
            contours: None,
            arrows: Vec::new(),
            pins: Vec::new(),
            furniture,
            rtl: false,
            dpi,
        };
        let mut kit = export::Kit::new(&h.gpu, specular_gfx::Text::bundled_only());
        let t0 = std::time::Instant::now();
        let one = export::render_tiles(
            &h.gpu,
            &mut images,
            &mut map,
            vectors.as_mut(),
            &l,
            &scene,
            &mut kit,
            8192,
        )
        .unwrap();
        let t_one = t0.elapsed();
        let tiled = export::render_tiles(
            &h.gpu,
            &mut images,
            &mut map,
            vectors.as_mut(),
            &l,
            &scene,
            &mut kit,
            300,
        )
        .unwrap();
        println!(
            "export {:?} {}×{} at {dpi} dpi: {:.0} ms in one tile",
            format,
            l.px.0,
            l.px.1,
            t_one.as_secs_f64() * 1e3
        );
        assert_eq!(one.len(), (l.px.0 * l.px.1 * 4) as usize);
        // Tiles join without seams: at most a few pixels differ (antialiasing).
        let differ = one
            .chunks_exact(4)
            .zip(tiled.chunks_exact(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 16))
            .count();
        if differ * 2000 >= one.len() / 4 {
            let (mut ys, mut xs) = (
                std::collections::BTreeMap::new(),
                std::collections::BTreeMap::new(),
            );
            for (i, (a, b)) in one.chunks_exact(4).zip(tiled.chunks_exact(4)).enumerate() {
                if a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 16) {
                    *ys.entry(i as u32 / l.px.0).or_insert(0) += 1;
                    *xs.entry(i as u32 % l.px.0).or_insert(0) += 1;
                }
            }
            println!(
                "rows {:?}",
                ys.iter().filter(|(_, n)| **n > 5).collect::<Vec<_>>()
            );
            println!(
                "cols {:?}",
                xs.iter().filter(|(_, n)| **n > 5).collect::<Vec<_>>()
            );
        }
        assert!(
            differ * 2000 < one.len() / 4,
            "{differ} pixels differ between tilings"
        );
        // The map frame has the parcels' ink; the strip has text.
        let px = |x: u32, y: u32| {
            let i = ((y * l.px.0 + x) * 4) as usize;
            [one[i], one[i + 1], one[i + 2]]
        };
        let inked = |x0: u32, y0: u32, x1: u32, y1: u32| {
            (y0..y1).step_by(3).any(|y| {
                (x0..x1)
                    .step_by(3)
                    .any(|x| px(x, y).iter().any(|c| *c < 128))
            })
        };
        let (fx, fy, fw, fh) = l.frame;
        assert!(inked(fx, fy, fx + fw, fy + fh), "the map is drawn");
        let (sx, sy, sw, sh) = l.strip;
        assert!(inked(sx, sy, sx + sw, sy + sh), "the furniture is drawn");
        let dir = std::env::temp_dir().join(format!("specular-map-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(if format == Format::Pdf {
            "m.pdf"
        } else {
            "m.png"
        });
        export::write_file(&path, &l, &one, dpi, "parcels", format == Format::Pdf).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        if format == Format::Pdf {
            assert!(bytes.starts_with(b"%PDF-"));
            assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
            let a4 = if w > hh { (1169, 827) } else { (827, 1169) };
            assert_eq!(l.px, a4, "A4 at 100 dpi, turned like the map");
        } else {
            assert!(bytes.starts_with(b"\x89PNG"));
            assert!(bytes.windows(4).any(|w| w == b"pHYs"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The map's panels through [`view::draw`] and [`view::a11y`], as the app builds them: the
/// layer panel, the attribute table with a selection, Go to, the measuring result, the
/// identify card, the overview, north arrow and scale; then the elevation chart with the
/// time slider and the services panel, the geotagging preview and the export panel. Each
/// passes the screen-reader audit, left to right and in Hebrew.
#[test]
fn map_panels_pass_the_audit() {
    use specular_geodesy::track::{Fix, Timeline};
    let Some(mut h) = Harness::new() else { return };
    let f = fixture();
    let (w, hh) = (f64::from(f.size.0), f64::from(f.size.1));
    let t0 = specular_geodesy::time::parse("2024-06-03T08:00:00Z")
        .unwrap()
        .secs;
    let fixes: Vec<Fix> = (0..=24)
        .map(|i| {
            let t = f64::from(i) / 24.0;
            let (lon, lat) = f
                .geo
                .to_lonlat(w * (0.1 + 0.8 * t), hh * (0.75 - 0.5 * t))
                .unwrap();
            Fix {
                t: t0 + 3600.0 * t,
                lat,
                lon,
                ele: Some(400.0 + 50.0 * t),
            }
        })
        .collect();
    let track = Arc::new(Timeline::new(fixes));
    let pts: Vec<(f64, f64)> = [0.15, 0.5, 0.85]
        .iter()
        .map(|&t| {
            let (lon, lat) = f.geo.to_lonlat(w * t, hh * (0.8 - 0.6 * t)).unwrap();
            (lat, lon)
        })
        .collect();
    let profile = measure::elevation(&pts, |lat, _| Some(400.0 + 1000.0 * (lat - 45.0)));
    let ring: Vec<(f64, f64)> = [(0.0, 0.0), (w, 0.0), (w, hh), (0.0, hh)]
        .iter()
        .map(|&s| f.geo.to_lonlat(s.0, s.1).unwrap())
        .collect();
    for case in 0..4 {
        for (lang, rtl) in [
            (specular_i18n::langid!("en-US"), false),
            (specular_i18n::langid!("he"), true),
        ] {
            specular_i18n::I18n::with_thread_language(lang, || {
                let mut map = super::Map {
                    layers: f.layers.clone(),
                    selected: [(0, 7)].into(),
                    ..Default::default()
                };
                let mut v = view::MapView::default();
                let name = match case {
                    0 => {
                        let src = &f.layers[0].data.layers[f.layers[0].layer];
                        let mut t = table::Table::new(0);
                        t.sort_by(1);
                        t.refresh(src, &[7].into());
                        t.reveal(7, 6);
                        map.table = Some(t);
                        map.focus = Some(super::Focus::Table);
                        v.table_name = f.layers[0].shown_name();
                        v.panel = Some(panel(&f));
                        v.goto = Some(furniture::GoTo {
                            text: "45.1, 15.2".into(),
                            error: Some("Not a coordinate".into()),
                        });
                        v.tool = Some(view::ToolView {
                            title: specular_i18n::t!("palette-map-measure-distance"),
                            lines: vec!["1.24 km".into(), "3 points".into()],
                            path: Vec::new(),
                            closed: false,
                            points: Vec::new(),
                            cross: None,
                            focused: true,
                        });
                        v.identify = Some(super::Identify {
                            title: "Parcel 14".into(),
                            rows: vec![
                                ("name".into(), "Parcel 14".into()),
                                ("area".into(), "1204".into()),
                            ],
                            more: 2,
                        });
                        v.overview = Some(furniture::Overview {
                            window: furniture::window_around(&ring).unwrap(),
                            visible: ring.clone(),
                            extent: ring.clone(),
                        });
                        v.north = Some(0.3);
                        v.scale = Some("100 m".into());
                        "map-panels"
                    }
                    1 => {
                        v.chart = Some((
                            profile.clone(),
                            Some(5),
                            specular_geodesy::units::Units::Metric,
                        ));
                        let mut bar = super::timebar::TimeBar::track(track.clone());
                        bar.pos = t0 + 1500.0;
                        v.time = Some((bar, true));
                        let mut s = super::services::Services {
                            servers: Some(vec![super::services::Server {
                                id: "s1".into(),
                                name: "Swiss geodata".into(),
                                url: "https://wms.geo.example/ows".into(),
                                kind: Some(specular_map::ogc::Kind::Wms),
                            }]),
                            ..Default::default()
                        };
                        s.go(super::services::Page::List);
                        v.services = s.view(true);
                        "map-time-services"
                    }
                    2 => {
                        use super::photos::{Outcome, Planned};
                        let plan: Vec<Planned> = [
                            (
                                "IMG_0101.JPG",
                                Outcome::Write {
                                    lat: 45.1234,
                                    lon: 15.2345,
                                    ele: Some(420.0),
                                    fix: t0,
                                },
                            ),
                            ("IMG_0103.CR3", Outcome::HasPosition),
                            ("scan.png", Outcome::NoTime),
                        ]
                        .into_iter()
                        .map(|(n, o)| Planned {
                            path: n.into(),
                            sidecar: (format!("{n}.xmp").into(), false),
                            taken: Some(t0),
                            outcome: o,
                        })
                        .collect();
                        let g = super::tools::Geotag {
                            plan: Arc::new(plan),
                            scroll: 0,
                            writing: false,
                        };
                        map.t = super::tools::Tools {
                            offset: 3600.0,
                            scan: Some(Arc::new(Vec::new())),
                            ..Default::default()
                        };
                        v.geotag = Some(view::geotag_view(&map, &g, true));
                        "map-geotag"
                    }
                    _ => {
                        let p = super::export::Panel {
                            print: false,
                            format: super::export::Format::Pdf,
                            scale: 25_000.0,
                            dpi: 300,
                            paper: specular_print::layout::Paper::A4,
                            row: 1,
                            status: None,
                        };
                        let l = super::export::layout(&p, (520.0, 400.0), 10.0).ok();
                        v.export = Some(p.view(l, true));
                        "map-export"
                    }
                };
                let pal = Palette::dark();
                let mut drawn = None;
                h.render(
                    SIZE,
                    1.0,
                    |_| {},
                    |dl, text| {
                        drawn = Some(view::draw(
                            dl,
                            text,
                            &pal,
                            &map,
                            &v,
                            (0.0, 0.0, SIZE.0, SIZE.1),
                            rtl,
                            1.0,
                        ));
                    },
                );
                map.drawn = drawn.expect("drawn");
                let name = format!("{name}{}", if rtl { "-he" } else { "" });
                crate::app::a11y_audit::audit_screen(&name, SIZE, rtl, |t| {
                    view::a11y(t, &map, &v, 1.0);
                });
            });
        }
    }
}
