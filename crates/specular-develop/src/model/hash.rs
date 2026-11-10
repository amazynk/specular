//! Stable hashes: per module, and per upstream prefix.
//!
//! - A module hash covers what changes its pixels: kind, version, params, blend and the
//!   content of the shapes and masks the blend uses. Labels, instance numbers and field
//!   order do not count.
//! - The prefix hash after module *i* chains the source, the process version and every
//!   enabled module up to *i*. Checkpoint cache keys add ROI, scale, precision and graph
//!   version to it; the last one is the edit hash that keys edited thumbnails.
//! - Bytes are the codec's little-endian form with -0.0 and NaN made canonical, hashed with
//!   BLAKE3 in derive-key mode, so values are the same on every platform.

use super::codec::{Writer, write_blend, write_fields, write_shape, write_source};
use super::edit::{Edit, ModuleInstance};
use super::fields::Fields;
use super::mask::MaskExpr;

pub type Hash = [u8; 32];

const MODULE_CONTEXT: &str = "specular-develop 2026-10-31 module hash v1";
const PREFIX_CONTEXT: &str = "specular-develop 2026-10-31 prefix hash v1";

/// "Use mask from" chains longer than this hash as a cycle marker.
const MAX_REFS: u32 = 8;

fn sorted(f: &Fields) -> Fields {
    let mut v = f.0.clone();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    Fields(v)
}

fn write_module(w: &mut Writer, edit: &Edit, m: &ModuleInstance, depth: u32) {
    w.u16(m.kind.0);
    w.u16(m.version);
    write_fields(w, &sorted(&m.params.to_fields()));
    w.bool(m.blend.is_some());
    if let Some(b) = &m.blend {
        write_blend(w, b, &mut |w, e| resolve(w, edit, e, depth));
    }
}

/// Writes shape and module references by content; false leaves `e` to the plain writer.
fn resolve(w: &mut Writer, edit: &Edit, e: &MaskExpr, depth: u32) -> bool {
    match e {
        MaskExpr::Shape(id) => {
            w.u8(0x81);
            match edit.mask(*id) {
                Some(s) => {
                    w.bool(true);
                    // The id is renumbered on paste; hash the shape only.
                    let mut inner = Writer {
                        buf: Vec::new(),
                        canonical: true,
                    };
                    write_shape(&mut inner, s);
                    let id_len = varint_len(u64::from(s.id));
                    w.bytes(&inner.buf[id_len..]);
                }
                None => w.bool(false),
            }
            true
        }
        MaskExpr::FromModule { kind, instance } => {
            w.u8(0x84);
            match edit.module(*kind, *instance) {
                Some(m) if depth < MAX_REFS => {
                    w.bool(true);
                    match &m.blend {
                        Some(b) => {
                            w.bool(true);
                            super::codec::write_expr(w, &b.mask, &mut |w, e| {
                                resolve(w, edit, e, depth + 1)
                            });
                            // The borrowed mask includes the source's invert.
                            if b.invert {
                                w.u8(0x86);
                            }
                            // And its contrast, offset and refinement (only when set, so
                            // other hashes keep their values).
                            let f = &b.refine;
                            if *f != super::mask::Refine::default() {
                                w.u8(0x87);
                                w.f32s(&[
                                    f.feather,
                                    f.blur,
                                    f.contrast,
                                    f.offset,
                                    f.edge_aware,
                                    f.details,
                                    f.radius,
                                ]);
                            }
                        }
                        None => w.bool(false),
                    }
                }
                _ => w.bool(false),
            }
            true
        }
        _ => false,
    }
}

fn varint_len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

/// The hash of one module instance in `edit` (enabled or not).
pub fn module_hash(edit: &Edit, m: &ModuleInstance) -> Hash {
    let mut w = Writer {
        buf: Vec::new(),
        canonical: true,
    };
    write_module(&mut w, edit, m, 0);
    let mut h = blake3::Hasher::new_derive_key(MODULE_CONTEXT);
    h.update(&w.buf);
    *h.finalize().as_bytes()
}

/// The hash of the source and process version, before any module.
pub fn base_hash(edit: &Edit) -> Hash {
    let mut w = Writer::default();
    write_source(&mut w, &edit.source);
    w.u16(edit.process);
    let mut h = blake3::Hasher::new_derive_key(PREFIX_CONTEXT);
    h.update(&w.buf);
    *h.finalize().as_bytes()
}

/// The prefix hash after each module, in stack order. A disabled module repeats the hash
/// before it.
pub fn prefix_hashes(edit: &Edit) -> Vec<Hash> {
    let mut cur = base_hash(edit);
    edit.modules
        .iter()
        .map(|m| {
            if m.enabled {
                let mut h = blake3::Hasher::new_derive_key(PREFIX_CONTEXT);
                h.update(&cur);
                h.update(&module_hash(edit, m));
                cur = *h.finalize().as_bytes();
            }
            cur
        })
        .collect()
}

/// The hash of the whole edit's output (edited thumbnails, export dedup).
pub fn edit_hash(edit: &Edit) -> Hash {
    prefix_hashes(edit)
        .last()
        .copied()
        .unwrap_or_else(|| base_hash(edit))
}

/// Lowercase hex, for logs and test fixtures.
pub fn hex(h: &Hash) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}
