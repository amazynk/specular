//! Every Develop control acts: for a raw (DNG) and a JPEG chart built to show each one
//! (colour fringes at hard edges, stuck pixels, clipped highlights, noise, vignetting, fine
//! stripes, colour patches, a gradient), each slider, choice, wheel and the curve is moved
//! far in a fresh session, the way the panel reports it, and the developed picture must
//! change by more than 0.01 somewhere, or by 1/255 on 2 % of it; one move is one history
//! step. A control dimmed in the fresh session (see `prereq.rs`) gets what its reason
//! asks for first and must then no longer be dimmed. Controls exempt by design are listed
//! in [`exempt`] with why.
//!
//! The luma graph is checked too: the scopes of each render follow the newest picture.

use std::path::{Path, PathBuf};

use specular_develop::gpu::pool::Budget;
use specular_develop::model::History;
use specular_develop::ops::output::OutputSpace;
use specular_develop::ops::view;
use specular_develop::preview::{Developer, Input};
use specular_raw::sensor::{BlackLevels, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};
use specular_widgets::develop_panel::{Body, Event};

use super::controls::{self, Session, id};
use super::diffuse::id as did;

const W: u32 = 960;
const H: u32 = 640;

/// A deterministic hash in 0..1.
fn hash(x: u32, y: u32, k: u32) -> f32 {
    let mut h =
        x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ k.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let k = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [k(5.0), k(3.0), k(1.0)]
}

/// The chart in linear Rec.2020 at (x, y), sampled at fractional positions (the raw's
/// lateral colour shifts the red and blue planes).
fn scene(x: f32, y: f32) -> [f32; 3] {
    let (u, v) = (x / W as f32, y / H as f32);
    let mut c = if v < 0.22 {
        // Eight colour patches around the hue circle, two strengths.
        let i = (u * 8.0).floor().clamp(0.0, 7.0);
        let s = if v < 0.11 { 0.75 } else { 0.4 };
        hsv(i / 8.0 + 0.02, s, 0.45)
    } else if (0.32..0.68).contains(&v) && (0.08..0.42).contains(&u) {
        // A blown-out window: clipped on the sensor, warm so channels clip apart, with a
        // core where every channel clips (a sun).
        if (0.42..0.58).contains(&v) && (0.18..0.32).contains(&u) {
            [40.0, 40.0, 40.0]
        } else {
            [6.0, 4.0, 2.5]
        }
    } else if (0.32..0.68).contains(&v) && (0.55..0.92).contains(&u) {
        // Fine stripes, two pixels each, and a grey step.
        let s = (x as u32 / 2).is_multiple_of(2);
        let g = if s { 0.55 } else { 0.06 };
        [g, g, g]
    } else {
        // A gradient, dark to bright, slightly warm.
        // Logarithmic: from 11 stops under mid grey up, so every tone zone has pixels.
        let g = 0.18 * (u * 12.0 - 11.0).exp2();
        [g * 1.05, g, g * 0.92]
    };
    // Purple and green fringes beside the window's sides.
    if (0.32..0.68).contains(&v) {
        let l = 0.08 * W as f32;
        let r = 0.42 * W as f32;
        if (l - 3.0..l).contains(&x) {
            c = [0.30, 0.04, 0.42];
        }
        if (r..r + 3.0).contains(&x) {
            c = [0.05, 0.32, 0.08];
        }
    }
    // Vignetting.
    let (dx, dy) = (u - 0.5, (v - 0.5) * H as f32 / W as f32);
    let f = 1.0 - 1.2 * (dx * dx + dy * dy);
    c.map(|k| k * f)
}

/// Stuck pixels: position and how many times its neighbours (dark part, under the window).
const STUCK: [(u32, u32, f32); 4] = [
    (620, 530, 1.8),
    (700, 540, 3.5),
    (780, 520, 9.0),
    (860, 560, 40.0),
];

fn noisy(c: [f32; 3], x: u32, y: u32) -> [f32; 3] {
    std::array::from_fn(|k| {
        let n = hash(x, y, k as u32) - 0.5;
        (c[k] + n * 0.1 * c[k].max(0.0).sqrt()).max(0.0)
    })
}

/// The non-raw chart (linear Rec.2020 RGBA).
fn rgb_input() -> Input {
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            // Lateral colour as on the raw: red and blue scaled about the centre.
            let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);
            let at = |k: f32| {
                scene(
                    cx + (x as f32 + 0.5 - cx) * k,
                    cy + (y as f32 + 0.5 - cy) * k,
                )
            };
            let c = [at(1.003)[0], at(1.0)[1], at(0.997)[2]];
            let mut c = noisy(c, x, y);
            if let Some(s) = STUCK.iter().find(|s| s.0 == x && s.1 == y) {
                c = c.map(|k| k * s.2);
            }
            rgba.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    Input::Rgb {
        width: W,
        height: H,
        rgba,
    }
}

/// The raw chart: an RGGB sensor (black 512, white 15000) with lateral colour (red and blue
/// planes scaled about the centre), clipped highlights, noise and stuck samples.
fn raw_input() -> Input {
    let (black, white) = (512.0f32, 15000.0f32);
    let layout = Layout::Bayer([0, 1, 1, 2]);
    let wb = [1.9f32, 1.0, 1.4];
    let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);
    let mut samples = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let col = usize::from(layout.color(x, y));
            let k = [1.003f32, 1.0, 0.997][col];
            let (sx, sy) = (
                cx + (x as f32 + 0.5 - cx) * k,
                cy + (y as f32 + 0.5 - cy) * k,
            );
            let mut v = noisy(scene(sx, sy), x, y)[col] / wb[col] * 0.5;
            if let Some(s) = STUCK.iter().find(|s| s.0 == x && s.1 == y) {
                v *= s.2;
            }
            samples.push((black + v * (white - black)).round().clamp(0.0, white) as u16);
        }
    }
    Input::Raw(Box::new(SensorData {
        width: W,
        height: H,
        cpp: 1,
        samples,
        float: None,
        crop: Rect {
            x: 0,
            y: 0,
            w: W,
            h: H,
        },
        meta: SensorMeta {
            layout,
            black: BlackLevels::simple([black; 4], 1),
            white,
            clip_white: white,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb,
            wb_source: WbSource::AsShot,
            rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.4, -0.2], [0.0, -0.5, 1.5]],
            matrix_source: MatrixSource::Generic,
            dng: None,
            profile: None,
            orientation: 1,
            truncated: false,
            lens: None,
            default_scale: None,
        },
    }))
}

/// Why a control is not moved here (`None`: it is).
fn exempt(id: u32, body: &Body) -> Option<&'static str> {
    use super::depth_tools::id as dt;
    Some(match (id, body) {
        (_, Body::Choice(c)) if c.is_button() => {
            "a button: opens a picker or dialog, or turns an on-picture tool on"
        }
        (_, Body::Note(_)) => "text to read",
        (id::SDR_PREVIEW, _) => "a way of looking at an HDR display, not an edit",
        (id::NOISE_MODE, _) => "AI denoise needs a downloaded model (ai_denoise.rs tests)",
        (id::LENS_ON, _) => "the lens profile's switch: the test photos match no profile",
        (id::FLAT_MODE | id::FLAT_USE, _) => {
            "needs flat or dark frames on disk (ops/correct/calib tests)"
        }
        (id::FRAME_BORDER..=id::FRAME_SHOW, _) => {
            "drawn around the developed picture by the frame preview and export (effects tests)"
        }
        (dt::RL_NEAR..=319, _) => "needs the Depth add-on's map (depth and removal tests)",
        (super::perspective::id::MODE, _) => {
            "upright reads the picture's lines on request (perspective tests)"
        }
        (id::CROP_ASPECT, _) => {
            "the application crops with the photo's frame on screen (crop.rs tests)"
        }
        (super::perspective::id::AUTO_SCALE, _) => {
            "fills only when a keystone leaves empty edges (perspective tests)"
        }
        (c, _) if super::masks::is_mask(c) => "sets the selected mask (masks.rs tests)",
        (c, _) if super::retouch::is_retouch(c) => {
            "sets the next or selected spot (retouch.rs tests)"
        }
        _ => return None,
    })
}

/// A cube LUT that is far from the identity (a warm cross-process), written once.
fn lut_file() -> PathBuf {
    let p = std::env::temp_dir().join(format!("specular-acts-{}.cube", std::process::id()));
    if !p.exists() {
        let n = 5;
        let mut s = format!("LUT_3D_SIZE {n}\n");
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let f = |i: usize| i as f32 / (n - 1) as f32;
                    let (r, g, b) = (f(r), f(g), f(b));
                    // Channels mixed (not one curve per channel): tetrahedral and
                    // trilinear interpolation differ on it.
                    let (o0, o1, o2) = ((g * b).sqrt(), (r * b).max(g * g), 1.0 - r * g);
                    s.push_str(&format!("{o0} {o1} {o2}\n"));
                }
            }
        }
        std::fs::write(&p, s).expect("write the LUT");
    }
    p
}

struct Rig {
    dev: Developer,
    t: i64,
}

impl Rig {
    /// The developed picture of a session's edit (`peak`: the display's, 1 for SDR).
    fn render(&mut self, s: &Session, peak: f32) -> (u32, u32, Vec<f32>) {
        let e = s.shown_edit();
        let headroom = view::headroom(&e, peak);
        let shown = view::for_headroom(&e, headroom);
        let out = OutputSpace {
            peak: headroom,
            ..OutputSpace::rec709()
        };
        self.dev.prepare(&shown, W).expect("prepare");
        let r = self.dev.render(&shown, None, &out).expect("render");
        let px = self.dev.read(&r).expect("read back");
        let size = (r.roi.w, r.roi.h);
        self.dev.recycle(r);
        (size.0, size.1, px)
    }

    fn send(&mut self, s: &mut Session, control: u32, part: u8) {
        self.t += 5_000;
        s.handle(
            &[Event::Value {
                control,
                part,
                done: true,
            }],
            self.t,
        );
    }
}

/// How far two pictures are apart: the largest channel difference (sizes differ: 1).
fn apart(a: &(u32, u32, Vec<f32>), b: &(u32, u32, Vec<f32>)) -> f32 {
    if (a.0, a.1) != (b.0, b.1) {
        return 1.0;
    }
    let d: Vec<f32> =
        a.2.chunks_exact(4)
            .zip(b.2.chunks_exact(4))
            .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f32::max))
            .collect();
    // A small change over a wide area counts too (denoise): a step of 1/255 or more on
    // at least 2 % of the picture.
    let wide = d.iter().filter(|&&v| v >= 1.0 / 255.0).count() * 50 >= d.len();
    let most = d.iter().copied().fold(0.0, f32::max);
    if wide { most.max(CHANGED) } else { most }
}

/// A change this small is not one (two renders of one edit agree far closer).
const CHANGED: f32 = 0.01;

/// What a dimmed control's reason asks for, done as the user would.
fn meet(rig: &mut Rig, s: &mut Session, control: u32) {
    // The card turned on.
    let card = s
        .panel
        .cards
        .iter()
        .find(|c| c.controls.iter().any(|k| k.id == control))
        .map(|c| (c.id, c.enabled));
    if let Some((card, Some(false))) = card {
        if let Some(c) = s.panel.card_mut(card) {
            c.enabled = Some(true);
        }
        rig.t += 5_000;
        s.handle(&[Event::Enabled { card, on: true }], rig.t);
    }
    // Diffuse's settings act on what its preset diffuses: Deblur sets every speed and
    // anisotropy; highlights fill only with Inpaint highlights.
    if super::diffuse::is_diffuse(control) && !matches!(control, did::PRESET | did::NOTE) {
        let p = if control == did::HIGHLIGHTS { 5 } else { 2 };
        if let Some(Body::Choice(ch)) = s.panel.control_mut(did::PRESET).map(|k| &mut k.body) {
            ch.set(p);
        }
        rig.send(s, did::PRESET, 0);
    }
    for _ in 0..3 {
        if s.panel.dimmed_reason(control).is_none() {
            return;
        }
        let picked = s
            .edit
            .module(specular_develop::model::ModuleKind::POINT_COLOUR, 0)
            .is_some_and(|m| m.params.to_fields().bool_or("picked", false));
        match control {
            id::POINT_RANGE..=id::POINT_LIGHTNESS if !picked => {
                // The red patch.
                rig.t += 5_000;
                s.set_point_target(scene(0.5 * W as f32 / 8.0, 4.0).map(f64::from), rig.t);
                s.sync_panel();
            }
            id::POINT_RANGE..=id::POINT_LIGHTNESS => {
                s.panel.slider_mut(id::POINT_HUE).expect("hue").set(40.0);
                rig.send(s, id::POINT_HUE, 0);
            }
            id::LUT_SPACE | id::LUT_INTERPOLATION | id::LUT_STRENGTH => {
                rig.t += 5_000;
                s.set_lut_path(&lut_file(), rig.t);
                s.sync_panel();
            }
            id::SMOOTHING => {
                s.panel.slider_mut(id::SHADOWS).expect("shadows").set(50.0);
                rig.send(s, id::SHADOWS, 0);
            }
            _ => return,
        }
    }
}

/// A fresh session of `path` with `control` ready to move (its reason met when dimmed).
fn ready(rig: &mut Rig, path: &Path, control: u32) -> Session {
    let mut s = Session::new(
        path.to_owned(),
        controls::new_edit(path),
        History::default(),
    );
    meet(rig, &mut s, control);
    s
}

/// What a control is moved to, one variant at a time: (part, how).
enum Move {
    /// A slider to the end farther from its value in the ready session, then (when that
    /// end does nothing, such as a midpoint at its end) to the other end.
    Far,
    Slider(f32),
    Option(usize),
    Wheel(usize),
    Curve,
}

fn moves(body: &Body) -> Vec<(u8, Move)> {
    match body {
        Body::Slider(_) => vec![(0, Move::Far)],
        Body::Choice(c) => (0..c.options.len())
            .filter(|&i| i != c.selected)
            .map(|i| (0, Move::Option(i)))
            .collect(),
        Body::Wheels(ws) => (0..ws.len())
            .flat_map(|i| {
                [
                    (2 * i as u8, Move::Wheel(i)),
                    (2 * i as u8 + 1, Move::Slider(1.0)),
                ]
            })
            .collect(),
        Body::Curve(_) => vec![(0, Move::Curve)],
        Body::Note(_) => Vec::new(),
    }
}

/// A slider's ends other than its value, the farther first.
fn ends(s: &Session, control: u32) -> Vec<f32> {
    match s.panel.control(control).map(|c| &c.body) {
        Some(Body::Slider(sl)) => {
            let e = if (sl.spec.max - sl.value).abs() >= (sl.value - sl.spec.min).abs() {
                [sl.spec.max, sl.spec.min]
            } else {
                [sl.spec.min, sl.spec.max]
            };
            e.into_iter().filter(|&v| v != sl.value).collect()
        }
        _ => Vec::new(),
    }
}

fn apply(s: &mut Session, control: u32, part: u8, m: &Move) {
    let Some(ctl) = s.panel.control_mut(control) else {
        return;
    };
    match (&mut ctl.body, m) {
        (Body::Slider(sl), Move::Slider(v)) => {
            sl.set(*v);
        }
        (Body::Choice(c), Move::Option(i)) => {
            c.set(*i);
        }
        (Body::Wheels(ws), Move::Wheel(i)) => {
            ws[*i].0.set(30.0, 0.8);
        }
        (Body::Wheels(ws), Move::Slider(v)) => {
            ws[usize::from(part / 2)].1.set(*v);
        }
        (Body::Curve(c), Move::Curve) => {
            c.set_points(&[[0.0, 0.0], [0.3, 0.6], [1.0, 1.0]]);
        }
        _ => {}
    }
}

/// The app's GPU, or `None` to skip (without SPECULAR_REQUIRE_GPU).
fn gpu() -> Option<specular_gfx::Gpu> {
    match specular_gfx::Gpu::new() {
        Ok(g) => Some(g),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no GPU: {e}"),
    }
}

#[test]
fn every_develop_control_changes_the_picture() {
    let Some(gpu) = gpu() else { return };
    let mut rig = Rig {
        dev: Developer::new(
            &gpu.device,
            &gpu.queue,
            Budget::for_device(true, None, None),
        ),
        t: 1_000_000,
    };
    rig.dev.crop_on = true;
    rig.dev.wait_highlight_stats = true;
    rig.dev.set_lens(None, None);
    let mut failed = Vec::new();
    let mut exempted = std::collections::BTreeMap::<&str, Vec<u32>>::new();
    let mut moved = 0;
    for (path, input) in [
        ("/charts/chart.dng", raw_input()),
        ("/charts/chart.jpg", rgb_input()),
    ] {
        let path = PathBuf::from(path);
        rig.dev.set_input(input).expect("input");
        let fresh = Session::new(path.clone(), controls::new_edit(&path), History::default());
        let a = rig.render(&fresh, 1.0);
        let b = rig.render(&fresh, 1.0);
        assert!(apart(&a, &b) < CHANGED / 10.0, "renders of one edit agree");
        let controls: Vec<(u32, Body)> = fresh
            .panel
            .cards
            .iter()
            .flat_map(|c| c.controls.iter().map(|k| (k.id, k.body.clone())))
            .collect();
        for (control, body) in controls {
            if let Some(why) = exempt(control, &body) {
                exempted.entry(why).or_default().push(control);
                continue;
            }
            // A dimmed control says why in a few words.
            if let Some(why) = fresh.panel.dimmed_reason(control) {
                assert!(!why.is_empty() && why.len() < 80, "{control}: {why:?}");
            }
            // HDR's limit only shows above SDR white.
            let peak = if control == id::HDR_LIMIT { 64.0 } else { 1.0 };
            for (part, m) in moves(&body) {
                if control == id::CA_MODE && matches!(m, Move::Option(2)) {
                    exempted
                        .entry("Manual starts at no shift: Red and Blue shift move it")
                        .or_default()
                        .push(control);
                    continue;
                }
                let mut s = ready(&mut rig, &path, control);
                if let Some(why) = s.panel.dimmed_reason(control) {
                    failed.push(format!("{}: {control} still dimmed: {why}", path.display()));
                    continue;
                }
                let before = rig.render(&s, peak);
                let tries: Vec<Move> = match m {
                    Move::Far => ends(&s, control).into_iter().map(Move::Slider).collect(),
                    m => vec![m],
                };
                let mut d = 0.0f32;
                let mut last = None;
                for (k, m) in tries.into_iter().enumerate() {
                    if d >= CHANGED {
                        break;
                    }
                    if k > 0 {
                        s = ready(&mut rig, &path, control);
                    }
                    let steps = s.history.steps.len();
                    apply(&mut s, control, part, &m);
                    rig.send(&mut s, control, part);
                    moved += 1;
                    // One move, one history step.
                    if s.history.steps.len() != steps + 1 {
                        failed.push(format!(
                            "{}: {control}/{part}: {} history steps",
                            path.display(),
                            s.history.steps.len() - steps
                        ));
                    }
                    let after = rig.render(&s, peak);
                    d = d.max(apart(&before, &after));
                    last = Some(m);
                }
                if d < CHANGED {
                    let to = match last.unwrap_or(Move::Curve) {
                        Move::Far => "far".into(),
                        Move::Slider(v) => format!("to {v}"),
                        Move::Option(i) => format!("option {i}"),
                        Move::Wheel(i) => format!("wheel {i}"),
                        Move::Curve => "curve".into(),
                    };
                    failed.push(format!(
                        "{}: {control}/{part} {to} changes nothing ({d:.4})",
                        path.display()
                    ));
                }
            }
        }
    }
    for (why, ids) in &exempted {
        eprintln!("exempt ({} controls): {why}: {ids:?}", ids.len());
    }
    eprintln!("{moved} moves checked");
    assert!(
        failed.is_empty(),
        "{} dead:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// The luma graph follows each edit: it is binned from the developed picture, so the bins
/// of a render after a change are the changed picture's, and two quick renders in a row
/// leave the newest one's.
#[test]
fn the_luma_graph_follows_the_edit() {
    let Some(gpu) = gpu() else { return };
    let mut rig = Rig {
        dev: Developer::new(
            &gpu.device,
            &gpu.queue,
            Budget::for_device(true, None, None),
        ),
        t: 1_000_000,
    };
    rig.dev.crop_on = true;
    let path = PathBuf::from("/charts/chart.jpg");
    rig.dev.set_input(rgb_input()).expect("input");
    let mut s = Session::new(path.clone(), controls::new_edit(&path), History::default());
    let bins = |rig: &mut Rig, s: &Session| {
        let e = s.shown_edit();
        let out = OutputSpace::rec709();
        rig.dev.prepare(&e, W).expect("prepare");
        let r = rig.dev.render(&e, None, &out).expect("render");
        rig.dev.scopes(&r, 1.0);
        rig.dev.recycle(r);
        rig.dev.wait_scopes().expect("bins").words
    };
    let first = bins(&mut rig, &s);
    let again = bins(&mut rig, &s);
    assert_eq!(first, again, "one edit, one graph");
    let mut seen = vec![first];
    for (control, v) in [
        (id::EXPOSURE, 1.5),
        (id::CONTRAST, 60.0),
        (id::EXPOSURE, -2.0),
    ] {
        s.panel.slider_mut(control).expect("slider").set(v);
        rig.send(&mut s, control, 0);
        let b = bins(&mut rig, &s);
        assert!(
            !seen.contains(&b),
            "the graph after {control} = {v} is a new one"
        );
        seen.push(b);
    }
    // Two renders without waiting: the bins read are the newest picture's.
    s.panel.slider_mut(id::EXPOSURE).expect("slider").set(0.5);
    rig.send(&mut s, id::EXPOSURE, 0);
    let e1 = s.shown_edit().into_owned();
    s.panel.slider_mut(id::EXPOSURE).expect("slider").set(2.5);
    rig.send(&mut s, id::EXPOSURE, 0);
    let e2 = s.shown_edit().into_owned();
    let out = OutputSpace::rec709();
    for e in [&e1, &e2] {
        rig.dev.prepare(e, W).expect("prepare");
        let r = rig.dev.render(e, None, &out).expect("render");
        rig.dev.scopes(&r, 1.0);
        rig.dev.recycle(r);
    }
    let newest = rig.dev.wait_scopes().expect("bins").words;
    assert_eq!(newest, bins(&mut rig, &s), "the newest picture's graph");
}
