//! The edit model: what an edit is, independent of how it renders.
//!
//! - [`kind`]: module kinds and the registry (names, pipeline order, versions, migrations).
//! - [`fields`]: named param fields and their values.
//! - [`params`]: typed params of the alpha modules; [`Params::Other`] for the rest.
//! - [`mask`]: mask shapes, parametric ranges, the combine tree and blending.
//! - [`edit`]: [`Edit`] and [`ModuleInstance`].
//! - [`history`]: undo and redo steps, compress.
//! - [`style`]: styles (partial edits) and presets (one module's fields).
//! - [`codec`], [`migrate`], [`hash`]: storage, migrations and stable hashes.

pub mod codec;
pub mod edit;
pub mod fields;
pub mod hash;
pub mod history;
pub mod kind;
pub mod mask;
pub mod mask_preset;
pub mod migrate;
pub mod params;
pub mod style;
pub mod summary;

pub use edit::{Edit, ModuleInstance, Snapshot, SourceKey};
pub use fields::{Fields, Value};
pub use history::{Change, FieldChange, History, Stack, Step};
pub use kind::{ModuleKind, ModuleSpec, Registry, Stage};
pub use mask::{Blend, BlendMode, MaskExpr, MaskShape, Refine, Shape};
pub use mask_preset::MaskPreset;
pub use params::{ModuleParams, Params};
pub use style::{ApplyMode, AutoRule, FileType, PhotoFacts, Preset, Style};

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod tests;
