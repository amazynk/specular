//! Running a model on a thumbnail.
//!
//! A [`Runtime`] is one loaded model: thumbnail pixels in, [`Outputs`] out, on one of the
//! engines it supports. Which engine runs a given thumbnail is the PathSelector's choice
//! (`Task::Inference`, DESIGN §12.3.1) through [`Dispatch`]: the CPU is the reference, an
//! accelerator is used once it has matched the reference, and the measured times teach the
//! selector which is faster on this machine. [`crate::infer::InferRuntime`] runs a model
//! pack on our own engine; no pack ships yet (which models the pack uses is decision 7), and
//! [`StubRuntime`] stands in for tests.

use std::sync::Arc;

use specular_core::paths::{
    Engine, Failure, Objective, PathSelector, Reason, Request, Task, Trust,
};

use crate::infer::GpuCheck;
use crate::manifest::Feature;

/// A cached thumbnail, RGBA8.
#[derive(Clone, Copy, Debug)]
pub struct Thumb<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// A face box, in fractions of the image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaceBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
    /// Face embedding, f16 bits (empty without one).
    pub vec: Vec<u16>,
    /// The eye line's angle in the image, radians, clockwise (0 upright).
    pub roll: f32,
}

/// What a model gives for one thumbnail.
#[derive(Clone, Debug, PartialEq)]
pub enum Outputs {
    /// Unit-length image embedding, f16 bits.
    Embedding(Vec<u16>),
    Faces(Vec<FaceBox>),
    /// Text found, and its boxes as JSON.
    Text {
        text: String,
        boxes: String,
    },
    Scores(specular_library::ai::Scores),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The engine can't run this model (the dispatcher falls back to the CPU).
    Unsupported,
    Failed(String),
    /// The GPU device was lost (a driver reset): the CPU takes this run, and the path stays
    /// open for the new device.
    Lost,
}

impl From<specular_infer::Error> for RunError {
    fn from(e: specular_infer::Error) -> RunError {
        match e {
            specular_infer::Error::Lost => RunError::Lost,
            e => RunError::Failed(e.to_string()),
        }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Unsupported => f.write_str("engine not supported"),
            RunError::Failed(s) => f.write_str(s),
            RunError::Lost => f.write_str("the GPU device was lost"),
        }
    }
}

/// One loaded model.
pub trait Runtime: Send + Sync {
    fn feature(&self) -> Feature;
    /// `<id>-<version>`: what the catalogue marks work done with.
    fn model(&self) -> &str;
    /// The thumbnail side it wants (the job picks the cached size class at least this big).
    fn input_side(&self) -> u32;
    /// Engines it can run on; the CPU first, always.
    fn engines(&self) -> &[Engine];
    fn run(&self, engine: Engine, thumb: &Thumb) -> Result<Outputs, RunError>;
}

/// The cosine two embeddings need to count as the same output.
pub(crate) const AGREE: f32 = 0.999;

/// Whether two outputs agree closely enough for an accelerator to be trusted.
pub fn agree(a: &Outputs, b: &Outputs) -> bool {
    match (a, b) {
        (Outputs::Embedding(x), Outputs::Embedding(y)) => cosine(x, y) >= AGREE,
        (Outputs::Faces(x), Outputs::Faces(y)) => {
            x.len() == y.len()
                && x.iter().zip(y).all(|(p, q)| {
                    (p.x - q.x).abs() < 0.01
                        && (p.y - q.y).abs() < 0.01
                        && (p.w - q.w).abs() < 0.01
                        && (p.h - q.h).abs() < 0.01
                })
        }
        (Outputs::Text { text: x, .. }, Outputs::Text { text: y, .. }) => x == y,
        (Outputs::Scores(x), Outputs::Scores(y)) => {
            let c = |p: Option<f32>, q: Option<f32>| match (p, q) {
                (Some(p), Some(q)) => (p - q).abs() < 0.02,
                (None, None) => true,
                _ => false,
            };
            c(x.sharpness, y.sharpness)
                && c(x.eyes_open, y.eyes_open)
                && c(x.eye_focus, y.eye_focus)
                && c(x.expression, y.expression)
        }
        _ => false,
    }
}

/// How close two embeddings are (their cosine), for the log; `None` for other outputs.
pub(crate) fn similarity(a: &Outputs, b: &Outputs) -> Option<f32> {
    match (a, b) {
        (Outputs::Embedding(x), Outputs::Embedding(y)) => Some(cosine(x, y)),
        _ => None,
    }
}

fn cosine(a: &[u16], b: &[u16]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let f = |h: u16| half::f16::from_bits(h).to_f32();
    let (mut ab, mut aa, mut bb) = (0f32, 0f32, 0f32);
    for (&x, &y) in a.iter().zip(b) {
        let (x, y) = (f(x), f(y));
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 && bb == 0.0 {
        return 1.0;
    }
    ab / (aa.sqrt() * bb.sqrt()).max(f32::MIN_POSITIVE)
}

/// Runs a model where the PathSelector says, as background work.
pub struct Dispatch {
    runtime: Arc<dyn Runtime>,
    selector: &'static PathSelector,
    /// The engine last logged as running the model (the log says when it changes).
    logged: parking_lot::Mutex<Option<Engine>>,
}

impl Dispatch {
    /// Tells the selector which engines the runtime has (unverified until they match the CPU).
    pub fn new(runtime: Arc<dyn Runtime>, selector: &'static PathSelector) -> Dispatch {
        for &e in runtime.engines() {
            if e != Engine::Cpu && selector.trust(Task::Inference, path_of(e)) == Trust::Absent {
                selector.set_available(Task::Inference, e);
            }
        }
        Dispatch {
            runtime,
            selector,
            logged: parking_lot::Mutex::new(None),
        }
    }

    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.runtime
    }

    /// Runs the model on `thumb`. An unverified accelerator runs alongside the CPU
    /// reference; it is trusted if they agree and blocked if not, and the reference's output
    /// is used. A failing accelerator falls back to the CPU.
    pub fn run(&self, thumb: &Thumb) -> Result<Outputs, RunError> {
        let pixels = thumb.width as u64 * thumb.height as u64;
        let choice = self.selector.choose(Task::Inference, &request(pixels));
        let supported = self.runtime.engines().contains(&choice.engine);
        let choice = if supported {
            choice
        } else {
            choice.reference()
        };
        {
            let mut logged = self.logged.lock();
            if *logged != Some(choice.engine) {
                *logged = Some(choice.engine);
                specular_log::info!(target: specular_log::JOBS,
                    "ai: {} runs on {:?}{}", self.runtime.model(), choice.engine,
                    if choice.verify { " (checked against the CPU)" } else { "" });
            }
        }
        let timed = |c: &specular_core::paths::Choice| {
            let run = self.selector.start(c);
            let out = self.runtime.run(c.engine, thumb);
            if out.is_ok() {
                run.finish();
            }
            out
        };
        if choice.is_reference() {
            return timed(&choice);
        }
        let fast = timed(&choice);
        match fast {
            Ok(out) if !choice.verify => Ok(out),
            Ok(out) => {
                let reference = timed(&choice.reference())?;
                let ok = agree(&out, &reference);
                self.selector.verified(Task::Inference, choice.engine, ok);
                if !ok {
                    specular_log::warn!(target: specular_log::JOBS,
                        "ai: {} on {:?} differs from the CPU: not used again",
                        self.runtime.model(), choice.engine);
                }
                Ok(reference)
            }
            Err(e) => {
                specular_log::warn!(target: specular_log::JOBS,
                    "ai: {} failed on {:?}, falls back to the CPU: {e}",
                    self.runtime.model(), choice.engine);
                if !matches!(e, RunError::Unsupported | RunError::Lost) {
                    self.selector
                        .block(Task::Inference, choice.engine, &e.to_string());
                }
                timed(&choice.reference())
            }
        }
    }
}

/// The selector request of one [`Dispatch::run`]: background work that can check an
/// unverified accelerator (it runs the CPU reference alongside; after "Measure again" the GPU
/// is unverified until such a run matches).
pub fn request(pixels: u64) -> Request {
    Request::new(pixels, 0)
        .objective(Objective::Background)
        .verifying()
}

/// Where a loaded model's background work runs, for Settings › Local AI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsOn {
    /// The GPU: it matched the CPU reference and the selector picks it.
    Gpu,
    /// The CPU, and why.
    Cpu(CpuWhy),
}

/// Why a model runs on the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuWhy {
    /// The runtime was given no GPU.
    NoGpu,
    /// The GPU's output differed from the CPU reference: not used on this machine.
    Differed,
    /// The model could not be built or run on the GPU, or the GPU path failed later.
    Failed,
    /// Settings › Performance asks for the processor only.
    Setting,
    /// The GPU is being checked against the CPU again ("Measure again"); background runs
    /// check it.
    Checking,
    /// The selector picks the CPU here (measured cheaper, or saving power).
    Chosen,
}

impl RunsOn {
    /// Where [`Dispatch::run`] sends a `side`² thumbnail now, from the runtime's `engines` and
    /// its GPU check at load and the selector's state (trust, blocklist, timings, settings),
    /// without exploring: what the runs do apart from the occasional re-check of the other
    /// engine.
    pub fn of(engines: &[Engine], check: &GpuCheck, sel: &PathSelector, side: u32) -> RunsOn {
        let path = path_of(Engine::Gpu);
        let differed = *check == GpuCheck::Differed;
        match sel.trust(Task::Inference, path) {
            Trust::Blocked => {
                let checked = sel
                    .blocked_info(Task::Inference, path)
                    .is_some_and(|b| b.failure == Failure::CheckFailed);
                return RunsOn::Cpu(if checked || differed {
                    CpuWhy::Differed
                } else {
                    CpuWhy::Failed
                });
            }
            _ if differed => return RunsOn::Cpu(CpuWhy::Differed),
            _ => {}
        }
        match check {
            GpuCheck::Failed(_) => return RunsOn::Cpu(CpuWhy::Failed),
            GpuCheck::NotTried => return RunsOn::Cpu(CpuWhy::NoGpu),
            GpuCheck::Matched | GpuCheck::Differed => {}
        }
        if !engines.contains(&Engine::Gpu) {
            return RunsOn::Cpu(CpuWhy::Failed);
        }
        let side = u64::from(side.max(1));
        let c = sel.preview(Task::Inference, &request(side * side));
        match (c.engine, c.reason) {
            (Engine::Gpu, _) => RunsOn::Gpu,
            (_, Reason::CpuOnly | Reason::Forced) => RunsOn::Cpu(CpuWhy::Setting),
            _ if sel.trust(Task::Inference, path) == Trust::Unverified => {
                RunsOn::Cpu(CpuWhy::Checking)
            }
            _ => RunsOn::Cpu(CpuWhy::Chosen),
        }
    }
}

fn path_of(e: Engine) -> usize {
    Task::Inference.path_of(e).unwrap_or(0)
}

/// A deterministic stand-in model for tests: outputs derived from the pixels.
pub struct StubRuntime {
    pub feature: Feature,
    pub model: String,
    pub dims: usize,
    pub engines: Vec<Engine>,
    /// Make the GPU disagree with the CPU (tests the verification).
    pub gpu_differs: bool,
    /// The GPU device is lost: its runs fail with [`RunError::Lost`].
    pub gpu_lost: bool,
    /// How many runs were asked of the GPU.
    pub gpu_runs: std::sync::atomic::AtomicUsize,
}

impl StubRuntime {
    pub fn new(feature: Feature, model: &str) -> StubRuntime {
        StubRuntime {
            feature,
            model: model.into(),
            dims: 512,
            engines: vec![Engine::Cpu],
            gpu_differs: false,
            gpu_lost: false,
            gpu_runs: Default::default(),
        }
    }
}

impl Runtime for StubRuntime {
    fn feature(&self) -> Feature {
        self.feature
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn input_side(&self) -> u32 {
        128
    }

    fn engines(&self) -> &[Engine] {
        &self.engines
    }

    fn run(&self, engine: Engine, thumb: &Thumb) -> Result<Outputs, RunError> {
        if !self.engines.contains(&engine) {
            return Err(RunError::Unsupported);
        }
        if thumb.rgba.len() != thumb.width as usize * thumb.height as usize * 4 {
            return Err(RunError::Failed("bad thumbnail".into()));
        }
        if engine == Engine::Gpu {
            self.gpu_runs
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if engine == Engine::Gpu && self.gpu_lost {
            return Err(RunError::Lost);
        }
        let mean = |c: usize| {
            let n = (thumb.rgba.len() / 4).max(1);
            thumb
                .rgba
                .iter()
                .skip(c)
                .step_by(4)
                .map(|&v| v as f32)
                .sum::<f32>()
                / n as f32
                / 255.0
        };
        let (r, g, b) = (mean(0), mean(1), mean(2));
        let flip = if engine != Engine::Cpu && self.gpu_differs {
            -1.0
        } else {
            1.0
        };
        Ok(match self.feature {
            Feature::Search => {
                let v: Vec<f32> = (0..self.dims)
                    .map(|i| {
                        let t = i as f32 * 0.37;
                        flip * (r * t.sin() + g * (t * 1.3).cos() + b * (t * 0.7).sin() + 0.01)
                    })
                    .collect();
                Outputs::Embedding(crate::search::normalize_f16(&v))
            }
            // Speech never runs on a thumbnail (the app reads sound), nor do masks
            // (Develop runs them on the photo), nor denoise, remove or depth: nothing.
            Feature::Speech
            | Feature::Masks
            | Feature::Denoise
            | Feature::Remove
            | Feature::Depth => Outputs::Text {
                text: String::new(),
                boxes: "[]".into(),
            },
            Feature::Text => Outputs::Text {
                text: if r > 0.5 {
                    "STOP".into()
                } else {
                    String::new()
                },
                boxes: "[]".into(),
            },
            Feature::Culling => Outputs::Scores(specular_library::ai::Scores {
                sharpness: Some(g),
                ..Default::default()
            }),
            Feature::People => Outputs::Faces(if b > 0.5 {
                vec![FaceBox {
                    x: 0.25,
                    y: 0.25,
                    w: 0.5 * flip.abs(),
                    h: if flip < 0.0 { 0.1 } else { 0.5 },
                    score: 0.9,
                    vec: crate::search::normalize_f16(&[r, g, b, 1.0]),
                    roll: 0.0,
                }]
            } else {
                Vec::new()
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use specular_core::paths::{EXPLORE_EVERY, GpuClass, Mode};

    use super::*;

    fn thumb(rgba: &[u8]) -> Thumb<'_> {
        Thumb {
            width: 2,
            height: 2,
            rgba,
        }
    }

    #[test]
    fn stub_is_deterministic() {
        let px = [200u8, 10, 30, 255].repeat(4);
        let s = StubRuntime::new(Feature::Search, "stub-1");
        let a = s.run(Engine::Cpu, &thumb(&px)).unwrap();
        assert_eq!(a, s.run(Engine::Cpu, &thumb(&px)).unwrap());
        let Outputs::Embedding(v) = &a else { panic!() };
        assert_eq!(v.len(), 512);
        assert!(agree(&a, &a));
        assert_eq!(s.run(Engine::Gpu, &thumb(&px)), Err(RunError::Unsupported));
    }

    #[test]
    fn unverified_gpu_is_checked_against_the_cpu() {
        static SEL: PathSelector = PathSelector::new();
        let px = [10u8, 10, 250, 255].repeat(4);
        let mut stub = StubRuntime::new(Feature::People, "faces-1");
        stub.engines = vec![Engine::Cpu, Engine::Gpu];
        stub.gpu_differs = true;
        let d = Dispatch::new(Arc::new(stub), &SEL);
        // Whatever path is chosen, the output is the CPU reference's.
        for _ in 0..20 {
            let out = d.run(&thumb(&px)).unwrap();
            let Outputs::Faces(f) = out else { panic!() };
            assert_eq!(f[0].h, 0.5);
        }
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        assert_ne!(SEL.trust(Task::Inference, gpu), Trust::Trusted);
    }

    /// A run on a lost device (a driver reset) falls back to the CPU and leaves the GPU path
    /// open for the new device; any other GPU failure blocks it.
    #[test]
    fn a_lost_device_does_not_block_the_gpu() {
        static SEL: PathSelector = PathSelector::new();
        SEL.set_machine(GpuClass::Integrated, 8);
        let px = [10u8, 120, 250, 255].repeat(4);
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        SEL.verified(Task::Inference, Engine::Gpu, true);
        let mut stub = StubRuntime::new(Feature::Search, "embed-1");
        stub.engines = vec![Engine::Cpu, Engine::Gpu];
        let want = stub.run(Engine::Cpu, &thumb(&px)).unwrap();
        stub.gpu_lost = true;
        let stub = Arc::new(stub);
        let d = Dispatch::new(stub.clone(), &SEL);
        let key = SEL.key_of(&request(4));
        for _ in 0..32 {
            SEL.record(Task::Inference, 0, key, 0.01, 5e5, None);
        }
        for _ in 0..2 * EXPLORE_EVERY {
            assert_eq!(d.run(&thumb(&px)).unwrap(), want);
        }
        assert!(stub.gpu_runs.load(std::sync::atomic::Ordering::Relaxed) > 0);
        assert_eq!(SEL.trust(Task::Inference, gpu), Trust::Trusted);
        assert_eq!(RunError::from(specular_infer::Error::Lost), RunError::Lost);
        assert!(matches!(
            RunError::from(specular_infer::Error::Gpu("x".into())),
            RunError::Failed(_)
        ));
    }

    #[test]
    fn agreeing_gpu_becomes_trusted() {
        static SEL: PathSelector = PathSelector::new();
        let px = [10u8, 120, 250, 255].repeat(4);
        let mut stub = StubRuntime::new(Feature::Search, "embed-1");
        stub.engines = vec![Engine::Cpu, Engine::Gpu];
        let d = Dispatch::new(Arc::new(stub), &SEL);
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        for _ in 0..50 {
            d.run(&thumb(&px)).unwrap();
        }
        let t = SEL.trust(Task::Inference, gpu);
        assert!(matches!(t, Trust::Trusted | Trust::Unverified), "{t:?}");
    }

    #[test]
    fn a_gpu_unverified_again_is_checked_by_the_job() {
        // "Measure again" makes the trusted GPU unverified mid-session: the job's own runs
        // check it again (before, its requests could not verify and it stayed on the CPU).
        static SEL: PathSelector = PathSelector::new();
        SEL.set_machine(GpuClass::Integrated, 8);
        let px = [10u8, 120, 250, 255].repeat(4);
        let mut stub = StubRuntime::new(Feature::Search, "embed-1");
        stub.engines = vec![Engine::Cpu, Engine::Gpu];
        SEL.verified(Task::Inference, Engine::Gpu, true);
        let d = Dispatch::new(Arc::new(stub), &SEL);
        SEL.reset_measurements();
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        assert_eq!(SEL.trust(Task::Inference, gpu), Trust::Unverified);
        assert_eq!(
            RunsOn::of(d.runtime().engines(), &GpuCheck::Matched, &SEL, 256),
            RunsOn::Cpu(CpuWhy::Checking)
        );
        // A CPU pass as slow as a real model's (the stub's takes microseconds, which would
        // put the GPU's prior too far off to explore).
        let key = SEL.key_of(&request(4));
        for _ in 0..32 {
            SEL.record(Task::Inference, 0, key, 0.01, 5e5, None);
        }
        for _ in 0..EXPLORE_EVERY {
            d.run(&thumb(&px)).unwrap();
        }
        assert_eq!(SEL.trust(Task::Inference, gpu), Trust::Trusted);
        assert_eq!(
            RunsOn::of(d.runtime().engines(), &GpuCheck::Matched, &SEL, 256),
            RunsOn::Gpu
        );
    }

    #[test]
    fn runs_on_follows_the_check_and_the_selector() {
        let both = [Engine::Cpu, Engine::Gpu];
        let cpu = [Engine::Cpu];
        let fresh = || {
            let s = PathSelector::new();
            s.set_machine(GpuClass::Integrated, 8);
            s
        };
        let s = fresh();
        let on = |s: &PathSelector, e: &[Engine], c: &GpuCheck| RunsOn::of(e, c, s, 256);
        assert_eq!(
            on(&s, &cpu, &GpuCheck::NotTried),
            RunsOn::Cpu(CpuWhy::NoGpu)
        );
        assert_eq!(
            on(&s, &cpu, &GpuCheck::Failed("out of memory".into())),
            RunsOn::Cpu(CpuWhy::Failed)
        );
        // A failed check blocks the GPU: it differs, now and in later sessions (NotTried).
        s.verified(Task::Inference, Engine::Gpu, false);
        assert_eq!(
            on(&s, &cpu, &GpuCheck::Differed),
            RunsOn::Cpu(CpuWhy::Differed)
        );
        assert_eq!(
            on(&s, &cpu, &GpuCheck::NotTried),
            RunsOn::Cpu(CpuWhy::Differed)
        );
        // Matched: the GPU, unless the settings, the timings or a later error say otherwise.
        let s = fresh();
        s.verified(Task::Inference, Engine::Gpu, true);
        assert_eq!(on(&s, &both, &GpuCheck::Matched), RunsOn::Gpu);
        s.set_mode(Mode::CpuOnly);
        assert_eq!(
            on(&s, &both, &GpuCheck::Matched),
            RunsOn::Cpu(CpuWhy::Setting)
        );
        s.set_mode(Mode::Automatic);
        let key = s.key_of(&request(256 * 256));
        for _ in 0..32 {
            s.record(Task::Inference, 1, key, 0.0655, 9e6, None);
            s.record(Task::Inference, 0, key, 0.0655, 5e5, Some(4e6));
        }
        assert_eq!(
            on(&s, &both, &GpuCheck::Matched),
            RunsOn::Cpu(CpuWhy::Chosen)
        );
        s.block(Task::Inference, Engine::Gpu, "device lost");
        assert_eq!(
            on(&s, &both, &GpuCheck::Matched),
            RunsOn::Cpu(CpuWhy::Failed)
        );
    }
}
