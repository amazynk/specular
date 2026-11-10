//! Develop mode (DESIGN.md §6.10): non-destructive photo editing.
//!
//! This crate holds the edit model, the GPU graph engine and the raw
//! front end. The other ops, the sidecar writer and export come in later lanes.
//!
//! - [`model`]: [`Edit`](model::Edit), module instances with typed, versioned params, masks
//!   and blending, history, styles and presets.
//! - [`model::codec`]: the versioned binary form the catalogue stores (`edits.edit`,
//!   `edit_steps.step`, `styles.items`, `presets.params`).
//! - [`model::migrate`]: per-module param versions; at most two live kernel versions per
//!   module; older params migrate on load with a notice.
//! - [`model::hash`]: stable per-module and upstream-prefix hashes, the basis of the
//!   checkpoint cache keys and of edited-thumbnail keys.
//! - [`graph`]: the executable node graph of an edit: node kinds, point-op fusion,
//!   checkpoints, ROI and halo propagation, the tile plan.
//! - [`gpu`]: the wgpu engine: generated kernels, pipeline and bind-group caches, buffer
//!   pool, checkpoint LRU, tiled dispatch, GPU timestamps; global statistics on a
//!   whole-image grid and local pyramid programs.
//! - [`ops::tone`]: tone equalizer, local contrast, curves and levels.
//! - [`ops::raw`]: the raw front end: stored sensor samples → linear Rec.2020 scene-referred
//!   RGB on the GPU (black/white/linearise, DNG opcodes, white balance, demosaic, camera
//!   matrix or DNG profile), the graph's source.
//!
//! # Rules
//!
//! - The source file is never written. Edits live in the catalogue and in a sidecar.
//! - Every param is a **named field**. Storage, migrations, presets and the sidecar all work
//!   on names, never on struct layouts, so a field can be added without a format change.
//! - Hashes and the binary form are byte-for-byte the same on every platform and endianness.
//! - Unknown module kinds (from a newer Specular) are kept and written back unchanged.
//!
//! # Sidecar sketch
//!
//! `IMG_1234.CR2.specular.xmp`, namespace `http://ns.specular.app/develop/1/` (prefix `spd:`).
//! The model maps onto it one to one:
//!
//! ```xml
//! <rdf:Description spd:schema="1" spd:process="1"
//!     spd:source="size=24510322 mtime=1730000000000000000 hash=9f2c…">
//!   <spd:modules><rdf:Seq>
//!     <rdf:li spd:module="exposure" spd:version="1" spd:instance="0" spd:enabled="True"
//!         spd:exposure_ev="0.7" spd:black_offset="0" spd:auto="False"/>
//!     <rdf:li spd:module="colour_calibration" spd:version="1" spd:instance="0"
//!         spd:enabled="True" spd:adaptation="cat16" spd:white_x="0.3457" …>
//!       <spd:blend spd:mode="normal" spd:opacity="1"
//!           spd:mask="combine(subtract, shape(3), range(luminance, input, -6 -4 0 2))"/>
//!     </rdf:li>
//!   </rdf:Seq></spd:modules>
//!   <spd:masks><rdf:Seq>
//!     <rdf:li spd:id="3" spd:shape="ellipse" spd:centre="0.4 0.5" spd:radii="0.2 0.1" …/>
//!     <rdf:li spd:id="4" spd:shape="brush" spd:strokes="base64 of the codec's stroke list"/>
//!   </rdf:Seq></spd:masks>
//!   <spd:history> one rdf:li per step: target module, changed fields before and after </spd:history>
//! </rdf:Description>
//! ```
//!
//! - Each [`Value`](model::Value) has one text form: `True`/`False`, integers, floats in the
//!   shortest round-trip form, text, and float lists separated by spaces.
//! - Choices (enums) are written by their stable names (`cat16`, `sigmoid`).
//! - Module and field names are the registry's ([`model::kind`]), never translated.

pub mod chart;
pub mod clip;
pub mod colour;
pub mod drag;
pub mod export;
pub mod gpu;
pub mod graph;
pub mod interop;
pub mod mask;
pub mod matching;
pub mod merge;
pub mod model;
pub mod ops;
pub mod paths;
pub mod preview;
pub mod prewarp;
pub mod proof;
pub mod proxy;
pub mod scopes;
pub mod sidecar;
pub mod source;
pub mod thumb;

#[cfg(test)]
pub(crate) mod testgpu;
