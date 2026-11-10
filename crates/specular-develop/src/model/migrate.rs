//! Param migrations and process versions.
//!
//! - Each module kind has a param version. A change that alters the look bumps it, and adds a
//!   [`Migration`](super::kind::Migration) from the previous version.
//! - At most two versions per module have a live kernel. Params at a live version render
//!   unchanged; the user moves them forward with "Update to current" ([`update_step`]).
//! - Params at a version without a live kernel migrate on load to the nearest live version
//!   above, the closest look still rendered. The [`MigrationReport`] carries the notice to show.
//! - Params newer than this build (or of an unknown kind) are kept as fields, untouched, and
//!   written back as they came.

use super::edit::{Edit, ModuleInstance};
use super::fields::Fields;
use super::history::{Change, Step};
use super::kind::{ModuleKind, ModuleSpec, Registry};
use super::params::Params;

/// Fluent id of the notice after a load migrated modules (`$count` = modules).
pub const NOTICE_MIGRATED: &str = "toast-develop-edit-migrated";
/// Fluent id of the notice when modules come from a newer Specular (`$count` = modules).
pub const NOTICE_NEWER: &str = "toast-develop-edit-newer";

/// One instance migrated on load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migrated {
    pub kind: ModuleKind,
    pub instance: u16,
    pub from: u16,
    pub to: u16,
}

/// What a load did to module versions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub migrated: Vec<Migrated>,
    /// Instances this build cannot render: a newer version, a missing migration or version 0.
    /// They are kept as fields and skipped by the renderer.
    pub not_rendered: Vec<(ModuleKind, u16, u16)>,
}

/// A notice for the UI: a Fluent id and its `$count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    pub id: &'static str,
    pub count: usize,
}

impl MigrationReport {
    pub fn is_empty(&self) -> bool {
        self.migrated.is_empty() && self.not_rendered.is_empty()
    }

    /// The notices to show, most important first.
    pub fn notices(&self) -> Vec<Notice> {
        let mut out = Vec::new();
        if !self.not_rendered.is_empty() {
            out.push(Notice {
                id: NOTICE_NEWER,
                count: self.not_rendered.len(),
            });
        }
        if !self.migrated.is_empty() {
            out.push(Notice {
                id: NOTICE_MIGRATED,
                count: self.migrated.len(),
            });
        }
        out
    }
}

/// Runs the migrations of `spec` from version `from` to `to`; `Err(v)` names the version
/// with no way forward (or `from` when going backwards or from version 0).
pub fn run(spec: &ModuleSpec, from: u16, to: u16, f: &mut Fields) -> Result<(), u16> {
    if from == 0 || from > to {
        return Err(from);
    }
    for v in from..to {
        (spec.migration_from(v).ok_or(v)?.run)(f);
    }
    Ok(())
}

/// Whether `m` renders with this build's kernels.
pub fn renders(reg: &Registry, m: &ModuleInstance) -> bool {
    reg.get(m.kind).is_some_and(|s| s.live.contains(&m.version))
}

/// Brings loaded fields to a live version and types them (see the module docs).
pub(crate) fn on_load(
    reg: &Registry,
    kind: ModuleKind,
    instance: u16,
    version: &mut u16,
    fields: &mut Fields,
    report: &mut MigrationReport,
) -> Params {
    let Some(spec) = reg.get(kind) else {
        // Unknown kind: kept as is; not rendered, but not something to warn about twice.
        report.not_rendered.push((kind, instance, *version));
        return Params::Other(kind, std::mem::take(fields));
    };
    if spec.live.contains(version) {
        return Params::from_fields(kind, std::mem::take(fields));
    }
    // The nearest live version above: the closest look still rendered.
    if let Some(to) = spec.live.iter().copied().find(|&l| l > *version)
        && *version != 0
    {
        let mut migrated = fields.clone();
        if run(spec, *version, to, &mut migrated).is_ok() {
            report.migrated.push(Migrated {
                kind,
                instance,
                from: *version,
                to,
            });
            *version = to;
            return Params::from_fields(kind, migrated);
        }
    }
    report.not_rendered.push((kind, instance, *version));
    Params::Other(kind, std::mem::take(fields))
}

/// The instance moved to its kind's current version, or `None` when it already is (or cannot
/// be: unknown kind, newer version).
pub fn updated(reg: &Registry, m: &ModuleInstance) -> Option<ModuleInstance> {
    let spec = reg.get(m.kind)?;
    if m.version >= spec.current {
        return None;
    }
    let mut f = m.params.to_fields();
    run(spec, m.version, spec.current, &mut f).ok()?;
    Some(ModuleInstance {
        version: spec.current,
        params: Params::from_fields(m.kind, f),
        ..m.clone()
    })
}

/// "Update to current" for one instance, as an undoable history step.
pub fn update_step(
    reg: &Registry,
    edit: &Edit,
    kind: ModuleKind,
    instance: u16,
    time_ms: i64,
) -> Option<Step> {
    let before = edit.module(kind, instance)?;
    let after = updated(reg, before)?;
    Some(Step {
        kind,
        instance,
        change: Change::Replace {
            before: Box::new(before.clone()),
            after: Box::new(after),
        },
        time_ms,
    })
}

/// Whether any instance of the edit has a newer version available.
pub fn has_updates(reg: &Registry, edit: &Edit) -> bool {
    edit.modules
        .iter()
        .any(|m| reg.get(m.kind).is_some_and(|s| m.version < s.current))
}
