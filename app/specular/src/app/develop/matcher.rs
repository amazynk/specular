//! Match to reference in the app: the reference and each target are read in the
//! background (the photo and its saved edit), developed small on the GPU up to their
//! exposure (`Developer::scene_pixels`), and solved on the CPU
//! (`specular_develop::matching`). One photo is developed per frame.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, unbounded};
use specular_develop::gpu::pool::Budget;
use specular_develop::matching::{self, Scene, Solved};
use specular_develop::model::{Edit, History, ModuleKind, Value};
use specular_develop::preview::{Developer, Input};
use specular_gfx::Gpu;
use specular_library::Library;

use crate::loader::Loader;

/// The long side the photos are developed at to be measured (px).
const SIDE: u32 = 384;
/// Values at or above this count as clipped.
const CLIP: f32 = 0.98;

/// A photo read for matching.
struct Read {
    path: PathBuf,
    reference: bool,
    input: Result<(Input, Option<[f64; 2]>), String>,
    edit: Edit,
    history: History,
    name: String,
}

/// A target solved: its new edit as one history step, or why not.
pub(super) struct Matched {
    pub path: PathBuf,
    pub result: Result<(Edit, History, String), String>,
}

/// The work in progress.
#[derive(Default)]
pub(super) struct Matcher {
    rx: Option<Receiver<Read>>,
    dev: Option<Developer>,
    reference: Option<(Scene, Edit)>,
    /// Targets read before the reference was measured.
    waiting: VecDeque<Read>,
    pub done: Vec<Matched>,
    /// Targets still to come.
    pub left: usize,
    /// The reference could not be measured: why.
    pub failed: Option<String>,
}

/// The edit with the as-shot white the photo was read with (as opening it would set).
fn with_as_shot(mut e: Edit, white: Option<[f64; 2]>) -> Edit {
    if let Some(w) = white
        && let Some(m) = e.module_mut(ModuleKind::COLOUR_CALIBRATION, 0)
    {
        let mut f = m.params.to_fields();
        let custom = f.text("white_source") == Some("custom");
        f.set("as_shot_x", Value::Float(w[0] as f32));
        f.set("as_shot_y", Value::Float(w[1] as f32));
        if !custom {
            f.set("white_x", Value::Float(w[0] as f32));
            f.set("white_y", Value::Float(w[1] as f32));
        }
        m.params.set_fields(f);
    }
    e
}

impl Matcher {
    pub fn busy(&self) -> bool {
        self.left > 0
    }

    /// Starts matching `targets` to `reference` (photo and edit read in the background,
    /// the open ones' edits given in `open`).
    pub fn start(
        &mut self,
        reference: &Path,
        targets: Vec<PathBuf>,
        open: Vec<(PathBuf, Edit, History, String)>,
        lib: Option<Arc<Library>>,
        loader: &Loader,
    ) {
        let (tx, rx) = unbounded();
        self.rx = Some(rx);
        self.reference = None;
        self.waiting.clear();
        self.failed = None;
        self.left = targets.len();
        let registry = loader.registry();
        let mut all = vec![(reference.to_path_buf(), true)];
        all.extend(targets.into_iter().map(|p| (p, false)));
        loader.background("develop-match", move |l| {
            for (p, reference) in all {
                let (edit, history, name) = match open.iter().find(|o| o.0 == p) {
                    Some(o) => (o.1.clone(), o.2.clone(), o.3.clone()),
                    None => match super::store::load_version(&p, 0, lib.as_deref()) {
                        Some(f) => (f.edit, f.history, f.name),
                        None => (
                            super::controls::new_edit(&p),
                            History::default(),
                            String::new(),
                        ),
                    },
                };
                let input = super::preview::read_source(&registry, &p);
                if tx
                    .send(Read {
                        path: p,
                        reference,
                        input,
                        edit,
                        history,
                        name,
                    })
                    .is_err()
                {
                    return;
                }
                l.waker()();
            }
        });
    }

    /// Measures one photo read (on the GPU, from the draw); whether there is more to do.
    pub fn step(&mut self, gpu: &Gpu) -> bool {
        let next = match &self.reference {
            Some(_) => self
                .waiting
                .pop_front()
                .or_else(|| self.rx.as_ref().and_then(|r| r.try_recv().ok())),
            None => {
                let got = self.rx.as_ref().and_then(|r| r.try_recv().ok());
                match got {
                    Some(r) if !r.reference => {
                        self.waiting.push_back(r);
                        return true;
                    }
                    r => r,
                }
            }
        };
        let Some(read) = next else {
            return self.busy();
        };
        let dev = self.dev.get_or_insert_with(|| {
            Developer::new(
                &gpu.device,
                &gpu.queue,
                Budget::for_device(true, None, None),
            )
        });
        let measured = read.input.and_then(|(input, white)| {
            let edit = with_as_shot(read.edit.clone(), white);
            dev.set_input(input).map_err(|e| e.to_string())?;
            dev.prepare(&edit, SIDE).map_err(|e| e.to_string())?;
            let (_, _, px) = dev
                .scene_pixels(&edit, ModuleKind::EXPOSURE)
                .map_err(|e| e.to_string())?;
            let scene = Scene::from_rgba(&px, CLIP).ok_or_else(String::new)?;
            Ok((scene, edit))
        });
        if read.reference {
            match measured {
                Ok(m) => self.reference = Some(m),
                Err(e) => {
                    self.failed = Some(e);
                    self.left = 0;
                    self.waiting.clear();
                    self.rx = None;
                }
            }
            return self.busy();
        }
        self.left = self.left.saturating_sub(1);
        let result = match (&self.reference, measured) {
            (Some((rs, re)), Ok((scene, edit))) => match matching::solve(rs, re, &scene, &edit) {
                Some(s) => Ok(apply(edit, read.history, read.name, &s)),
                None => Err(String::new()),
            },
            (_, Err(e)) => Err(e),
            (None, _) => Err(String::new()),
        };
        self.done.push(Matched {
            path: read.path,
            result,
        });
        if self.left == 0 {
            self.dev = None;
            self.rx = None;
        }
        self.busy()
    }
}

/// The solved exposure and white as one history step.
fn apply(edit: Edit, mut history: History, name: String, s: &Solved) -> (Edit, History, String) {
    let mut e = edit;
    let after = matching::applied(&e, s);
    let what = specular_i18n::t!("develop-history-match");
    let step = specular_develop::model::Step::stack(&what, &e, &after, now_ms());
    if history.push(&mut e, step).is_ok() {
        history.trim(specular_develop::model::history::BUDGET);
    }
    (e, history, name)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
