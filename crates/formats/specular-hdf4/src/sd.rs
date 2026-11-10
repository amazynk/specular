//! Scientific datasets (the SD interface): `Var0.0` Vgroups and old DFSD numeric data groups.

use std::sync::OnceLock;

use crate::dd::{
    TAG_CAL, TAG_FV, TAG_NDG, TAG_NT, TAG_SD, TAG_SDC, TAG_SDD, TAG_SDF, TAG_SDG, TAG_SDL, TAG_SDM,
    TAG_SDU,
};
use crate::elem::{self, Store};
use crate::nt::Nt;
use crate::vobj::{attr_of, value_of};
use crate::{Attr, Ctx, Error, File, Plane, PlaneSpec, Result, Value, bad, be16, bei32, limits};

/// A dimension of a dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dim {
    pub name: String,
    /// The record (unlimited) dimension.
    pub unlimited: bool,
}

/// A scientific dataset.
#[derive(Clone, Debug)]
pub struct Sds {
    pub name: String,
    pub nt: Nt,
    pub shape: Vec<u64>,
    pub dims: Vec<Dim>,
    pub attrs: Vec<Attr>,
    /// A coordinate variable (one dimension, named as it).
    pub coord: bool,
    /// The numeric data group's reference.
    pub ndg: u16,
    /// The data element's reference (`None`: never written, all fill).
    pub data_ref: Option<u16>,
    /// The fill of unwritten parts, as a stored number.
    pub fill: f64,
    store: OnceLock<std::result::Result<Store, Error>>,
}

impl Sds {
    /// An empty one-dimensional float dataset with dimension `dim`, for tests.
    #[cfg(test)]
    pub(crate) fn named(name: &str, dim: &str) -> Sds {
        Sds {
            name: name.into(),
            nt: Nt::from_code(5).expect("float32"),
            shape: vec![1],
            dims: vec![Dim {
                name: dim.into(),
                unlimited: false,
            }],
            attrs: Vec::new(),
            coord: false,
            ndg: 0,
            data_ref: None,
            fill: 0.0,
            store: OnceLock::new(),
        }
    }

    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    fn num_attr(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(Value::as_f64)
    }

    /// A text attribute (`long_name`, `units`, …), trimmed; `None` when absent or empty.
    pub fn text_attr(&self, name: &str) -> Option<String> {
        let s = self.attr(name)?.text()?;
        let s = s.trim().to_owned();
        (!s.is_empty()).then_some(s)
    }

    pub fn len(&self) -> u64 {
        self.shape.iter().fold(1, |a, &d| a.saturating_mul(d))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn store(&self, f: &File, ctx: Ctx) -> Result<&Store> {
        self.store
            .get_or_init(|| match self.data_ref {
                None => Ok(Store::Missing),
                Some(r) => elem::store(f, ctx, TAG_SD, r),
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// How the data is stored: "contiguous", "deflate (level 6), chunks 16 × 16", …
    pub fn storage(&self, f: &File, ctx: Ctx) -> String {
        match self.store(f, ctx) {
            Ok(s) => s.describe(),
            Err(e) => e.to_string(),
        }
    }

    /// The compression's name, when compressed.
    pub fn compression(&self, f: &File, ctx: Ctx) -> Option<&'static str> {
        self.store(f, ctx).ok()?.coder().map(|c| c.name())
    }

    /// How stored numbers become physical values.
    pub fn packing(&self) -> Packing {
        let scale = self.num_attr("scale_factor");
        let offset = self.num_attr("add_offset");
        // `SDsetcal` writes `scale_factor_err` and `calibrated_nt` along with the scale and
        // offset, and documents value = scale · (stored − offset).
        let hdf = self.attr("calibrated_nt").is_some() || self.attr("scale_factor_err").is_some();
        let fill = self.num_attr("_FillValue");
        let range = match self.attr("valid_range").and_then(Value::as_f64s).as_deref() {
            Some([a, b, ..]) => Some((a.min(*b), a.max(*b))),
            _ => match (self.num_attr("valid_min"), self.num_attr("valid_max")) {
                (None, None) => None,
                (a, b) => Some((a.unwrap_or(f64::NEG_INFINITY), b.unwrap_or(f64::INFINITY))),
            },
        };
        Packing {
            scale: scale.unwrap_or(1.0),
            offset: offset.unwrap_or(0.0),
            scaled: scale.is_some() || offset.is_some(),
            hdf_style: hdf,
            fill: fill.or_else(|| self.nt.default_float_fill()),
            missing: self.num_attr("missing_value"),
            valid: range,
        }
    }
}

/// Stored numbers to physical values, with the no-data masks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Packing {
    pub scale: f64,
    pub offset: f64,
    /// A scale or an offset is given.
    pub scaled: bool,
    /// value = scale · (stored − offset) (HDF4 calibration); otherwise stored · scale + offset
    /// (the CF convention).
    pub hdf_style: bool,
    /// Stored values that mean no data.
    pub fill: Option<f64>,
    pub missing: Option<f64>,
    /// Stored values outside it mean no data.
    pub valid: Option<(f64, f64)>,
}

impl Packing {
    /// The physical value of a stored number, NaN for no data.
    #[inline]
    pub fn apply(&self, v: f64) -> f64 {
        if v.is_nan()
            || self.fill == Some(v)
            || self.missing == Some(v)
            || self.valid.is_some_and(|(a, b)| v < a || v > b)
        {
            return f64::NAN;
        }
        if !self.scaled {
            v
        } else if self.hdf_style {
            self.scale * (v - self.offset)
        } else {
            v * self.scale + self.offset
        }
    }

    /// A short description: "value = 0.02 × (stored − 0)".
    pub fn describe(&self) -> Option<String> {
        if !self.scaled {
            return None;
        }
        Some(if self.hdf_style {
            format!("{} × (stored − {})", self.scale, self.offset)
        } else {
            format!("stored × {} + {}", self.scale, self.offset)
        })
    }
}

fn parse_sdd(b: &[u8]) -> Result<(Vec<u64>, u16)> {
    let rank = usize::from(be16(b, 0).ok_or_else(|| bad("dimension record"))?);
    if rank > limits::MAX_RANK {
        return Err(bad("rank"));
    }
    let mut shape = Vec::with_capacity(rank);
    for k in 0..rank {
        let d = bei32(b, 2 + 4 * k).ok_or_else(|| bad("dimension record"))?;
        shape.push(d.max(0) as u64);
    }
    let nt_ref = be16(b, 2 + 4 * rank + 2).ok_or_else(|| bad("dimension record"))?;
    Ok((shape, nt_ref))
}

fn nt_of(f: &File, ctx: Ctx, rf: u16) -> Result<Nt> {
    let b = f
        .dds
        .raw(ctx.data, TAG_NT, rf)
        .ok_or_else(|| bad("number type"))?;
    Nt::from_element(b)
}

/// The members of a numeric data group.
fn ndg_members(f: &File, ctx: Ctx, tag: u16, rf: u16) -> Vec<(u16, u16)> {
    let Some(b) = f.dds.raw(ctx.data, tag, rf) else {
        return Vec::new();
    };
    b.chunks_exact(4)
        .map(|c| {
            (
                u16::from_be_bytes([c[0], c[1]]),
                u16::from_be_bytes([c[2], c[3]]),
            )
        })
        .collect()
}

pub(crate) fn scan(f: &mut File, ctx: Ctx) -> Result<()> {
    let mut out = Vec::new();
    let mut used_ndg = std::collections::HashSet::new();
    for vg in f.vgroups.iter().filter(|v| v.class == "Var0.0") {
        if out.len() >= limits::MAX_ITEMS {
            break;
        }
        match var(f, ctx, vg) {
            Ok(Some(s)) => {
                used_ndg.insert(s.ndg);
                out.push(s);
            }
            Ok(None) => {}
            Err(e) => f.warnings.push(format!("dataset {}: {e}", vg.name)),
        }
    }
    // Old DFSD datasets: numeric data groups no Var0.0 Vgroup describes.
    let mut k = 0;
    for tag in [TAG_NDG, TAG_SDG] {
        let refs: Vec<u16> = f.dds.refs(tag).collect();
        for rf in refs {
            if used_ndg.contains(&rf) || out.len() >= limits::MAX_ITEMS {
                continue;
            }
            used_ndg.insert(rf);
            match dfsd(f, ctx, tag, rf, k) {
                Ok(Some(s)) => {
                    out.push(s);
                    k += 1;
                }
                Ok(None) => {}
                Err(e) => f.warnings.push(format!("data group {rf}: {e}")),
            }
        }
    }
    // The SD interface's file attributes.
    if let Some(cdf) = f.vgroups.iter().find(|v| v.class == "CDF0.0") {
        let attrs = cdf
            .child_vdatas()
            .filter_map(|r| f.vdata(r))
            .filter(|v| v.class == "Attr0.0")
            .take(limits::MAX_ATTRS)
            .filter_map(|v| attr_of(f, ctx, v, false))
            .collect();
        f.attrs = attrs;
    }
    f.sds = out;
    Ok(())
}

fn var(f: &File, ctx: Ctx, vg: &crate::Vgroup) -> Result<Option<Sds>> {
    let ndg = vg.member(TAG_NDG);
    let ndg_m = ndg
        .map(|r| ndg_members(f, ctx, TAG_NDG, r))
        .unwrap_or_default();
    let find = |tag: u16| {
        vg.member(tag)
            .or_else(|| ndg_m.iter().find(|m| m.0 == tag).map(|m| m.1))
    };
    let Some(sdd_ref) = find(TAG_SDD) else {
        return Ok(None);
    };
    let sdd = f
        .dds
        .raw(ctx.data, TAG_SDD, sdd_ref)
        .ok_or_else(|| bad("dimension record"))?;
    let (mut shape, sdd_nt) = parse_sdd(sdd)?;
    let nt = nt_of(f, ctx, find(TAG_NT).unwrap_or(sdd_nt))?;
    let mut dims = Vec::new();
    for r in vg.child_groups() {
        if let Some(d) = f.vgroup(r)
            && (d.class == "Dim0.0" || d.class == "UDim0.0")
        {
            dims.push(Dim {
                name: d.name.clone(),
                unlimited: d.class == "UDim0.0",
            });
        }
    }
    let data_ref = find(TAG_SD).filter(|&r| f.dds.has(TAG_SD, r));
    // A record dataset has as many records as its data holds: the dimension record may
    // predate records appended later, and other datasets' records do not count.
    if dims.len() == shape.len()
        && dims.first().is_some_and(|d| d.unlimited)
        && let Some(r) = data_ref
        && let Some(n) = record_count(f, ctx, r, nt.size(), &shape)
    {
        shape[0] = n;
    }
    if dims.len() != shape.len() {
        dims = (0..shape.len())
            .map(|k| Dim {
                name: format!("fakeDim{k}"),
                unlimited: false,
            })
            .collect();
    }
    let attrs: Vec<Attr> = vg
        .child_vdatas()
        .filter_map(|r| f.vdata(r))
        .filter(|v| v.class == "Attr0.0")
        .take(limits::MAX_ATTRS)
        .filter_map(|v| attr_of(f, ctx, v, false))
        .collect();
    let coord = shape.len() == 1 && dims[0].name == vg.name;
    let fill = attrs
        .iter()
        .find(|a| a.name == "_FillValue")
        .and_then(|a| a.value.as_f64())
        .unwrap_or_else(|| nt.get(&nt.default_fill()));
    Ok(Some(Sds {
        name: vg.name.clone(),
        nt,
        shape,
        dims,
        attrs,
        coord,
        ndg: ndg.unwrap_or(0),
        data_ref,
        fill,
        store: OnceLock::new(),
    }))
}

/// The whole records a record dataset's data element holds: its logical length over the bytes of one
/// record, or a chunked element's first dimension.
fn record_count(f: &File, ctx: Ctx, data_ref: u16, nt_size: usize, shape: &[u64]) -> Option<u64> {
    let record: u64 = shape[1..]
        .iter()
        .try_fold(nt_size as u64, |a, &d| a.checked_mul(d))?;
    if record == 0 {
        return None;
    }
    let len = match elem::store(f, ctx, TAG_SD, data_ref).ok()? {
        elem::Store::Plain { length, .. } => u64::from(length),
        elem::Store::Linked { length, .. }
        | elem::Store::External { length, .. }
        | elem::Store::Compressed { length, .. } => length,
        elem::Store::Chunked(c) => return c.dims.first().copied(),
        elem::Store::Missing => return None,
    };
    Some(len / record)
}

fn dfsd(f: &File, ctx: Ctx, tag: u16, rf: u16, k: usize) -> Result<Option<Sds>> {
    let m = ndg_members(f, ctx, tag, rf);
    let Some(sdd_ref) = m.iter().find(|m| m.0 == TAG_SDD).map(|m| m.1) else {
        return Ok(None);
    };
    let sdd = f
        .dds
        .raw(ctx.data, TAG_SDD, sdd_ref)
        .ok_or_else(|| bad("dimension record"))?;
    let (shape, sdd_nt) = parse_sdd(sdd)?;
    let nt = match nt_of(f, ctx, sdd_nt) {
        Ok(n) => n,
        // DFSD's default number type.
        Err(_) => Nt::from_code(5)?,
    };
    let dims = (0..shape.len())
        .map(|j| Dim {
            name: format!("fakeDim{j}"),
            unlimited: false,
        })
        .collect();
    let mut attrs = Vec::new();
    let mut fill = None;
    if let Some(r) = m.iter().find(|m| m.0 == TAG_FV).map(|m| m.1)
        && let Some(b) = f.dds.raw(ctx.data, TAG_FV, r)
        && b.len() >= nt.size()
    {
        let v = value_of(nt, &b[..nt.size()]);
        fill = v.as_f64();
        attrs.push(Attr {
            name: "_FillValue".into(),
            value: v,
        });
    }
    dfsd_attrs(f, ctx, &m, nt, &mut attrs);
    let data_ref = m
        .iter()
        .find(|m| m.0 == TAG_SD)
        .map(|m| m.1)
        .filter(|&r| f.dds.has(TAG_SD, r));
    Ok(Some(Sds {
        name: format!("Data-Set-{}", k + 2),
        nt,
        shape,
        dims,
        attrs,
        coord: false,
        ndg: rf,
        data_ref,
        fill: fill.unwrap_or_else(|| nt.get(&nt.default_fill())),
        store: OnceLock::new(),
    }))
}

/// The attributes the SD interface makes of a DFSD data group's other members, under the
/// same names: the data's label, unit, format and coordinate system (the first of each
/// element's NUL-terminated strings; the rest describe the dimensions), its maximum and
/// minimum (`valid_max`, `valid_min`) and its calibration (`scale_factor`, `scale_factor_err`,
/// `add_offset`, `add_offset_err`, `calibrated_nt`; 4 float64 and an int32, or in files from
/// before HDF 3.3 4 float32 and an int16).
fn dfsd_attrs(f: &File, ctx: Ctx, m: &[(u16, u16)], nt: Nt, attrs: &mut Vec<Attr>) {
    let raw = |tag: u16| {
        m.iter()
            .find(|m| m.0 == tag)
            .and_then(|m| f.dds.raw(ctx.data, tag, m.1))
    };
    for (tag, name) in [
        (TAG_SDL, "long_name"),
        (TAG_SDU, "units"),
        (TAG_SDF, "format"),
        (TAG_SDC, "cordsys"),
    ] {
        if let Some(b) = raw(tag) {
            let first = b.split(|&c| c == 0).next().unwrap_or_default();
            if !first.is_empty() {
                attrs.push(Attr {
                    name: name.into(),
                    value: Value::Str(vec![crate::clean_text(first)]),
                });
            }
        }
    }
    let n = nt.size();
    if let Some(b) = raw(TAG_SDM)
        && n > 0
        && b.len() >= 2 * n
    {
        for (k, name) in ["valid_max", "valid_min"].into_iter().enumerate() {
            attrs.push(Attr {
                name: name.into(),
                value: value_of(nt, &b[k * n..(k + 1) * n]),
            });
        }
    }
    if let Some(b) = raw(TAG_CAL) {
        let (cal, kind): (Vec<f64>, i64) = if b.len() >= 36 {
            (
                b[..32]
                    .chunks_exact(8)
                    .map(|c| f64::from_be_bytes(c.try_into().unwrap_or([0; 8])))
                    .collect(),
                i64::from(bei32(b, 32).unwrap_or(0)),
            )
        } else if b.len() >= 18 {
            (
                b[..16]
                    .chunks_exact(4)
                    .map(|c| f64::from(f32::from_be_bytes(c.try_into().unwrap_or([0; 4]))))
                    .collect(),
                i64::from(be16(b, 16).unwrap_or(0) as i16),
            )
        } else {
            return;
        };
        let names = [
            "scale_factor",
            "scale_factor_err",
            "add_offset",
            "add_offset_err",
        ];
        for (name, v) in names.into_iter().zip(cal) {
            attrs.push(Attr {
                name: name.into(),
                value: Value::Float(vec![v]),
            });
        }
        attrs.push(Attr {
            name: "calibrated_nt".into(),
            value: Value::Int(vec![kind]),
        });
    }
}

/// Calls `f` for every position of an odometer over `count` (row-major), with the position.
fn odometer(count: &[u64], mut f: impl FnMut(&[u64])) {
    if count.contains(&0) {
        return;
    }
    let mut pos = vec![0u64; count.len()];
    loop {
        f(&pos);
        let mut k = count.len();
        loop {
            if k == 0 {
                return;
            }
            k -= 1;
            pos[k] += 1;
            if pos[k] < count[k] {
                break;
            }
            pos[k] = 0;
        }
    }
}

/// A box of stored values, row-major over the box.
pub(crate) fn read_box(
    f: &File,
    ctx: Ctx,
    sds: &Sds,
    start: &[u64],
    count: &[u64],
) -> Result<Vec<f64>> {
    let rank = sds.shape.len();
    if start.len() != rank || count.len() != rank {
        return Err(bad("box rank"));
    }
    for k in 0..rank {
        if start[k] + count[k] > sds.shape[k] {
            return Err(bad("box outside the dataset"));
        }
    }
    let n = count
        .iter()
        .try_fold(1u64, |a, &d| a.checked_mul(d))
        .filter(|&n| n <= limits::MAX_ELEMENT / 8)
        .ok_or(Error::TooLarge)?;
    let _res = specular_core::budget::reserve_n(n, 8)?;
    let mut out = vec![sds.fill; n as usize];
    if n == 0 || rank == 0 {
        return Ok(out);
    }
    let nt = sds.nt;
    let size = nt.size();
    // Output strides.
    let mut ostride = vec![1u64; rank];
    for k in (0..rank - 1).rev() {
        ostride[k] = ostride[k + 1] * count[k + 1];
    }
    match sds.store(f, ctx)? {
        Store::Missing => {}
        Store::Chunked(c) => {
            if c.dims.len() != rank {
                return Err(bad("chunk rank"));
            }
            let fill_v = if c.fill.len() == size {
                nt.get(&c.fill)
            } else {
                sds.fill
            };
            let lo: Vec<u64> = (0..rank).map(|k| start[k] / c.chunk[k]).collect();
            let hi: Vec<u64> = (0..rank)
                .map(|k| (start[k] + count[k] - 1) / c.chunk[k])
                .collect();
            let span: Vec<u64> = (0..rank).map(|k| hi[k] - lo[k] + 1).collect();
            let mut cstride = vec![1u64; rank];
            for k in (0..rank - 1).rev() {
                cstride[k] = cstride[k + 1].saturating_mul(c.chunk[k + 1]);
            }
            let mut err = None;
            odometer(&span, |g| {
                if err.is_some() {
                    return;
                }
                let pos: Vec<u64> = (0..rank).map(|k| lo[k] + g[k]).collect();
                let bytes = match elem::chunk(f, ctx, c, (TAG_SD, sds.data_ref.unwrap_or(0)), &pos)
                {
                    Ok(b) => b,
                    Err(e) => {
                        err = Some(e);
                        return;
                    }
                };
                // The intersection of the chunk and the box, in dataset coordinates.
                let c0: Vec<u64> = (0..rank)
                    .map(|k| (pos[k] * c.chunk[k]).max(start[k]))
                    .collect();
                let c1: Vec<u64> = (0..rank)
                    .map(|k| ((pos[k] + 1) * c.chunk[k]).min(start[k] + count[k]))
                    .collect();
                let ext: Vec<u64> = (0..rank).map(|k| c1[k] - c0[k]).collect();
                odometer(&ext, |p| {
                    let mut o = 0u64;
                    let mut i = 0u64;
                    for k in 0..rank {
                        let d = c0[k] + p[k];
                        o += (d - start[k]) * ostride[k];
                        i = i.saturating_add((d - pos[k] * c.chunk[k]).saturating_mul(cstride[k]));
                    }
                    out[o as usize] = match &bytes {
                        Some(b) => {
                            let s = (i as usize).saturating_mul(size);
                            b.get(s..s.saturating_add(size))
                                .map_or(fill_v, |x| nt.get(x))
                        }
                        None => fill_v,
                    };
                });
            });
            if let Some(e) = err {
                return Err(e);
            }
        }
        st => {
            let bytes = elem::read(f, ctx, st, (TAG_SD, sds.data_ref.unwrap_or(0)))?;
            let mut stride = vec![1u64; rank];
            for k in (0..rank - 1).rev() {
                stride[k] = stride[k + 1].saturating_mul(sds.shape[k + 1]);
            }
            let last = rank - 1;
            let mut outer = count.to_vec();
            outer[last] = 1;
            odometer(&outer, |p| {
                let mut i = 0u64;
                let mut o = 0u64;
                for k in 0..rank {
                    i = i.saturating_add((start[k] + p[k]).saturating_mul(stride[k]));
                    o += p[k] * ostride[k];
                }
                for j in 0..count[last] {
                    let s = (i.saturating_add(j) as usize).saturating_mul(size);
                    if let Some(x) = bytes.get(s..s.saturating_add(size)) {
                        out[(o + j) as usize] = nt.get(x);
                    }
                }
            });
        }
    }
    Ok(out)
}

pub(crate) fn read_all(f: &File, ctx: Ctx, sds: &Sds, max: u64) -> Result<Vec<f64>> {
    if sds.len() > max {
        return Err(Error::TooLarge);
    }
    let start = vec![0; sds.shape.len()];
    read_box(f, ctx, sds, &start, &sds.shape)
}

pub(crate) fn read_plane(f: &File, ctx: Ctx, sds: &Sds, spec: &PlaneSpec) -> Result<Plane<f64>> {
    let rank = sds.shape.len();
    if spec.x_dim >= rank || spec.y_dim >= rank || spec.x_dim == spec.y_dim && rank > 1 {
        return Err(bad("plane dimensions"));
    }
    let mut start = vec![0u64; rank];
    let mut count = vec![1u64; rank];
    for k in 0..rank {
        if k == spec.x_dim || k == spec.y_dim {
            count[k] = sds.shape[k];
        } else {
            start[k] = spec
                .index
                .get(k)
                .copied()
                .unwrap_or(0)
                .min(sds.shape[k].saturating_sub(1));
        }
    }
    let w = sds.shape[spec.x_dim] as usize;
    let h = if rank > 1 {
        sds.shape[spec.y_dim] as usize
    } else {
        1
    };
    let v = read_box(f, ctx, sds, &start, &count)?;
    let values = if rank > 1 && spec.x_dim < spec.y_dim {
        // Stored with x varying slowest: transpose.
        let mut t = vec![0.0; v.len()];
        for x in 0..w {
            for y in 0..h {
                t[y * w + x] = v[x * h + y];
            }
        }
        t
    } else {
        v
    };
    Ok(Plane {
        width: w,
        height: h,
        values,
        truncated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_variants() {
        let p = Packing {
            scale: 0.02,
            offset: 100.0,
            scaled: true,
            hdf_style: true,
            fill: Some(0.0),
            missing: None,
            valid: Some((7500.0, 65535.0)),
        };
        assert_eq!(p.apply(15000.0), 0.02 * (15000.0 - 100.0));
        assert!(p.apply(0.0).is_nan());
        assert!(p.apply(7000.0).is_nan());
        let cf = Packing {
            hdf_style: false,
            ..p
        };
        assert_eq!(cf.apply(15000.0), 15000.0 * 0.02 + 100.0);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    fn sds(shape: Vec<u64>) -> Sds {
        Sds {
            name: String::new(),
            nt: Nt::from_code(21).unwrap(),
            dims: Vec::new(),
            shape,
            attrs: Vec::new(),
            coord: false,
            ndg: 0,
            data_ref: Some(1),
            fill: 0.0,
            store: OnceLock::new(),
        }
    }

    const M: u64 = i32::MAX as u64;

    #[test]
    fn dataset_length_overflow() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let s = sds(vec![M; 3]);
        assert_eq!(s.len(), u64::MAX);
        assert!(matches!(
            read_all(&f, Ctx::new(&d), &s, 1 << 20),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn plane_of_two_huge_dimensions() {
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let spec = PlaneSpec {
            x_dim: 1,
            y_dim: 0,
            index: vec![0; 2],
        };
        assert!(matches!(
            read_plane(&f, Ctx::new(&d), &sds(vec![M; 2]), &spec),
            Err(Error::TooLarge)
        ));
    }

    #[test]
    fn plane_with_overflowing_strides() {
        // A 1 × 1 plane of a dataset whose outer strides overflow.
        let d = crate::hostile::tiny();
        let f = File::open(&d).unwrap();
        let spec = PlaneSpec {
            x_dim: 5,
            y_dim: 4,
            index: vec![1, 0, 0, 0, 0, 0],
        };
        let s = sds(vec![2, M, M, M, 1, 1]);
        let p = read_plane(&f, Ctx::new(&d), &s, &spec).unwrap();
        assert_eq!(p.values, [0.0]);
    }
}
