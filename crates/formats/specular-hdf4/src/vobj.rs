//! Vgroups (groupings of elements) and Vdatas (tables of records).

use crate::dd::{TAG_VG, TAG_VH, TAG_VS};
use crate::nt::Nt;
use crate::{Attr, Ctx, Error, File, Result, Value, bad, be16, bei32, limits};

/// A Vgroup: a named, classed list of (tag, ref) members.
#[derive(Clone, Debug)]
pub struct Vgroup {
    pub rf: u16,
    pub name: String,
    pub class: String,
    pub members: Vec<(u16, u16)>,
    /// Attribute Vdatas (version 4 Vgroups).
    pub attrs: Vec<(u16, u16)>,
}

impl Vgroup {
    /// Member Vgroups' references.
    pub fn child_groups(&self) -> impl Iterator<Item = u16> + '_ {
        self.members.iter().filter(|m| m.0 == TAG_VG).map(|m| m.1)
    }
    /// Member Vdatas' references.
    pub fn child_vdatas(&self) -> impl Iterator<Item = u16> + '_ {
        self.members.iter().filter(|m| m.0 == TAG_VH).map(|m| m.1)
    }
    /// The reference of the first member with this tag.
    pub fn member(&self, tag: u16) -> Option<u16> {
        self.members.iter().find(|m| m.0 == tag).map(|m| m.1)
    }
}

/// One field of a Vdata.
#[derive(Clone, Debug)]
pub struct VdataField {
    pub name: String,
    /// `None` for a number type this reader does not know.
    pub nt: Option<Nt>,
    /// Numbers per record.
    pub order: usize,
    /// Bytes per record in the file.
    pub size: usize,
}

/// A Vdata: records of fields.
#[derive(Clone, Debug)]
pub struct Vdata {
    pub rf: u16,
    pub name: String,
    pub class: String,
    /// 0: record after record; 1: field after field.
    pub interlace: u16,
    pub nrecords: u32,
    pub fields: Vec<VdataField>,
    /// Attribute Vdatas: (field index or −1 for the whole Vdata, tag, ref).
    pub attrs: Vec<(i32, u16, u16)>,
}

impl Vdata {
    pub fn record_size(&self) -> usize {
        self.fields.iter().map(|f| f.size).sum()
    }

    /// The byte offset of a field within a record (interlace 0), or the start of the
    /// field's column (interlace 1).
    pub fn field_offset(&self, k: usize) -> usize {
        let before: usize = self.fields[..k].iter().map(|f| f.size).sum();
        if self.interlace == 1 {
            before * self.nrecords as usize
        } else {
            before
        }
    }

    /// Every number of field `k` (record after record), from the Vdata's bytes.
    pub fn field_values(&self, bytes: &[u8], k: usize) -> Option<Vec<f64>> {
        let f = self.fields.get(k)?;
        let nt = f.nt?;
        let rec = self.record_size();
        // Every number comes from `bytes`.
        let mut out =
            Vec::with_capacity((self.nrecords as usize * f.order).min(bytes.len() / nt.size()));
        for r in 0..self.nrecords as usize {
            let base = if self.interlace == 1 {
                self.field_offset(k) + r * f.size
            } else {
                r * rec + self.field_offset(k)
            };
            for j in 0..f.order {
                let s = base + j * nt.size();
                out.push(nt.get(bytes.get(s..s + nt.size())?));
            }
        }
        Some(out)
    }
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl Rd<'_> {
    fn u16(&mut self) -> Result<u16> {
        let v = be16(self.b, self.p).ok_or_else(|| bad("truncated Vgroup or Vdata"))?;
        self.p += 2;
        Ok(v)
    }
    fn i32(&mut self) -> Result<i32> {
        let v = bei32(self.b, self.p).ok_or_else(|| bad("truncated Vgroup or Vdata"))?;
        self.p += 4;
        Ok(v)
    }
    fn str(&mut self) -> Result<String> {
        let n = usize::from(self.u16()?);
        let s = self
            .b
            .get(self.p..self.p + n)
            .ok_or_else(|| bad("truncated name"))?;
        self.p += n;
        Ok(name_text(s))
    }
    fn peek16(&self) -> Option<u16> {
        be16(self.b, self.p)
    }
}

/// A name: Latin-1 up to the first NUL.
pub(crate) fn name_text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end].iter().map(|&c| c as char).collect()
}

fn parse_vgroup(rf: u16, b: &[u8]) -> Result<Vgroup> {
    let mut r = Rd { b, p: 0 };
    let n = usize::from(r.u16()?);
    if n > limits::MAX_VG_MEMBERS {
        return Err(Error::TooLarge);
    }
    let tags: Vec<u16> = (0..n).map(|_| r.u16()).collect::<Result<_>>()?;
    let refs: Vec<u16> = (0..n).map(|_| r.u16()).collect::<Result<_>>()?;
    let name = r.str()?;
    let class = r.str()?;
    let _extag = r.u16()?;
    let _exref = r.u16()?;
    let mut attrs = Vec::new();
    // Version 4 inserts flags (and attributes) before the trailing version.
    if matches!(r.peek16(), Some(v) if v != 2 && v != 3) {
        let flags = r.i32()?;
        if flags & 1 != 0 {
            let na = r.i32()?.max(0) as usize;
            for _ in 0..na.min(limits::MAX_ATTRS) {
                let t = r.u16()?;
                let a = r.u16()?;
                attrs.push((t, a));
            }
        }
    }
    Ok(Vgroup {
        rf,
        name,
        class,
        members: tags.into_iter().zip(refs).collect(),
        attrs,
    })
}

fn parse_vdata(rf: u16, b: &[u8]) -> Result<Vdata> {
    let mut r = Rd { b, p: 0 };
    let interlace = r.u16()?;
    let nrec = r.i32()?;
    let _ivsize = r.u16()?;
    let nf = usize::from(r.u16()?);
    if nf > limits::MAX_FIELDS || nrec < 0 {
        return Err(bad("Vdata header"));
    }
    let types: Vec<u16> = (0..nf).map(|_| r.u16()).collect::<Result<_>>()?;
    let isize: Vec<u16> = (0..nf).map(|_| r.u16()).collect::<Result<_>>()?;
    let _off: Vec<u16> = (0..nf).map(|_| r.u16()).collect::<Result<_>>()?;
    let order: Vec<u16> = (0..nf).map(|_| r.u16()).collect::<Result<_>>()?;
    let names: Vec<String> = (0..nf).map(|_| r.str()).collect::<Result<_>>()?;
    let name = r.str()?;
    let class = r.str()?;
    let _extag = r.u16()?;
    let _exref = r.u16()?;
    // The version and a "more" word, then (version 4) the flags and attribute list, then
    // the version again.
    let mut attrs = Vec::new();
    let version = r.u16().unwrap_or(3);
    let _more = r.u16();
    if version >= 4 {
        let flags = r.i32()?;
        if flags & 1 != 0 {
            let na = r.i32()?.max(0) as usize;
            for _ in 0..na.min(limits::MAX_ATTRS) {
                let fi = r.i32()?;
                let t = r.u16()?;
                let a = r.u16()?;
                attrs.push((fi, t, a));
            }
        }
    }
    let fields = (0..nf)
        .map(|k| {
            let nt = Nt::from_code(types[k]).ok();
            let ord = usize::from(order[k]).max(1);
            let size = nt.map_or(usize::from(isize[k]), |n| n.size() * ord);
            VdataField {
                name: names[k].clone(),
                nt,
                order: ord,
                size,
            }
        })
        .collect();
    Ok(Vdata {
        rf,
        name,
        class,
        interlace,
        nrecords: nrec as u32,
        fields,
        attrs,
    })
}

/// Parses every Vgroup and Vdata header.
pub(crate) fn scan(f: &mut File, ctx: Ctx, cancel: &(dyn Fn() -> bool + Sync)) -> Result<()> {
    let mut n = 0;
    for dd in &f.dds.list {
        let tag = dd.base_tag();
        if tag != TAG_VG && tag != TAG_VH {
            continue;
        }
        n += 1;
        if n > limits::MAX_VOBJECTS {
            return Err(Error::TooLarge);
        }
        if n % 4096 == 0 && cancel() {
            return Err(Error::Cancelled);
        }
        let Some(b) = crate::dd::raw_of(ctx.data, dd) else {
            f.warnings
                .push(format!("element {tag}/{} outside the file", dd.rf));
            continue;
        };
        if tag == TAG_VG {
            match parse_vgroup(dd.rf, b) {
                Ok(v) => f.vgroups.push(v),
                Err(e) => f.warnings.push(format!("Vgroup {}: {e}", dd.rf)),
            }
        } else {
            match parse_vdata(dd.rf, b) {
                Ok(v) => f.vdatas.push(v),
                Err(e) => f.warnings.push(format!("Vdata {}: {e}", dd.rf)),
            }
        }
    }
    for (i, v) in f.vgroups.iter().enumerate() {
        f.vg_index.entry(v.rf).or_insert(i);
    }
    for (i, v) in f.vdatas.iter().enumerate() {
        f.vd_index.entry(v.rf).or_insert(i);
    }
    Ok(())
}

/// The records of a Vdata as stored.
pub(crate) fn vdata_bytes(f: &File, ctx: Ctx, vd: &Vdata) -> Result<Vec<u8>> {
    let st = crate::elem::store(f, ctx, TAG_VS, vd.rf)?;
    let want = vd.record_size() as u64 * u64::from(vd.nrecords);
    if want > limits::MAX_ELEMENT {
        return Err(Error::TooLarge);
    }
    let _res = specular_core::budget::reserve(want)?;
    let mut b = crate::elem::read(f, ctx, &st, (TAG_VS, vd.rf))?.into_owned();
    b.truncate(want as usize);
    Ok(b)
}

/// An attribute stored as a Vdata (one field; the name is the Vdata's name, or the field's
/// name for raster image attributes).
pub(crate) fn attr_of(f: &File, ctx: Ctx, vd: &Vdata, use_field_name: bool) -> Option<Attr> {
    let fl = vd.fields.first()?;
    let nt = fl.nt?;
    let b = vdata_bytes(f, ctx, vd).ok()?;
    let count = fl.order * vd.nrecords as usize;
    let b = b.get(..count * nt.size()).unwrap_or(&b);
    let value = value_of(nt, b);
    Some(Attr {
        name: if use_field_name {
            fl.name.clone()
        } else {
            vd.name.clone()
        },
        value,
    })
}

/// Stored numbers as an attribute value.
pub(crate) fn value_of(nt: Nt, b: &[u8]) -> Value {
    use crate::Num;
    if nt.is_char() {
        return Value::Str(vec![crate::clean_text(b)]);
    }
    let v = nt.values(b);
    match nt.num {
        Num::F16 | Num::F32 | Num::F64 => Value::Float(v),
        Num::U8 | Num::U16 | Num::U32 => Value::UInt(v.into_iter().map(|x| x as u64).collect()),
        Num::U64 => Value::UInt(
            b.chunks_exact(8)
                .map(|c| {
                    let a: [u8; 8] = c.try_into().unwrap_or([0; 8]);
                    if nt.little {
                        u64::from_le_bytes(a)
                    } else {
                        u64::from_be_bytes(a)
                    }
                })
                .collect(),
        ),
        Num::I64 => Value::Int(
            b.chunks_exact(8)
                .map(|c| {
                    let a: [u8; 8] = c.try_into().unwrap_or([0; 8]);
                    if nt.little {
                        i64::from_le_bytes(a)
                    } else {
                        i64::from_be_bytes(a)
                    }
                })
                .collect(),
        ),
        _ => Value::Int(v.into_iter().map(|x| x as i64).collect()),
    }
}

impl File {
    pub fn vgroup(&self, rf: u16) -> Option<&Vgroup> {
        self.vg_index.get(&rf).map(|&i| &self.vgroups[i])
    }
    pub fn vdata(&self, rf: u16) -> Option<&Vdata> {
        self.vd_index.get(&rf).map(|&i| &self.vdatas[i])
    }

    /// The attributes of a Vdata (whole-Vdata attributes only).
    pub fn vdata_attrs(&self, ctx: Ctx, vd: &Vdata) -> Vec<Attr> {
        vd.attrs
            .iter()
            .filter(|a| a.0 == -1 && a.1 == TAG_VH)
            .filter_map(|a| self.vdata(a.2))
            .filter_map(|a| attr_of(self, ctx, a, false))
            .collect()
    }
}
