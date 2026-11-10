//! The executable node graph built from an [`Edit`].
//!
//! - Every enabled module with a registered [`Op`] becomes a [`Node`]. Modules without one
//!   (kinds whose op lands in a later lane) pass pixels through and are listed in
//!   [`Graph::skipped`].
//! - Raw-stage modules ([`Stage::Raw`]) are not nodes: the raw front end
//!   ([`crate::ops::raw`]) runs them before the graph and makes its source (checkpoint C1),
//!   since they change the image size. They are listed in [`Graph::front_end`]; their params
//!   still enter every prefix hash.
//! - Consecutive [`NodeKind::Point`] nodes form one fused [`Group`]: one generated kernel, one
//!   dispatch. Every other node is a group of its own.
//! - A group is a **checkpoint** when its last node is heavy, or when it ends right before the
//!   focus module (the one the user is editing). The engine keeps checkpoint outputs in its
//!   cache, so a slider on the focus module re-runs only that module and what follows.
//! - [`Graph::rois`] propagates the requested region backwards: each group needs its output
//!   region grown by its halo, clipped to the image.
//! - Nothing here touches the GPU; the engine is in [`crate::gpu`].

pub mod tile;

#[cfg(test)]
pub(crate) mod testops;

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, LazyLock};

use crate::mask::{self, BLEND_WORDS, MaskEnv, MaskTable};
use crate::model::hash::{Hash, base_hash, prefix_hashes};
use crate::model::{Edit, ModuleKind, Params, Registry, Stage};

/// Bumped whenever a kernel's arithmetic changes: part of every cache key.
pub const GRAPH_VERSION: u32 = 4;

/// Uniform words before a group's op params: the dispatch header (see [`crate::gpu`]).
pub const HEADER_WORDS: u32 = 20;
/// Uniform words per dispatch (2 KiB, well within every adapter's uniform binding limit).
pub const PARAM_WORDS: u32 = 512;

/// A region in image pixels (at the request's scale).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Roi {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Roi {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// The whole of a `w`×`h` image.
    pub const fn full(w: u32, h: u32) -> Self {
        Self { x: 0, y: 0, w, h }
    }

    pub fn pixels(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn right(&self) -> u32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> u32 {
        self.y + self.h
    }

    /// Grown by `halo` on every side, clipped to a `w`×`h` image.
    pub fn grow(&self, halo: u32, w: u32, h: u32) -> Roi {
        let x0 = self.x.saturating_sub(halo);
        let y0 = self.y.saturating_sub(halo);
        let x1 = self.right().saturating_add(halo).min(w);
        let y1 = self.bottom().saturating_add(halo).min(h);
        Roi::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
    }

    /// Clipped to a `w`×`h` image.
    pub fn clip(&self, w: u32, h: u32) -> Roi {
        self.grow(0, w, h)
    }

    pub fn contains(&self, o: &Roi) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }
}

/// Storage precision of intermediate and output buffers. Arithmetic is always f32.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Precision {
    /// rgba32float: export, and the tests' bit-exact checks.
    F32,
    /// rgba16float packed in two u32 words per pixel: interactive views (half the bandwidth).
    F16,
}

impl Precision {
    pub const fn bytes_per_pixel(self) -> u64 {
        match self {
            Precision::F32 => 16,
            Precision::F16 => 8,
        }
    }
}

/// How a node runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Each output pixel depends only on the same input pixel. Runs of these fuse.
    Point,
    /// Reads input pixels within [`Op::halo`] of each output pixel.
    Neighbourhood,
    /// Statistics over the whole image, then a pixel pass with them fixed (auto tools,
    /// percentiles). The statistics come from the whole-image low-resolution pass
    /// ([`Op::stats`]), so the pixel pass is a point op and tile-safe; it fuses with the
    /// point ops after it.
    Global,
    /// A pyramid of up to `levels` scales built over the node's input region by its local
    /// program ([`Op::local`]). The graph treats it as a neighbourhood op whose halo covers
    /// every level; coarse levels of large-support ops come from the global pass.
    MultiScale { levels: u8 },
}

impl NodeKind {
    /// Runs as a point kernel `fn(c, px, py, p)` (fusable).
    pub fn pointlike(self) -> bool {
        matches!(self, NodeKind::Point | NodeKind::Global)
    }
}

/// A WGSL kernel of a [`Program`] step. Its source is [`crate::gpu::wgsl::step_source`]:
/// the prelude, the helpers, then `body` as `fn(t: vec2<u32>, p: u32)` run once per thread
/// `t` of the step's `threads`, its words at uniform word `p`.
///
/// A body starting with [`WORKGROUP_BODY`] runs as `fn(w: u32, li: u32, p: u32)` once per
/// workgroup of 16×16 threads instead: `w` the workgroup's index (row-major over the
/// dispatch; `threads` are 16 per workgroup in each direction), `li` the thread within it
/// (0–255). No thread returns before the body, so it may use barriers. A body starting
/// with [`U32_BODY`] sees the scratch as `array<u32>`. Every step may read the WGSL
/// constant `STEP_F16`: whether the pictures are stored as f16.
#[derive(Debug, PartialEq, Eq)]
pub struct Kernel {
    /// A WGSL identifier, unique per kernel and version.
    pub name: &'static str,
    /// The scratch buffer `S` is `array<atomic<u32>>` (histograms) instead of `array<f32>`.
    pub atomic: bool,
    /// Module-scope WGSL the body calls (added once each).
    pub helpers: &'static [&'static str],
    pub body: &'static str,
}

/// The first line of a [`Kernel`] body whose scratch `S` is `array<u32>` (bit casts for
/// f32, packed halves for f16).
pub const U32_BODY: &str = "//u32\n";

/// The first line of a [`Kernel`] body that runs once per workgroup.
pub const WORKGROUP_BODY: &str = "//workgroup\n";

/// One dispatch of a [`Program`].
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub kernel: &'static Kernel,
    /// Threads in x and y (0 skips the work but keeps the dispatch, so the structure never
    /// changes with a value).
    pub threads: [u32; 2],
    /// The step's uniform words (at most [`PARAM_WORDS`] − [`HEADER_WORDS`]).
    pub words: Vec<u32>,
}

/// Compute steps over one f32 scratch buffer (pyramids, box filters, reductions), in order.
///
/// - Statistics ([`Op::stats`]) run over the whole image at the low-resolution grid; the
///   scratch is kept (cached by the input's prefix hash and [`Program::hash`]) and the
///   node's kernel reads it with `st(i)`.
/// - Local programs ([`Op::local`]) run over the node's input region before its kernel,
///   which reads the scratch with `lc(i)`. Their steps read the statistics with `st(i)` too.
///
/// Header words 16–19 of every step and of the kernel hold the region the program covers
/// (x, y, width, height in image pixels).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    /// f32 words of the scratch buffer.
    pub words: u64,
    pub steps: Vec<Step>,
    /// Keep the scratch of this local program from one interactive render to the next.
    pub persist: Option<Persist>,
}

/// A local program whose scratch the engine keeps between interactive renders (requests
/// with checkpoints) while the node, its input (prefix hash, source, region, scale,
/// precision) and the scratch size stay the same: the program may reuse what it wrote the
/// last time (retouch keeps solved spots). A new or changed scratch first runs `reset`.
/// One-off renders (exports) always run `reset` on a scratch of their own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Persist {
    pub reset: Vec<Step>,
    /// The scratch is a function of the input and the program alone: kept scratch last
    /// written by the same program ([`Program::hash`]) for the same input is used as it is,
    /// without running a step (a node whose blend alone changed: diffuse or sharpen).
    pub reuse: bool,
}

impl Program {
    pub fn step(&mut self, kernel: &'static Kernel, threads: [u32; 2], words: Vec<u32>) {
        self.steps.push(Step {
            kernel,
            threads,
            words,
        });
    }

    /// Identifies what the program computes from its input (kernels, sizes and words).
    pub fn hash(&self) -> Hash {
        let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-10-31 program v1");
        h.update(&self.words.to_le_bytes());
        for s in &self.steps {
            h.update(s.kernel.name.as_bytes());
            h.update(&[0]);
            for v in s.threads.iter().chain(&s.words) {
                h.update(&v.to_le_bytes());
            }
            h.update(&[1]);
        }
        *h.finalize().as_bytes()
    }
}

/// Long side of the whole-image grid that statistics are taken on (smaller images use
/// their own size).
pub const STATS_SIDE: u32 = 1024;

/// The statistics grid of a `w`×`h` image: the long side at most [`STATS_SIDE`].
pub fn stats_grid(w: u32, h: u32) -> (u32, u32) {
    stats_grid_at(w, h, STATS_SIDE)
}

/// The statistics grid of a `w`×`h` image with the long side at most `side` (half-scale
/// drafts use a smaller one).
pub fn stats_grid_at(w: u32, h: u32, side: u32) -> (u32, u32) {
    let side = side.max(1);
    let long = w.max(h).max(1);
    if long <= side {
        return (w.max(1), h.max(1));
    }
    let f = f64::from(side) / f64::from(long);
    let d = |v: u32| ((f64::from(v) * f).round() as u32).clamp(1, side);
    (d(w), d(h))
}

/// One module's GPU op: its kind, halo, uniform params and WGSL.
///
/// WGSL contract (see [`crate::gpu`] for the prelude):
/// - [`NodeKind::Point`] and [`NodeKind::Global`]: the body of
///   `fn(c: vec4<f32>, px: i32, py: i32, p: u32) -> vec4<f32>`, returning the new pixel
///   (`px`, `py` are its image coordinates); params are read with `pf(p + i)` / `pu(p + i)`.
/// - Other kinds: the body of `fn(x: i32, y: i32, p: u32) -> vec4<f32>`, returning output
///   pixel (x, y) in image coordinates; input pixels are read with `src_at(x, y)`, which
///   clamps to the image edge.
/// - Statistics are read with `st(i)` and a local program's scratch with `lc(i)`.
pub trait Op: Send + Sync {
    /// A WGSL identifier, unique per op and kernel version (`gain_v1`).
    fn name(&self) -> &'static str;
    fn kind(&self) -> NodeKind;
    /// Input pixels needed around each output pixel at `scale` (0 for point ops).
    fn halo(&self, _params: &Params, _scale: f32) -> u32 {
        0
    }
    /// Expensive enough that its output is kept as a checkpoint.
    fn heavy(&self) -> bool {
        false
    }
    /// Uniform words the params take: fixed per op version, so the kernel never changes
    /// when a value does.
    fn param_words(&self) -> u32;
    /// Writes the params for `scale` into `out` (`param_words` long).
    fn write_params(&self, params: &Params, scale: f32, out: &mut [u32]);
    fn wgsl(&self) -> &'static str;
    /// Module-scope WGSL the body calls (functions with unique names). Each chunk is added
    /// once per kernel, so ops may share one.
    fn helpers(&self) -> &'static [&'static str] {
        &[]
    }
    /// Whether these params leave every pixel as it is, so the graph leaves the node out
    /// (no kernel, no statistics, no pyramid). Leaving the identity then is like switching
    /// the module on: its kernels are built once and cached.
    fn identity(&self, _params: &Params) -> bool {
        false
    }
    /// Whether the node takes statistics with these params. May depend on a switch
    /// (exposure's auto) but never on a value a slider moves.
    fn has_stats(&self, _params: &Params) -> bool {
        false
    }
    /// The statistics program, run over the whole input image reduced to the `w`×`h` grid
    /// ([`stats_grid`]); `scale` is the grid's scale (grid pixels per source pixel × the
    /// request's scale). Called only when [`Op::has_stats`].
    fn stats(&self, _params: &Params, _w: u32, _h: u32, _scale: f32) -> Option<Program> {
        None
    }
    /// The local program over the node's `input` region (image pixels at `scale`), run
    /// before its kernel. Only for kinds other than point and global.
    fn local(&self, _params: &Params, _input: Roi, _scale: f32) -> Option<Program> {
        None
    }
    /// A table the kernel reads with `dt(i)` (a 3D LUT): uploaded once per content hash
    /// and kept by the engine, so a slider never uploads it again. A group holds at most
    /// one node with data.
    fn data(&self, _params: &Params) -> Option<Arc<OpData>> {
        None
    }
    /// Params resolved against the picture the graph runs on (retouch spots, kept in
    /// source coordinates, mapped through the warp). The result is the node's params for
    /// every other call; `None` keeps them as they are.
    fn place(&self, _params: &Params, _at: &Placement<'_>) -> Option<Params> {
        None
    }
}

/// The picture a graph runs on, relative to the source frame ([`Op::place`]).
#[derive(Clone, Copy, Debug)]
pub struct Placement<'a> {
    pub geometry: mask::Geometry<'a>,
    /// The graph image's size (pixels).
    pub image: (u32, u32),
    /// Sensor pixels per pixel of the source the frame is measured in (2 for a raw binned
    /// 2×2 for a thumbnail): lengths in sensor pixels scale by it.
    pub sensor_bin: f64,
    /// The module is being dragged (the graph's focus): a quicker first render that the
    /// next one without focus refines.
    pub draft: bool,
    /// The photo's identity ([`crate::mask::depth::photo`]): what cached results computed
    /// for it (depth maps, removal patches) are keyed by.
    pub photo: [u8; 32],
}

/// A node's table ([`Op::data`]): f32 words and a hash of them (folded into the prefix
/// hashes after the node, so a file whose content changes under the same name renders
/// anew).
#[derive(Debug, PartialEq)]
pub struct OpData {
    pub hash: Hash,
    pub words: Vec<f32>,
}

impl OpData {
    pub fn new(words: Vec<f32>) -> OpData {
        let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-10-31 op data v1");
        for w in &words {
            h.update(&w.to_bits().to_le_bytes());
        }
        OpData {
            hash: *h.finalize().as_bytes(),
            words,
        }
    }
}

/// The ops this build can run, by module kind and param version.
#[derive(Clone, Default)]
pub struct OpRegistry {
    ops: HashMap<(ModuleKind, u16), Arc<dyn Op>>,
}

impl std::fmt::Debug for OpRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<_> = self.ops.values().map(|o| o.name()).collect();
        names.sort_unstable();
        f.debug_struct("OpRegistry").field("ops", &names).finish()
    }
}

impl OpRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    /// The shipped ops (each module family adds its own in
    /// [`crate::ops::register_builtin`]).
    pub fn builtin() -> &'static OpRegistry {
        static BUILTIN: LazyLock<OpRegistry> = LazyLock::new(|| {
            let mut r = OpRegistry::empty();
            crate::ops::register_builtin(&mut r);
            r
        });
        &BUILTIN
    }

    /// Registers `op` for `kind` at param `version`, replacing an earlier one.
    pub fn register(&mut self, kind: ModuleKind, version: u16, op: Arc<dyn Op>) {
        self.ops.insert((kind, version), op);
    }

    pub fn get(&self, kind: ModuleKind, version: u16) -> Option<&Arc<dyn Op>> {
        self.ops.get(&(kind, version))
    }
}

/// One op instance in the graph.
#[derive(Clone)]
pub struct Node {
    /// The module (kind, instance).
    pub id: (ModuleKind, u16),
    pub op: Arc<dyn Op>,
    pub params: Params,
    /// The edit's prefix hash after this module.
    pub prefix: Hash,
    /// Takes statistics ([`Op::has_stats`]).
    pub stats: bool,
    /// Its table ([`Op::data`]).
    pub data: Option<Arc<OpData>>,
    /// Its blend's uniform words ([`crate::mask`]), after the op's own.
    pub blend: Option<[u32; BLEND_WORDS as usize]>,
}

impl Node {
    /// Uniform words of the node: the op's, then its blend's.
    pub fn words(&self) -> u32 {
        self.op.param_words() + if self.blend.is_some() { BLEND_WORDS } else { 0 }
    }
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("id", &self.id)
            .field("op", &self.op.name())
            .finish()
    }
}

/// One dispatch: a fused run of point nodes, or one other node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub nodes: Range<usize>,
    pub fused: bool,
    /// The engine caches this group's output.
    pub checkpoint: bool,
}

/// What a group reads and writes for one requested region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupRoi {
    pub input: Roi,
    pub output: Roi,
}

/// Why a graph cannot be built.
#[derive(Clone, Debug, PartialEq)]
pub enum GraphError {
    /// An op's params do not fit one dispatch's uniform block.
    ParamsTooLarge(&'static str),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::ParamsTooLarge(n) => write!(f, "the params of {n} are too large"),
        }
    }
}

impl std::error::Error for GraphError {}

/// The executable graph of one edit (with one focus module).
#[derive(Clone, Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    /// Enabled modules without an op in this build: passed through.
    pub skipped: Vec<(ModuleKind, u16)>,
    /// Enabled modules whose params are the identity ([`Op::identity`]): left out.
    pub identity: Vec<(ModuleKind, u16)>,
    /// Enabled raw-stage modules and their params, for the raw front end.
    pub front_end: Vec<((ModuleKind, u16), Params)>,
    /// Enabled warp-stage modules (lens, crop and orientation): the geometry warp
    /// ([`crate::ops::geometry`]) runs them between the front end and the graph.
    pub warp: Vec<(ModuleKind, u16)>,
    /// The prefix hash before any module (source and process version).
    pub base: Hash,
    /// The mask table the kernels read ([`crate::mask`]), when a module blends.
    pub masks: Option<Arc<MaskTable>>,
    /// Masks that render as nothing, and why.
    pub mask_issues: Vec<((ModuleKind, u16), mask::Issue)>,
    /// A tint overlay asked for but left out: a node after the module overwrites the
    /// alpha the mask travels in (a refine pass, or any non-point op). The caller draws
    /// it from a grey overlay render instead ([`crate::preview::Developer::render`]).
    pub tint_apart: bool,
}

impl Graph {
    /// Builds the graph of `edit`. `focus` is the module being edited: fusion splits before
    /// it and the group before it is a checkpoint.
    pub fn build(
        edit: &Edit,
        ops: &OpRegistry,
        focus: Option<(ModuleKind, u16)>,
    ) -> Result<Graph, GraphError> {
        Graph::build_for(edit, ops, focus, None)
    }

    /// [`Graph::build`] with the output transform appended (it fuses into the last group).
    pub fn build_for(
        edit: &Edit,
        ops: &OpRegistry,
        focus: Option<(ModuleKind, u16)>,
        output: Option<&crate::ops::output::OutputSpace>,
    ) -> Result<Graph, GraphError> {
        Graph::build_masked(edit, ops, focus, output, None)
    }

    /// [`Graph::build_for`] with the masks evaluated against `env` (the warp and image the
    /// graph runs on). Without one, masks take the image as the whole frame, square.
    pub fn build_masked(
        edit: &Edit,
        ops: &OpRegistry,
        focus: Option<(ModuleKind, u16)>,
        output: Option<&crate::ops::output::OutputSpace>,
        env: Option<&mut MaskEnv<'_>>,
    ) -> Result<Graph, GraphError> {
        let mut overlay = None;
        let placement = env.as_deref().map(|e| Placement {
            geometry: e.geometry,
            image: e.image,
            draft: false,
            sensor_bin: e.sensor_bin,
            photo: mask::depth::photo(&edit.source),
        });
        let compiled = if mask::has_blends(edit) {
            match env {
                Some(env) => {
                    overlay = env.overlay;
                    mask::compile(edit, env)
                }
                None => {
                    let mut cache = mask::MaskCache::default();
                    mask::compile(
                        edit,
                        &mut MaskEnv {
                            geometry: mask::Geometry::Frame { w: 1.0, h: 1.0 },
                            image: (1024, 1024),
                            scale: 1.0,
                            cache: &mut cache,
                            sensor_bin: 1.0,
                            overlay: None,
                        },
                    )
                }
            }
        } else {
            None
        };
        let prefixes = prefix_hashes(edit);
        let mut nodes = Vec::new();
        let mut skipped = Vec::new();
        let mut identity = Vec::new();
        let mut front_end = Vec::new();
        let mut warp = Vec::new();
        let registry = Registry::builtin();
        // Per node: does fusion break (and a checkpoint go) right before it?
        let mut focus_at = None;
        // The hash of the tables so far, folded into every later prefix.
        let mut salt: Option<Hash> = None;
        for (m, prefix) in edit.modules.iter().zip(prefixes) {
            if !m.enabled {
                continue;
            }
            if registry.get(m.kind).is_some_and(|s| s.stage == Stage::Raw) {
                front_end.push((m.id(), m.params.clone()));
                continue;
            }
            if registry.get(m.kind).is_some_and(|s| s.stage == Stage::Warp) {
                warp.push(m.id());
                continue;
            }
            // Borders and watermark: added after the render (exports, prints).
            if registry
                .get(m.kind)
                .is_some_and(|s| s.stage == Stage::Output)
            {
                continue;
            }
            let Some(op) = ops.get(m.kind, m.version) else {
                skipped.push(m.id());
                continue;
            };
            if op.identity(&m.params) {
                identity.push(m.id());
                continue;
            }
            let nb = compiled.as_ref().and_then(|c| c.blends.get(&m.id()));
            let blend_words = if nb.is_some() { BLEND_WORDS } else { 0 };
            if op.param_words() + blend_words > PARAM_WORDS - HEADER_WORDS {
                return Err(GraphError::ParamsTooLarge(op.name()));
            }
            if focus == Some(m.id()) {
                focus_at = Some(nodes.len());
            }
            if let Some(spec) = nb.and_then(|b| b.refine.as_ref()) {
                let before = nodes
                    .last()
                    .map_or_else(|| base_hash(edit), |n: &Node| n.prefix);
                let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 refine v1");
                h.update(&before);
                h.update(&prefix);
                nodes.push(Node {
                    id: (mask::refine::REFINE, m.instance),
                    op: Arc::new(mask::refine::RefineOp::for_module(
                        op,
                        &m.params,
                        spec.output,
                    )),
                    params: mask::refine::params(spec),
                    prefix: *h.finalize().as_bytes(),
                    stats: false,
                    data: None,
                    blend: None,
                });
            }
            let draft = focus == Some(m.id());
            let placed = placement
                .as_ref()
                .and_then(|at| op.place(&m.params, &Placement { draft, ..*at }));
            if draft && placed.is_some() {
                // A draft is cached apart from the refined render, and so is all after it.
                let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 draft v1");
                h.update(salt.as_ref().unwrap_or(&[0u8; 32]));
                salt = Some(*h.finalize().as_bytes());
            }
            let params = placed.unwrap_or_else(|| m.params.clone());
            let data = op.data(&params);
            if let Some(d) = &data {
                let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-10-31 salt v1");
                h.update(salt.as_ref().unwrap_or(&[0u8; 32]));
                h.update(&d.hash);
                salt = Some(*h.finalize().as_bytes());
            }
            let prefix = match &salt {
                Some(s) => {
                    let mut h =
                        blake3::Hasher::new_derive_key("specular-develop 2026-10-31 salted v1");
                    h.update(&prefix);
                    h.update(s);
                    *h.finalize().as_bytes()
                }
                None => prefix,
            };
            nodes.push(Node {
                id: m.id(),
                op: op.clone(),
                stats: op.has_stats(&params),
                params,
                prefix,
                data,
                blend: nb.map(|b| b.words),
            });
        }
        if let Some(o) = output {
            let params = o.params();
            let before = nodes
                .last()
                .map_or_else(|| base_hash(edit), |n: &Node| n.prefix);
            let mut h =
                blake3::Hasher::new_derive_key("specular-develop 2026-10-31 output hash v1");
            h.update(&before);
            let mut w = crate::model::codec::Writer {
                buf: Vec::new(),
                canonical: true,
            };
            crate::model::codec::write_fields(&mut w, &params.to_fields());
            h.update(&w.buf);
            nodes.push(Node {
                id: (crate::ops::output::OUTPUT, 0),
                op: Arc::new(crate::ops::output::OutputOp),
                params,
                prefix: *h.finalize().as_bytes(),
                stats: false,
                data: None,
                blend: None,
            });
        }
        // The overlay reads the module's mask from the alpha at the end. A grey overlay
        // shows the mask alone, so nothing after the module is needed (and a later refine
        // pass would overwrite the alpha). A tint needs the whole picture: when a later
        // node may overwrite the alpha, the caller composes it from a grey render.
        let mut tint_apart = false;
        let at = overlay.and_then(|(target, _)| {
            nodes
                .iter()
                .position(|n| n.id == target && n.blend.is_some())
        });
        if let (Some((_, o)), Some(t)) = (overlay, at) {
            match o {
                mask::Overlay::Grey => nodes.truncate(t + 1),
                mask::Overlay::Tint => {
                    tint_apart = nodes[t + 1..].iter().any(|n| !n.op.kind().pointlike());
                }
            }
        }
        if let Some((_, o)) = overlay
            && at.is_some()
            && !tint_apart
        {
            let before = nodes.last().map_or_else(|| base_hash(edit), |n| n.prefix);
            let mut h = blake3::Hasher::new_derive_key("specular-develop 2026-11-01 overlay v1");
            h.update(&before);
            h.update(&[o as u8]);
            nodes.push(Node {
                id: (mask::refine::OVERLAY, 0),
                op: Arc::new(mask::refine::OverlayOp),
                params: mask::refine::overlay_params(o),
                prefix: *h.finalize().as_bytes(),
                stats: false,
                data: None,
                blend: None,
            });
        }
        let groups = group_nodes(&nodes, focus_at);
        let (masks, mask_issues) = match compiled {
            Some(c) => (Some(c.table), c.issues),
            None => (None, Vec::new()),
        };
        Ok(Graph {
            nodes,
            groups,
            skipped,
            identity,
            front_end,
            warp,
            base: base_hash(edit),
            masks,
            mask_issues,
            tint_apart,
        })
    }

    /// The prefix hash of a group's output.
    pub fn group_hash(&self, g: usize) -> Hash {
        let r = &self.groups[g].nodes;
        self.nodes[r.end - 1].prefix
    }

    /// The prefix hash of a group's input.
    pub fn input_hash(&self, g: usize) -> Hash {
        match self.groups[g].nodes.start {
            0 => self.base,
            i => self.nodes[i - 1].prefix,
        }
    }

    /// The halo of group `g` at `scale`.
    pub fn halo(&self, g: usize, scale: f32) -> u32 {
        self.groups[g]
            .nodes
            .clone()
            .map(|i| {
                let n = &self.nodes[i];
                if n.op.kind().pointlike() {
                    0
                } else {
                    n.op.halo(&n.params, scale)
                }
            })
            .fold(0, u32::saturating_add)
    }

    /// The node of group `g` that takes statistics (always its first).
    pub fn stats_node(&self, g: usize) -> Option<&Node> {
        let n = &self.nodes[self.groups[g].nodes.start];
        n.stats.then_some(n)
    }

    /// The table of group `g` (its one node with data).
    pub fn data(&self, g: usize) -> Option<&Arc<OpData>> {
        self.nodes[self.groups[g].nodes.clone()]
            .iter()
            .find_map(|n| n.data.as_ref())
    }

    /// The local program of group `g` (a lone node that is not point-like) over `input`.
    pub fn local(&self, g: usize, input: Roi, scale: f32) -> Option<Program> {
        if self.groups[g].fused {
            return None;
        }
        let n = &self.nodes[self.groups[g].nodes.start];
        n.op.local(&n.params, input, scale)
    }

    /// Input and output regions of every group so the last one produces `out` of a
    /// `width`×`height` image. Halos grow backwards. Statistics come from their own
    /// whole-image pass, so they add nothing here.
    pub fn rois(&self, out: Roi, width: u32, height: u32, scale: f32) -> Vec<GroupRoi> {
        let mut rois = vec![GroupRoi::default(); self.groups.len()];
        let mut need = out.clip(width, height);
        for g in (0..self.groups.len()).rev() {
            let input = need.grow(self.halo(g, scale), width, height);
            rois[g] = GroupRoi {
                input,
                output: need,
            };
            need = input;
        }
        rois
    }

    /// The uniform params of group `g` (after the header), op after op.
    pub fn params(&self, g: usize, scale: f32) -> Vec<u32> {
        let mut words = Vec::new();
        for n in &self.nodes[self.groups[g].nodes.clone()] {
            let at = words.len();
            words.resize(at + n.words() as usize, 0);
            let ow = n.op.param_words() as usize;
            n.op.write_params(&n.params, scale, &mut words[at..at + ow]);
            if let Some(b) = &n.blend {
                words[at + ow..].copy_from_slice(b);
            }
        }
        words
    }
}

/// Splits nodes into dispatch groups.
fn group_nodes(nodes: &[Node], focus_at: Option<usize>) -> Vec<Group> {
    let budget = PARAM_WORDS - HEADER_WORDS;
    let mut groups: Vec<Group> = Vec::new();
    let mut words = 0;
    let mut has_data = false;
    for (i, n) in nodes.iter().enumerate() {
        let point = n.op.kind().pointlike();
        let pw = n.words();
        // A node taking statistics starts its group: they are taken on the group's input.
        // A group binds one table.
        let extend = point
            && !n.stats
            && !(has_data && n.data.is_some())
            && focus_at != Some(i)
            && groups
                .last()
                .is_some_and(|g| g.fused && g.nodes.end == i && words + pw <= budget);
        if extend {
            let g = groups.last_mut().expect("a group to extend");
            g.nodes.end = i + 1;
            words += pw;
            has_data |= n.data.is_some();
        } else {
            has_data = n.data.is_some();
            groups.push(Group {
                nodes: i..i + 1,
                fused: point,
                checkpoint: false,
            });
            words = pw;
        }
    }
    for g in &mut groups {
        let last = &nodes[g.nodes.end - 1];
        g.checkpoint = last.op.heavy() || focus_at == Some(g.nodes.end);
    }
    groups
}

#[cfg(test)]
mod tests;
