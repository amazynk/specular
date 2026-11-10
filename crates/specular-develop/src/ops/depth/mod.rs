//! The depth tools: modules driven by the photo's depth map
//! ([`crate::mask::depth`]).
//!
//! - [`blur`]: a lens blur by depth: each pixel's circle of confusion grows with its depth's
//!   distance from the focus plane (linear in inverse depth, as a thin lens's), gathered as
//!   a scatter would spread it, with nearer blurred pixels spilling over sharper ones and
//!   farther ones held back behind them; bright lights bloom into discs.
//! - [`relight`]: an exposure gain by depth band: nearer and farther parts brightened or
//!   darkened apart, in scene-linear light.
//!
//! Both read the depth map from their table ([`Op::data`]): [`Op::place`] resamples the
//! frame raster onto the graph's picture ([`DepthMap::on_picture`], kept in a small cache
//! by photo, model, geometry and size), and the table's hash keys every render after the
//! node, so the picture renders anew when the depth map arrives. Without a depth map
//! (its model not installed, or not computed yet) both pass pixels through.

pub mod blur;
pub mod relight;

#[cfg(test)]
pub(crate) mod cpu;
#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex, OnceLock};

use crate::graph::{OpData, Placement};
use crate::mask::depth::{self, DepthMap};
use crate::model::mask::DEPTH_MODEL;
use crate::model::{Fields, ModuleKind, Params, Value};

/// Whether modules of `kind` read the depth map.
pub fn uses_depth(kind: ModuleKind) -> bool {
    kind == ModuleKind::RELIGHT || kind == ModuleKind::DEPTH_BLUR
}

/// Field of placed params: the depth table's id in [`tables`].
const TABLE: &str = "_depth_table";
/// Field of placed params: the picture's size (pixels).
pub(crate) const IMAGE: &str = "_image";
/// Field of placed params: a draft render (the module is being dragged).
pub(crate) const DRAFT: &str = "_draft";

/// The model a module's fields name (the default when none).
pub fn model_of(f: &Fields) -> String {
    f.text("model")
        .filter(|m| !m.is_empty())
        .unwrap_or(DEPTH_MODEL)
        .to_owned()
}

/// Depth tables by id.
type Tables = Mutex<Vec<([u8; 32], Arc<OpData>)>>;

/// Depth tables made for recent placements, by id (a few: the preview and an export).
fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Mutex::new(Vec::new()))
}

const TABLES_KEPT: usize = 6;

/// The depth table for `fields` on the picture `at` describes: its id, made (or found)
/// in [`tables`].
fn table_for(fields: &Fields, at: &Placement<'_>) -> [u8; 32] {
    let key = depth::key(&at.photo, &model_of(fields));
    let raster = crate::mask::ai::store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key);
    let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 depth table v1");
    h.update(&key);
    h.update(&at.geometry.key());
    h.update(&at.image.0.to_le_bytes());
    h.update(&at.image.1.to_le_bytes());
    h.update(&[u8::from(raster.is_some())]);
    let id = *h.finalize().as_bytes();
    let mut t = tables().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = t.iter().position(|(k, _)| *k == id) {
        let e = t.remove(i);
        t.push(e);
        return id;
    }
    let map = raster.map_or_else(DepthMap::none, |r| {
        DepthMap::on_picture(&r, &at.geometry, at.image)
    });
    t.push((id, Arc::new(OpData::new(map.words()))));
    if t.len() > TABLES_KEPT {
        t.remove(0);
    }
    id
}

/// The table a placed module reads (an empty one when its id is gone or it was never
/// placed).
pub(crate) fn data_of(p: &Params) -> Arc<OpData> {
    let f = p.to_fields();
    let found = f.text(TABLE).and_then(|hex| {
        let id = unhex(hex)?;
        let t = tables().lock().unwrap_or_else(|e| e.into_inner());
        t.iter().find(|(k, _)| *k == id).map(|(_, d)| d.clone())
    });
    found.unwrap_or_else(|| Arc::new(OpData::new(DepthMap::none().words())))
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// `p` placed on the picture `at` describes: its fields with the depth table's id, the
/// picture's size and whether it is a draft.
pub(crate) fn place(kind: ModuleKind, p: &Params, at: &Placement<'_>) -> Params {
    let f = p.to_fields();
    let id = table_for(&f, at);
    Params::Other(
        kind,
        f.with(TABLE, Value::Text(hex(&id)))
            .with(
                IMAGE,
                Value::Floats(vec![at.image.0 as f32, at.image.1 as f32]),
            )
            .with(DRAFT, Value::Bool(at.draft)),
    )
}

/// The picture's long side a placed module saw (1024 when never placed).
pub(crate) fn image_long(f: &Fields) -> f32 {
    let [w, h] = f.floats_or(IMAGE, [1024.0, 1024.0]);
    w.max(h).max(1.0)
}
