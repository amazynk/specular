//! Local copies for offline editing: the process-wide handle to
//! the Library's [`LocalCopies`], and the hooks the writers call. The Library's side (the
//! keep and remove questions, the offline banner, writing waiting changes when an original
//! is back and asking about conflicts) is `gallery/local.rs`.
//!
//! While a location is offline, reads of a kept original go to its copy
//! (`specular_core::local_copy`), and every writer that would write beside the original
//! (marks and keywords into its XMP sidecar, a Develop edit into its `.specular.xmp`, IPTC
//! fields) queues the change here instead. The catalogue's own copy of marks, keywords and
//! edits is written as always.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use specular_library::local::{self as lc, ChangeKind, LocalCopies};

use crate::app::gallery::marks::Mark;

static HANDLE: RwLock<Option<Arc<LocalCopies>>> = RwLock::new(None);

/// Writes a photo's IPTC fields into its sidecar (the metadata editor's `meta_edit` writer, set by
/// the app when it has one): a waiting [`ChangeKind::Iptc`] change is written through it.
pub(in crate::app) type FieldsWriter = fn(&Path, &[(String, String)]) -> std::io::Result<()>;

static FIELDS_WRITER: RwLock<Option<FieldsWriter>> = RwLock::new(None);

/// Sets the IPTC fields writer (see [`FieldsWriter`]).
#[allow(dead_code)]
pub(in crate::app) fn set_fields_writer(w: Option<FieldsWriter>) {
    *FIELDS_WRITER.write() = w;
}

pub(in crate::app) fn fields_writer() -> Option<FieldsWriter> {
    *FIELDS_WRITER.read()
}

/// Makes `l` the process's local copies: reads of offline originals go to their copies.
pub(in crate::app) fn install(l: Option<Arc<LocalCopies>>) {
    let resolver: Option<specular_core::local_copy::Resolver> = l
        .clone()
        .map(|l| Arc::new(move |p: &Path| l.resolve(p)) as specular_core::local_copy::Resolver);
    specular_core::local_copy::set_resolver(resolver);
    *HANDLE.write() = l;
}

pub(in crate::app) fn handle() -> Option<Arc<LocalCopies>> {
    HANDLE.read().clone()
}

/// Whether `path` is an original that cannot be reached now and whose changes wait: it is
/// not there, and it has a local copy or its folder is gone too (a location offline, not a
/// file deleted).
pub(in crate::app) fn offline(path: &Path) -> bool {
    let Some(l) = handle() else { return false };
    if std::fs::metadata(path).is_ok() {
        return false;
    }
    l.copy_of(path).is_some() || path.parent().is_some_and(|d| !d.exists())
}

/// Queues a change of an offline original; false when it is not offline or not catalogued
/// (the caller writes as usual).
fn queue(path: &Path, kind: ChangeKind, version: u32, payload: &[u8]) -> bool {
    if !offline(path) {
        return false;
    }
    let Some(l) = handle() else { return false };
    match l.enqueue(path, kind, version, payload) {
        Ok(q) => q,
        Err(e) => {
            specular_log::warn!(target: specular_log::FOLDER, "local copies: cannot queue a change of {}: {e}", path.display());
            false
        }
    }
}

/// The marks of offline originals wait; the others are returned to be written.
pub(in crate::app) fn queue_marks(changes: Vec<(PathBuf, Mark)>) -> Vec<(PathBuf, Mark)> {
    if handle().is_none() {
        return changes;
    }
    changes
        .into_iter()
        .filter(|(p, m)| {
            let lm = specular_library::Mark {
                rating: m.rating,
                label: m.label,
                flag: m.flag,
            };
            !queue(p, ChangeKind::Marks, 0, &lc::encode_marks(lm))
        })
        .collect()
}

/// The keywords of an offline original wait; false when it is not offline.
pub(in crate::app) fn queue_keywords(path: &Path, keywords: &[String]) -> bool {
    queue(
        path,
        ChangeKind::Keywords,
        0,
        &lc::encode_keywords(keywords),
    )
}

/// The sidecar of version `version` of an offline original's Develop edit waits (the
/// catalogue keeps the edit); false when it is not offline.
pub(in crate::app) fn queue_edit(path: &Path, version: u32) -> bool {
    queue(path, ChangeKind::Edit, version, &[])
}

/// IPTC fields of an offline original wait; false when it is not offline (the metadata
/// editor's writer calls this first).
#[allow(dead_code)]
pub(in crate::app) fn queue_fields(path: &Path, fields: &[(String, String)]) -> bool {
    queue(path, ChangeKind::Iptc, 0, &lc::encode_fields(fields))
}

/// An offline original's keywords as the catalogue holds them (with a waiting change);
/// None when it is reachable (read its sidecar) or not catalogued.
pub(in crate::app) fn offline_keywords(path: &Path) -> Option<Vec<String>> {
    if !offline(path) {
        return None;
    }
    handle()?.keywords_of(path).ok().flatten()
}

/// Whether `path` would be read from a proxy now (its original is offline): exports say so.
pub(in crate::app) fn reads_proxy(path: &Path) -> bool {
    handle().is_some_and(|l| {
        l.copy_of(path)
            .is_some_and(|c| c.kind == lc::CopyKind::Proxy)
            && std::fs::metadata(path).is_err()
    })
}
