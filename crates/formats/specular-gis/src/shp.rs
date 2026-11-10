//! ESRI Shapefile: the `.shp` geometry, its `.shx` index, the `.dbf` attribute table, the
//! `.prj` CRS (WKT1 in the ESRI dialect) and the `.cpg` code page.
//!
//! Every shape type is read: (Multi)Point, PolyLine and Polygon with their Z and M forms
//! (Z and M are dropped; the view is 2D) and MultiPatch (rings as polygons, strips and fans
//! as triangles). Polygon rings are grouped the way the format defines them: a clockwise
//! ring starts a polygon, the counter-clockwise rings after it are its holes.

use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};
use crate::wkt;

/// The files of one shapefile; only `.shp` is required.
#[derive(Clone, Copy, Debug, Default)]
pub struct Parts<'a> {
    pub shp: &'a [u8],
    pub shx: Option<&'a [u8]>,
    pub dbf: Option<&'a [u8]>,
    pub prj: Option<&'a str>,
    pub cpg: Option<&'a str>,
}

/// Is this a `.shp` main file?
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 100 && be_i32(head, 0) == Some(9994) && le_i32(head, 28) == Some(1000)
}

fn be_i32(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn le_i32(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn le_f64(b: &[u8], o: usize) -> Option<f64> {
    Some(f64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

/// The shape type's name.
pub fn type_name(t: i32) -> &'static str {
    match t {
        0 => "Null",
        1 => "Point",
        3 => "PolyLine",
        5 => "Polygon",
        8 => "MultiPoint",
        11 => "PointZ",
        13 => "PolyLineZ",
        15 => "PolygonZ",
        18 => "MultiPointZ",
        21 => "PointM",
        23 => "PolyLineM",
        25 => "PolygonM",
        28 => "MultiPointM",
        31 => "MultiPatch",
        _ => "unknown",
    }
}

pub fn read(p: Parts<'_>, layer_name: &str) -> Result<Dataset, String> {
    if !sniff(p.shp) {
        return Err("not a Shapefile (.shp)".into());
    }
    let mut ds = Dataset {
        format: "Shapefile",
        ..Dataset::default()
    };
    let shape_type = le_i32(p.shp, 32).unwrap_or(0);
    let mut layer = Layer::new(layer_name);
    layer.declared_type = Some(type_name(shape_type).to_string());

    if let Some(prj) = p.prj {
        let prj = prj.trim_start_matches('\u{feff}').trim();
        match wkt::crs_from_wkt(prj) {
            Some(c) => ds.crs = Some(c),
            None if !prj.is_empty() => {
                ds.crs_note = wkt::parse(prj).and_then(|n| n.name().map(str::to_string));
            }
            None => {}
        }
    }

    // Records: through the index when there is one (it survives a damaged record length),
    // else one after another.
    let mut records: Vec<Option<(usize, usize)>> = Vec::new();
    match p.shx.filter(|x| x.len() >= 100) {
        Some(shx) => {
            let n = (shx.len() - 100) / 8;
            for i in 0..n {
                let off = be_i32(shx, 100 + i * 8).unwrap_or(-1);
                let len = be_i32(shx, 104 + i * 8).unwrap_or(-1);
                let (off, mut len) = (off as i64 * 2, len as i64 * 2);
                // Some writers put a length in the index that runs past the file: the
                // record's own header then gives it.
                if off >= 100
                    && off + 8 + len > p.shp.len() as i64
                    && let Some(own) = be_i32(p.shp, off as usize + 4)
                    && own >= 0
                    && off + 8 + own as i64 * 2 <= p.shp.len() as i64
                {
                    len = own as i64 * 2;
                }
                let ok = off >= 100 && len >= 0 && off + 8 + len <= p.shp.len() as i64;
                records.push(ok.then_some((off as usize + 8, len as usize)));
            }
        }
        None => {
            let mut o = 100usize;
            while o + 8 <= p.shp.len() {
                let len = be_i32(p.shp, o + 4).unwrap_or(-1);
                if len < 0 {
                    ds.warn("a record has a negative length; the rest are not read");
                    break;
                }
                let len = len as usize * 2;
                if o + 8 + len > p.shp.len() {
                    ds.warn("the last record is truncated");
                    break;
                }
                records.push(Some((o + 8, len)));
                o += 8 + len;
            }
        }
    }

    let table = match p.dbf {
        Some(d) => match Dbf::open(d, p.cpg) {
            Ok(t) => {
                ds.encoding = Some(t.encoding.name().to_string());
                layer.set_fields(t.fields.iter().map(|f| f.name.clone()).collect());
                Some(t)
            }
            Err(e) => {
                ds.warn(format!("the attribute table (.dbf) is unreadable: {e}"));
                None
            }
        },
        None => None,
    };
    if let Some(t) = &table
        && t.records != records.len()
    {
        ds.warn(format!(
            "the attribute table has {} rows for {} shapes",
            t.records,
            records.len()
        ));
    }

    let mut vertices = 0usize;
    for (i, rec) in records.iter().enumerate() {
        if layer.features.len() >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let values = match &table {
            Some(t) if i < t.records => match t.row(i) {
                Some(v) => v,
                None => continue, // deleted
            },
            Some(t) => vec![Value::Null; t.fields.len()],
            None => Vec::new(),
        };
        let geom = match rec {
            Some((o, len)) => match shape(&p.shp[*o..o + len]) {
                Ok(g) => g,
                Err(e) => {
                    ds.warn(format!("a shape is unreadable: {e}"));
                    None
                }
            },
            None => {
                ds.warn("an index entry points outside the file");
                None
            }
        };
        if let Some(g) = &geom {
            vertices += g.vertex_count();
            if vertices > MAX_VERTICES {
                ds.warn("too many vertices; the rest are not shown");
                break;
            }
        }
        layer.features.push(crate::model::Feature {
            id: Some(i.to_string()),
            geometry: geom,
            values,
        });
    }
    // A table without a .shp record count to drive it (a .dbf longer than the .shp) is
    // reported above; its extra rows have no shapes and are left out, as GDAL does.
    ds.layers.push(layer);
    Ok(ds)
}

/// One record's content (from its shape type on).
fn shape(b: &[u8]) -> Result<Option<Geometry>, String> {
    let t = le_i32(b, 0).ok_or("empty record")?;
    let short = || "record too short".to_string();
    match t {
        0 => Ok(None),
        1 | 11 | 21 => {
            let x = le_f64(b, 4).ok_or_else(short)?;
            let y = le_f64(b, 12).ok_or_else(short)?;
            // NaN coordinates: an empty point.
            Ok((x.is_finite() && y.is_finite()).then_some(Geometry::Point([x, y])))
        }
        8 | 18 | 28 => {
            let n = le_i32(b, 36).ok_or_else(short)?;
            let pts = points(b, 40, n)?;
            Ok(Some(Geometry::MultiPoint(pts)))
        }
        3 | 13 | 23 | 5 | 15 | 25 => {
            let np = le_i32(b, 36).ok_or_else(short)?;
            let n = le_i32(b, 40).ok_or_else(short)?;
            if !(0..=10_000_000).contains(&np) {
                return Err("bad part count".into());
            }
            let parts: Vec<i32> = (0..np as usize)
                .map(|k| le_i32(b, 44 + k * 4).ok_or_else(short))
                .collect::<Result<_, _>>()?;
            let pts = points(b, 44 + np as usize * 4, n)?;
            let rings = split(&parts, &pts)?;
            if rings.is_empty() {
                return Ok(None);
            }
            Ok(Some(if matches!(t, 5 | 15 | 25) {
                polygons(rings)
            } else if rings.len() == 1 {
                Geometry::LineString(rings.into_iter().next().unwrap_or_default())
            } else {
                Geometry::MultiLineString(rings)
            }))
        }
        31 => {
            let np = le_i32(b, 36).ok_or_else(short)?;
            let n = le_i32(b, 40).ok_or_else(short)?;
            if !(0..=10_000_000).contains(&np) {
                return Err("bad part count".into());
            }
            let np = np as usize;
            let parts: Vec<i32> = (0..np)
                .map(|k| le_i32(b, 44 + k * 4).ok_or_else(short))
                .collect::<Result<_, _>>()?;
            let types: Vec<i32> = (0..np)
                .map(|k| le_i32(b, 44 + np * 4 + k * 4).ok_or_else(short))
                .collect::<Result<_, _>>()?;
            let pts = points(b, 44 + np * 8, n)?;
            let rings = split(&parts, &pts)?;
            let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
            for (r, ty) in rings.into_iter().zip(types) {
                match ty {
                    // Triangle strip, triangle fan.
                    0 | 1 => {
                        for k in 2..r.len() {
                            let tri = if ty == 0 {
                                vec![r[k - 2], r[k - 1], r[k], r[k - 2]]
                            } else {
                                vec![r[0], r[k - 1], r[k], r[0]]
                            };
                            polys.push(vec![tri]);
                        }
                    }
                    // Inner ring: a hole of the last polygon.
                    3 if !polys.is_empty() => {
                        if let Some(p) = polys.last_mut() {
                            p.push(r);
                        }
                    }
                    _ => polys.push(vec![r]),
                }
            }
            Ok((!polys.is_empty()).then_some(Geometry::MultiPolygon(polys)))
        }
        _ => Err(format!("unknown shape type {t}")),
    }
}

fn points(b: &[u8], o: usize, n: i32) -> Result<Vec<Pt>, String> {
    if n < 0 || o + n as usize * 16 > b.len() {
        return Err("point count beyond the record".into());
    }
    Ok((0..n as usize)
        .map(|k| {
            let q = o + k * 16;
            [
                le_f64(b, q).unwrap_or(f64::NAN),
                le_f64(b, q + 8).unwrap_or(f64::NAN),
            ]
        })
        .collect())
}

fn split(parts: &[i32], pts: &[Pt]) -> Result<Vec<Vec<Pt>>, String> {
    // The first part starts at 0 and the starts never go back (shapelib rejects others).
    if parts.first().is_some_and(|&s| s != 0)
        || parts.windows(2).any(|w| w[1] < w[0])
        || parts.iter().any(|&s| s as usize > pts.len())
    {
        return Err("corrupt part starts".into());
    }
    let mut out = Vec::with_capacity(parts.len());
    for (k, &s) in parts.iter().enumerate() {
        let e = parts.get(k + 1).copied().unwrap_or(pts.len() as i32);
        let (s, e) = (s.max(0) as usize, (e.max(0) as usize).min(pts.len()));
        if s < e {
            out.push(pts[s..e].to_vec());
        }
    }
    Ok(out)
}

/// Twice the signed area (positive counter-clockwise, y up).
pub fn signed_area2(r: &[Pt]) -> f64 {
    let mut a = 0.0;
    for w in r.windows(2) {
        a += w[0][0] * w[1][1] - w[1][0] * w[0][1];
    }
    if let (Some(f), Some(l)) = (r.first(), r.last()) {
        a += l[0] * f[1] - f[0] * l[1];
    }
    a
}

/// Groups rings into polygons: clockwise rings are exteriors, counter-clockwise rings are
/// holes of the exterior that contains them (else of the one before them).
pub fn polygons(rings: Vec<Vec<Pt>>) -> Geometry {
    let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
    let mut holes = Vec::new();
    for r in rings {
        if signed_area2(&r) <= 0.0 {
            polys.push(vec![r]);
        } else {
            holes.push((polys.len(), r));
        }
    }
    for (before, h) in holes {
        let probe = h.first().copied();
        let inside = probe.and_then(|p| polys.iter().position(|poly| ring_contains(&poly[0], p)));
        match inside.or(before.checked_sub(1)) {
            Some(k) => polys[k].push(h),
            // A hole before any exterior: the file has its orientation reversed.
            None => polys.push(vec![h]),
        }
    }
    if polys.len() == 1 {
        Geometry::Polygon(polys.pop().unwrap_or_default())
    } else {
        Geometry::MultiPolygon(polys)
    }
}

/// Point in ring, even-odd.
pub fn ring_contains(r: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = r.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------------------
// dBASE

#[derive(Clone, Debug)]
pub struct DbfField {
    pub name: String,
    pub kind: u8,
    pub len: usize,
    pub decimals: u8,
    offset: usize,
}

/// A dBASE III/IV table (as shapefiles use it).
pub struct Dbf<'a> {
    data: &'a [u8],
    pub fields: Vec<DbfField>,
    pub records: usize,
    header: usize,
    record_len: usize,
    pub encoding: &'static encoding_rs::Encoding,
}

impl<'a> Dbf<'a> {
    pub fn open(data: &'a [u8], cpg: Option<&str>) -> Result<Dbf<'a>, String> {
        if data.len() < 32 {
            return Err("too short".into());
        }
        let records = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let header = u16::from_le_bytes([data[8], data[9]]) as usize;
        let record_len = u16::from_le_bytes([data[10], data[11]]) as usize;
        let mut fields = Vec::new();
        let mut o = 32;
        let mut off = 1;
        while o + 32 <= data.len() && data[o] != 0x0D && o + 32 <= header.max(33) {
            let d = &data[o..o + 32];
            let name_end = d[..11].iter().position(|&c| c == 0).unwrap_or(11);
            let name = String::from_utf8_lossy(&d[..name_end]).trim().to_string();
            let len = usize::from(d[16]);
            // Character fields longer than 255 keep the high byte in the decimal count.
            let (len, decimals) = if d[11] == b'C' {
                (len + usize::from(d[17]) * 256, 0)
            } else {
                (len, d[17])
            };
            fields.push(DbfField {
                name,
                kind: d[11],
                len,
                decimals,
                offset: off,
            });
            off += len;
            o += 32;
        }
        if fields.is_empty() && records > 0 {
            return Err("no fields".into());
        }
        let record_len = if record_len == 0 { off } else { record_len };
        // Records that the file length cannot hold are not there.
        let avail = data.len().saturating_sub(header) / record_len.max(1);
        let records = records.min(avail);
        let ldid = data[29];
        let encoding = cpg
            .and_then(encoding_from_cpg)
            .or_else(|| encoding_from_ldid(ldid))
            .unwrap_or_else(|| {
                // No code page named: UTF-8 when the text is valid UTF-8, else Windows Latin.
                let body = data.get(header..).unwrap_or(&[]);
                if std::str::from_utf8(&body[..body.len().min(1 << 20)]).is_ok() {
                    encoding_rs::UTF_8
                } else {
                    encoding_rs::WINDOWS_1252
                }
            });
        Ok(Dbf {
            data,
            fields,
            records,
            header,
            record_len,
            encoding,
        })
    }

    /// Row `i`'s values, `None` when the row is deleted.
    pub fn row(&self, i: usize) -> Option<Vec<Value>> {
        let o = self.header + i * self.record_len;
        let r = self.data.get(o..o + self.record_len)?;
        if r[0] == b'*' {
            return None;
        }
        Some(
            self.fields
                .iter()
                .map(|f| {
                    r.get(f.offset..f.offset + f.len)
                        .map_or(Value::Null, |b| self.value(f, b))
                })
                .collect(),
        )
    }

    fn text(&self, b: &[u8]) -> String {
        let (s, _, _) = self.encoding.decode(b);
        s.trim_end_matches(['\0', ' ']).to_string()
    }

    fn value(&self, f: &DbfField, b: &[u8]) -> Value {
        let ascii = || {
            String::from_utf8_lossy(b)
                .trim_matches(['\0', ' '])
                .to_string()
        };
        match f.kind {
            b'C' | b'V' | b'W' => Value::Text(self.text(b)),
            b'N' | b'F' => {
                let s = ascii();
                if s.is_empty() || s.starts_with('*') {
                    return Value::Null;
                }
                if f.decimals == 0
                    && let Ok(i) = s.parse::<i64>()
                {
                    return Value::Int(i);
                }
                s.parse::<f64>().map_or(Value::Null, Value::Float)
            }
            b'L' => match b.first() {
                Some(b'T' | b't' | b'Y' | b'y') => Value::Bool(true),
                Some(b'F' | b'f' | b'N' | b'n') => Value::Bool(false),
                _ => Value::Null,
            },
            b'D' => {
                let s = ascii();
                if s.len() == 8 && s.bytes().all(|c| c.is_ascii_digit()) && s != "00000000" {
                    Value::Date(format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..]))
                } else {
                    Value::Null
                }
            }
            b'I' | b'+' if b.len() == 4 => {
                Value::Int(i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            }
            b'O' if b.len() == 8 => {
                Value::Float(f64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
            }
            b'Y' if b.len() == 8 => {
                Value::Float(i64::from_le_bytes(b.try_into().unwrap_or([0; 8])) as f64 / 1e4)
            }
            b'@' | b'T' if b.len() == 8 => {
                let day = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let ms = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
                if day == 0 {
                    return Value::Null;
                }
                let (y, m, d) = civil_from_julian(i64::from(day));
                let s = ms.max(0) / 1000;
                Value::Date(format!(
                    "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
                    s / 3600,
                    s / 60 % 60,
                    s % 60
                ))
            }
            b'M' | b'G' | b'B' | b'P' => {
                // Memo blocks live in a .dbt/.fpt file that shapefiles do not use.
                let s = ascii();
                if s.is_empty() {
                    Value::Null
                } else {
                    Value::Text(format!("(memo {s})"))
                }
            }
            _ => Value::Text(self.text(b)),
        }
    }
}

/// Julian day number to (year, month, day), proleptic Gregorian.
fn civil_from_julian(jd: i64) -> (i64, i64, i64) {
    // Days since 1970-01-01, then Howard Hinnant's civil_from_days.
    let z = jd - 2_440_588 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The encoding a `.cpg` file names ("UTF-8", "1252", "ANSI 1251", "8859_1", "CP936"...).
pub fn encoding_from_cpg(s: &str) -> Option<&'static encoding_rs::Encoding> {
    let t = s.trim().trim_start_matches('\u{feff}').trim();
    let up = t.to_ascii_uppercase().replace(['_', ' '], "-");
    let up = up
        .trim_start_matches("ANSI-")
        .trim_start_matches("CP-")
        .to_string();
    let up = up.strip_prefix("CP").map_or(up.clone(), str::to_string);
    let digits = up.chars().all(|c| c.is_ascii_digit());
    let label = if digits {
        match up.as_str() {
            "65001" => "utf-8".to_string(),
            "932" => "shift_jis".to_string(),
            "936" => "gbk".to_string(),
            "949" => "euc-kr".to_string(),
            "950" => "big5".to_string(),
            "866" => "ibm866".to_string(),
            "874" => "windows-874".to_string(),
            n if n.starts_with("125") => format!("windows-{n}"),
            n if n.starts_with("8859") => format!("iso-8859-{}", &n[4..]),
            n => format!("windows-{n}"),
        }
    } else if let Some(n) = up
        .strip_prefix("8859-")
        .or_else(|| up.strip_prefix("ISO8859-"))
    {
        format!("iso-8859-{n}")
    } else if let Some(n) = up.strip_prefix("ISO88591") {
        format!("iso-8859-1{n}")
    } else {
        up.clone()
    };
    encoding_rs::Encoding::for_label(label.as_bytes())
}

/// The encoding a dBASE language driver id stands for, when encoding_rs has it.
fn encoding_from_ldid(id: u8) -> Option<&'static encoding_rs::Encoding> {
    let label: &[u8] = match id {
        0x03 | 0x57 | 0x58 | 0x59 => b"windows-1252",
        0x13 | 0x7B => b"shift_jis",
        0x26 | 0x65 => b"ibm866",
        0x4D | 0x7A => b"gbk",
        0x4E | 0x79 => b"euc-kr",
        0x4F | 0x78 => b"big5",
        0x50 | 0x7C => b"windows-874",
        0xC8 => b"windows-1250",
        0xC9 => b"windows-1251",
        0xCA => b"windows-1254",
        0xCB => b"windows-1253",
        0xCC => b"windows-1257",
        _ => return None,
    };
    encoding_rs::Encoding::for_label(label)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a .shp/.shx pair from record contents.
    pub fn build_shp(shape_type: i32, records: &[Vec<u8>]) -> (Vec<u8>, Vec<u8>) {
        let mut shp = vec![0u8; 100];
        let mut shx = vec![0u8; 100];
        let mut off = 100usize;
        for (i, r) in records.iter().enumerate() {
            shx.extend_from_slice(&((off / 2) as i32).to_be_bytes());
            shx.extend_from_slice(&((r.len() / 2) as i32).to_be_bytes());
            shp.extend_from_slice(&(i as i32 + 1).to_be_bytes());
            shp.extend_from_slice(&((r.len() / 2) as i32).to_be_bytes());
            shp.extend_from_slice(r);
            off += 8 + r.len();
        }
        for h in [&mut shp, &mut shx] {
            let len = h.len();
            h[0..4].copy_from_slice(&9994i32.to_be_bytes());
            h[24..28].copy_from_slice(&((len / 2) as i32).to_be_bytes());
            h[28..32].copy_from_slice(&1000i32.to_le_bytes());
            h[32..36].copy_from_slice(&shape_type.to_le_bytes());
        }
        (shp, shx)
    }

    /// A PolyLine/Polygon record.
    pub fn poly_record(t: i32, rings: &[&[Pt]]) -> Vec<u8> {
        let mut r = Vec::new();
        r.extend_from_slice(&t.to_le_bytes());
        r.extend_from_slice(&[0u8; 32]);
        r.extend_from_slice(&(rings.len() as i32).to_le_bytes());
        let n: usize = rings.iter().map(|r| r.len()).sum();
        r.extend_from_slice(&(n as i32).to_le_bytes());
        let mut s = 0;
        for ring in rings {
            r.extend_from_slice(&(s as i32).to_le_bytes());
            s += ring.len();
        }
        for p in rings.iter().flat_map(|r| r.iter()) {
            r.extend_from_slice(&p[0].to_le_bytes());
            r.extend_from_slice(&p[1].to_le_bytes());
        }
        r
    }

    /// A dBASE table: fields (name, type, length, decimals) and rows of raw text.
    pub fn build_dbf(fields: &[(&str, u8, u8, u8)], rows: &[(bool, Vec<&[u8]>)]) -> Vec<u8> {
        let header = 32 + fields.len() * 32 + 1;
        let rec: usize = 1 + fields.iter().map(|f| usize::from(f.2)).sum::<usize>();
        let mut d = vec![0u8; 32];
        d[0] = 3;
        d[4..8].copy_from_slice(&(rows.len() as u32).to_le_bytes());
        d[8..10].copy_from_slice(&(header as u16).to_le_bytes());
        d[10..12].copy_from_slice(&(rec as u16).to_le_bytes());
        for (name, t, len, dec) in fields {
            let mut f = [0u8; 32];
            f[..name.len()].copy_from_slice(name.as_bytes());
            f[11] = *t;
            f[16] = *len;
            f[17] = *dec;
            d.extend_from_slice(&f);
        }
        d.push(0x0D);
        for (deleted, vals) in rows {
            d.push(if *deleted { b'*' } else { b' ' });
            for (v, f) in vals.iter().zip(fields) {
                let mut cell = vec![b' '; usize::from(f.2)];
                cell[..v.len()].copy_from_slice(v);
                d.extend_from_slice(&cell);
            }
        }
        d.push(0x1A);
        d
    }

    #[test]
    fn polygons_with_holes_and_attributes() {
        // Exterior clockwise, hole counter-clockwise; a second exterior.
        let ext: &[Pt] = &[
            [0.0, 0.0],
            [0.0, 10.0],
            [10.0, 10.0],
            [10.0, 0.0],
            [0.0, 0.0],
        ];
        let hole: &[Pt] = &[[2.0, 2.0], [4.0, 2.0], [4.0, 4.0], [2.0, 4.0], [2.0, 2.0]];
        let ext2: &[Pt] = &[[20.0, 0.0], [20.0, 5.0], [25.0, 5.0], [20.0, 0.0]];
        let recs = vec![
            poly_record(5, &[ext, hole, ext2]),
            poly_record(5, &[ext]),
            vec![0, 0, 0, 0],
        ];
        let (shp, shx) = build_shp(5, &recs);
        let dbf = build_dbf(
            &[
                ("NAME", b'C', 10, 0),
                ("POP", b'N', 8, 0),
                ("AREA", b'N', 10, 3),
                ("D", b'D', 8, 0),
            ],
            &[
                (false, vec![b"Z\xc3\xbcrich", b"42", b"1.500", b"20240131"]),
                (true, vec![b"gone", b"1", b"1", b""]),
                (false, vec![b"null", b"", b"*****", b""]),
            ],
        );
        let ds = read(
            Parts {
                shp: &shp,
                shx: Some(&shx),
                dbf: Some(&dbf),
                prj: Some(r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#),
                cpg: Some("UTF-8\n"),
            },
            "t",
        )
        .unwrap();
        assert_eq!(ds.crs.as_ref().unwrap().epsg, Some(32633));
        let l = &ds.layers[0];
        assert_eq!(l.features.len(), 2);
        assert_eq!(l.fields(), ["NAME", "POP", "AREA", "D"]);
        match l.features[0].geometry.as_ref().unwrap() {
            Geometry::MultiPolygon(p) => {
                assert_eq!(p.len(), 2);
                assert_eq!(p[0].len(), 2);
            }
            g => panic!("{g:?}"),
        }
        assert_eq!(l.features[0].values[0], Value::Text("Zürich".into()));
        assert_eq!(l.features[0].values[1], Value::Int(42));
        assert_eq!(l.features[0].values[2], Value::Float(1.5));
        assert_eq!(l.features[0].values[3], Value::Date("2024-01-31".into()));
        assert_eq!(l.features[1].id.as_deref(), Some("2"));
        assert_eq!(l.features[1].geometry, None);
        assert_eq!(l.features[1].values[1], Value::Null);
    }

    #[test]
    fn cpg_labels() {
        assert_eq!(encoding_from_cpg("1252"), Some(encoding_rs::WINDOWS_1252));
        assert_eq!(
            encoding_from_cpg("ANSI 1251"),
            Some(encoding_rs::WINDOWS_1251)
        );
        assert_eq!(encoding_from_cpg("8859_5"), Some(encoding_rs::ISO_8859_5));
        assert_eq!(encoding_from_cpg("UTF8"), Some(encoding_rs::UTF_8));
        assert_eq!(encoding_from_cpg("CP936"), Some(encoding_rs::GBK));
        assert_eq!(civil_from_julian(2_440_588), (1970, 1, 1));
    }
}
