//! Several IFC models in one scene (`Alt+Shift+M` lists them).
//!
//! "Add model…" (the palette, or A in the list) adds another IFC file to the open model's
//! scene; "Add models from a folder…" (Shift+A) adds every IFC file of a folder. Each one is
//! placed against the opened model by [`specular_bim::federation::align`]: by both map
//! conversions, else the sites' reference points, else shared project coordinates. When it
//! cannot be aligned (only one is georeferenced, or their coordinate systems differ) the
//! user is told why and may place it at the origin.
//!
//! The list: Space shows or hides the chosen model, C colours each model in its own colour,
//! Enter goes to it, Delete removes it from the scene (after asking; the file on disk
//! stays). The opened model stays first and cannot be removed.
//!
//! Every model is read on a worker through the decoders (their memory budget applies), and
//! while it is in the scene it holds a share of the budget for its mesh: a model that would
//! not fit is refused with the memory it needs. The scene is one merged mesh
//! ([`specular_bim::federation::merge`]), so model checks see every model: clashes between
//! models, and IDS, changes and quantities per model.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_bim::apply::{self, ElementLook};
use specular_bim::bcf::HeaderFile;
use specular_bim::federation::{self, Basis, Member, Placement, Unaligned};
use specular_core::budget::Reservation;
use specular_decode::specular_mesh::ifc::data::{self as ifcdata, IfcData};
use specular_decode::specular_mesh::{Limits, Mesh};
use specular_decode::{IfcIndex, MeshModel};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_render_3d::canonical_bounds;

use super::MeshView;
use super::bim::{PanelRow, PanelView};
use crate::app::App;

/// "Remove this model from the scene?"
pub(in crate::app) const REMOVE_ITEM: u64 = crate::app::items::BIM_MODEL_REMOVE;
/// "Place it at the origin?" (a model that cannot be aligned).
pub(in crate::app) const ORIGIN_ITEM: u64 = crate::app::items::BIM_MODEL_ORIGIN;

/// Colours by model (sRGB), in turn.
const COLOURS: [[u8; 4]; 8] = [
    [66, 133, 244, 255],
    [234, 67, 53, 255],
    [52, 168, 83, 255],
    [251, 188, 5, 255],
    [171, 71, 188, 255],
    [0, 172, 193, 255],
    [255, 112, 67, 255],
    [124, 179, 66, 255],
];

/// A model in the scene.
pub(in crate::app) struct Model {
    pub path: PathBuf,
    pub name: String,
    /// As read, in its own frame.
    pub own: MeshModel,
    pub data: Option<Arc<IfcData>>,
    /// Its project (metres) into the scene's (the first model's).
    pub place: Placement,
    pub basis: Basis,
    pub visible: bool,
    /// Its elements in the merged scene.
    pub elements: Range<usize>,
    /// The IFC project's GUID and the file's date, for BCF headers.
    project: Option<String>,
    date: Option<String>,
    /// Its share of the memory budget while it is in the scene.
    _hold: Option<Reservation>,
}

impl Model {
    fn ifc(&self) -> Option<&IfcIndex> {
        self.own.ifc.as_deref()
    }

    /// The BCF header entry naming this model's file.
    pub fn header(&self) -> HeaderFile {
        federation::header_file(&self.name, self.project.as_deref(), self.date.as_deref())
    }

    /// A point of its own mesh frame in the scene's project (metres).
    fn to_project(&self, p: [f64; 3]) -> [f64; 3] {
        let (s, o) = self
            .ifc()
            .map_or((1.0, [0.0; 3]), |i| (i.mesh_metres, i.origin_metres));
        self.place.point(std::array::from_fn(|k| p[k] * s + o[k]))
    }
}

/// A model read, waiting to be placed.
pub(in crate::app) struct Loaded {
    path: PathBuf,
    model: MeshModel,
    data: Option<Arc<IfcData>>,
    project: Option<String>,
    date: Option<String>,
    hold: Option<Reservation>,
}

enum Job {
    /// This many more are being read.
    Reading(usize),
    Added(Box<Result<Loaded, (PathBuf, String)>>),
    /// A folder without IFC files.
    Empty(PathBuf),
}

/// The models of the scene (empty: only the opened file, as before).
pub(in crate::app) struct Fed {
    pub models: Vec<Model>,
    /// Each model drawn in its own colour.
    pub by_model: bool,
    /// Every model merged, as drawn before hiding and colouring.
    pub full: Option<MeshModel>,
    /// Being read.
    adding: usize,
    /// One that could not be aligned, and why, while the user is asked.
    pending: Option<(Loaded, String)>,
    /// Waiting their turn behind the question.
    queued: Vec<Loaded>,
    tx: Sender<Job>,
    rx: Receiver<Job>,
}

impl Default for Fed {
    fn default() -> Fed {
        let (tx, rx) = crossbeam_channel::unbounded();
        Fed {
            models: Vec::new(),
            by_model: false,
            full: None,
            adding: 0,
            pending: None,
            queued: Vec::new(),
            tx,
            rx,
        }
    }
}

impl Fed {
    /// More than one model is in the scene.
    pub fn active(&self) -> bool {
        self.models.len() > 1
    }

    /// The model an element of the merged scene is from.
    pub fn model_of(&self, element: usize) -> Option<usize> {
        if !self.active() {
            return Some(0);
        }
        self.models
            .iter()
            .position(|m| m.elements.contains(&element))
    }

    /// The model's name for lists ("Building-Hvac.ifc").
    pub fn name(&self, k: usize) -> Option<&str> {
        self.models.get(k).map(|m| m.name.as_str())
    }
}

/// The IFC project's GUID and the STEP header's date.
fn project_and_date(model: &MeshModel, data: Option<&IfcData>) -> (Option<String>, Option<String>) {
    let project = data.and_then(|d| {
        d.objects
            .iter()
            .find(|o| o.class.eq_ignore_ascii_case("IFCPROJECT"))
            .map(|o| o.global_id.clone())
    });
    let _ = model;
    (project, None)
}

/// The STEP header's time stamp (`FILE_NAME`'s second value) from the file's start.
fn step_date(head: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(head);
    let at = text.find("FILE_NAME")?;
    let rest = &text[at..];
    // The name, then the time stamp: the second quoted string.
    let mut quotes = rest.match_indices('\'').map(|(i, _)| i);
    let (_, _, a, b) = (
        quotes.next()?,
        quotes.next()?,
        quotes.next()?,
        quotes.next()?,
    );
    let d = rest[a + 1..b].trim();
    (!d.is_empty()).then(|| d.to_owned())
}

/// The first few kilobytes of a file.
fn head_of(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(8192).read_to_end(&mut buf);
    }
    buf
}

/// Reads one model off the window thread.
fn load(path: &Path) -> Result<Loaded, String> {
    let s = specular_decode::Registry::new()
        .open_path(path)
        .map_err(|e| crate::error_text::decode_reason(&e))?;
    let Some(model) = s.mesh().cloned().filter(|m| m.ifc.is_some()) else {
        return Err(t!("fed-note", what = "not-ifc"));
    };
    // Its share of the budget while it is in the scene.
    let hold = specular_core::budget::reserve(federation::member_bytes(&model.mesh))
        .map_err(crate::error_text::needs_memory_text)?;
    let bytes = std::fs::read(path).map_err(|e| crate::error_text::io_reason(&e))?;
    let data = ifcdata::read(&bytes, &Limits::default()).ok().map(Arc::new);
    let (project, _) = project_and_date(&model, data.as_deref());
    let date = step_date(&bytes[..bytes.len().min(8192)]);
    Ok(Loaded {
        path: path.to_path_buf(),
        model,
        data,
        project,
        date,
        hold: Some(hold),
    })
}

/// The IFC files of a folder, by name.
fn ifc_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    e.eq_ignore_ascii_case("ifc") || e.eq_ignore_ascii_case("ifczip")
                })
        })
        .collect();
    v.sort();
    v
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Why a model cannot be aligned, in words.
fn unaligned_text(u: &Unaligned) -> String {
    match u {
        Unaligned::DifferentCrs { first, this } => {
            t!(
                "fed-unaligned",
                why = "crs",
                first = first.clone(),
                this = this.clone()
            )
        }
        Unaligned::OnlyFirst => t!("fed-unaligned", why = "only-first", first = "", this = ""),
        Unaligned::OnlyThis => t!("fed-unaligned", why = "only-this", first = "", this = ""),
        Unaligned::Broken => t!("fed-unaligned", why = "broken", first = "", this = ""),
    }
}

/// How a model was placed, in words.
fn basis_text(b: &Basis) -> String {
    match b {
        Basis::Reference => t!("fed-basis", basis = "reference", crs = ""),
        Basis::Map { crs } => t!(
            "fed-basis",
            basis = if crs.is_some() { "map-crs" } else { "map" },
            crs = crs.clone().unwrap_or_default()
        ),
        Basis::Site => t!("fed-basis", basis = "site", crs = ""),
        Basis::Project => t!("fed-basis", basis = "project", crs = ""),
        Basis::Origin => t!("fed-basis", basis = "origin", crs = ""),
    }
}

impl MeshView {
    /// The opened model as the first of the scene, once.
    fn fed_seed(&mut self, path: Option<PathBuf>, data: Option<Arc<IfcData>>) -> bool {
        if !self.bim.fed.models.is_empty() {
            return true;
        }
        let Some(model) = self.model.clone().filter(|m| m.ifc.is_some()) else {
            return false;
        };
        let path = path.unwrap_or_default();
        let (project, _) = project_and_date(&model, data.as_deref());
        let date = step_date(&head_of(&path));
        let n = model.ifc.as_ref().map_or(0, |i| i.elements.len());
        self.bim.fed.models.push(Model {
            name: file_name(&path),
            path,
            own: model,
            data,
            place: Placement::IDENTITY,
            basis: Basis::Reference,
            visible: true,
            elements: 0..n,
            project,
            date,
            _hold: None,
        });
        true
    }

    /// Merges the models again and shows the scene (refitted when `refit`).
    pub(super) fn fed_rebuild(&mut self, refit: bool) {
        let fed = &mut self.bim.fed;
        if fed.models.is_empty() {
            return;
        }
        if fed.models.len() == 1 {
            // Back to the opened model alone.
            let m = fed.models.remove(0);
            fed.full = None;
            fed.by_model = false;
            self.model = Some(m.own);
        } else {
            let members: Vec<Member> = fed
                .models
                .iter()
                .filter_map(|m| {
                    let i = m.ifc()?;
                    Some(Member {
                        mesh: &m.own.mesh,
                        elements: &i.elements,
                        mesh_metres: i.mesh_metres,
                        origin_metres: i.origin_metres,
                        place: m.place,
                    })
                })
                .collect();
            let merged = federation::merge(&members);
            for (m, r) in fed.models.iter_mut().zip(&merged.ranges) {
                m.elements = r.clone();
            }
            let first = fed.models[0].ifc().cloned().unwrap_or_default();
            let index = Arc::new(IfcIndex {
                elements: merged.elements,
                ..first
            });
            let full = MeshModel {
                mesh: Arc::new(merged.mesh),
                textures: Arc::new(Vec::new()),
                ifc: Some(index.clone()),
            };
            let styled = fed.by_model || fed.models.iter().any(|m| !m.visible);
            let shown = if styled {
                let mut looks = vec![ElementLook::default(); index.elements.len()];
                for (k, m) in fed.models.iter().enumerate() {
                    for l in &mut looks[m.elements.clone()] {
                        l.hidden = !m.visible;
                        if fed.by_model {
                            l.colour = Some(COLOURS[k % COLOURS.len()]);
                        }
                    }
                }
                MeshModel {
                    mesh: Arc::new(apply::restyle(&full.mesh, &index.elements, &looks)),
                    textures: full.textures.clone(),
                    ifc: Some(index),
                }
            } else {
                full.clone()
            };
            fed.full = Some(full);
            self.model = Some(shown);
        }
        self.checks_scene_changed();
        if let Some(m) = &self.model {
            self.bounds = canonical_bounds(&m.mesh);
        }
        self.gpu = None;
        self.bim.restyled = None;
        self.bim.sections_changed();
        if refit {
            self.fitted = false;
        }
        self.changed();
    }

    /// Places a model read: aligned, or the reason it cannot be.
    fn fed_place(&mut self, l: Loaded) -> Result<(), Box<(Loaded, String)>> {
        let fed = &self.bim.fed;
        let Some(first) = fed.models.first().and_then(|m| m.ifc()) else {
            return Err(Box::new((l, t!("fed-note", what = "not-ifc"))));
        };
        let Some(this) = l.model.ifc.as_deref() else {
            return Err(Box::new((l, t!("fed-note", what = "not-ifc"))));
        };
        match federation::align(&first.georef, &this.georef) {
            Ok((place, basis)) => {
                self.fed_push(l, place, basis);
                Ok(())
            }
            Err(u) => {
                let why = unaligned_text(&u);
                Err(Box::new((l, why)))
            }
        }
    }

    fn fed_push(&mut self, l: Loaded, place: Placement, basis: Basis) {
        self.bim.fed.models.push(Model {
            name: file_name(&l.path),
            path: l.path,
            own: l.model,
            data: l.data,
            place,
            basis,
            visible: true,
            elements: 0..0,
            project: l.project,
            date: l.date,
            _hold: l.hold,
        });
    }

    /// The model list.
    pub(super) fn fed_panel(&self) -> PanelView {
        let fed = &self.bim.fed;
        let mut v = PanelView {
            title: t!("fed-title", count = fed.models.len().max(1) as i64),
            rows: Vec::new(),
            empty: None,
            chosen: None,
            notes: Vec::new(),
            hints: t!("fed-note", what = "hints"),
            prompt: None,
        };
        if fed.adding > 0 {
            v.notes.push(t!("fed-adding", count = fed.adding as i64));
        }
        if fed.models.is_empty() {
            match self.model.as_ref().and_then(|m| m.ifc.as_ref()) {
                None => v.empty = Some(t!("fed-note", what = "not-ifc-open")),
                Some(i) => v.rows.push(PanelRow {
                    index: 0,
                    text: t!("fed-note", what = "this-model"),
                    detail: Some(t!(
                        "fed-row",
                        basis = basis_text(&Basis::Reference),
                        elements = i.elements.len() as i64,
                        shown = "shown"
                    )),
                }),
            }
            v.chosen = (!v.rows.is_empty()).then_some(0);
            return v;
        }
        for (k, m) in fed.models.iter().enumerate() {
            v.rows.push(PanelRow {
                index: k,
                text: m.name.clone(),
                detail: Some(t!(
                    "fed-row",
                    basis = basis_text(&m.basis),
                    elements = m.elements.len() as i64,
                    shown = if m.visible { "shown" } else { "hidden" }
                )),
            });
        }
        let row = self.bim.row.min(fed.models.len() - 1);
        if let Some(m) = fed.models.get(row)
            && !matches!(m.basis, Basis::Reference)
        {
            let o = m.place.offset();
            let d = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
            let loc = specular_i18n::I18n::global().localizer();
            v.notes.push(t!(
                "fed-placed",
                distance = super::checks::length_text(d),
                angle = loc.number(m.place.angle_deg(), 1)
            ));
        }
        v.chosen = Some(row);
        v
    }

    pub(super) fn fed_rows_len(&self) -> usize {
        self.bim.fed.models.len().max(usize::from(
            self.model.as_ref().is_some_and(|m| m.ifc.is_some()),
        ))
    }

    /// Goes to a model: the camera on its box.
    fn fed_go(&mut self, k: usize) {
        let Some(m) = self.bim.fed.models.get(k) else {
            return;
        };
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let Some(i) = model.ifc.as_ref() else { return };
        let which: Vec<usize> = m.elements.clone().collect();
        if let Some((lo, hi)) = super::checks::bounds_of(&model.mesh, &i.elements, &which) {
            self.checks_look_at(lo, hi);
        }
    }

    /// A point of model `k`'s own mesh frame in the scene's mesh frame.
    pub(super) fn fed_to_scene(&self, k: usize, p: [f64; 3]) -> [f64; 3] {
        let Some(m) = self.bim.fed.models.get(k) else {
            return p;
        };
        let w = m.to_project(p);
        let (s, o) = self
            .model
            .as_ref()
            .and_then(|x| x.ifc.as_ref())
            .map_or((1.0, [0.0; 3]), |i| (i.mesh_metres, i.origin_metres));
        std::array::from_fn(|c| (w[c] - o[c]) / s.max(1e-12))
    }

    /// Model `k`'s own mesh moved into the scene's frame (for a mesh made in its frame).
    pub(super) fn fed_placed_mesh(&self, k: usize, mesh: &Mesh) -> Option<Mesh> {
        let fed = &self.bim.fed;
        if !fed.active() {
            return None;
        }
        let m = fed.models.get(k)?;
        let own = m.ifc()?;
        let scene = self.model.as_ref()?.ifc.as_ref()?;
        Some(federation::place_mesh(
            mesh,
            (own.mesh_metres, own.origin_metres),
            &m.place,
            (scene.mesh_metres, scene.origin_metres),
        ))
    }
}

impl App {
    /// Asks for IFC files (or a folder of them) to add to the scene.
    pub(super) fn fed_add_dialog(&mut self, folder: bool) {
        let has_ifc = self.mesh.model.as_ref().is_some_and(|m| m.ifc.is_some());
        if !has_ifc {
            let now = self.now();
            self.chrome
                .toast(t!("fed-note", what = "not-ifc-open"), None, now);
            return;
        }
        let here = self.bim_path();
        let dir = here
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        let tx = self.mesh.bim.fed.tx.clone();
        let present: Vec<PathBuf> = self
            .mesh
            .bim
            .fed
            .models
            .iter()
            .map(|m| m.path.clone())
            .chain(here)
            .collect();
        let waker = self.loader.clone();
        self.loader.background("dlg-ifc-add", move |l| {
            let mut dlg = rfd::FileDialog::new();
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let paths: Vec<PathBuf> = if folder {
                let Some(d) = dlg.pick_folder() else { return };
                let v: Vec<PathBuf> = ifc_files(&d)
                    .into_iter()
                    .filter(|p| !present.contains(p))
                    .collect();
                if v.is_empty() {
                    let _ = tx.send(Job::Empty(d));
                    l.waker()();
                    return;
                }
                v
            } else {
                dlg.add_filter("IFC", &["ifc", "ifczip"])
                    .pick_files()
                    .unwrap_or_default()
            };
            if paths.is_empty() {
                return;
            }
            fed_read(&waker, &tx, paths);
        });
    }

    /// Workers' results.
    pub(super) fn fed_poll(&mut self) {
        let now = self.now();
        while let Ok(j) = self.mesh.bim.fed.rx.try_recv() {
            self.redraw = true;
            match j {
                Job::Reading(n) => self.mesh.bim.fed.adding += n,
                Job::Empty(d) => {
                    self.chrome
                        .toast(t!("fed-no-files", folder = file_name(&d)), None, now);
                }
                Job::Added(r) => {
                    let fed = &mut self.mesh.bim.fed;
                    fed.adding = fed.adding.saturating_sub(1);
                    match *r {
                        Err((path, why)) => {
                            let msg = t!("fed-not-added", file = file_name(&path), reason = why);
                            self.chrome.toast(msg, None, now);
                        }
                        Ok(l) => self.fed_take(l),
                    }
                }
            }
        }
        // Models waiting behind a question put away without an answer.
        let fed = &self.mesh.bim.fed;
        if fed.pending.is_some() && !fed.queued.is_empty() && self.chrome.modal().is_none() {
            self.mesh.bim.fed.pending = None;
            self.fed_next_queued();
        }
    }

    fn fed_take(&mut self, l: Loaded) {
        if self.mesh.bim.fed.pending.is_some() {
            if self.chrome.modal().is_some() {
                self.mesh.bim.fed.queued.push(l);
                return;
            }
            // The question was put away without "Place at origin".
            self.mesh.bim.fed.pending = None;
        }
        let path = self.bim_path();
        let data = self.mesh.bim.checks.base_data();
        if !self.mesh.fed_seed(path, data) {
            return;
        }
        let name = file_name(&l.path);
        match self.mesh.fed_place(l) {
            Ok(()) => {
                self.mesh.fed_rebuild(true);
                let n = self.mesh.bim.fed.models.len();
                self.chrome
                    .announce_status(t!("fed-added", file = name, count = n as i64));
            }
            Err(e) => {
                let (l, why) = *e;
                self.chrome.confirm_with_detail(
                    t!("fed-ask-origin", file = name),
                    why.clone(),
                    t!("fed-note", what = "origin-ok"),
                    ORIGIN_ITEM,
                );
                self.mesh.bim.fed.pending = Some((l, why));
            }
        }
    }

    /// The answer to "Place it at the origin?": yes.
    pub(in crate::app) fn fed_origin_confirmed(&mut self) {
        if let Some((l, _)) = self.mesh.bim.fed.pending.take() {
            self.mesh.fed_push(l, Placement::IDENTITY, Basis::Origin);
            self.mesh.fed_rebuild(true);
        }
        self.fed_next_queued();
    }

    fn fed_next_queued(&mut self) {
        let q = std::mem::take(&mut self.mesh.bim.fed.queued);
        for l in q {
            self.fed_take(l);
        }
        self.redraw = true;
    }

    fn fed_ask_remove(&mut self) {
        let row = self.mesh.bim.row;
        let now = self.now();
        let Some(m) = self.mesh.bim.fed.models.get(row) else {
            return;
        };
        if row == 0 {
            self.chrome
                .toast(t!("fed-note", what = "first-stays"), None, now);
            return;
        }
        self.chrome.confirm_destructive(
            t!("fed-remove-title", file = m.name.clone()),
            t!("fed-note", what = "remove-detail"),
            t!("fed-note", what = "remove-ok"),
            REMOVE_ITEM,
        );
        self.redraw = true;
    }

    /// The confirmation's answer: the chosen model leaves the scene.
    pub(in crate::app) fn fed_remove_confirmed(&mut self) {
        let row = self.mesh.bim.row;
        let fed = &mut self.mesh.bim.fed;
        if row == 0 || row >= fed.models.len() {
            return;
        }
        let m = fed.models.remove(row);
        self.mesh.bim.row = row.min(self.mesh.bim.fed.models.len().saturating_sub(1));
        self.mesh.fed_rebuild(false);
        self.chrome
            .announce_status(t!("fed-removed", file = m.name));
        self.redraw = true;
    }

    /// Enter in the list.
    pub(super) fn fed_enter(&mut self) {
        let row = self.mesh.bim.row;
        self.mesh.fed_go(row);
        self.redraw = true;
    }

    /// The list's own keys; true when taken.
    pub(super) fn fed_key(&mut self, k: &KeyCombo) -> bool {
        let plain = !k.mods.ctrl && !k.mods.alt && !k.mods.meta;
        if !plain {
            return false;
        }
        let ch = match &k.key {
            Key::Char(x) => Some(x.to_ascii_lowercase()),
            _ => None,
        };
        let row = self.mesh.bim.row;
        match (&k.key, ch) {
            (_, Some('a')) => self.fed_add_dialog(k.mods.shift),
            (Key::Named(NamedKey::Space), _) | (_, Some(' ')) => {
                let fed = &mut self.mesh.bim.fed;
                let Some(m) = fed.models.get_mut(row) else {
                    return false;
                };
                m.visible = !m.visible;
                let (name, shown) = (m.name.clone(), m.visible);
                self.mesh.fed_rebuild(false);
                self.chrome.announce_status(t!(
                    "fed-shown",
                    file = name,
                    shown = if shown { "shown" } else { "hidden" }
                ));
            }
            (_, Some('c')) => {
                let fed = &mut self.mesh.bim.fed;
                if !fed.active() {
                    return false;
                }
                fed.by_model = !fed.by_model;
                let on = fed.by_model;
                self.mesh.fed_rebuild(false);
                self.chrome
                    .announce_status(t!("fed-colours", on = if on { "on" } else { "off" }));
            }
            (Key::Named(NamedKey::Delete | NamedKey::Backspace), _) => self.fed_ask_remove(),
            _ => return false,
        }
        self.redraw = true;
        true
    }
}

/// Reads each of `paths` on a worker and sends the result.
fn fed_read(loader: &crate::loader::Loader, tx: &Sender<Job>, paths: Vec<PathBuf>) {
    let _ = tx.send(Job::Reading(paths.len()));
    for path in paths {
        let tx = tx.clone();
        loader.background("fed-ifc-read", move |l| {
            let r = load(&path).map_err(|e| (path.clone(), e));
            let _ = tx.send(Job::Added(Box::new(r)));
            l.waker()();
        });
    }
}

#[cfg(test)]
mod shots;
