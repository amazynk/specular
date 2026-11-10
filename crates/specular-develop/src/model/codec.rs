//! The binary form the catalogue stores: little-endian, varint lengths, every record in a
//! length-prefixed frame.
//!
//! - Each blob starts with a 4-byte magic and the format version ([`FORMAT`]). A newer format
//!   is refused, never guessed at.
//! - Params are stored as named fields, so params never need a format change; their own
//!   versions go through [`super::migrate`] on load.
//! - Frames let a newer writer append to a record: this reader skips what it does not know
//!   at the end of a frame.
//! - Output is the same bytes on every platform. [`super::hash`] uses the same writer.

use super::edit::{Edit, ModuleInstance, Snapshot, SourceKey};
use super::fields::{Choice, Fields, Value};
use super::history::{Change, FieldChange, History, Stack, Step};
use super::kind::{ModuleKind, Registry};
use super::mask::{
    AiMask, AiPoint, Blend, ColourRange, DepthRange, Details, MaskExpr, MaskShape, PathPoint,
    PressureUse, REFINE_RADIUS, Range, Refine, Shape, Stroke,
};
use super::migrate::{self, MigrationReport};
use super::style::{Preset, Style};

/// The current binary format version of every blob kind.
pub const FORMAT: u16 = 1;

const MAGIC_EDIT: [u8; 4] = *b"SPDE";
const MAGIC_HISTORY: [u8; 4] = *b"SPDH";
const MAGIC_STEP: [u8; 4] = *b"SPDS";
const MAGIC_STYLE: [u8; 4] = *b"SPDY";
const MAGIC_PRESET: [u8; 4] = *b"SPDP";

/// Mask trees deeper than this are refused (a damaged blob, not an edit).
const MAX_DEPTH: u32 = 64;

/// Why a blob did not decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    BadMagic,
    /// Written by a newer Specular; the number is its format version.
    NewerFormat(u16),
    BadTag(u8),
    BadUtf8,
    TooDeep,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("edit data ends early"),
            DecodeError::BadMagic => f.write_str("not Specular edit data"),
            DecodeError::NewerFormat(v) => {
                write!(f, "edit data format {v} is newer than this build")
            }
            DecodeError::BadTag(t) => write!(f, "unknown record tag {t}"),
            DecodeError::BadUtf8 => f.write_str("text is not UTF-8"),
            DecodeError::TooDeep => f.write_str("mask expression nests too deeply"),
        }
    }
}

impl std::error::Error for DecodeError {}

type R<T> = Result<T, DecodeError>;

// ---------------------------------------------------------------------------------------------
// Writer and reader

/// Appends the canonical little-endian form.
#[derive(Default)]
pub(crate) struct Writer {
    pub buf: Vec<u8>,
    /// Hashing: -0.0 is written as 0.0 and every NaN as one quiet NaN.
    pub canonical: bool,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn var(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
    }
    pub fn len(&mut self, n: usize) {
        self.var(n as u64);
    }
    pub fn f32(&mut self, v: f32) {
        let v = if self.canonical {
            if v.is_nan() {
                f32::NAN
            } else if v == 0.0 {
                0.0
            } else {
                v
            }
        } else {
            v
        };
        self.u32(v.to_bits());
    }
    pub fn f32s(&mut self, v: &[f32]) {
        for &x in v {
            self.f32(x);
        }
    }
    pub fn str(&mut self, s: &str) {
        self.len(s.len());
        self.buf.extend_from_slice(s.as_bytes());
    }
    pub fn opt_str(&mut self, s: Option<&str>) {
        self.bool(s.is_some());
        if let Some(s) = s {
            self.str(s);
        }
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    /// Writes `f`'s output behind its varint length.
    pub fn frame(&mut self, f: impl FnOnce(&mut Writer)) {
        let mut inner = Writer {
            buf: Vec::new(),
            canonical: self.canonical,
        };
        f(&mut inner);
        self.len(inner.buf.len());
        self.buf.extend_from_slice(&inner.buf);
    }
}

pub(crate) struct Reader<'a> {
    b: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b }
    }
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if n > self.b.len() {
            return Err(DecodeError::Truncated);
        }
        let (h, t) = self.b.split_at(n);
        self.b = t;
        Ok(h)
    }
    fn arr<const N: usize>(&mut self) -> R<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap_or([0; N]))
    }
    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> R<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    pub fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    pub fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
    pub fn i64(&mut self) -> R<i64> {
        Ok(i64::from_le_bytes(self.arr()?))
    }
    pub fn var(&mut self) -> R<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError::Truncated)
    }
    /// A count of items at least `min_size` bytes each, checked against what is left.
    pub fn count(&mut self, min_size: usize) -> R<usize> {
        let n = self.var()?;
        if n.saturating_mul(min_size.max(1) as u64) > self.b.len() as u64 {
            return Err(DecodeError::Truncated);
        }
        Ok(n as usize)
    }
    pub fn f32(&mut self) -> R<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f32n<const N: usize>(&mut self) -> R<[f32; N]> {
        let mut out = [0.0; N];
        for x in &mut out {
            *x = self.f32()?;
        }
        Ok(out)
    }
    pub fn str(&mut self) -> R<String> {
        let n = self.count(1)?;
        let b = self.take(n)?;
        String::from_utf8(b.to_vec()).map_err(|_| DecodeError::BadUtf8)
    }
    pub fn opt_str(&mut self) -> R<Option<String>> {
        Ok(if self.bool()? {
            Some(self.str()?)
        } else {
            None
        })
    }
    /// The next frame as its own reader; whatever it leaves unread is skipped.
    pub fn frame(&mut self) -> R<Reader<'a>> {
        let n = self.count(1)?;
        Ok(Reader::new(self.take(n)?))
    }
    fn header(&mut self, magic: [u8; 4]) -> R<()> {
        if self.arr::<4>()? != magic {
            return Err(DecodeError::BadMagic);
        }
        match self.u16()? {
            v if v > FORMAT => Err(DecodeError::NewerFormat(v)),
            _ => Ok(()),
        }
    }
}

fn header(w: &mut Writer, magic: [u8; 4]) {
    w.bytes(&magic);
    w.u16(FORMAT);
}

// ---------------------------------------------------------------------------------------------
// Values and fields

pub(crate) fn write_value(w: &mut Writer, v: &Value) {
    w.u8(v.tag());
    match v {
        Value::Bool(b) => w.bool(*b),
        // Zigzag, so small negatives stay short.
        Value::Int(i) => w.var(((*i << 1) ^ (*i >> 63)) as u64),
        Value::Float(f) => w.f32(*f),
        Value::Text(s) => w.str(s),
        Value::Floats(v) => {
            w.len(v.len());
            w.f32s(v);
        }
    }
}

fn read_value(r: &mut Reader) -> R<Value> {
    Ok(match r.u8()? {
        0 => Value::Bool(r.bool()?),
        1 => {
            let z = r.var()?;
            Value::Int((z >> 1) as i64 ^ -((z & 1) as i64))
        }
        2 => Value::Float(r.f32()?),
        3 => Value::Text(r.str()?),
        4 => {
            let n = r.count(4)?;
            Value::Floats((0..n).map(|_| r.f32()).collect::<R<_>>()?)
        }
        t => return Err(DecodeError::BadTag(t)),
    })
}

fn write_opt_value(w: &mut Writer, v: Option<&Value>) {
    w.bool(v.is_some());
    if let Some(v) = v {
        write_value(w, v);
    }
}

fn read_opt_value(r: &mut Reader) -> R<Option<Value>> {
    Ok(if r.bool()? {
        Some(read_value(r)?)
    } else {
        None
    })
}

pub(crate) fn write_fields(w: &mut Writer, f: &Fields) {
    w.len(f.0.len());
    for (n, v) in &f.0 {
        w.str(n);
        write_value(w, v);
    }
}

fn read_fields(r: &mut Reader) -> R<Fields> {
    let n = r.count(2)?;
    let mut f = Fields(Vec::with_capacity(n));
    for _ in 0..n {
        let name = r.str()?;
        f.0.push((name, read_value(r)?));
    }
    Ok(f)
}

/// A choice by name; an unknown name (from a newer build) reads as the default.
fn read_choice<C: Choice + Default>(r: &mut Reader) -> R<C> {
    Ok(C::from_name(&r.str()?).unwrap_or_default())
}

fn write_choice<C: Choice + PartialEq>(w: &mut Writer, c: C) {
    w.str(c.name());
}

// ---------------------------------------------------------------------------------------------
// Masks and blends

pub(crate) fn write_shape(w: &mut Writer, s: &MaskShape) {
    w.var(u64::from(s.id));
    match &s.shape {
        Shape::Brush { strokes } => {
            w.u8(0);
            w.len(strokes.len());
            for st in strokes {
                w.f32s(&[st.radius, st.hardness, st.flow]);
                w.bool(st.erase);
                w.len(st.points.len());
                for p in &st.points {
                    w.f32s(p);
                }
            }
            // What pressure changes, per stroke, only when a stroke isn't the
            // original default (older readers skip it at the end of the shape's frame).
            if strokes.iter().any(|s| s.pressure != PressureUse::Flow) {
                w.len(strokes.len());
                for st in strokes {
                    write_choice(w, st.pressure);
                }
            }
        }
        Shape::Ellipse {
            centre,
            radii,
            angle_deg,
            feather,
        } => {
            w.u8(1);
            w.f32s(centre);
            w.f32s(radii);
            w.f32s(&[*angle_deg, *feather]);
        }
        Shape::Polygon { points, closed } => {
            w.u8(2);
            w.len(points.len());
            for p in points {
                w.f32s(&[p.x, p.y, p.feather]);
            }
            w.bool(*closed);
        }
        Shape::Rectangle {
            centre,
            half,
            angle_deg,
            round,
            feather,
        } => {
            w.u8(5);
            w.f32s(centre);
            w.f32s(half);
            w.f32s(&[*angle_deg, *round, *feather]);
        }
        Shape::Linear { start, end } => {
            w.u8(3);
            w.f32s(start);
            w.f32s(end);
        }
        Shape::Radial {
            centre,
            inner,
            outer,
            angle_deg,
        } => {
            w.u8(4);
            w.f32s(centre);
            w.f32s(inner);
            w.f32s(outer);
            w.f32(*angle_deg);
        }
    }
}

fn read_shape(r: &mut Reader) -> R<MaskShape> {
    let id = r.var()? as u32;
    let shape = match r.u8()? {
        0 => {
            let n = r.count(14)?;
            let mut strokes = Vec::with_capacity(n);
            for _ in 0..n {
                let [radius, hardness, flow] = r.f32n()?;
                let erase = r.bool()?;
                let np = r.count(12)?;
                let points = (0..np).map(|_| r.f32n::<3>()).collect::<R<_>>()?;
                strokes.push(Stroke {
                    radius,
                    hardness,
                    flow,
                    erase,
                    pressure: PressureUse::Flow,
                    points,
                });
            }
            if !r.b.is_empty() {
                let n = r.count(1)?;
                for i in 0..n {
                    let p = read_choice(r)?;
                    if let Some(s) = strokes.get_mut(i) {
                        s.pressure = p;
                    }
                }
            }
            Shape::Brush { strokes }
        }
        1 => {
            let centre = r.f32n()?;
            let radii = r.f32n()?;
            let [angle_deg, feather] = r.f32n()?;
            Shape::Ellipse {
                centre,
                radii,
                angle_deg,
                feather,
            }
        }
        2 => {
            let n = r.count(12)?;
            let points = (0..n)
                .map(|_| {
                    r.f32n::<3>()
                        .map(|[x, y, feather]| PathPoint { x, y, feather })
                })
                .collect::<R<_>>()?;
            Shape::Polygon {
                points,
                closed: r.bool()?,
            }
        }
        3 => Shape::Linear {
            start: r.f32n()?,
            end: r.f32n()?,
        },
        4 => Shape::Radial {
            centre: r.f32n()?,
            inner: r.f32n()?,
            outer: r.f32n()?,
            angle_deg: r.f32()?,
        },
        5 => {
            let centre = r.f32n()?;
            let half = r.f32n()?;
            let [angle_deg, round, feather] = r.f32n()?;
            Shape::Rectangle {
                centre,
                half,
                angle_deg,
                round,
                feather,
            }
        }
        t => return Err(DecodeError::BadTag(t)),
    };
    Ok(MaskShape { id, shape })
}

/// Writes a mask expression; `resolve` lets the hasher replace references by content.
pub(crate) fn write_expr(
    w: &mut Writer,
    e: &MaskExpr,
    resolve: &mut dyn FnMut(&mut Writer, &MaskExpr) -> bool,
) {
    if resolve(w, e) {
        return;
    }
    match e {
        MaskExpr::All => w.u8(0),
        MaskExpr::Shape(id) => {
            w.u8(1);
            w.var(u64::from(*id));
        }
        MaskExpr::Range(Range {
            channel,
            on_output,
            limits,
        }) => {
            w.u8(2);
            write_choice(w, *channel);
            w.bool(*on_output);
            w.f32s(limits);
        }
        MaskExpr::ColourRange(c) => {
            w.u8(3);
            w.len(c.colours.len());
            for col in &c.colours {
                w.f32s(col);
            }
            w.f32s(&[c.distance, c.falloff]);
        }
        MaskExpr::FromModule { kind, instance } => {
            w.u8(4);
            w.u16(kind.0);
            w.u16(*instance);
        }
        MaskExpr::Ai(m) => {
            w.u8(10);
            write_choice(w, m.kind);
            w.str(&m.model);
            w.len(m.points.len());
            for p in &m.points {
                w.f32s(&[p.x, p.y]);
                w.bool(p.positive);
            }
            w.bool(m.rect.is_some());
            if let Some(r) = &m.rect {
                w.f32s(r);
            }
            w.bytes(&m.key);
        }
        MaskExpr::Invert(a) => {
            w.u8(6);
            write_expr(w, a, resolve);
        }
        MaskExpr::Combine(op, a, b) => {
            w.u8(7);
            write_choice(w, *op);
            write_expr(w, a, resolve);
            write_expr(w, b, resolve);
        }
        MaskExpr::Opacity(k, a) => {
            w.u8(8);
            w.f32(*k);
            write_expr(w, a, resolve);
        }
        MaskExpr::Details(d) => {
            w.u8(9);
            w.f32s(&[d.threshold, d.softness]);
        }
        MaskExpr::Depth(d) => {
            w.u8(11);
            w.str(&d.model);
            w.f32s(&[d.near, d.far, d.feather]);
        }
    }
}

fn read_expr(r: &mut Reader, depth: u32) -> R<MaskExpr> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::TooDeep);
    }
    Ok(match r.u8()? {
        0 => MaskExpr::All,
        1 => MaskExpr::Shape(r.var()? as u32),
        2 => MaskExpr::Range(Range {
            channel: read_choice(r)?,
            on_output: r.bool()?,
            limits: r.f32n()?,
        }),
        3 => {
            let n = r.count(12)?;
            let colours = (0..n).map(|_| r.f32n::<3>()).collect::<R<_>>()?;
            let [distance, falloff] = r.f32n()?;
            MaskExpr::ColourRange(ColourRange {
                colours,
                distance,
                falloff,
            })
        }
        4 => MaskExpr::FromModule {
            kind: ModuleKind(r.u16()?),
            instance: r.u16()?,
        },
        // An older placeholder (never written by a build that made AI masks): its label
        // names the kind when it can.
        5 => {
            let label = r.str()?;
            let key: [u8; 32] = r.arr()?;
            let kind = crate::model::fields::Choice::from_name(&label).unwrap_or_default();
            MaskExpr::Ai(AiMask {
                key,
                ..AiMask::new(kind, "")
            })
        }
        10 => {
            let kind = read_choice(r)?;
            let model = r.str()?;
            let n = r.count(9)?;
            let points = (0..n)
                .map(|_| {
                    let [x, y] = r.f32n()?;
                    Ok(AiPoint {
                        x,
                        y,
                        positive: r.bool()?,
                    })
                })
                .collect::<R<_>>()?;
            let rect = if r.bool()? { Some(r.f32n()?) } else { None };
            MaskExpr::Ai(AiMask {
                kind,
                model,
                points,
                rect,
                key: r.arr()?,
            })
        }
        6 => MaskExpr::Invert(Box::new(read_expr(r, depth + 1)?)),
        7 => {
            let op = read_choice(r)?;
            let a = read_expr(r, depth + 1)?;
            let b = read_expr(r, depth + 1)?;
            MaskExpr::Combine(op, Box::new(a), Box::new(b))
        }
        8 => {
            let k = r.f32()?;
            MaskExpr::Opacity(k, Box::new(read_expr(r, depth + 1)?))
        }
        9 => {
            let [threshold, softness] = r.f32n()?;
            MaskExpr::Details(Details {
                threshold,
                softness,
            })
        }
        11 => {
            let model = r.str()?;
            let [near, far, feather] = r.f32n()?;
            MaskExpr::Depth(DepthRange {
                model,
                near,
                far,
                feather,
            })
        }
        t => return Err(DecodeError::BadTag(t)),
    })
}

pub(crate) fn write_blend(
    w: &mut Writer,
    b: &Blend,
    resolve: &mut dyn FnMut(&mut Writer, &MaskExpr) -> bool,
) {
    write_choice(w, b.mode);
    w.f32(b.opacity);
    write_expr(w, &b.mask, resolve);
    let f = &b.refine;
    w.f32s(&[
        f.feather,
        f.blur,
        f.contrast,
        f.offset,
        f.edge_aware,
        f.details,
    ]);
    // Appended (invert, then refine radius) only when set: older readers skip
    // them at the end of the frame, and blends without them keep their bytes and hashes.
    let radius = f.radius != REFINE_RADIUS;
    if b.invert || radius {
        w.bool(b.invert);
    }
    if radius {
        w.f32(f.radius);
    }
}

fn read_blend(r: &mut Reader) -> R<Blend> {
    let mode = read_choice(r)?;
    let opacity = r.f32()?;
    let mask = read_expr(r, 0)?;
    let [feather, blur, contrast, offset, edge_aware, details] = r.f32n()?;
    let invert = !r.b.is_empty() && r.bool()?;
    let radius = if r.b.is_empty() {
        REFINE_RADIUS
    } else {
        r.f32()?
    };
    Ok(Blend {
        mode,
        opacity,
        mask,
        invert,
        refine: Refine {
            feather,
            blur,
            contrast,
            offset,
            edge_aware,
            details,
            radius,
        },
    })
}

fn no_resolve(_: &mut Writer, _: &MaskExpr) -> bool {
    false
}

fn write_opt_blend(w: &mut Writer, b: Option<&Blend>) {
    w.bool(b.is_some());
    if let Some(b) = b {
        w.frame(|w| write_blend(w, b, &mut no_resolve));
    }
}

fn read_opt_blend(r: &mut Reader) -> R<Option<Blend>> {
    Ok(if r.bool()? {
        Some(read_blend(&mut r.frame()?)?)
    } else {
        None
    })
}

// ---------------------------------------------------------------------------------------------
// Modules

fn write_module(w: &mut Writer, m: &ModuleInstance) {
    w.frame(|w| {
        w.u16(m.kind.0);
        w.u16(m.version);
        w.u16(m.instance);
        w.opt_str(m.label.as_deref());
        w.bool(m.enabled);
        write_fields(w, &m.params.to_fields());
        write_opt_blend(w, m.blend.as_ref());
    });
}

fn read_module(r: &mut Reader, reg: &Registry, report: &mut MigrationReport) -> R<ModuleInstance> {
    let mut r = r.frame()?;
    let kind = ModuleKind(r.u16()?);
    let mut version = r.u16()?;
    let instance = r.u16()?;
    let label = r.opt_str()?;
    let enabled = r.bool()?;
    let mut fields = read_fields(&mut r)?;
    let blend = read_opt_blend(&mut r)?;
    let params = migrate::on_load(reg, kind, instance, &mut version, &mut fields, report);
    Ok(ModuleInstance {
        kind,
        version,
        instance,
        label,
        enabled,
        params,
        blend,
    })
}

fn write_modules(w: &mut Writer, ms: &[ModuleInstance]) {
    w.len(ms.len());
    for m in ms {
        write_module(w, m);
    }
}

fn read_modules(
    r: &mut Reader,
    reg: &Registry,
    report: &mut MigrationReport,
) -> R<Vec<ModuleInstance>> {
    let n = r.count(1)?;
    (0..n).map(|_| read_module(r, reg, report)).collect()
}

fn write_masks(w: &mut Writer, ms: &[MaskShape]) {
    w.len(ms.len());
    for s in ms {
        w.frame(|w| write_shape(w, s));
    }
}

fn read_masks(r: &mut Reader) -> R<Vec<MaskShape>> {
    let n = r.count(1)?;
    (0..n).map(|_| read_shape(&mut r.frame()?)).collect()
}

pub(crate) fn write_source(w: &mut Writer, s: &SourceKey) {
    w.u64(s.size);
    w.i64(s.mtime_ns);
    w.bytes(&s.head_tail);
}

// ---------------------------------------------------------------------------------------------
// Blobs

/// An edit as the catalogue's `edits.edit` blob.
pub fn encode_edit(e: &Edit) -> Vec<u8> {
    let mut w = Writer::default();
    header(&mut w, MAGIC_EDIT);
    w.u16(e.process);
    write_source(&mut w, &e.source);
    w.bool(e.custom_order);
    write_modules(&mut w, &e.modules);
    write_masks(&mut w, &e.masks);
    w.len(e.snapshots.len());
    for s in &e.snapshots {
        w.frame(|w| {
            w.str(&s.name);
            write_modules(w, &s.modules);
            write_masks(w, &s.masks);
        });
    }
    w.buf
}

/// Decodes an edit, migrating module params older than their live kernels. The report says
/// what changed; when it is not empty, show its notice and start a fresh history.
pub fn decode_edit(b: &[u8], reg: &Registry) -> R<(Edit, MigrationReport)> {
    let mut r = Reader::new(b);
    r.header(MAGIC_EDIT)?;
    let mut report = MigrationReport::default();
    let process = r.u16()?;
    let source = SourceKey {
        size: r.u64()?,
        mtime_ns: r.i64()?,
        head_tail: r.arr()?,
    };
    let custom_order = r.bool()?;
    let modules = read_modules(&mut r, reg, &mut report)?;
    let masks = read_masks(&mut r)?;
    let n = r.count(1)?;
    let mut snapshots = Vec::with_capacity(n);
    for _ in 0..n {
        let mut f = r.frame()?;
        snapshots.push(Snapshot {
            name: f.str()?,
            modules: read_modules(&mut f, reg, &mut report)?,
            masks: read_masks(&mut f)?,
        });
    }
    Ok((
        Edit {
            schema: super::edit::SCHEMA,
            process,
            source,
            modules,
            custom_order,
            masks,
            snapshots,
        },
        report,
    ))
}

fn write_step_body(w: &mut Writer, s: &Step) {
    w.u16(s.kind.0);
    w.u16(s.instance);
    w.i64(s.time_ms);
    match &s.change {
        Change::Params(cs) => {
            w.u8(0);
            w.len(cs.len());
            for c in cs {
                w.str(&c.name);
                write_opt_value(w, c.before.as_ref());
                write_opt_value(w, c.after.as_ref());
            }
        }
        Change::Enabled(on) => {
            w.u8(1);
            w.bool(*on);
        }
        Change::Blend { before, after } => {
            w.u8(2);
            write_opt_blend(w, before.as_deref());
            write_opt_blend(w, after.as_deref());
        }
        Change::Label { before, after } => {
            w.u8(3);
            w.opt_str(before.as_deref());
            w.opt_str(after.as_deref());
        }
        Change::Replace { before, after } => {
            w.u8(4);
            write_module(w, before);
            write_module(w, after);
        }
        Change::Add { at, module } => {
            w.u8(5);
            w.u32(*at);
            write_module(w, module);
        }
        Change::Remove { at, module } => {
            w.u8(6);
            w.u32(*at);
            write_module(w, module);
        }
        Change::Move {
            from,
            to,
            custom_before,
        } => {
            w.u8(7);
            w.u32(*from);
            w.u32(*to);
            w.bool(*custom_before);
        }
        Change::Masks { before, after } => {
            w.u8(8);
            write_masks(w, before);
            write_masks(w, after);
        }
        Change::Stack {
            what,
            before,
            after,
        } => {
            w.u8(9);
            w.str(what);
            for s in [before, after] {
                w.frame(|w| {
                    w.bool(s.custom_order);
                    write_modules(w, &s.modules);
                    write_masks(w, &s.masks);
                });
            }
        }
    }
}

fn read_step_body(r: &mut Reader, reg: &Registry) -> R<Step> {
    // Modules inside steps migrate like the edit's; the report goes with the edit's load.
    let mut rep = MigrationReport::default();
    let kind = ModuleKind(r.u16()?);
    let instance = r.u16()?;
    let time_ms = r.i64()?;
    let change = match r.u8()? {
        0 => {
            let n = r.count(3)?;
            let mut cs = Vec::with_capacity(n);
            for _ in 0..n {
                cs.push(FieldChange {
                    name: r.str()?,
                    before: read_opt_value(r)?,
                    after: read_opt_value(r)?,
                });
            }
            Change::Params(cs)
        }
        1 => Change::Enabled(r.bool()?),
        2 => Change::Blend {
            before: read_opt_blend(r)?.map(Box::new),
            after: read_opt_blend(r)?.map(Box::new),
        },
        3 => Change::Label {
            before: r.opt_str()?,
            after: r.opt_str()?,
        },
        4 => Change::Replace {
            before: Box::new(read_module(r, reg, &mut rep)?),
            after: Box::new(read_module(r, reg, &mut rep)?),
        },
        5 => Change::Add {
            at: r.u32()?,
            module: Box::new(read_module(r, reg, &mut rep)?),
        },
        6 => Change::Remove {
            at: r.u32()?,
            module: Box::new(read_module(r, reg, &mut rep)?),
        },
        7 => Change::Move {
            from: r.u32()?,
            to: r.u32()?,
            custom_before: r.bool()?,
        },
        8 => Change::Masks {
            before: read_masks(r)?,
            after: read_masks(r)?,
        },
        9 => {
            let what = r.str()?;
            let mut stack = || -> R<Box<Stack>> {
                let mut f = r.frame()?;
                Ok(Box::new(Stack {
                    custom_order: f.bool()?,
                    modules: read_modules(&mut f, reg, &mut rep)?,
                    masks: read_masks(&mut f)?,
                }))
            };
            let before = stack()?;
            let after = stack()?;
            Change::Stack {
                what,
                before,
                after,
            }
        }
        t => return Err(DecodeError::BadTag(t)),
    };
    Ok(Step {
        kind,
        instance,
        change,
        time_ms,
    })
}

/// One step as the catalogue's `edit_steps.step` blob.
pub fn encode_step(s: &Step) -> Vec<u8> {
    let mut w = Writer::default();
    header(&mut w, MAGIC_STEP);
    write_step_body(&mut w, s);
    w.buf
}

pub fn decode_step(b: &[u8], reg: &Registry) -> R<Step> {
    let mut r = Reader::new(b);
    r.header(MAGIC_STEP)?;
    read_step_body(&mut r, reg)
}

/// A whole history (steps and position) in one blob.
pub fn encode_history(h: &History) -> Vec<u8> {
    let mut w = Writer::default();
    header(&mut w, MAGIC_HISTORY);
    w.len(h.position);
    w.len(h.steps.len());
    for s in &h.steps {
        w.frame(|w| write_step_body(w, s));
    }
    w.buf
}

pub fn decode_history(b: &[u8], reg: &Registry) -> R<History> {
    let mut r = Reader::new(b);
    r.header(MAGIC_HISTORY)?;
    let position = r.var()? as usize;
    let n = r.count(1)?;
    let steps = (0..n)
        .map(|_| read_step_body(&mut r.frame()?, reg))
        .collect::<R<Vec<_>>>()?;
    Ok(History {
        position: position.min(steps.len()),
        steps,
    })
}

/// A style as the catalogue's `styles.items` blob.
pub fn encode_style(s: &Style) -> Vec<u8> {
    let mut w = Writer::default();
    header(&mut w, MAGIC_STYLE);
    w.str(&s.name);
    write_modules(&mut w, &s.items);
    write_masks(&mut w, &s.masks);
    w.buf
}

pub fn decode_style(b: &[u8], reg: &Registry) -> R<(Style, MigrationReport)> {
    let mut r = Reader::new(b);
    r.header(MAGIC_STYLE)?;
    let mut report = MigrationReport::default();
    let name = r.str()?;
    let items = read_modules(&mut r, reg, &mut report)?;
    let masks = read_masks(&mut r)?;
    Ok((Style { name, items, masks }, report))
}

/// A preset as the catalogue's `presets.params` blob.
pub fn encode_preset(p: &Preset) -> Vec<u8> {
    let mut w = Writer::default();
    header(&mut w, MAGIC_PRESET);
    w.u16(p.kind.0);
    w.str(&p.name);
    w.u16(p.version);
    write_fields(&mut w, &p.fields);
    w.buf
}

/// Decodes a preset; its fields migrate to the module's current version.
pub fn decode_preset(b: &[u8], reg: &Registry) -> R<Preset> {
    let mut r = Reader::new(b);
    r.header(MAGIC_PRESET)?;
    let kind = ModuleKind(r.u16()?);
    let name = r.str()?;
    let mut version = r.u16()?;
    let mut fields = read_fields(&mut r)?;
    if let Some(spec) = reg.get(kind)
        && migrate::run(spec, version, spec.current, &mut fields).is_ok()
    {
        version = spec.current;
    }
    Ok(Preset {
        kind,
        name,
        version,
        fields,
    })
}
