//! The cases: one per §3.9 row (and per module family, size and mosaic where the row has
//! several), plus scopes, retouch, the diffuse presets and the AI networks (no budget there:
//! gated by the +10 % rule only).

use std::collections::BTreeMap;
use std::time::Instant;

use specular_convert::OutputDir;
use specular_develop::export::batch::{self, Job};
use specular_develop::export::preset::Preset as ExportPreset;
use specular_develop::gpu::pool::Budget as PoolBudget;
use specular_develop::gpu::{Engine, Request};
use specular_develop::graph::{Graph, OpRegistry, Roi};
use specular_develop::mask::{Geometry, MaskCache, MaskEnv};
use specular_develop::model::mask::{PressureUse, Stroke};
use specular_develop::model::{
    Blend, Edit, Fields, MaskExpr, MaskShape, ModuleInstance, ModuleKind, Params, Registry, Shape,
    SourceKey, Value,
};
use specular_develop::ops::detail::RawNoise;
use specular_develop::ops::detail::diffuse::{LONG_FIELD, Preset as Diffuse};
use specular_develop::ops::output::OutputSpace;
use specular_develop::ops::retouch::{Retouch, Spot};
use specular_develop::preview::{self, Developer, Input, Orient};
use specular_develop::scopes::ScopeEngine;
use specular_develop::thumb::{self, Thumbnailer};

use crate::gate::{Budget, CaseResult, Metric, Stat};
use crate::inputs::{Mosaic, Raw};

/// The window of §3.9's interactive rows (fit view).
pub const WINDOW: (u32, u32) = (2560, 1600);

/// One measurement: wall milliseconds and, when timed, GPU milliseconds.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub wall: f64,
    pub gpu: Option<f64>,
}

/// The bench's device and settings, and what has been measured.
pub struct Bench {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub timestamps: bool,
    pub software: bool,
    /// Rounds per case (the best is kept).
    pub rounds: usize,
    /// Samples per round = the case's count × this (≥ 1 sample).
    pub scale: f64,
    pub only: Vec<String>,
    pub results: BTreeMap<String, CaseResult>,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

impl Bench {
    /// Whether `name` is selected (`DV_BENCH_ONLY`: comma-separated parts of names).
    pub fn wants(&self, name: &str) -> bool {
        self.only.is_empty() || self.only.iter().any(|o| name.contains(o.as_str()))
    }

    /// Whether any case whose name starts with `prefix` may be selected (a narrower
    /// `DV_BENCH_ONLY` part such as `slider/point/exposure` counts).
    pub fn wants_prefix(&self, prefix: &str) -> bool {
        self.wants(prefix) || self.only.iter().any(|o| o.starts_with(prefix))
    }

    fn wants_any(&self, names: &[String]) -> bool {
        names.iter().any(|n| self.wants(n))
    }

    pub fn count(&self, n: usize) -> usize {
        ((n as f64 * self.scale).round() as usize).max(1)
    }

    /// Runs `round` [`Bench::rounds`] times and keeps the round with the lowest p50 of the
    /// gated metric (GPU time when the case times it and the device can, else wall).
    pub fn case(
        &mut self,
        name: &str,
        family: &str,
        gpu_gated: bool,
        budget: Option<Budget>,
        note: &str,
        mut round: impl FnMut(&mut Bench) -> Vec<Sample>,
    ) {
        if !self.wants(name) {
            return;
        }
        let mut best: Option<CaseResult> = None;
        for _ in 0..self.rounds {
            let samples = round(self);
            let wall: Vec<f64> = samples.iter().map(|s| s.wall).collect();
            let gpu: Vec<f64> = samples.iter().filter_map(|s| s.gpu).collect();
            let Some(wall) = Stat::of(&wall) else {
                continue;
            };
            let gpu = Stat::of(&gpu);
            let gated = if gpu_gated && gpu.is_some() {
                Metric::Gpu
            } else {
                Metric::Wall
            };
            let r = CaseResult {
                family: family.to_owned(),
                wall,
                gpu,
                gated,
                // Software adapters are not held to the integrated-GPU ceilings.
                budget: budget.filter(|_| !self.software),
                rounds: self.rounds,
                note: note.to_owned(),
            };
            if best
                .as_ref()
                .is_none_or(|b| r.gated_stat().p50 < b.gated_stat().p50)
            {
                best = Some(r);
            }
        }
        if let Some(r) = best {
            let g = r.gated_stat();
            eprintln!(
                "{name:<44} p50 {:>9.2}  p95 {:>9.2}  best {:>9.2} ms ({:?}, n {}){}",
                g.p50,
                g.p95,
                g.best,
                r.gated,
                g.n,
                match r.over_budget() {
                    v if v.is_empty() => String::new(),
                    v => format!("  OVER: {}", v.join(", ")),
                }
            );
            self.results.insert(name.to_owned(), r);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Edits

/// Sets a field of the module (added with its defaults when the edit has none).
fn set(e: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    if e.module_mut(kind, 0).is_none() {
        add(e, kind, Fields::new());
    }
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

fn add(e: &mut Edit, kind: ModuleKind, f: Fields) {
    let reg = Registry::builtin();
    let mut m = ModuleInstance::new(reg, kind, 0);
    let mut d = m.params.to_fields();
    d.overlay(&f);
    m.params = Params::from_fields(kind, d);
    e.insert(reg, m);
}

/// §3.9's default export edit: demosaic (the raw front end), lens, CAT, tone equalizer,
/// local contrast, colour balance, sigmoid, wavelet denoise and output sharpening.
pub fn default_edit(input: Option<&Input>) -> Edit {
    let raw = matches!(input, Some(Input::Raw(_)));
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), raw);
    if let Some(Input::Raw(s)) = input
        && let Some(xy) = preview::as_shot_white(s)
    {
        batch::set_as_shot(&mut e, xy);
    }
    add(
        &mut e,
        ModuleKind::LENS,
        Fields::new().with("distortion", Value::Float(3.0)),
    );
    add(
        &mut e,
        ModuleKind::TONE_EQUALIZER,
        Fields::new().with("shadows_ev", Value::Float(0.5)),
    );
    add(
        &mut e,
        ModuleKind::LOCAL_CONTRAST,
        Fields::new()
            .with("texture", Value::Float(0.2))
            .with("clarity", Value::Float(0.2)),
    );
    add(
        &mut e,
        ModuleKind::COLOUR_BALANCE,
        Fields::new().with("vibrance", Value::Float(0.2)),
    );
    add(
        &mut e,
        ModuleKind::DENOISE,
        Fields::new().with("luminance", Value::Float(0.3)),
    );
    add(
        &mut e,
        ModuleKind::SHARPEN,
        Fields::new().with("amount", Value::Float(0.5)),
    );
    e
}

// ---------------------------------------------------------------------------------------------
// Interactive rows on the Developer (fit view of a 24 MP raw in a 2560×1600 window)

struct View {
    dv: Developer,
    tex: wgpu::Texture,
    out: OutputSpace,
    edit: Edit,
}

impl View {
    fn new(b: &Bench, input: Input, edit: Edit) -> View {
        let mut dv = Developer::new(&b.device, &b.queue, PoolBudget::fixed(3 << 30));
        // `DV_BENCH_OLD=1`: without the half-scale drafts and kept results (before/after).
        if std::env::var_os("DV_BENCH_OLD").is_some() {
            dv.half_drafts = false;
            dv.keep_results = false;
            dv.checkpoint_every = false;
        } else {
            // As the app: a photo's first picture is a draft, refined next; a refined
            // picture after a change leaves sharpening out, the whole one follows
            // (`DV_BENCH_SHARPEN_NOW=1`: the whole one at once, as before).
            dv.draft_first = true;
            dv.defer_sharpen = std::env::var_os("DV_BENCH_SHARPEN_NOW").is_none();
        }
        dv.set_input(input).expect("input");
        let (w, h) = dv.prepare(&edit, WINDOW.0).expect("prepare");
        let tex = texture(&b.device, w, h);
        View {
            dv,
            tex,
            out: OutputSpace::rec709(),
            edit,
        }
    }

    /// Render and present, to the GPU being done.
    fn frame(&mut self, focus: Option<(ModuleKind, u16)>) -> f64 {
        let t = Instant::now();
        let r = self
            .dv
            .render(&self.edit, focus, &self.out)
            .expect("render");
        self.dv
            .present(r, Orient::default(), &self.tex)
            .expect("present");
        self.dv.wait().expect("wait");
        ms(t)
    }

    /// The graph's GPU time of one render (timestamps; blocks).
    fn gpu(&mut self, focus: Option<(ModuleKind, u16)>) -> Option<f64> {
        self.dv.profile = true;
        let r = self
            .dv
            .render(&self.edit, focus, &self.out)
            .expect("render");
        self.dv.profile = false;
        let g = self.dv.stats.render_gpu_ms;
        breakdown(&r.stats);
        self.dv.recycle(r);
        g
    }
}

/// `DV_BENCH_PROFILE=1`: prints a profiled render's GPU milliseconds per dispatch label
/// (summed over tiles), the largest first.
fn breakdown(stats: &specular_develop::gpu::RenderStats) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if std::env::var_os("DV_BENCH_PROFILE").is_none()
        || !N
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .is_multiple_of(16)
    {
        return;
    }
    print_breakdown(stats);
}

fn print_breakdown(stats: &specular_develop::gpu::RenderStats) {
    let Some(v) = &stats.gpu_ms else { return };
    // `DV_BENCH_PROFILE=seq`: every dispatch in order.
    if std::env::var("DV_BENCH_PROFILE").is_ok_and(|v| v == "seq") {
        let seq: Vec<String> = v.iter().map(|(l, ms)| format!("{l} {ms:.3}")).collect();
        eprintln!("  gpu seq: {}", seq.join(", "));
    }
    let mut by: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    for (l, ms) in v {
        let e = by.entry(l.as_str()).or_default();
        e.0 += ms;
        e.1 += 1;
    }
    let mut rows: Vec<_> = by.into_iter().collect();
    rows.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
    let total: f64 = rows.iter().map(|r| r.1.0).sum();
    // `DV_BENCH_PROFILE=all`: every label, not only the eight largest.
    let all = std::env::var("DV_BENCH_PROFILE").is_ok_and(|v| v == "all");
    let top: Vec<String> = rows
        .iter()
        .take(if all { usize::MAX } else { 8 })
        .map(|(l, (ms, n))| format!("{l} {ms:.2}×{n}"))
        .collect();
    eprintln!("  gpu {total:.2} ms: {}", top.join(", "));
}

fn texture(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("develop bench window"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

type Slide = fn(usize) -> Value;

struct Slider {
    name: &'static str,
    kind: ModuleKind,
    field: &'static str,
    value: Slide,
    /// Module to add first (heavy ops not in the default edit), with its fields.
    extra: Option<fn() -> Fields>,
}

const fn slider(name: &'static str, kind: ModuleKind, field: &'static str, value: Slide) -> Slider {
    Slider {
        name,
        kind,
        field,
        value,
        extra: None,
    }
}

fn fl(v: f32) -> Value {
    Value::Float(v)
}

const POINT: [Slider; 5] = [
    slider("exposure", ModuleKind::EXPOSURE, "exposure_ev", |i| {
        fl((i % 40) as f32 * 0.05)
    }),
    slider(
        "white-balance",
        ModuleKind::COLOUR_CALIBRATION,
        "white_x",
        |i| fl(0.30 + (i % 40) as f32 * 0.001),
    ),
    slider(
        "colour-balance",
        ModuleKind::COLOUR_BALANCE,
        "vibrance",
        |i| fl(0.1 + (i % 40) as f32 * 0.02),
    ),
    slider("curves", ModuleKind::CURVES, "points", |i| {
        Value::Floats(vec![0.0, 0.0, 0.5, 0.5 + (i % 20) as f32 * 0.01, 1.0, 1.0])
    }),
    slider(
        "view-transform",
        ModuleKind::VIEW_TRANSFORM,
        "contrast",
        |i| fl(1.0 + (i % 40) as f32 * 0.01),
    ),
];

const NEIGHBOURHOOD: [Slider; 3] = [
    slider(
        "tone-equalizer",
        ModuleKind::TONE_EQUALIZER,
        "shadows_ev",
        |i| fl(0.2 + (i % 40) as f32 * 0.05),
    ),
    slider(
        "local-contrast",
        ModuleKind::LOCAL_CONTRAST,
        "clarity",
        |i| fl(0.1 + (i % 40) as f32 * 0.02),
    ),
    slider(
        "colour-equalizer",
        ModuleKind::COLOUR_EQUALIZER,
        "saturation_blue",
        |i| fl(0.1 + (i % 40) as f32 * 0.02),
    ),
];

const HEAVY: [Slider; 3] = [
    slider("denoise", ModuleKind::DENOISE, "luminance", |i| {
        fl(0.2 + (i % 40) as f32 * 0.01)
    }),
    Slider {
        name: "dehaze",
        kind: ModuleKind::DEHAZE,
        field: "strength",
        value: |i| fl(0.2 + (i % 40) as f32 * 0.01),
        extra: Some(|| Fields::new().with("strength", Value::Float(0.3))),
    },
    Slider {
        name: "highlight-inpaint",
        kind: ModuleKind::DIFFUSE,
        field: "iterations",
        value: |i| Value::Int(8 + (i % 4) as i64),
        extra: Some(|| Diffuse::InpaintHighlights.params().to_fields()),
    },
];

const fn budget(p50: f64, p95: Option<f64>) -> Option<Budget> {
    Some(Budget {
        p50: Some(p50),
        p95,
        gpu_p50: None,
    })
}

/// Slider moves per module family, draft (the module in focus) and refined (released);
/// undo/redo; scopes.
pub fn sliders(b: &mut Bench, input: &Input) {
    let fams: [(&str, &[Slider]); 3] = [
        ("point", &POINT),
        ("neighbourhood", &NEIGHBOURHOOD),
        ("heavy", &HEAVY),
    ];
    let names: Vec<String> = fams
        .iter()
        .flat_map(|(f, s)| {
            s.iter().flat_map(move |s| {
                ["draft", "refined"].map(|d| format!("slider/{f}/{}/{d}", s.name))
            })
        })
        .chain(["undo-redo".into(), "scopes/2560".into()])
        .collect();
    if !b.wants_any(&names) {
        return;
    }
    let base = default_edit(Some(input));
    for (fam, sliders) in fams {
        for s in sliders {
            let mut edit = base.clone();
            if let Some(f) = s.extra {
                add(&mut edit, s.kind, f());
            }
            if s.kind == ModuleKind::COLOUR_CALIBRATION {
                set(
                    &mut edit,
                    s.kind,
                    "white_source",
                    Value::Text("custom".into()),
                );
            }
            if s.kind == ModuleKind::DIFFUSE {
                set(&mut edit, s.kind, LONG_FIELD, fl(WINDOW.0 as f32));
            }
            let draft_name = format!("slider/{fam}/{}/draft", s.name);
            let refined_name = format!("slider/{fam}/{}/refined", s.name);
            if !b.wants(&draft_name) && !b.wants(&refined_name) {
                continue;
            }
            let mut v = View::new(b, input.clone(), edit);
            // Slider values come back every 40 moves: each move is measured as a render,
            // not as a copy of a kept result (undo-redo measures those).
            v.dv.keep_results = false;
            let (kind, field, value) = (s.kind, s.field, s.value);
            for draft in [true, false] {
                let name = if draft { &draft_name } else { &refined_name };
                let focus = draft.then_some((kind, 0));
                let (bud, n, gpu_n) = match (fam, draft) {
                    ("point", _) => (
                        Some(Budget {
                            p50: Some(16.0),
                            p95: Some(33.0),
                            gpu_p50: Some(8.0),
                        }),
                        40,
                        20,
                    ),
                    ("neighbourhood", _) => (budget(33.0, Some(50.0)), 40, 20),
                    (_, true) => (budget(50.0, None), 20, 10),
                    (_, false) => (budget(300.0, Some(500.0)), 8, 4),
                };
                let (n, gpu_n) = (b.count(n), b.count(gpu_n));
                let note = if draft {
                    "render + present of the module in focus"
                } else {
                    "render + present after release"
                };
                b.case(name, fam, true, bud, note, |b| {
                    let mut out = Vec::new();
                    for i in 0..n + 3 {
                        set(&mut v.edit, kind, field, value(i));
                        let wall = v.frame(focus);
                        if i >= 3 {
                            out.push(Sample { wall, gpu: None });
                        }
                    }
                    if b.timestamps {
                        for (i, s) in out.iter_mut().enumerate().take(gpu_n) {
                            set(&mut v.edit, kind, field, value(i + 7));
                            s.gpu = v.gpu(focus);
                        }
                    }
                    out
                });
                // When the first refined picture leaves sharpening out, the time to
                // the whole one too (both renders, as the app makes them).
                let whole = format!("{refined_name}-whole");
                if !draft && v.dv.defer_sharpen && b.wants(&whole) {
                    b.case(
                        &whole,
                        fam,
                        false,
                        None,
                        "render + present after release, then the whole picture (sharpened)",
                        |_| {
                            let mut out = Vec::new();
                            for i in 0..n + 3 {
                                set(&mut v.edit, kind, field, value(i));
                                let mut wall = v.frame(None);
                                if v.dv.refine_due() {
                                    wall += v.frame(None);
                                }
                                if i >= 3 {
                                    out.push(Sample { wall, gpu: None });
                                }
                            }
                            out
                        },
                    );
                }
            }
        }
    }
    // Undo and redo: a late module (colour balance) back and forth with the earlier
    // modules' checkpoints cached.
    let mut v = View::new(b, input.clone(), base.clone());
    b.case(
        "undo-redo",
        "undo",
        false,
        budget(16.0, Some(50.0)),
        "colour balance back and forth, refined, checkpoints cached",
        |b| {
            let n = b.count(40);
            let mut out = Vec::new();
            for i in 0..n + 4 {
                let vib = if i % 2 == 0 { 0.2 } else { 0.45 };
                set(&mut v.edit, ModuleKind::COLOUR_BALANCE, "vibrance", fl(vib));
                let wall = v.frame(None);
                if i >= 4 {
                    out.push(Sample { wall, gpu: None });
                }
            }
            out
        },
    );
    // Scopes of the fit-view picture.
    let r = v.dv.render(&v.edit, None, &v.out).expect("render");
    let mut sc = ScopeEngine::new(&b.device, &b.queue);
    sc.profile = b.timestamps;
    b.case(
        "scopes/2560",
        "scopes",
        true,
        None,
        "waveform, parade and vectorscope bins",
        |b| {
            let n = b.count(30);
            let mut out = Vec::new();
            for i in 0..n + 3 {
                let t = Instant::now();
                sc.run(r.buffer(), r.roi.w, r.roi.h, r.precision, 1.0);
                let _ = sc.wait();
                let wall = ms(t);
                if i >= 3 {
                    out.push(Sample {
                        wall,
                        gpu: sc.gpu_ms,
                    });
                }
            }
            out
        },
    );
    v.dv.recycle(r);
}

/// Adaptive preview while dragging (`specular_develop::drag`): exposure and white balance
/// dragged on a plain edit (point ops only) and with texture and clarity, as the app drives
/// it: each frame drawn at the controller's level and its time (render, present, GPU done)
/// fed back. `before`: no controller (half scale only with a neighbourhood op after the
/// slider); `auto`, `sharp`, `fast`: the setting's three choices. The level reached is
/// printed.
pub fn drags(b: &mut Bench, input: &Input) {
    use specular_develop::drag::{Level, Mode, Pace};
    let modes: [(&str, Option<Mode>); 4] = [
        ("before", None),
        ("auto", Some(Mode::Automatic)),
        ("sharp", Some(Mode::Sharp)),
        ("fast", Some(Mode::Fast)),
    ];
    let mut plain = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    if let Input::Raw(s) = input
        && let Some(xy) = preview::as_shot_white(s)
    {
        batch::set_as_shot(&mut plain, xy);
    }
    let mut texture = plain.clone();
    add(
        &mut texture,
        ModuleKind::LOCAL_CONTRAST,
        Fields::new()
            .with("texture", Value::Float(0.2))
            .with("clarity", Value::Float(0.2)),
    );
    for s in &POINT[..2] {
        for (shape, base) in [("plain", &plain), ("texture", &texture)] {
            let mut edit = base.clone();
            if s.kind == ModuleKind::COLOUR_CALIBRATION {
                set(
                    &mut edit,
                    s.kind,
                    "white_source",
                    Value::Text("custom".into()),
                );
            }
            let names: Vec<String> = modes
                .iter()
                .map(|(m, _)| format!("drag/{}/{shape}/{m}", s.name))
                .collect();
            if !b.wants_any(&names) {
                continue;
            }
            let mut v = View::new(b, input.clone(), edit);
            v.dv.keep_results = false;
            let (kind, field, value) = (s.kind, s.field, s.value);
            let focus = (kind, 0);
            for ((_, mode), name) in modes.iter().zip(&names) {
                let n = b.count(60);
                let mut levels = BTreeMap::<String, usize>::new();
                b.case(
                    name,
                    "drag",
                    false,
                    None,
                    "render + present of the slider dragged, at the setting's level",
                    |_| {
                        let mut pace = mode.map(Pace::new);
                        if let Some(p) = pace.as_mut() {
                            let start = v.dv.drag_default(&v.edit, focus);
                            p.begin(1, start);
                        }
                        levels.clear();
                        let mut out = Vec::new();
                        for i in 0..n + 3 {
                            set(&mut v.edit, kind, field, value(i));
                            v.dv.drag_level = pace.as_ref().map(Pace::level);
                            let wall = v.frame(Some(focus));
                            let divisor = v.dv.stats.draft_divisor;
                            if let Some(p) = pace.as_mut() {
                                p.record(divisor, wall);
                            }
                            if i >= 3 {
                                out.push(Sample { wall, gpu: None });
                                *levels
                                    .entry(format!("{:?}", Level::of_divisor(divisor)))
                                    .or_default() += 1;
                            }
                        }
                        v.dv.drag_level = None;
                        out
                    },
                );
                if b.wants(name) {
                    eprintln!("  {name}: frames per level {levels:?}");
                }
            }
        }
    }
}

/// Enter Develop on a raw open in the viewer (its file read, sensor data decoded here) and
/// the next raw already prefetched (decoded): to the first developed frame presented.
pub fn open(b: &mut Bench, raws: &[Raw]) {
    for raw in raws {
        let enter = format!("open/enter/{}", raw.name());
        let next = format!("open/next-prefetched/{}", raw.name());
        if !b.wants(&enter) && !b.wants(&next) && !b.wants(&format!("open/refine/{}", raw.name())) {
            continue;
        }
        let bytes = std::fs::read(raw.path()).expect("read the raw");
        let input = preview::decode_raw(&bytes).expect("decode");
        let edit = default_edit(Some(&input));
        // The app's Developer exists already (pipelines compiled): warm it on this photo.
        let mut v = View::new(b, input.clone(), edit.clone());
        v.frame(None);
        let device = b.device.clone();
        // `Err(tag)`: the image developed ahead with that tag.
        let first = |v: &mut View, input: Result<Input, u64>| -> f64 {
            let t = Instant::now();
            let phases = std::env::var_os("DV_BENCH_PROFILE").is_some();
            let mut lap = Vec::new();
            let mut mark = |dv: &mut Developer, t0: &mut Instant| {
                if phases {
                    dv.wait().expect("wait");
                    lap.push(ms(*t0));
                    *t0 = Instant::now();
                }
            };
            let mut t0 = Instant::now();
            match input {
                Ok(input) => v.dv.set_input(input).expect("input"),
                Err(tag) => assert!(v.dv.take_prefetched(tag), "prefetched"),
            }
            mark(&mut v.dv, &mut t0);
            let (w, h) = v.dv.prepare(&edit, WINDOW.0).expect("prepare");
            mark(&mut v.dv, &mut t0);
            if (v.tex.width(), v.tex.height()) != (w, h) {
                v.tex = texture(&device, w, h);
            }
            v.dv.profile = phases;
            let r = v.dv.render(&edit, None, &v.out).expect("render");
            v.dv.profile = false;
            if phases {
                print_breakdown(&r.stats);
                eprintln!(
                    "  tiles {} {:?}",
                    r.stats.tiles,
                    r.stats.gpu_ms.as_ref().map(|v| v
                        .iter()
                        .filter(|(l, _)| l.contains("nlm") || l.contains("vst"))
                        .map(|(l, m)| format!("{l} {m:.1}"))
                        .collect::<Vec<_>>())
                );
            }
            mark(&mut v.dv, &mut t0);
            v.dv.present(r, Orient::default(), &v.tex).expect("present");
            v.dv.wait().expect("wait");
            mark(&mut v.dv, &mut t0);
            if phases {
                eprintln!(
                    "  open phases: set_input {:.0}, prepare {:.0}, render {:.0}, present {:.0} ms",
                    lap[0], lap[1], lap[2], lap[3]
                );
            }
            ms(t)
        };
        b.case(
            &enter,
            "open",
            false,
            budget(150.0, Some(250.0)),
            "sensor decode + front end + render + present",
            |b| {
                (0..b.count(5))
                    .map(|_| {
                        let t = Instant::now();
                        let i = preview::decode_raw(&bytes).expect("decode");
                        let decode = ms(t);
                        Sample {
                            wall: decode + first(&mut v, Ok(i)),
                            gpu: None,
                        }
                    })
                    .collect()
            },
        );
        b.case(
            &next,
            "open",
            false,
            budget(100.0, Some(200.0)),
            "developed ahead (decode, upload, noise fit, front end): render + present",
            |b| {
                // `DV_BENCH_OLD=1`: decoded ahead only (front end timed), as before developing ahead.
                let old = std::env::var_os("DV_BENCH_OLD").is_some();
                let noise = match &input {
                    Input::Raw(s) => Some(RawNoise::of_sensor(s)),
                    Input::Rgb { .. } => None,
                };
                (0..b.count(5))
                    .map(|k| {
                        let i = input.clone();
                        if old {
                            return Sample {
                                wall: first(&mut v, Ok(i)),
                                gpu: None,
                            };
                        }
                        // The app's prefetcher, while the photo before is shown (not timed):
                        // developed ahead, then again once its highlight statistics are in.
                        let tag = k as u64 + 1;
                        // As the app does, the photo shown refines first.
                        for _ in 0..500 {
                            if !v.dv.refining() {
                                break;
                            }
                            if v.dv.refine_due() {
                                v.dv.prepare(&edit, WINDOW.0).expect("prepare");
                            }
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        let kept =
                            v.dv.prefetch(tag, i, noise, &edit, WINDOW.0, Orient::default())
                                .expect("prefetch");
                        assert!(kept, "the prefetch fits the budget");
                        v.dv.wait().expect("wait");
                        for _ in 0..200 {
                            if v.dv.refine_prefetched().expect("refine") {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        v.dv.wait().expect("wait");
                        Sample {
                            wall: first(&mut v, Err(tag)),
                            gpu: None,
                        }
                    })
                    .collect()
            },
        );
        // The first source is binned; the refined one (full demosaic reduced in
        // stripes) follows when nothing is dragged.
        let refine = format!("open/refine/{}", raw.name());
        if b.wants(&refine) {
            b.case(
                &refine,
                "open",
                false,
                budget(300.0, Some(500.0)),
                "binned first picture → full demosaic reduced + render + present",
                |b| {
                    (0..b.count(5))
                        .map(|_| {
                            first(&mut v, Ok(input.clone()));
                            let t = Instant::now();
                            assert!(v.dv.refine_due(), "the refined source is due");
                            // GPU column: the front end's (timestamps; waits for it).
                            v.dv.profile = true;
                            v.dv.prepare(&edit, WINDOW.0).expect("prepare");
                            v.dv.profile = false;
                            v.frame(None);
                            Sample {
                                wall: ms(t),
                                gpu: v.dv.stats.front_end_gpu_ms,
                            }
                        })
                        .collect()
                },
            );
        }
    }
}

/// The edited thumbnail (512 px, developed at the app's default `thumb::RAW_LONG`) of a raw
/// file: read, decode, develop, read back.
pub fn thumbnails(b: &mut Bench, raws: &[Raw]) {
    for raw in raws {
        let name = format!("thumbnail/512/{}", raw.name());
        if !b.wants(&name) {
            continue;
        }
        let path = raw.path();
        let input = preview::decode_raw(&std::fs::read(&path).expect("read")).expect("decode");
        let edit = default_edit(Some(&input));
        drop(input);
        let mut th = Thumbnailer::new(&b.device, &b.queue, PoolBudget::fixed(2 << 30));
        if let Some(v) = std::env::var("DV_BENCH_THUMB_RAW_LONG")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            th.raw_long = Some(v);
        }
        let one = |th: &mut Thumbnailer| -> f64 {
            let t = Instant::now();
            let src = thumb::read_raw(&path).expect("read").expect("a raw");
            let _ = th.render(src, &edit, 512).expect("thumbnail");
            ms(t)
        };
        one(&mut th);
        b.case(
            &name,
            "thumbnail",
            false,
            budget(150.0, None),
            "read + decode + develop + read back",
            |b| {
                (0..b.count(6))
                    .map(|_| Sample {
                        wall: one(&mut th),
                        gpu: None,
                    })
                    .collect()
            },
        );
    }
}

/// Export to JPEG q90 at full size, decode included; the batch throughput at 24 MP.
pub fn exports(b: &mut Bench, raws: &[Raw]) {
    let reg = specular_decode::Registry::new();
    let out = crate::inputs::out_dir().join("export");
    for raw in raws {
        let name = format!("export/{}", raw.name());
        let batch_name = format!("export/batch/{}", raw.name());
        let batch_too = raw.mp == 24 && raw.mosaic == Mosaic::Bayer;
        if !b.wants(&name) && !(batch_too && b.wants(&batch_name)) {
            continue;
        }
        let path = raw.path();
        let input = preview::decode_raw(&std::fs::read(&path).expect("read")).expect("decode");
        let edit = default_edit(Some(&input));
        drop(input);
        let preset = ExportPreset {
            folder: OutputDir::Folder(out.clone()),
            ..ExportPreset::default()
        };
        let limit = match raw.mp {
            24 => 1000.0,
            45 => 1800.0,
            _ => 4000.0,
        };
        let device = b.device.clone();
        let queue = b.queue.clone();
        let run = |n: usize| {
            let jobs = (0..n)
                .map(|_| Job {
                    source: path.clone(),
                    edit: edit.clone(),
                    orient: None,
                })
                .collect();
            let rep = batch::run(
                &device,
                &queue,
                PoolBudget::for_device(true, None, Some(32 << 30)),
                jobs,
                &preset,
                &reg,
                &specular_core::CancelToken::new(),
                &|_| {},
            );
            assert_eq!(rep.written(), n, "{:?}", rep.results);
            let _ = std::fs::remove_dir_all(&out);
            if std::env::var_os("DV_BENCH_PROFILE").is_some() {
                for t in &rep.timings {
                    eprintln!(
                        "  read {:.0}, source {:.0}, render {:.0}, encode {:.0}, write {:.0} ms",
                        t.read_ms, t.source_ms, t.render_ms, t.encode_ms, t.write_ms
                    );
                }
            }
            rep
        };
        if std::env::var_os("DV_BENCH_PROFILE").is_some() && b.wants(&name) {
            profile_export(&device, &queue, &path, &edit, &preset, &reg);
        }
        let n = if raw.mp == 24 { 3 } else { 2 };
        b.case(
            &name,
            "export",
            false,
            budget(limit, None),
            "one image: read, decode, render, JPEG q90, write",
            |b| {
                (0..b.count(n))
                    .map(|_| Sample {
                        wall: run(1).elapsed.as_secs_f64() * 1e3,
                        gpu: None,
                    })
                    .collect()
            },
        );
        if batch_too {
            // ≥ 1.5 images/s: at most 667 ms per image.
            b.case(
                &batch_name,
                "export",
                false,
                budget(1000.0 / 1.5, None),
                "ms per image of a batch of 4, overlapped",
                |_| {
                    let rep = run(4);
                    vec![Sample {
                        wall: rep.elapsed.as_secs_f64() * 1e3 / 4.0,
                        gpu: None,
                    }]
                },
            );
        }
    }
}

/// `DV_BENCH_PROFILE`: one export of `path` (twice) with the raw front end's passes and the
/// engine's dispatches timed, the milliseconds summed per label (the warp and the read-back
/// coding are not timed; the timestamps add a little).
fn profile_export(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &std::path::Path,
    edit: &Edit,
    preset: &ExportPreset,
    reg: &specular_decode::Registry,
) {
    let mut ex = specular_develop::export::Exporter::new(
        device,
        queue,
        PoolBudget::for_device(true, None, Some(32 << 30)),
    );
    ex.profile = true;
    let preset = preset.clone().normalised();
    for _ in 0..2 {
        let read = batch::read_input(path, reg).expect("read");
        let mut edit = edit.clone();
        if let Some(xy) = read.as_shot {
            batch::set_as_shot(&mut edit, xy);
        }
        ex.profiled.clear();
        let t = Instant::now();
        ex.render(read.input, &edit, Orient::default(), &preset, &mut |_| true)
            .expect("export");
        let wall = ms(t);
        let mut by: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for (l, m) in ex.profiled.drain(..) {
            let e = by.entry(l).or_default();
            e.0 += m;
            e.1 += 1;
        }
        let mut rows: Vec<_> = by.into_iter().collect();
        rows.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
        let total: f64 = rows.iter().map(|r| r.1.0).sum();
        let top: Vec<String> = rows
            .iter()
            .take(14)
            .map(|(l, (m, n))| format!("{l} {m:.1}×{n}"))
            .collect();
        let front: f64 = rows
            .iter()
            .filter(|r| r.0.starts_with("raw "))
            .map(|r| r.1.0)
            .sum();
        let denoise: f64 = rows
            .iter()
            .filter(|r| r.0.starts_with("detail_") || r.0.starts_with("denoise"))
            .map(|r| r.1.0)
            .sum();
        eprintln!(
            "  export profile: wall {wall:.0} ms (profiled), gpu {total:.1} ms (raw front end \
             {front:.1}, denoise {denoise:.1}): {}",
            top.join(", ")
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Engine rows: 1:1 tiles, masks, retouch, diffuse presets

/// A linear Rec.2020 scene: gradients, texture and a few highlights over 1.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let tex = 0.05 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            let hi = if ((fx * 6.0).fract() - 0.5).abs() < 0.05 && fy < 0.2 {
                2.0
            } else {
                0.0
            };
            v.extend_from_slice(&[
                0.02 + 0.5 * fx + tex + hi,
                0.03 + 0.4 * fy + tex + hi,
                0.04 + 0.3 * (1.0 - fx) + tex + hi,
                1.0,
            ]);
        }
    }
    v
}

fn engine(b: &Bench) -> Engine {
    Engine::new(&b.device, &b.queue, PoolBudget::fixed(3 << 30))
}

fn graph(
    e: &Edit,
    w: u32,
    h: u32,
    frame: (u32, u32),
    cache: &mut MaskCache,
    focus: Option<(ModuleKind, u16)>,
) -> Graph {
    let mut env = MaskEnv {
        geometry: Geometry::Frame {
            w: f64::from(frame.0),
            h: f64::from(frame.1),
        },
        image: (w, h),
        scale: w as f32 / frame.0 as f32,
        cache,
        sensor_bin: 1.0,
        overlay: None,
    };
    let out = OutputSpace::rec709();
    Graph::build_masked(e, OpRegistry::builtin(), focus, Some(&out), Some(&mut env)).expect("graph")
}

/// One render of `g` on `src`: wall to the GPU being done, and GPU time when profiled.
fn render(
    en: &mut Engine,
    g: &Graph,
    src: &specular_develop::gpu::Source,
    req: &Request,
) -> Sample {
    let t = Instant::now();
    let r = en.render(g, src, req).expect("render");
    en.wait().expect("wait");
    let wall = ms(t);
    let gpu = r.stats.gpu_total_ms();
    breakdown(&r.stats);
    en.recycle(r);
    Sample { wall, gpu }
}

/// Pan at 1:1 with the full edit: new 512² tiles of a 24 MP picture.
pub fn tiles(b: &mut Bench) {
    let name = "pan-1to1/tile-512";
    if !b.wants(name) {
        return;
    }
    // A 24 MP f32 source is one 384 MB buffer: more than a software adapter binds (128 MB
    // on lavapipe), where the picture is made smaller to fit (noted in the results).
    let max = b.device.limits().max_storage_buffer_binding_size;
    let (w, h, note) = if 6000 * 4000 * 16 <= max {
        (
            6000u32,
            4000u32,
            "a tile not rendered before (the view pans by one tile)",
        )
    } else {
        (
            2880,
            1920,
            "a tile not rendered before; 5.5 MP source (24 MP exceeds the adapter's binding limit)",
        )
    };
    let mut en = engine(b);
    let src = en.upload(w, h, &scene(w, h)).expect("upload");
    let edit = default_edit(None);
    let mut cache = MaskCache::default();
    let g = graph(&edit, w, h, (w, h), &mut cache, None);
    let profile = b.timestamps;
    let (cols, rows) = ((w - 160) / 512, (h - 240) / 512);
    b.case(name, "pan", true, budget(16.0, Some(33.0)), note, |b| {
        let n = b.count(40);
        let mut out = Vec::new();
        // Walk the tiles row by row; each is new to the cache.
        for i in 0..n + 3 {
            let (tx, ty) = ((i as u32 * 3) % cols, (i as u32 / cols) % rows);
            let req = Request {
                profile,
                ..Request::new(Roi::new(tx * 512 + 80, ty * 512 + 120, 512, 512))
            };
            // A pan keeps the statistics (the edit is the same): only the tiles' work is new.
            en.drop_checkpoints();
            let s = render(&mut en, &g, &src, &req);
            if i >= 3 {
                out.push(s);
            }
        }
        out
    });
}

/// The default edit with a brush-masked exposure (instance 1) of one dab.
fn brush_edit(input: Option<&Input>) -> Edit {
    let mut e = default_edit(input);
    let mut m = ModuleInstance::new(Registry::builtin(), ModuleKind::EXPOSURE, 1);
    m.params = Params::from_fields(
        ModuleKind::EXPOSURE,
        Fields::new().with("exposure_ev", Value::Float(1.0)),
    );
    m.blend = Some(Blend {
        mask: MaskExpr::Shape(1),
        ..Blend::default()
    });
    e.insert(Registry::builtin(), m);
    e.masks = vec![MaskShape {
        id: 1,
        shape: Shape::Brush {
            strokes: vec![Stroke {
                radius: 0.02,
                hardness: 0.5,
                flow: 0.6,
                erase: false,
                pressure: PressureUse::Flow,
                points: vec![[0.2, 0.5, 1.0]],
            }],
        },
    }];
    e
}

/// Adds the next dab of the stroke.
fn dab(e: &mut Edit) {
    if let Shape::Brush { strokes } = &mut e.masks[0].shape {
        let k = strokes[0].points.len() as f32;
        strokes[0]
            .points
            .push([(0.2 + 0.004 * k) % 0.9, 0.5 + 0.1 * (k * 0.2).sin(), 1.0]);
    }
}

/// A brush stroke growing by one dab per frame on a masked exposure, as the app paints it
/// (fit view of the 24 MP raw, the masked module in focus: a draft), and on the engine at
/// full scale (no budget: the refined picture's cost).
pub fn masks(b: &mut Bench, raw: &Raw) {
    let name = "mask/brush-dab";
    if b.wants(name) {
        let input = preview::decode_raw(&std::fs::read(raw.path()).expect("read")).expect("decode");
        let mut v = View::new(b, input.clone(), brush_edit(Some(&input)));
        v.frame(None);
        let focus = Some((ModuleKind::EXPOSURE, 1));
        b.case(
            name,
            "mask",
            true,
            budget(16.0, Some(33.0)),
            "stamp one dab, render + present (masked module in focus)",
            |b| {
                let n = b.count(60);
                let mut out = Vec::new();
                for i in 0..n + 5 {
                    dab(&mut v.edit);
                    let wall = v.frame(focus);
                    if i >= 5 {
                        out.push(Sample { wall, gpu: None });
                    }
                }
                if b.timestamps {
                    for s in out.iter_mut().take(b.count(20)) {
                        dab(&mut v.edit);
                        s.gpu = v.gpu(focus);
                    }
                }
                out
            },
        );
    }
    let name = "mask/brush-dab/engine";
    if !b.wants(name) {
        return;
    }
    let (w, h) = (2560u32, 1707u32);
    let frame = (6000u32, 4000u32);
    let mut en = engine(b);
    let src = en.upload(w, h, &scene(w, h)).expect("upload");
    let mut cache = MaskCache::default();
    let mut e = brush_edit(None);
    let focus = Some((ModuleKind::EXPOSURE, 1));
    let profile = b.timestamps;
    b.case(
        name,
        "mask",
        true,
        None,
        "stamp one dab, build the graph, render at full scale on the engine",
        |b| {
            let n = b.count(60);
            let mut out = Vec::new();
            for i in 0..n + 5 {
                dab(&mut e);
                let t = Instant::now();
                let g = graph(&e, w, h, frame, &mut cache, focus);
                let req = Request {
                    profile,
                    scale: w as f32 / frame.0 as f32,
                    ..Request::new(Roi::new(0, 0, w, h))
                };
                let mut s = render(&mut en, &g, &src, &req);
                s.wall = ms(t);
                if i >= 5 {
                    out.push(s);
                }
            }
            out
        },
    );
}

fn retouch_edit(r: &Retouch) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    let mut m = ModuleInstance::new(Registry::builtin(), ModuleKind::RETOUCH, 0);
    m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
    e.insert(Registry::builtin(), m);
    e
}

/// Dragging one of ten heal spots: the draft while dragging, the refined render after.
pub fn retouch(b: &mut Bench) {
    let (dn, rn) = ("retouch/drag/draft", "retouch/drag/refined");
    if !b.wants(dn) && !b.wants(rn) {
        return;
    }
    let (w, h) = (2560u32, 1707u32);
    let frame = (6000u32, 4000u32);
    let mut en = engine(b);
    let src = en.upload(w, h, &scene(w, h)).expect("upload");
    let mut cache = MaskCache::default();
    let mut r = Retouch::default();
    for i in 0..10 {
        let f = i as f32 / 10.0;
        r.spots.push(Spot {
            target: [0.1 + 0.8 * f, 0.3 + 0.4 * (f * 6.0).sin().abs()],
            source: [0.15 + 0.7 * f, 0.6],
            size: 0.02,
            feather: 0.3,
            opacity: 1.0,
            ..Spot::default()
        });
    }
    let profile = b.timestamps;
    let scale = w as f32 / frame.0 as f32;
    for draft in [true, false] {
        let name = if draft { dn } else { rn };
        let focus = draft.then_some((ModuleKind::RETOUCH, 0));
        let bud = if draft {
            budget(16.0, Some(33.0))
        } else {
            budget(300.0, Some(500.0))
        };
        b.case(
            name,
            "retouch",
            true,
            bud,
            "one spot moved, nine kept; budget of the brush row (draft) or heavy refined",
            |b| {
                let n = b.count(if draft { 40 } else { 12 });
                let mut out = Vec::new();
                for i in 0..n + 3 {
                    r.spots[4].target = [0.45 + 0.002 * (i % 50) as f32, 0.5];
                    let g = graph(&retouch_edit(&r), w, h, frame, &mut cache, focus);
                    let req = Request {
                        profile,
                        scale,
                        ..Request::new(Roi::new(0, 0, w, h))
                    };
                    let s = render(&mut en, &g, &src, &req);
                    if i >= 3 {
                        out.push(s);
                    }
                }
                out
            },
        );
    }
}

/// The diffuse presets at fit view (2560×1600), draft and refined (heavy-op budget).
pub fn diffuse(b: &mut Bench) {
    let (w, h) = WINDOW;
    let names: Vec<String> = Diffuse::ALL
        .iter()
        .flat_map(|p| ["draft", "refined"].map(|d| format!("diffuse/{p:?}/{d}")))
        .collect();
    if !b.wants_any(&names) {
        return;
    }
    let mut en = engine(b);
    let src = en.upload(w, h, &scene(w, h)).expect("upload");
    let mut cache = MaskCache::default();
    let profile = b.timestamps;
    for p in Diffuse::ALL {
        let mut e = Edit::empty(SourceKey::default());
        add(
            &mut e,
            ModuleKind::EXPOSURE,
            Fields::new().with("exposure_ev", Value::Float(0.01)),
        );
        let mut f = p.params().to_fields();
        f.set(LONG_FIELD, Value::Float(w as f32));
        add(&mut e, ModuleKind::DIFFUSE, f);
        for draft in [true, false] {
            let name = format!("diffuse/{p:?}/{}", if draft { "draft" } else { "refined" });
            let focus = draft.then_some((ModuleKind::DIFFUSE, 0));
            let g = graph(&e, w, h, (w, h), &mut cache, focus);
            let bud = if draft {
                budget(50.0, None)
            } else {
                budget(300.0, Some(500.0))
            };
            b.case(&name, "diffuse", true, bud, "f16, checkpoints off", |b| {
                let n = b.count(if draft { 10 } else { 5 });
                let req = Request {
                    profile,
                    checkpoints: false,
                    ..Request::new(Roi::new(0, 0, w, h))
                };
                let mut out = Vec::new();
                for i in 0..n + 1 {
                    let s = render(&mut en, &g, &src, &req);
                    if i >= 1 {
                        out.push(s);
                    }
                }
                out
            });
        }
    }
}
