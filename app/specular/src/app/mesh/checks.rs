//! Model checks on an IFC model (`Alt+Q`): a panel in the review panel's place with four
//! tabs (Tab and Shift+Tab, or 1 to 4).
//!
//! - IDS: O opens an `.ids` file and checks the model against it. Each specification shows
//!   how many objects pass and fail; Enter lists the failures, Enter on one goes to it
//!   (the element selected, the rest ghosted) and its reasons show under the list;
//!   Backspace goes back. Ctrl+S exports the failures as BCF topics, Ctrl+E as CSV.
//! - Clashes: two sets (A and B: all, a class or a storey), hard (deeper than a tolerance)
//!   or clearance (closer than a distance); R runs it on a worker. Each clash is an element
//!   pair with its depth or distance and status (new, active, resolved: kept next to the
//!   model in `<model>.specular-clashes.json`); Enter goes to it, both elements selected
//!   and the rest ghosted; S steps the status. Ctrl+S BCF, Ctrl+E CSV.
//! - Changes: O picks another version of the model; elements added (green), removed (red,
//!   from the other version) and changed (amber: geometry or which fields) are listed and
//!   drawn, F filters by kind, Enter goes to one and lists what changed. Ctrl+S BCF,
//!   Ctrl+E CSV.
//! - Quantities: count, length, area and volume of each element, from the file's quantity
//!   sets or computed from the mesh (marked), G groups by class, storey or type. Ctrl+E
//!   CSV, Ctrl+S XLSX (every grouping as a sheet).
//!
//! P shows the chosen element's properties. The model's information is read from its file
//! on a worker when the panel first opens; nothing is ever written into the model.

use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_bim::apply::{self, ElementLook};
use specular_bim::bcf::Version;
use specular_bim::clash::{self, Kind, Mode, Selector, Sidecar, Status};
use specular_bim::diff::{self, Change, ChangeKind, Field};
use specular_bim::export::{self, Cell, Issue};
use specular_bim::idscheck::{self, Cardinality, Facet, FacetKind, Ids, Reason, Report, Why};
use specular_bim::qto::{self, By};
use specular_decode::MeshModel;
use specular_decode::specular_mesh::ifc::Element;
use specular_decode::specular_mesh::ifc::data::{self as ifcdata, IfcData};
use specular_decode::specular_mesh::{Limits, Mesh};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_render_3d::{Pose, Projection};

use super::MeshView;
use super::bim::{MAX_ROWS, PanelRow, PanelView, author, centre};
use crate::app::App;

/// The tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) enum Tab {
    #[default]
    Ids,
    Clashes,
    Changes,
    Quantities,
}

const TABS: [Tab; 4] = [Tab::Ids, Tab::Clashes, Tab::Changes, Tab::Quantities];

impl Tab {
    fn key(self) -> &'static str {
        match self {
            Tab::Ids => "ids",
            Tab::Clashes => "clashes",
            Tab::Changes => "changes",
            Tab::Quantities => "quantities",
        }
    }
}

/// Distances the clash tolerance and clearance step through (mm).
const STEPS_MM: [f64; 10] = [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 500.0];

/// The other version of the model, for changes.
pub(in crate::app) struct Other {
    data: IfcData,
    mesh: Mesh,
    elements: Vec<Element>,
    mesh_metres: f64,
    origin_metres: [f64; 3],
}

/// A clash found, with its status.
#[derive(Clone, Debug)]
pub(super) struct Found {
    clash: clash::Clash,
    status: Status,
}

enum Job {
    Data(Result<Arc<IfcData>, String>),
    Ids(PathBuf, Result<Arc<Ids>, String>),
    Report(u64, Report),
    Clashes(u64, Vec<clash::Clash>, f64),
    Other(PathBuf, Result<Arc<Other>, String>),
    Saved(Result<PathBuf, String>),
}

/// A clash test's settings.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Setup {
    a: Selector,
    b: Selector,
    /// In a scene of several models: the model each side is taken from (None: all).
    pub(super) model_a: Option<usize>,
    pub(super) model_b: Option<usize>,
    hard: bool,
    tolerance_mm: f64,
    clearance_mm: f64,
}

impl Default for Setup {
    fn default() -> Setup {
        Setup {
            a: Selector::All,
            b: Selector::All,
            model_a: None,
            model_b: None,
            hard: true,
            tolerance_mm: 10.0,
            clearance_mm: 50.0,
        }
    }
}

/// The checks' state for the current model.
pub(in crate::app) struct Checks {
    pub tab: Tab,
    /// The information of the model IDS, changes and quantities are about (the model
    /// picked in a scene of several, else the opened one).
    pub(super) data: Option<Arc<IfcData>>,
    /// The opened model's information.
    pub(super) base: Option<Arc<IfcData>>,
    /// In a scene of several models: the one IDS, changes and quantities are about.
    pub(super) pick: usize,
    reading: bool,
    error: Option<String>,
    ids: Option<(PathBuf, Arc<Ids>)>,
    report: Option<Report>,
    checking: bool,
    /// The specification whose failures are listed, and the row it was on.
    open_spec: Option<(usize, usize)>,
    pub(super) setup: Setup,
    pub(super) clashes: Option<Vec<Found>>,
    running: bool,
    clash_ms: f64,
    pub(super) sidecar: Sidecar,
    pub(super) other: Option<(PathBuf, Arc<Other>)>,
    reading_other: bool,
    pub(super) changes: Option<Vec<Change>>,
    filter: Option<ChangeKind>,
    quantities: Option<Vec<qto::Row>>,
    by: Option<By>,
    /// The chosen element's properties are shown under the list.
    props: bool,
    /// Something is drawn highlighted or in colour by the checks.
    styled: bool,
    run: u64,
    tx: Sender<Job>,
    rx: Receiver<Job>,
}

impl Default for Checks {
    fn default() -> Checks {
        let (tx, rx) = crossbeam_channel::unbounded();
        Checks {
            tab: Tab::default(),
            data: None,
            base: None,
            pick: 0,
            reading: false,
            error: None,
            ids: None,
            report: None,
            checking: false,
            open_spec: None,
            setup: Setup::default(),
            clashes: None,
            running: false,
            clash_ms: 0.0,
            sidecar: Sidecar::default(),
            other: None,
            reading_other: false,
            changes: None,
            filter: None,
            quantities: None,
            by: None,
            props: false,
            styled: false,
            run: 0,
            tx,
            rx,
        }
    }
}

impl Checks {
    /// The opened model's information, once read.
    pub(super) fn base_data(&self) -> Option<Arc<IfcData>> {
        self.base.clone()
    }

    /// Kept across files: the tab, the IDS file and the clash settings.
    pub(super) fn carry(&mut self) -> Checks {
        Checks {
            tab: self.tab,
            ids: self.ids.take(),
            setup: Setup {
                model_a: None,
                model_b: None,
                ..self.setup.clone()
            },
            ..Checks::default()
        }
    }
}

/// A length in metres as the UI's language writes it, in mm, cm or m.
pub(super) fn length_text(m: f64) -> String {
    length(m)
}

/// A length in metres as the UI's language writes it, in mm, cm or m.
fn length(m: f64) -> String {
    let loc = specular_i18n::I18n::global().localizer();
    let (v, unit, digits) = if m.abs() < 0.1 {
        (m * 1000.0, "mm", 0)
    } else if m.abs() < 10.0 {
        (m, "m", 2)
    } else {
        (m, "m", 1)
    };
    specular_i18n::quantity(&loc.number(v, digits), unit)
}

fn number(v: f64, digits: u8) -> String {
    specular_i18n::I18n::global().localizer().number(v, digits)
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

/// "IfcWall Basic Wall:200" for a list.
fn element_label(data: Option<&IfcData>, e: &Element) -> String {
    let class = data.map_or(e.kind.as_str(), |d| d.schema_table().spelled(&e.kind));
    if e.name.trim().is_empty() {
        class.to_owned()
    } else {
        format!("{class} {}", e.name)
    }
}

fn spelled(data: Option<&IfcData>, class: &str) -> String {
    data.map_or(class, |d| d.schema_table().spelled(class))
        .to_owned()
}

/// The rows of a long list near the chosen one (the panel shows at most a dozen).
fn window(len: usize, chosen: usize) -> std::ops::Range<usize> {
    let lo = chosen.saturating_sub(MAX_ROWS);
    lo..(chosen + MAX_ROWS).min(len)
}

impl MeshView {
    /// The elements IDS, changes and quantities are about: the picked model's in a scene of
    /// several (indices into it start at [`MeshView::checks_offset`]), else all.
    pub(super) fn checks_elements(&self) -> Option<&[Element]> {
        let all = &self.model.as_ref()?.ifc.as_ref()?.elements;
        let fed = &self.bim.fed;
        if fed.active() {
            let r = fed.models.get(self.bim.checks.pick)?.elements.clone();
            return all.get(r);
        }
        Some(all)
    }

    /// Where [`MeshView::checks_elements`] starts among all the scene's elements.
    pub(super) fn checks_offset(&self) -> usize {
        let fed = &self.bim.fed;
        if fed.active() {
            fed.models
                .get(self.bim.checks.pick)
                .map_or(0, |m| m.elements.start)
        } else {
            0
        }
    }

    /// Every element of the scene.
    fn scene_elements(&self) -> Option<&[Element]> {
        Some(&self.model.as_ref()?.ifc.as_ref()?.elements)
    }

    /// The information of the model element `i` (of the scene) is from.
    fn data_of(&self, i: usize) -> Option<&IfcData> {
        let fed = &self.bim.fed;
        if fed.active() {
            let k = fed.model_of(i)?;
            return fed.models.get(k)?.data.as_deref();
        }
        let c = &self.bim.checks;
        c.base.as_deref().or(c.data.as_deref())
    }

    /// "IfcWall W1", with its model's file in a scene of several.
    fn scene_label(&self, i: usize) -> String {
        let Some(e) = self.scene_elements().and_then(|v| v.get(i)) else {
            return String::new();
        };
        let l = element_label(self.data_of(i), e);
        let fed = &self.bim.fed;
        match fed
            .model_of(i)
            .and_then(|k| fed.name(k))
            .filter(|_| fed.active())
        {
            Some(n) => t!("checks-in-model", element = l, model = n.to_owned()),
            None => l,
        }
    }

    /// The scene's models changed: what pointed at its elements goes.
    pub(super) fn checks_scene_changed(&mut self) {
        let n = self.bim.fed.models.len();
        let c = &mut self.bim.checks;
        c.styled = false;
        c.clashes = None;
        c.running = false;
        c.run += 1;
        let s = &mut c.setup;
        if s.model_a.is_some_and(|k| k >= n) {
            s.model_a = None;
        }
        if s.model_b.is_some_and(|k| k >= n) {
            s.model_b = None;
        }
        if c.pick > 0 && c.pick >= n {
            c.pick = 0;
            c.data = c.base.clone();
            c.report = None;
            c.open_spec = None;
            c.other = None;
            c.changes = None;
            c.quantities = None;
        }
    }

    /// The picked model's own model and data, in its own frame (the opened one alone).
    fn checks_own(&self) -> Option<(MeshModel, Option<Arc<IfcData>>)> {
        let fed = &self.bim.fed;
        if fed.active() {
            let m = fed.models.get(self.bim.checks.pick)?;
            return Some((m.own.clone(), m.data.clone()));
        }
        Some((self.model.clone()?, self.bim.checks.data.clone()))
    }

    /// What the facet of requirement `r` is about, in words.
    fn facet_subject(f: &Facet) -> String {
        match f {
            Facet::Entity { .. } => t!("checks-facet", facet = "entity", name = ""),
            Facet::Attribute { name, .. } => {
                t!("checks-facet", facet = "attribute", name = name.describe())
            }
            Facet::Property { pset, name, .. } => {
                format!("{}.{}", pset.describe(), name.describe())
            }
            Facet::Classification { .. } => t!("checks-facet", facet = "classification", name = ""),
            Facet::Material { .. } => t!("checks-facet", facet = "material", name = ""),
            Facet::PartOf { name, .. } => {
                t!("checks-facet", facet = "partof", name = name.describe())
            }
        }
    }

    /// One failed requirement in words.
    fn reason_text(ids: &Ids, spec: usize, r: &Reason) -> String {
        let what = ids
            .specs
            .get(spec)
            .and_then(|s| s.requirements.get(r.requirement))
            .map(Self::facet_subject)
            .unwrap_or_default();
        let why = match r.why {
            Why::Class => "class",
            Why::PredefinedType => "predefined",
            Why::Missing => "missing",
            Why::Empty => "empty",
            Why::Value => "value",
            Why::DataType => "datatype",
            Why::System => "system",
            Why::Whole => "whole",
            Why::Prohibited => "prohibited",
            Why::Unsupported => "unsupported",
        };
        let _ = r.facet == FacetKind::Entity;
        t!(
            "checks-reason",
            why = why,
            what = what,
            actual = r.actual.clone().unwrap_or_else(|| t!("checks-none"))
        )
    }

    /// The chosen object's properties, a line each.
    fn property_lines(&self, guid: &str) -> Vec<String> {
        self.property_lines_in(self.bim.checks.data.as_deref(), guid)
    }

    /// The properties of `guid` in `data`, a line each.
    fn property_lines_in(&self, data: Option<&IfcData>, guid: &str) -> Vec<String> {
        let Some(d) = data else {
            return Vec::new();
        };
        let Some(o) = d.by_guid(guid) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(s) = d.storey(o).and_then(|s| s.name()) {
            out.push(t!("checks-prop", what = "storey", value = s));
        }
        if let Some(ty) = d.type_of(o).and_then(|t| t.name()) {
            out.push(t!("checks-prop", what = "type", value = ty));
        }
        if let Some(m) = d.material(o) {
            let n = m.names().join(", ");
            if !n.is_empty() {
                out.push(t!("checks-prop", what = "material", value = n));
            }
        }
        for ps in d.psets(o) {
            for p in &ps.props {
                let v = p.text();
                if !v.is_empty() {
                    out.push(cut(&format!("{}.{} = {v}", ps.name, p.name), 70));
                }
            }
        }
        out.truncate(14);
        out
    }

    /// The checks panel to draw.
    pub(super) fn checks_panel(&self) -> PanelView {
        let c = &self.bim.checks;
        let row = self.bim.row;
        let tab_name = t!("checks-tab", tab = c.tab.key());
        let title = t!("checks-title", tab = tab_name);
        let mut v = PanelView {
            title,
            rows: Vec::new(),
            empty: None,
            chosen: None,
            notes: Vec::new(),
            hints: if self.bim.fed.active() {
                t!("checks-hints-models", tab = c.tab.key())
            } else {
                t!("checks-note", what = format!("{}-hints", c.tab.key()))
            },
            prompt: None,
        };
        let Some(elements) = self.checks_elements() else {
            v.empty = Some(t!("checks-note", what = "not-ifc"));
            return v;
        };
        let data = c.data.as_deref();
        if let Some(e) = &c.error {
            v.empty = Some(e.clone());
            return v;
        }
        if data.is_none() {
            v.empty = Some(t!("checks-note", what = "reading"));
            return v;
        }
        let fed = &self.bim.fed;
        if fed.active()
            && c.tab != Tab::Clashes
            && let Some(n) = fed.name(c.pick)
        {
            v.notes.push(t!("checks-model", model = n.to_owned()));
        }
        match c.tab {
            Tab::Ids => self.ids_view(&mut v, row),
            Tab::Clashes => self.clash_view(&mut v, row),
            Tab::Changes => self.changes_view(&mut v, row),
            Tab::Quantities => self.qto_view(&mut v, row, elements),
        }
        v.chosen = (!v.rows.is_empty()).then_some(row);
        v
    }

    fn ids_view(&self, v: &mut PanelView, row: usize) {
        let c = &self.bim.checks;
        let Some((path, ids)) = &c.ids else {
            v.empty = Some(t!("checks-note", what = "ids-empty"));
            return;
        };
        let Some(report) = &c.report else {
            v.empty = Some(t!("checks-note", what = "ids-checking"));
            return;
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ok = report.specs.iter().filter(|s| s.ok).count();
        let summary = t!(
            "checks-ids-summary",
            file = file,
            ok = ok as i64,
            count = report.specs.len() as i64
        );
        let data = c.data.as_deref();
        match c.open_spec {
            None => {
                v.notes.push(summary);
                for i in window(report.specs.len(), row) {
                    let s = &report.specs[i];
                    let spec = &ids.specs[s.spec];
                    let state = match (spec.usage, s.ok, s.applicable) {
                        (Cardinality::Prohibited, true, _) => "prohibited-ok",
                        (Cardinality::Prohibited, false, _) => "prohibited",
                        (_, true, 0) => "none-ok",
                        (_, false, 0) => "none",
                        (_, true, _) => "pass",
                        (_, false, _) => "fail",
                    };
                    v.rows.push(PanelRow {
                        index: i,
                        text: if spec.name.is_empty() {
                            t!("checks-note", what = "unnamed")
                        } else {
                            spec.name.clone()
                        },
                        detail: Some(t!(
                            "checks-spec",
                            state = state,
                            applicable = s.applicable as i64,
                            failed = s.failed.len() as i64
                        )),
                    });
                }
                if let Some(s) = report.specs.get(row) {
                    let spec = &ids.specs[s.spec];
                    if let Some(d) = &spec.description {
                        v.notes.push(cut(d, 90));
                    }
                }
            }
            Some((si, _)) => {
                let Some(s) = report.specs.get(si) else {
                    return;
                };
                let spec = &ids.specs[s.spec];
                v.notes.push(cut(&spec.name, 70));
                if s.failed.is_empty() {
                    v.empty = Some(t!("checks-note", what = "ids-no-failures"));
                    return;
                }
                for i in window(s.failed.len(), row) {
                    let f = &s.failed[i];
                    let label = data
                        .and_then(|d| d.get(f.id))
                        .map(|o| {
                            let class = spelled(data, &o.class);
                            match o.name() {
                                Some(n) => format!("{class} {n}"),
                                None => class,
                            }
                        })
                        .unwrap_or_else(|| format!("#{}", f.id));
                    v.rows.push(PanelRow {
                        index: i,
                        text: label,
                        detail: f.reasons.first().map(|r| Self::reason_text(ids, s.spec, r)),
                    });
                }
                if let Some(f) = s.failed.get(row) {
                    for r in &f.reasons {
                        v.notes.push(cut(&Self::reason_text(ids, s.spec, r), 90));
                    }
                    if c.props {
                        v.notes.extend(self.property_lines(&f.global_id));
                    }
                }
            }
        }
    }

    /// A clash set's name, with its model in a scene of several.
    fn side_name(&self, s: &Selector, model: Option<usize>) -> String {
        let set = self.set_name(s);
        let fed = &self.bim.fed;
        if !fed.active() {
            return set;
        }
        match model.and_then(|k| fed.name(k)) {
            Some(n) => t!("checks-in-model", element = set, model = n.to_owned()),
            None => t!(
                "checks-in-model",
                element = set,
                model = t!("checks-every-model")
            ),
        }
    }

    fn set_name(&self, s: &Selector) -> String {
        let c = &self.bim.checks;
        let data = c.base.as_deref().or(c.data.as_deref());
        match s {
            Selector::All => t!("checks-set", kind = "all", name = ""),
            Selector::Classes(c) => t!(
                "checks-set",
                kind = "class",
                name = c
                    .iter()
                    .map(|c| spelled(data, c))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Selector::Storeys(s) => t!("checks-set", kind = "storey", name = s.join(", ")),
        }
    }

    fn clash_view(&self, v: &mut PanelView, row: usize) {
        let c = &self.bim.checks;
        let s = &c.setup;
        let metres = self.metres_per_unit();
        v.notes.push(t!(
            "checks-clash-setup",
            a = self.side_name(&s.a, s.model_a),
            b = self.side_name(&s.b, s.model_b),
            mode = if s.hard { "hard" } else { "clearance" },
            distance = length(
                if s.hard {
                    s.tolerance_mm
                } else {
                    s.clearance_mm
                } / 1000.0
            )
        ));
        if c.running {
            v.empty = Some(t!("checks-note", what = "clash-running"));
            return;
        }
        let Some(found) = &c.clashes else {
            v.empty = Some(t!("checks-note", what = "clash-empty"));
            return;
        };
        let new = found.iter().filter(|f| f.status == Status::New).count();
        let resolved = c
            .sidecar
            .clashes
            .iter()
            .filter(|k| k.status == Status::Resolved)
            .count();
        v.notes.push(t!(
            "checks-clash-count",
            count = found.len() as i64,
            new = new as i64,
            resolved = resolved as i64,
            seconds = number(c.clash_ms / 1000.0, 1)
        ));
        if found.is_empty() {
            v.empty = Some(t!("checks-note", what = "clash-none"));
            return;
        }
        let Some(elements) = self.scene_elements() else {
            return;
        };
        for i in window(found.len(), row) {
            let f = &found[i];
            v.rows.push(PanelRow {
                index: i,
                text: format!(
                    "{} × {}",
                    self.scene_label(f.clash.a),
                    self.scene_label(f.clash.b)
                ),
                detail: Some(t!(
                    "checks-clash",
                    kind = match f.clash.kind {
                        Kind::Hard => "hard",
                        Kind::Clearance => "clearance",
                    },
                    depth = length(f.clash.depth * metres),
                    status = status_text(f.status)
                )),
            });
        }
        if c.props
            && let Some(f) = found.get(row)
        {
            for x in [f.clash.a, f.clash.b] {
                v.notes.push(self.scene_label(x));
                v.notes.extend(
                    self.property_lines_in(self.data_of(x), &elements[x].global_id)
                        .into_iter()
                        .take(6),
                );
            }
        }
    }

    /// The changes shown under the filter, as indices into the list.
    fn shown_changes(&self) -> Vec<usize> {
        let c = &self.bim.checks;
        c.changes
            .as_ref()
            .map(|ch| {
                ch.iter()
                    .enumerate()
                    .filter(|(_, x)| c.filter.is_none_or(|f| x.kind == f))
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn changes_view(&self, v: &mut PanelView, row: usize) {
        let c = &self.bim.checks;
        if c.reading_other {
            v.empty = Some(t!("checks-note", what = "changes-reading"));
            return;
        }
        let (Some((path, _)), Some(changes)) = (&c.other, &c.changes) else {
            v.empty = Some(t!("checks-note", what = "changes-empty"));
            return;
        };
        let [added, removed, changed] = diff::counts(changes);
        v.notes.push(t!(
            "checks-changes-summary",
            file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            added = added as i64,
            removed = removed as i64,
            changed = changed as i64,
            filter = match c.filter {
                None => "all",
                Some(ChangeKind::Added) => "added",
                Some(ChangeKind::Removed) => "removed",
                Some(ChangeKind::Changed) => "changed",
            }
        ));
        let shown = self.shown_changes();
        if shown.is_empty() {
            v.empty = Some(t!("checks-note", what = "changes-none"));
            return;
        }
        let data = c.data.as_deref();
        for k in window(shown.len(), row) {
            let x = &changes[shown[k]];
            let class = spelled(data, &x.class);
            v.rows.push(PanelRow {
                index: k,
                text: match &x.name {
                    Some(n) => format!("{class} {n}"),
                    None => class,
                },
                detail: Some(change_detail(x)),
            });
        }
        if let Some(&i) = shown.get(row) {
            let x = &changes[i];
            if let Some(d) = x.moved_m {
                v.notes.push(t!("checks-moved", distance = length(d)));
            }
            for f in x.fields.iter().take(10) {
                v.notes.push(cut(&field_line(f), 90));
            }
            if c.props && x.kind != ChangeKind::Removed {
                v.notes
                    .extend(self.property_lines(&x.guid).into_iter().take(8));
            }
        }
    }

    fn qto_view(&self, v: &mut PanelView, row: usize, elements: &[Element]) {
        let c = &self.bim.checks;
        let Some(rows) = &c.quantities else {
            v.empty = Some(t!("checks-note", what = "reading"));
            return;
        };
        let data = c.data.as_deref();
        let by = match c.by {
            None => "element",
            Some(By::Class) => "class",
            Some(By::Storey) => "storey",
            Some(By::Type) => "type",
        };
        let total = |f: fn(&qto::Row) -> Option<qto::Q>| {
            rows.iter().filter_map(f).map(|q| q.value).sum::<f64>()
        };
        v.notes.push(t!(
            "checks-qto-summary",
            by = by,
            count = rows.len() as i64,
            area = area(total(|r| r.area)),
            volume = volume(total(|r| r.volume))
        ));
        let line = |l: Option<f64>, a: Option<f64>, vol: Option<f64>| {
            let mut parts = Vec::new();
            if let Some(l) = l.filter(|x| *x > 0.0) {
                parts.push(length(l));
            }
            if let Some(a) = a.filter(|x| *x > 0.0) {
                parts.push(area(a));
            }
            if let Some(vol) = vol.filter(|x| *x > 0.0) {
                parts.push(volume(vol));
            }
            specular_i18n::dot_join(&parts)
        };
        match c.by {
            None => {
                for i in window(rows.len(), row) {
                    let r = &rows[i];
                    let src = source(&[r.length, r.area, r.volume]);
                    let mut d = line(
                        r.length.map(|q| q.value),
                        r.area.map(|q| q.value),
                        r.volume.map(|q| q.value),
                    );
                    if !src.is_empty() {
                        d = format!("{d} ({src})");
                    }
                    v.rows.push(PanelRow {
                        index: i,
                        text: element_label(data, &elements[r.element]),
                        detail: Some(d),
                    });
                }
                if c.props
                    && let Some(r) = rows.get(row)
                {
                    v.notes.extend(self.property_lines(&r.guid));
                }
            }
            Some(b) => {
                let groups = qto::groups(rows, b);
                for i in window(groups.len(), row) {
                    let g = &groups[i];
                    let key = if g.key.is_empty() {
                        t!("checks-none")
                    } else if b == By::Class {
                        spelled(data, &g.key)
                    } else {
                        g.key.clone()
                    };
                    let src = match (g.from_file, g.computed) {
                        (0, 0) => String::new(),
                        (_, 0) => t!("checks-source", source = "file"),
                        (0, _) => t!("checks-source", source = "computed"),
                        _ => t!("checks-source", source = "mixed"),
                    };
                    let mut d = specular_i18n::dot_join(&[
                        number(g.count as f64, 0),
                        line(Some(g.length), Some(g.area), Some(g.volume)),
                    ]);
                    if !src.is_empty() {
                        d = format!("{d} ({src})");
                    }
                    v.rows.push(PanelRow {
                        index: i,
                        text: key,
                        detail: Some(d),
                    });
                }
            }
        }
    }

    /// Metres per mesh unit.
    fn metres_per_unit(&self) -> f64 {
        self.model
            .as_ref()
            .and_then(|m| m.ifc.as_ref())
            .map_or(1.0, |i| i.mesh_metres)
    }
}

fn area(m2: f64) -> String {
    specular_i18n::quantity(&number(m2, 2), "m²")
}

fn volume(m3: f64) -> String {
    specular_i18n::quantity(&number(m3, 3), "m³")
}

fn source(q: &[Option<qto::Q>]) -> String {
    let f = q.iter().flatten().filter(|q| q.from_file).count();
    let c = q.iter().flatten().filter(|q| !q.from_file).count();
    match (f, c) {
        (0, 0) => String::new(),
        (_, 0) => t!("checks-source", source = "file"),
        (0, _) => t!("checks-source", source = "computed"),
        _ => t!("checks-source", source = "mixed"),
    }
}

fn status_text(s: Status) -> String {
    t!(
        "checks-status",
        status = match s {
            Status::New => "new",
            Status::Active => "active",
            Status::Resolved => "resolved",
        }
    )
}

fn change_detail(x: &Change) -> String {
    match x.kind {
        ChangeKind::Added => t!("checks-change", kind = "added"),
        ChangeKind::Removed => t!("checks-change", kind = "removed"),
        ChangeKind::Changed => {
            let mut parts = Vec::new();
            if x.geometry {
                parts.push(t!("checks-change", kind = "geometry"));
            }
            if !x.fields.is_empty() {
                parts.push(t!("checks-change-fields", count = x.fields.len() as i64));
            }
            parts.join(", ")
        }
    }
}

fn field_line(f: &diff::FieldChange) -> String {
    let name = match f.field {
        Field::Attribute => f.name.clone(),
        Field::Property => format!("{}.{}", f.set.clone().unwrap_or_default(), f.name),
        Field::Type => t!("checks-prop", what = "type", value = ""),
        Field::Container => t!("checks-prop", what = "container", value = ""),
        Field::Material => t!("checks-prop", what = "material", value = ""),
        Field::Classification => t!("checks-prop", what = "classification", value = ""),
    };
    let none = t!("checks-none");
    t!(
        "checks-field",
        name = name.trim_end_matches([':', ' ']).to_owned(),
        old = f.old.clone().unwrap_or_else(|| none.clone()),
        new = f.new.clone().unwrap_or(none)
    )
}

/// The box of some elements' vertices (mesh units).
pub(super) fn bounds_of(
    mesh: &Mesh,
    elements: &[Element],
    which: &[usize],
) -> Option<([f64; 3], [f64; 3])> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &i in which {
        let e = elements.get(i)?;
        let r = e.first_vertex as usize..(e.first_vertex + e.vertices) as usize;
        for p in mesh.positions.get(r).unwrap_or(&[]) {
            for k in 0..3 {
                lo[k] = lo[k].min(f64::from(p[k]));
                hi[k] = hi[k].max(f64::from(p[k]));
            }
        }
    }
    (lo[0] <= hi[0]).then_some((lo, hi))
}

impl MeshView {
    fn checks_unstyle(&mut self) {
        let c = &mut self.bim.checks;
        if c.styled {
            c.styled = false;
            if self.bim.restyled.take().is_some() {
                self.gpu = None;
                self.bim.sections_changed();
                self.changed();
            }
        }
    }

    pub(super) fn checks_compare(&mut self) {
        let c = &self.bim.checks;
        let (Some((_, other)), Some(d)) = (c.other.clone(), c.data.clone()) else {
            return;
        };
        // The picked model in its own frame.
        let Some((model, _)) = self.checks_own() else {
            return;
        };
        let Some(ifc) = model.ifc.as_ref() else {
            return;
        };
        let old = diff::Side {
            data: &other.data,
            elements: &other.elements,
            mesh: &other.mesh,
            mesh_metres: other.mesh_metres,
            origin_metres: other.origin_metres,
        };
        let new = diff::Side {
            data: &d,
            elements: &ifc.elements,
            mesh: &model.mesh,
            mesh_metres: ifc.mesh_metres,
            origin_metres: ifc.origin_metres,
        };
        let t0 = std::time::Instant::now();
        let changes = diff::diff(&old, &new);
        specular_log::debug!(
            target: specular_log::UI,
            "changes: {} in {:.0} ms",
            changes.len(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        self.bim.checks.changes = Some(changes);
        self.bim.row = 0;
        self.checks_show_changes(None);
    }

    /// The changes in colour, `selected` (an index into the changes) as selected.
    fn checks_show_changes(&mut self, selected: Option<usize>) {
        let c = &self.bim.checks;
        let (Some((_, other)), Some(d), Some(changes)) =
            (c.other.clone(), c.data.clone(), c.changes.as_ref())
        else {
            return;
        };
        let Some((model, _)) = self.checks_own() else {
            return;
        };
        let Some(ifc) = model.ifc.clone() else { return };
        let old = diff::Side {
            data: &other.data,
            elements: &other.elements,
            mesh: &other.mesh,
            mesh_metres: other.mesh_metres,
            origin_metres: other.origin_metres,
        };
        let new = diff::Side {
            data: &d,
            elements: &ifc.elements,
            mesh: &model.mesh,
            mesh_metres: ifc.mesh_metres,
            origin_metres: ifc.origin_metres,
        };
        let mesh = diff::coloured(&old, &new, changes, selected);
        let textures = model.textures.clone();
        // In a scene of several models: moved to where the picked one is drawn.
        let pick = self.bim.checks.pick;
        let (mesh, ifc) = match self.fed_placed_mesh(pick, &mesh) {
            Some(m) => (
                m,
                self.model
                    .as_ref()
                    .and_then(|x| x.ifc.clone())
                    .unwrap_or(ifc),
            ),
            None => (mesh, ifc),
        };
        self.checks_restyled(MeshModel {
            mesh: Arc::new(mesh),
            textures,
            ifc: Some(ifc),
        });
    }

    fn checks_restyled(&mut self, m: MeshModel) {
        let mv = self;
        mv.bim.restyled = Some(m);
        mv.bim.checks.styled = true;
        mv.gpu = None;
        mv.bim.sections_changed();
        mv.changed();
    }

    /// Selects `which` (element indices), ghosts the rest, and goes to them.
    fn checks_focus(&mut self, which: &[usize]) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let Some(ifc) = model.ifc.clone() else { return };
        let mut looks = vec![
            ElementLook {
                colour: Some(diff::GHOST),
                ..ElementLook::default()
            };
            ifc.elements.len()
        ];
        for &i in which {
            if let Some(l) = looks.get_mut(i) {
                *l = ElementLook {
                    selected: true,
                    ..ElementLook::default()
                };
            }
        }
        let mesh = apply::restyle(&model.mesh, &ifc.elements, &looks);
        let target = bounds_of(&model.mesh, &ifc.elements, which);
        let textures = model.textures.clone();
        self.checks_restyled(MeshModel {
            mesh: Arc::new(mesh),
            textures,
            ifc: Some(ifc),
        });
        if let Some((lo, hi)) = target {
            self.checks_look_at(lo, hi);
        }
    }

    /// Animates the camera to look at a box (mesh units).
    pub(super) fn checks_look_at(&mut self, lo: [f64; 3], hi: [f64; 3]) {
        let metres = self.metres_per_unit();
        // Not closer than half a metre across.
        let pad = (0.25 / metres.max(1e-12)).max(0.0);
        let lo2 = std::array::from_fn(|k| lo[k].min((lo[k] + hi[k]) / 2.0 - pad));
        let hi2 = std::array::from_fn(|k| hi[k].max((lo[k] + hi[k]) / 2.0 + pad));
        let cam = export::camera_on(lo2, hi2);
        let fov = match cam.kind {
            specular_bim::bcf::CameraKind::Perspective { fov } => fov.to_radians(),
            _ => specular_render_3d::FOV_Y,
        };
        let c = centre(&self.bounds);
        let to_c = [
            c[0] - cam.viewpoint[0],
            c[1] - cam.viewpoint[1],
            c[2] - cam.viewpoint[2],
        ];
        let distance =
            (to_c[0] * cam.direction[0] + to_c[1] * cam.direction[1] + to_c[2] * cam.direction[2])
                .abs();
        let to = Pose {
            eye: cam.viewpoint,
            direction: cam.direction,
            up: cam.up,
            projection: Projection::Perspective,
            fov,
            height: 1.0,
            distance,
        };
        self.animate_to(to);
    }

    pub(super) fn checks_rows_len(&self) -> usize {
        let m = self;
        let c = &m.bim.checks;
        match c.tab {
            Tab::Ids => match (c.open_spec, &c.report) {
                (None, Some(r)) => r.specs.len(),
                (Some((s, _)), Some(r)) => r.specs.get(s).map_or(0, |s| s.failed.len()),
                _ => 0,
            },
            Tab::Clashes => c.clashes.as_ref().map_or(0, Vec::len),
            Tab::Changes => m.shown_changes().len(),
            Tab::Quantities => match (c.by, &c.quantities) {
                (None, Some(r)) => r.len(),
                (Some(b), Some(r)) => qto::groups(r, b).len(),
                _ => 0,
            },
        }
    }

    /// Enter: into a specification's failures, or to the chosen element(s). False when the
    /// element chosen has no geometry to go to.
    pub(super) fn checks_enter(&mut self) -> bool {
        let row = self.bim.row;
        let c = &self.bim.checks;
        match c.tab {
            Tab::Ids => {
                let Some(r) = &c.report else { return true };
                match c.open_spec {
                    None => {
                        if r.specs.get(row).is_some_and(|s| !s.failed.is_empty()) {
                            self.bim.checks.open_spec = Some((row, row));
                            self.bim.row = 0;
                            return self.checks_go_failure();
                        }
                        true
                    }
                    Some(_) => self.checks_go_failure(),
                }
            }
            Tab::Clashes => {
                let Some(f) = c.clashes.as_ref().and_then(|v| v.get(row)) else {
                    return true;
                };
                let pair = [f.clash.a, f.clash.b];
                self.checks_focus(&pair);
                true
            }
            Tab::Changes => {
                let shown = self.shown_changes();
                let Some(&i) = shown.get(row) else {
                    return true;
                };
                self.checks_show_changes(Some(i));
                self.checks_go_change(i);
                true
            }
            Tab::Quantities => {
                let e = if c.by.is_none() {
                    c.quantities
                        .as_ref()
                        .and_then(|q| q.get(row))
                        .map(|r| r.element)
                } else {
                    None
                };
                if let Some(e) = e {
                    let off = self.checks_offset();
                    self.checks_focus(&[e + off]);
                }
                true
            }
        }
    }

    /// Goes to the chosen failing element; false when it is not drawn.
    fn checks_go_failure(&mut self) -> bool {
        let row = self.bim.row;
        let c = &self.bim.checks;
        let (Some((si, _)), Some(r)) = (c.open_spec, &c.report) else {
            return true;
        };
        let Some(f) = r.specs.get(si).and_then(|s| s.failed.get(row)) else {
            return true;
        };
        let guid = f.global_id.clone();
        let Some(elements) = self.checks_elements() else {
            return true;
        };
        match elements.iter().position(|e| e.global_id == guid) {
            Some(i) => {
                let off = self.checks_offset();
                self.checks_focus(&[i + off]);
                true
            }
            None => false,
        }
    }

    fn checks_go_change(&mut self, i: usize) {
        let c = &self.bim.checks;
        let (Some(changes), Some((_, other))) = (c.changes.as_ref(), c.other.as_ref()) else {
            return;
        };
        let Some(x) = changes.get(i) else { return };
        let Some((own, _)) = self.checks_own() else {
            return;
        };
        let Some(ifc) = own.ifc.as_ref() else {
            return;
        };
        let pick = self.bim.checks.pick;
        let fed = self.bim.fed.active();
        // A box of the picked model's own frame in the scene's.
        let placed = |(lo, hi): ([f64; 3], [f64; 3])| {
            if fed {
                let (a, b) = (self.fed_to_scene(pick, lo), self.fed_to_scene(pick, hi));
                (
                    std::array::from_fn(|k| a[k].min(b[k])),
                    std::array::from_fn(|k| a[k].max(b[k])),
                )
            } else {
                (lo, hi)
            }
        };
        let target = match (x.new_element, x.old_element) {
            (Some(e), _) => bounds_of(&own.mesh, &ifc.elements, &[e]).map(placed),
            (None, Some(e)) => bounds_of(&other.mesh, &other.elements, &[e]).map(|(lo, hi)| {
                let f = |p: [f64; 3]| -> [f64; 3] {
                    std::array::from_fn(|k| {
                        (p[k] * other.mesh_metres + other.origin_metres[k] - ifc.origin_metres[k])
                            / ifc.mesh_metres.max(1e-12)
                    })
                };
                placed((f(lo), f(hi)))
            }),
            _ => None,
        };
        if let Some((lo, hi)) = target {
            self.checks_look_at(lo, hi);
        }
    }

    /// A clash run's results, with their statuses from the sidecar of `model` (saved
    /// back). Returns how many.
    pub(super) fn checks_clashes_found(
        &mut self,
        found: Vec<clash::Clash>,
        ms: f64,
        model: Option<&std::path::Path>,
    ) -> usize {
        let Some(ifc) = self.model.as_ref().and_then(|m| m.ifc.clone()) else {
            return 0;
        };
        let metres = ifc.mesh_metres;
        let keys: Vec<(String, String, Kind, f64)> = found
            .iter()
            .map(|c| {
                (
                    self.clash_key(&ifc.elements, c.a),
                    self.clash_key(&ifc.elements, c.b),
                    c.kind,
                    c.depth * metres,
                )
            })
            .collect();
        let c = &mut self.bim.checks;
        let statuses = c.sidecar.update(&keys);
        c.running = false;
        c.clash_ms = ms;
        c.clashes = Some(
            found
                .into_iter()
                .zip(statuses)
                .map(|(clash, status)| Found { clash, status })
                .collect(),
        );
        let n = c.clashes.as_ref().map_or(0, Vec::len);
        self.checks_save_sidecar(model);
        n
    }

    /// An element's key in the clash sidecar: its GUID, qualified by its model's file in a
    /// scene of several.
    fn clash_key(&self, elements: &[Element], i: usize) -> String {
        let g = elements.get(i).map_or("", |e| e.global_id.as_str());
        let fed = &self.bim.fed;
        match fed
            .model_of(i)
            .and_then(|k| fed.name(k))
            .filter(|_| fed.active())
        {
            Some(n) => specular_bim::federation::qualified(n, g),
            None => g.to_owned(),
        }
    }

    /// Keeps the clash statuses next to the model.
    fn checks_save_sidecar(&mut self, model: Option<&std::path::Path>) {
        let Some(p) = model else { return };
        let c = &mut self.bim.checks;
        c.sidecar.model = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path = clash::sidecar_path(p);
        if let Err(e) = std::fs::write(&path, c.sidecar.to_json()) {
            specular_log::warn!(
                target: specular_log::UI,
                "clashes: cannot keep statuses next to the model: {e}"
            );
        }
    }

    /// The bytes of an export of the current tab.
    /// The bytes of an export of the current tab; `model` is the model file's name.
    pub(super) fn checks_bytes(&self, out: Out, model: Option<String>) -> Option<Vec<u8>> {
        let m = self;
        let c = &m.bim.checks;
        let d = c.data.as_deref()?;
        let elements = m.checks_elements()?;
        let frame = |lo: [f64; 3], hi: [f64; 3]| -> ([f64; 3], [f64; 3]) {
            let ifc = m.model.as_ref().and_then(|x| x.ifc.as_ref());
            let (s, o) = ifc.map_or((1.0, [0.0; 3]), |i| (i.mesh_metres, i.origin_metres));
            (
                std::array::from_fn(|k| lo[k] * s + o[k]),
                std::array::from_fn(|k| hi[k] * s + o[k]),
            )
        };
        let mesh = &m.model.as_ref()?.mesh;
        let col = |k: &str| Cell::from(t!("checks-col", col = k));
        match c.tab {
            Tab::Ids => {
                let (_, ids) = c.ids.as_ref()?;
                let r = c.report.as_ref()?;
                let mut rows = vec![vec![
                    col("spec"),
                    col("guid"),
                    col("class"),
                    col("name"),
                    col("reason"),
                ]];
                let mut issues = Vec::new();
                for s in &r.specs {
                    let spec = &ids.specs[s.spec];
                    for f in &s.failed {
                        let o = d.get(f.id);
                        let reasons: Vec<String> = f
                            .reasons
                            .iter()
                            .map(|x| MeshView::reason_text(ids, s.spec, x))
                            .collect();
                        let class = o.map(|o| spelled(Some(d), &o.class)).unwrap_or_default();
                        let name = o.and_then(|o| o.name()).unwrap_or_default();
                        rows.push(vec![
                            Cell::from(spec.name.as_str()),
                            Cell::from(f.global_id.as_str()),
                            Cell::from(class.clone()),
                            Cell::from(name.clone()),
                            Cell::from(reasons.join("; ")),
                        ]);
                        let el = elements.iter().position(|e| e.global_id == f.global_id);
                        issues.push(Issue {
                            title: cut(&format!("{}: {class} {name}", spec.name), 120),
                            description: Some(reasons.join("\n")),
                            guids: vec![f.global_id.clone()],
                            focus: el
                                .and_then(|i| bounds_of(mesh, elements, &[i]))
                                .map(|(lo, hi)| frame(lo, hi)),
                            labels: vec!["IDS".into()],
                            ..Issue::default()
                        });
                    }
                }
                Some(match out {
                    Out::Bcf => specular_bim::write(
                        &export::topics(&issues, &author(), model.as_deref(), Version::V3_0),
                        Version::V3_0,
                    ),
                    _ => export::csv(&rows),
                })
            }
            Tab::Clashes => {
                let found = c.clashes.as_ref()?;
                let metres = m.metres_per_unit();
                let mut rows = vec![vec![
                    col("a"),
                    col("a-name"),
                    col("b"),
                    col("b-name"),
                    col("kind"),
                    col("depth"),
                    col("status"),
                    col("x"),
                    col("y"),
                    col("z"),
                ]];
                let mut issues = Vec::new();
                let elements = m.scene_elements()?;
                let fed = &m.bim.fed;
                for f in found {
                    let (a, b) = (&elements[f.clash.a], &elements[f.clash.b]);
                    let (la, lb) = (m.scene_label(f.clash.a), m.scene_label(f.clash.b));
                    // Both model files in the topic's header, when they are two.
                    let mut files = Vec::new();
                    if fed.active() {
                        for x in [f.clash.a, f.clash.b] {
                            if let Some(md) = fed.model_of(x).and_then(|k| fed.models.get(k)) {
                                let h = md.header();
                                if !files.contains(&h) {
                                    files.push(h);
                                }
                            }
                        }
                    }
                    let kind = match f.clash.kind {
                        Kind::Hard => "hard",
                        Kind::Clearance => "clearance",
                    };
                    let (p, _) = frame(f.clash.point, f.clash.point);
                    rows.push(vec![
                        Cell::from(a.global_id.as_str()),
                        Cell::from(la.clone()),
                        Cell::from(b.global_id.as_str()),
                        Cell::from(lb.clone()),
                        Cell::from(t!("checks-clash-kind", kind = kind)),
                        Cell::Number(f.clash.depth * metres),
                        Cell::from(status_text(f.status)),
                        Cell::Number(p[0]),
                        Cell::Number(p[1]),
                        Cell::Number(p[2]),
                    ]);
                    issues.push(Issue {
                        title: cut(&t!("checks-clash-topic", a = la, b = lb), 120),
                        description: Some(t!(
                            "checks-clash",
                            kind = kind,
                            depth = length(f.clash.depth * metres),
                            status = status_text(f.status)
                        )),
                        guids: vec![a.global_id.clone(), b.global_id.clone()],
                        focus: Some(frame(f.clash.region.min, f.clash.region.max)),
                        labels: vec!["Clash".into()],
                        files,
                        status: Some(
                            match f.status {
                                Status::Resolved => "Closed",
                                _ => "Open",
                            }
                            .into(),
                        ),
                        ..Issue::default()
                    });
                }
                Some(match out {
                    Out::Bcf => specular_bim::write(
                        &export::topics(&issues, &author(), model.as_deref(), Version::V3_0),
                        Version::V3_0,
                    ),
                    _ => export::csv(&rows),
                })
            }
            Tab::Changes => {
                let changes = c.changes.as_ref()?;
                let mut rows = vec![vec![
                    col("change"),
                    col("guid"),
                    col("class"),
                    col("name"),
                    col("field"),
                    col("old"),
                    col("new"),
                ]];
                let mut issues = Vec::new();
                for x in changes {
                    let kind = change_detail(x);
                    let class = spelled(Some(d), &x.class);
                    let name = x.name.clone().unwrap_or_default();
                    if x.fields.is_empty() {
                        rows.push(vec![
                            Cell::from(kind.clone()),
                            Cell::from(x.guid.as_str()),
                            Cell::from(class.clone()),
                            Cell::from(name.clone()),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                        ]);
                    }
                    for f in &x.fields {
                        let field = match f.field {
                            Field::Property => {
                                format!("{}.{}", f.set.clone().unwrap_or_default(), f.name)
                            }
                            Field::Attribute => f.name.clone(),
                            Field::Type => t!("checks-col", col = "type"),
                            Field::Container => t!("checks-col", col = "container"),
                            Field::Material => t!("checks-col", col = "material"),
                            Field::Classification => t!("checks-col", col = "classification"),
                        };
                        rows.push(vec![
                            Cell::from(kind.clone()),
                            Cell::from(x.guid.as_str()),
                            Cell::from(class.clone()),
                            Cell::from(name.clone()),
                            Cell::from(field),
                            Cell::from(f.old.clone().unwrap_or_default()),
                            Cell::from(f.new.clone().unwrap_or_default()),
                        ]);
                    }
                    if x.kind != ChangeKind::Removed {
                        let el = x.new_element;
                        issues.push(Issue {
                            title: cut(&format!("{kind}: {class} {name}"), 120),
                            description: Some(
                                x.fields
                                    .iter()
                                    .map(field_line)
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ),
                            guids: vec![x.guid.clone()],
                            focus: el
                                .and_then(|i| bounds_of(mesh, elements, &[i]))
                                .map(|(lo, hi)| frame(lo, hi)),
                            labels: vec!["Change".into()],
                            ..Issue::default()
                        });
                    }
                }
                Some(match out {
                    Out::Bcf => specular_bim::write(
                        &export::topics(&issues, &author(), model.as_deref(), Version::V3_0),
                        Version::V3_0,
                    ),
                    _ => export::csv(&rows),
                })
            }
            Tab::Quantities => {
                let rows = c.quantities.as_ref()?;
                let labels = qto::Labels {
                    columns: [
                        "guid", "name", "class", "type", "storey", "count", "length", "area",
                        "volume", "source",
                    ]
                    .map(|k| t!("checks-col", col = k)),
                    from_file: t!("checks-source", source = "file"),
                    computed: t!("checks-source", source = "computed"),
                    mixed: t!("checks-source", source = "mixed"),
                };
                let cls = |k: &str| spelled(Some(d), k);
                let same = |k: &str| k.to_owned();
                let table = |b: Option<By>| match b {
                    None => qto::element_table(rows, &labels, &cls),
                    Some(By::Class) => qto::group_table(
                        &qto::groups(rows, By::Class),
                        &labels.columns[2],
                        &labels,
                        &cls,
                    ),
                    Some(By::Storey) => qto::group_table(
                        &qto::groups(rows, By::Storey),
                        &labels.columns[4],
                        &labels,
                        &same,
                    ),
                    Some(By::Type) => qto::group_table(
                        &qto::groups(rows, By::Type),
                        &labels.columns[3],
                        &labels,
                        &same,
                    ),
                };
                Some(match out {
                    Out::Xlsx => {
                        let sheet = |b: Option<By>| {
                            t!(
                                "checks-group",
                                by = match b {
                                    None => "element",
                                    Some(By::Class) => "class",
                                    Some(By::Storey) => "storey",
                                    Some(By::Type) => "type",
                                }
                            )
                        };
                        let all = [None, Some(By::Class), Some(By::Storey), Some(By::Type)];
                        export::xlsx(&all.map(|b| (sheet(b), table(b))))
                    }
                    _ => export::csv(&table(c.by)),
                })
            }
        }
    }
}

impl App {
    /// The panel opened: read the model's information if not yet.
    pub(super) fn checks_opened(&mut self) {
        let has_ifc = self.mesh.model.as_ref().is_some_and(|m| m.ifc.is_some());
        let path = self.bim_path();
        let c = &mut self.mesh.bim.checks;
        if !has_ifc || c.data.is_some() || c.reading {
            return;
        }
        let Some(path) = path else { return };
        c.reading = true;
        let tx = c.tx.clone();
        self.loader.background("chk-ifc-data", move |l| {
            let r = std::fs::read(&path)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|b| {
                    ifcdata::read(&b, &Limits::default())
                        .map(Arc::new)
                        .map_err(|e| e.to_string())
                });
            let _ = tx.send(Job::Data(r));
            l.waker()();
        });
    }

    /// The panel closed: the model drawn as it is again.
    pub(super) fn checks_closed(&mut self) {
        self.mesh.checks_unstyle();
        self.redraw = true;
    }

    /// Enter in the panel.
    pub(super) fn checks_enter(&mut self) {
        if !self.mesh.checks_enter() {
            let now = self.now();
            self.chrome
                .toast(t!("checks-note", what = "not-drawn"), None, now);
        }
        self.redraw = true;
    }

    pub(super) fn checks_rows_len(&self) -> usize {
        self.mesh.checks_rows_len()
    }

    /// Workers' results.
    pub(super) fn checks_poll(&mut self) {
        let now = self.now();
        while let Ok(j) = self.mesh.bim.checks.rx.try_recv() {
            self.redraw = true;
            match j {
                Job::Data(r) => {
                    let c = &mut self.mesh.bim.checks;
                    c.reading = false;
                    match r {
                        Ok(d) => {
                            c.base = Some(d.clone());
                            if c.pick == 0 {
                                c.data = Some(d.clone());
                            }
                            c.error = None;
                            if let Some(m) = self.mesh.bim.fed.models.first_mut()
                                && m.data.is_none()
                            {
                                m.data = Some(d);
                            }
                            self.checks_after_data();
                        }
                        Err(e) => c.error = Some(t!("checks-error", reason = e)),
                    }
                }
                Job::Ids(path, r) => match r {
                    Ok(ids) => {
                        let c = &mut self.mesh.bim.checks;
                        c.ids = Some((path, ids));
                        c.report = None;
                        c.open_spec = None;
                        self.mesh.bim.row = 0;
                        self.checks_run_ids();
                    }
                    Err(e) => {
                        self.chrome.toast(t!("checks-error", reason = e), None, now);
                    }
                },
                Job::Report(run, rep) => {
                    let c = &mut self.mesh.bim.checks;
                    if run == c.run {
                        c.checking = false;
                        let failed = rep.specs.iter().filter(|s| !s.ok).count();
                        c.report = Some(rep);
                        let msg = t!("checks-ids-done", failed = failed as i64);
                        self.chrome.announce_status(msg);
                    }
                }
                Job::Clashes(run, found, ms) => {
                    if run != self.mesh.bim.checks.run {
                        continue;
                    }
                    let model = self.bim_path();
                    let n = self.mesh.checks_clashes_found(found, ms, model.as_deref());
                    self.chrome
                        .announce_status(t!("checks-clash-done", count = n as i64));
                }
                Job::Other(path, r) => {
                    self.mesh.bim.checks.reading_other = false;
                    match r {
                        Ok(o) => {
                            self.mesh.bim.checks.other = Some((path, o));
                            self.mesh.checks_compare();
                        }
                        // Cancelled.
                        Err(e) if e.is_empty() => {}
                        Err(e) => {
                            self.chrome.toast(t!("checks-error", reason = e), None, now);
                        }
                    }
                }
                Job::Saved(r) => {
                    let msg = match r {
                        Ok(p) => t!(
                            "checks-saved",
                            file = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        ),
                        Err(e) => t!("checks-error", reason = e),
                    };
                    self.chrome.toast(msg, None, now);
                }
            }
        }
    }

    /// The information is in: check the IDS and take quantities.
    fn checks_after_data(&mut self) {
        if self.mesh.bim.checks.ids.is_some() {
            self.checks_run_ids();
        }
        let (Some(d), Some((model, _))) =
            (self.mesh.bim.checks.data.clone(), self.mesh.checks_own())
        else {
            return;
        };
        let Some(ifc) = model.ifc.as_ref() else {
            return;
        };
        let rows = qto::rows(&d, &ifc.elements, &model.mesh, ifc.mesh_metres);
        self.mesh.bim.checks.quantities = Some(rows);
        if let Some(p) = self.bim_path() {
            self.mesh.bim.checks.sidecar = Sidecar::load(&clash::sidecar_path(&p));
        }
    }

    fn checks_run_ids(&mut self) {
        let c = &mut self.mesh.bim.checks;
        let (Some((_, ids)), Some(d)) = (c.ids.clone(), c.data.clone()) else {
            return;
        };
        c.run += 1;
        c.checking = true;
        let run = c.run;
        let tx = c.tx.clone();
        self.loader.background("chk-ids", move |l| {
            let r = idscheck::check(&ids, &d);
            let _ = tx.send(Job::Report(run, r));
            l.waker()();
        });
    }

    fn checks_open_ids(&mut self) {
        let tx = self.mesh.bim.checks.tx.clone();
        let dir = self
            .bim_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        self.loader.background("dlg-ids-open", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter("IDS", &["ids", "xml"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(path) = dlg.pick_file() else { return };
            let r = std::fs::read_to_string(&path)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|s| idscheck::parse(&s).map(Arc::new).map_err(|e| e.to_string()));
            let _ = tx.send(Job::Ids(path, r));
            l.waker()();
        });
    }

    fn checks_open_other(&mut self) {
        let tx = self.mesh.bim.checks.tx.clone();
        let here = self.bim_path();
        let dir = here
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        self.mesh.bim.checks.reading_other = true;
        self.loader.background("dlg-ifc-other", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter("IFC", &["ifc", "ifczip"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let r = match dlg.pick_file() {
                None => Err(String::new()),
                Some(path) => {
                    let r = std::fs::read(&path)
                        .map_err(|e| crate::error_text::io_reason(&e))
                        .and_then(|b| {
                            let lim = Limits::default();
                            let m = specular_decode::specular_mesh::ifc::read(&b, &lim, None)
                                .map_err(|e| e.to_string())?;
                            let data = ifcdata::read(&b, &lim).map_err(|e| e.to_string())?;
                            let mut elements = m.elements;
                            elements.sort_by_key(|e| e.first_vertex);
                            Ok(Arc::new(Other {
                                data,
                                mesh: m.mesh,
                                elements,
                                mesh_metres: m.mesh_metres,
                                origin_metres: m.origin_metres,
                            }))
                        });
                    Ok((path, r))
                }
            };
            let msg = match r {
                Ok((path, r)) => Job::Other(path, r),
                // Cancelled: nothing to report.
                Err(_) => Job::Other(PathBuf::new(), Err(String::new())),
            };
            let _ = tx.send(msg);
            l.waker()();
        });
    }

    /// Runs the clash test.
    fn checks_run_clashes(&mut self) {
        let fed = &self.mesh.bim.fed;
        // Every model of the scene, hidden ones too.
        let Some(model) = fed
            .full
            .as_ref()
            .filter(|_| fed.active())
            .or(self.mesh.model.as_ref())
        else {
            return;
        };
        let Some(ifc) = model.ifc.clone() else { return };
        let mesh = model.mesh.clone();
        // Each element's model, and each model's information.
        let owner: Arc<Vec<usize>> = Arc::new(
            (0..ifc.elements.len())
                .map(|i| fed.model_of(i).unwrap_or(0))
                .collect(),
        );
        let datas: Vec<Option<Arc<IfcData>>> = if fed.active() {
            fed.models.iter().map(|m| m.data.clone()).collect()
        } else {
            let c = &self.mesh.bim.checks;
            vec![c.base.clone().or_else(|| c.data.clone())]
        };
        let c = &mut self.mesh.bim.checks;
        if datas.first().is_none_or(Option::is_none) {
            return;
        }
        let data_of = |i: usize| datas.get(owner[i]).and_then(|d| d.as_deref());
        let storey = |i: usize| -> Option<String> {
            let d = data_of(i)?;
            let o = d.by_guid(&ifc.elements[i].global_id)?;
            d.storey(o)?.name()
        };
        let member = |i: usize| Some(owner[i]);
        let side = |set: &Selector, model: Option<usize>| {
            specular_bim::federation::Scoped {
                model,
                set: set.clone(),
            }
            .pick(&ifc.elements, &member, &storey)
        };
        let a = side(&c.setup.a, c.setup.model_a);
        let b = side(&c.setup.b, c.setup.model_b);
        let metres = ifc.mesh_metres.max(1e-12);
        let tolerance = c.setup.tolerance_mm / 1000.0 / metres;
        let mode = if c.setup.hard {
            Mode::Hard { tolerance }
        } else {
            Mode::Clearance {
                distance: c.setup.clearance_mm / 1000.0 / metres,
                tolerance,
            }
        };
        c.run += 1;
        c.running = true;
        c.clashes = None;
        let run = c.run;
        let tx = c.tx.clone();
        self.mesh.bim.row = 0;
        self.loader.background("chk-clash", move |l| {
            let t0 = std::time::Instant::now();
            // Never a part against its whole, nor a door or window against the wall it
            // fills.
            let objs: Vec<Option<&ifcdata::Object>> = ifc
                .elements
                .iter()
                .enumerate()
                .map(|(i, e)| datas.get(owner[i])?.as_deref()?.by_guid(&e.global_id))
                .collect();
            let related = |x: usize, y: usize| -> bool {
                // Elements of two models are never parts of one another.
                if owner[x] != owner[y] {
                    return false;
                }
                let Some(d) = datas.get(owner[x]).and_then(|d| d.as_deref()) else {
                    return false;
                };
                let (Some(p), Some(q)) = (objs[x], objs[y]) else {
                    return false;
                };
                let whole = |a: &ifcdata::Object, b: &ifcdata::Object| a.aggregate == Some(b.id);
                let fills = |a: &ifcdata::Object, b: &ifcdata::Object| {
                    a.fills
                        .and_then(|op| d.get(op))
                        .is_some_and(|op| op.voids == Some(b.id))
                };
                whole(p, q)
                    || whole(q, p)
                    || fills(p, q)
                    || fills(q, p)
                    || (p.aggregate.is_some() && p.aggregate == q.aggregate)
            };
            let m = clash::Model {
                positions: &mesh.positions,
                indices: &mesh.indices,
                elements: &ifc.elements,
            };
            let found = clash::run(&m, &a, &b, mode, &related);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            specular_log::debug!(
                target: specular_log::UI,
                "clashes: {} in {ms:.0} ms ({} × {} elements)",
                found.len(),
                a.len(),
                b.len()
            );
            let _ = tx.send(Job::Clashes(run, found, ms));
            l.waker()();
        });
    }

    /// The next (or previous) set for side A or B: all, each class, each storey.
    fn checks_next_set(&mut self, side_b: bool, back: bool) {
        let Some(ifc) = self.mesh.model.as_ref().and_then(|m| m.ifc.clone()) else {
            return;
        };
        let fed = &self.mesh.bim.fed;
        let all: Vec<Arc<IfcData>> = if fed.active() {
            fed.models.iter().filter_map(|m| m.data.clone()).collect()
        } else {
            let c = &self.mesh.bim.checks;
            c.base
                .clone()
                .or_else(|| c.data.clone())
                .into_iter()
                .collect()
        };
        let mut classes: Vec<(String, usize)> = Vec::new();
        for e in &ifc.elements {
            match classes.iter_mut().find(|(c, _)| *c == e.kind) {
                Some(x) => x.1 += 1,
                None => classes.push((e.kind.clone(), 1)),
            }
        }
        classes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut storeys: Vec<String> = Vec::new();
        for d in &all {
            for o in &d.objects {
                if d.schema_table().is_a(&o.class, "IFCBUILDINGSTOREY")
                    && let Some(n) = o.name()
                    && !storeys.contains(&n)
                {
                    storeys.push(n);
                }
            }
        }
        let mut options = vec![Selector::All];
        options.extend(classes.into_iter().map(|(c, _)| Selector::Classes(vec![c])));
        options.extend(storeys.into_iter().map(|s| Selector::Storeys(vec![s])));
        let s = &mut self.mesh.bim.checks.setup;
        let cur = if side_b { &s.b } else { &s.a };
        let i = options.iter().position(|o| o == cur).unwrap_or(0);
        let n = options.len();
        let j = if back { (i + n - 1) % n } else { (i + 1) % n };
        let next = options[j].clone();
        if side_b {
            s.b = next;
        } else {
            s.a = next;
        }
    }

    /// The next (or previous) model a clash set is taken from: every model, then each.
    fn checks_next_model(&mut self, side_b: bool, back: bool) {
        let n = self.mesh.bim.fed.models.len();
        if n < 2 {
            return;
        }
        let s = &mut self.mesh.bim.checks.setup;
        let cur = if side_b {
            &mut s.model_b
        } else {
            &mut s.model_a
        };
        // None, 0, 1, …, n - 1 in a ring of n + 1.
        let i = cur.map_or(0, |k| k + 1);
        let j = if back {
            (i + n) % (n + 1)
        } else {
            (i + 1) % (n + 1)
        };
        *cur = j.checked_sub(1);
    }

    /// The next model IDS, changes and quantities are about.
    fn checks_next_pick(&mut self, back: bool) {
        let n = self.mesh.bim.fed.models.len();
        if n < 2 {
            return;
        }
        let c = &mut self.mesh.bim.checks;
        c.pick = if back {
            (c.pick + n - 1) % n
        } else {
            (c.pick + 1) % n
        };
        let pick = c.pick;
        c.data = self.mesh.bim.fed.models[pick]
            .data
            .clone()
            .or_else(|| if pick == 0 { c.base.clone() } else { None });
        c.report = None;
        c.open_spec = None;
        c.other = None;
        c.changes = None;
        c.quantities = None;
        self.mesh.bim.row = 0;
        self.mesh.checks_unstyle();
        let name = self.mesh.bim.fed.models[pick].name.clone();
        self.chrome
            .announce_status(t!("checks-model", model = name));
        self.checks_after_data();
    }

    fn checks_step_distance(&mut self, up: bool) {
        let s = &mut self.mesh.bim.checks.setup;
        let v = if s.hard {
            &mut s.tolerance_mm
        } else {
            &mut s.clearance_mm
        };
        let i = STEPS_MM
            .iter()
            .position(|x| *x >= *v - 1e-9)
            .unwrap_or(STEPS_MM.len() - 1);
        let j = if up {
            (i + 1).min(STEPS_MM.len() - 1)
        } else {
            i.saturating_sub(1)
        };
        *v = STEPS_MM[j];
    }

    /// The panel's own keys; true when taken.
    pub(super) fn checks_key(&mut self, k: &KeyCombo) -> bool {
        let plain = !k.mods.ctrl && !k.mods.alt && !k.mods.meta;
        let ctrl = k.mods.ctrl && !k.mods.alt;
        let ch = match &k.key {
            Key::Char(x) => Some(x.to_ascii_lowercase()),
            _ => None,
        };
        let tab = self.mesh.bim.checks.tab;
        // Tabs.
        let to_tab = match (&k.key, ch) {
            (Key::Named(NamedKey::Tab), _) if !k.mods.ctrl && !k.mods.alt => {
                let i = TABS.iter().position(|t| *t == tab).unwrap_or(0);
                Some(if k.mods.shift {
                    TABS[(i + 3) % 4]
                } else {
                    TABS[(i + 1) % 4]
                })
            }
            (_, Some(c @ '1'..='4')) if plain => Some(TABS[(c as u8 - b'1') as usize]),
            _ => None,
        };
        if let Some(t) = to_tab {
            self.mesh.checks_unstyle();
            let c = &mut self.mesh.bim.checks;
            c.tab = t;
            self.mesh.bim.row = 0;
            if t == Tab::Changes && c.changes.is_some() {
                self.mesh.checks_show_changes(None);
            }
            let name = t!("checks-tab", tab = t.key());
            self.chrome.announce_status(name);
            return true;
        }
        match (ch, plain, ctrl) {
            (Some('p'), true, _) => {
                let c = &mut self.mesh.bim.checks;
                c.props = !c.props;
                return true;
            }
            (Some('s'), _, true) => {
                self.checks_export(if tab == Tab::Quantities {
                    Out::Xlsx
                } else {
                    Out::Bcf
                });
                return true;
            }
            (Some('e'), _, true) => {
                self.checks_export(Out::Csv);
                return true;
            }
            _ => {}
        }
        if self.mesh.bim.checks.data.is_none() {
            return false;
        }
        if tab != Tab::Clashes && plain && ch == Some('m') && self.mesh.bim.fed.active() {
            self.checks_next_pick(k.mods.shift);
            return true;
        }
        match tab {
            Tab::Ids => match (&k.key, ch) {
                (_, Some('o')) if plain => self.checks_open_ids(),
                (Key::Named(NamedKey::Backspace), _) if plain => {
                    let c = &mut self.mesh.bim.checks;
                    let Some((_, back)) = c.open_spec.take() else {
                        return false;
                    };
                    self.mesh.bim.row = back;
                    self.mesh.checks_unstyle();
                }
                _ => return false,
            },
            Tab::Clashes => match ch {
                Some('r') if plain => self.checks_run_clashes(),
                Some('a') if plain => self.checks_next_set(false, k.mods.shift),
                Some('b') if plain => self.checks_next_set(true, k.mods.shift),
                Some('m') if plain => {
                    let s = &mut self.mesh.bim.checks.setup;
                    s.hard = !s.hard;
                }
                Some('g') if plain => self.checks_next_model(false, k.mods.shift),
                Some('h') if plain => self.checks_next_model(true, k.mods.shift),
                Some('+' | '=') if !k.mods.ctrl => self.checks_step_distance(true),
                Some('-') if !k.mods.ctrl => self.checks_step_distance(false),
                Some('s') if plain => {
                    let row = self.mesh.bim.row;
                    let Some(ifc) = self.mesh.model.as_ref().and_then(|m| m.ifc.clone()) else {
                        return false;
                    };
                    let c = &mut self.mesh.bim.checks;
                    let Some(f) = c.clashes.as_mut().and_then(|v| v.get_mut(row)) else {
                        return false;
                    };
                    f.status = match f.status {
                        Status::New => Status::Active,
                        Status::Active => Status::Resolved,
                        Status::Resolved => Status::New,
                    };
                    let (ia, ib) = (f.clash.a, f.clash.b);
                    let (kind, st) = (f.clash.kind, f.status);
                    let (a, b) = (
                        self.mesh.clash_key(&ifc.elements, ia),
                        self.mesh.clash_key(&ifc.elements, ib),
                    );
                    let c = &mut self.mesh.bim.checks;
                    c.sidecar.set(&a, &b, kind, st);
                    let model = self.bim_path();
                    self.mesh.checks_save_sidecar(model.as_deref());
                    self.chrome.announce_status(status_text(st));
                }
                _ => return false,
            },
            Tab::Changes => match ch {
                Some('o') if plain => self.checks_open_other(),
                Some('f') if plain => {
                    let c = &mut self.mesh.bim.checks;
                    c.filter = match c.filter {
                        None => Some(ChangeKind::Added),
                        Some(ChangeKind::Added) => Some(ChangeKind::Removed),
                        Some(ChangeKind::Removed) => Some(ChangeKind::Changed),
                        Some(ChangeKind::Changed) => None,
                    };
                    self.mesh.bim.row = 0;
                }
                _ => return false,
            },
            Tab::Quantities => match ch {
                Some('g') if plain => {
                    let c = &mut self.mesh.bim.checks;
                    c.by = match c.by {
                        None => Some(By::Class),
                        Some(By::Class) => Some(By::Storey),
                        Some(By::Storey) => Some(By::Type),
                        Some(By::Type) => None,
                    };
                    self.mesh.bim.row = 0;
                }
                _ => return false,
            },
        }
        true
    }

    /// Writes the current tab's results (the user picks where).
    fn checks_export(&mut self, out: Out) {
        let model = self.bim_path();
        let name = model
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let Some(bytes) = self.mesh.checks_bytes(out, name) else {
            return;
        };
        let stem = model
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "model".into());
        let tab = self.mesh.bim.checks.tab.key();
        let (ext, filter) = match out {
            Out::Csv => ("csv", "CSV"),
            Out::Bcf => ("bcfzip", "BCF"),
            Out::Xlsx => ("xlsx", "XLSX"),
        };
        let name = format!("{stem}-{tab}.{ext}");
        let dir = model
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let tx = self.mesh.bim.checks.tx.clone();
        self.loader.background("dlg-checks", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter(filter, &[ext])
                .set_file_name(name);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(path) = dlg.save_file() else { return };
            let r = if model.as_deref() == Some(path.as_path()) {
                Err(t!("bim-note", what = "not-over-model"))
            } else {
                std::fs::write(&path, bytes)
                    .map(|()| path)
                    .map_err(|e| crate::error_text::io_reason(&e))
            };
            let _ = tx.send(Job::Saved(r));
            l.waker()();
        });
    }
}

/// What an export writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Out {
    Csv,
    Bcf,
    Xlsx,
}

#[cfg(test)]
pub(super) mod shots;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_hold_the_chosen_row() {
        assert_eq!(window(5, 0), 0..5);
        let w = window(1000, 500);
        assert!(w.contains(&500) && w.len() <= 2 * MAX_ROWS);
        assert_eq!(window(1000, 999).end, 1000);
    }
}
