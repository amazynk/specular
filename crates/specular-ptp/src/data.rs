//! PTP's data encoding (ISO 15740 §5.3): little-endian integers, strings as a count of UCS-2
//! units (the terminating NUL counted) and arrays as a 32-bit count, and the datasets built
//! from them: DeviceInfo, StorageInfo, ObjectInfo and DevicePropDesc.

use crate::codes::ty;

/// A dataset that ends early or holds something it can't.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed(pub &'static str);

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed {}", self.0)
    }
}

impl std::error::Error for Malformed {}

/// Reads PTP data from a byte slice.
pub struct Reader<'a> {
    b: &'a [u8],
    what: &'static str,
}

impl<'a> Reader<'a> {
    /// `what` names the dataset in errors.
    pub fn new(b: &'a [u8], what: &'static str) -> Reader<'a> {
        Reader { b, what }
    }

    pub fn remaining(&self) -> usize {
        self.b.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        if self.b.len() < n {
            return Err(Malformed(self.what));
        }
        let (h, t) = self.b.split_at(n);
        self.b = t;
        Ok(h)
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    pub fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_le_bytes(self.arr()?))
    }

    pub fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.arr()?))
    }

    pub fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(self.arr()?))
    }

    /// A PTP string: a count of 16-bit units including the NUL (0 for the empty string).
    /// Unpaired surrogates become U+FFFD.
    pub fn string(&mut self) -> Result<String, Malformed> {
        let n = self.u8()? as usize;
        let mut units = Vec::with_capacity(n);
        for _ in 0..n {
            units.push(self.u16()?);
        }
        while units.last() == Some(&0) {
            units.pop();
        }
        Ok(char::decode_utf16(units)
            .map(|c| c.unwrap_or('\u{FFFD}'))
            .collect())
    }

    /// An array of 16-bit values.
    pub fn u16_array(&mut self) -> Result<Vec<u16>, Malformed> {
        let n = self.u32()? as usize;
        if n > self.b.len() / 2 {
            return Err(Malformed(self.what));
        }
        (0..n).map(|_| self.u16()).collect()
    }

    /// An array of 32-bit values.
    pub fn u32_array(&mut self) -> Result<Vec<u32>, Malformed> {
        let n = self.u32()? as usize;
        if n > self.b.len() / 4 {
            return Err(Malformed(self.what));
        }
        (0..n).map(|_| self.u32()).collect()
    }

    /// A value of data type `t`.
    pub fn value(&mut self, t: u16) -> Result<Value, Malformed> {
        if t == ty::STR {
            return Ok(Value::Str(self.string()?));
        }
        if t & ty::ARRAY != 0 && t != ty::STR {
            let el = t & !ty::ARRAY;
            if !(ty::INT8..=ty::UINT128).contains(&el) {
                return Err(Malformed(self.what));
            }
            let n = self.u32()? as usize;
            if n > self.b.len() {
                return Err(Malformed(self.what));
            }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(self.value(el)?);
            }
            return Ok(Value::Array(v));
        }
        Ok(match t {
            ty::INT8 => Value::Int(self.u8()? as i8 as i64),
            ty::UINT8 => Value::UInt(self.u8()? as u64),
            ty::INT16 => Value::Int(self.u16()? as i16 as i64),
            ty::UINT16 => Value::UInt(self.u16()? as u64),
            ty::INT32 => Value::Int(self.u32()? as i32 as i64),
            ty::UINT32 => Value::UInt(self.u32()? as u64),
            ty::INT64 => Value::Int(self.u64()? as i64),
            ty::UINT64 => Value::UInt(self.u64()?),
            ty::INT128 | ty::UINT128 => Value::Wide(self.arr()?),
            _ => return Err(Malformed(self.what)),
        })
    }
}

/// Writes PTP data.
#[derive(Default)]
pub struct Writer {
    pub b: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.b.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.b.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.b.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.b.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A PTP string (at most 254 units and the NUL; longer is cut).
    pub fn string(&mut self, s: &str) -> &mut Self {
        if s.is_empty() {
            return self.u8(0);
        }
        let units: Vec<u16> = s.encode_utf16().take(254).collect();
        self.u8(units.len() as u8 + 1);
        for u in units {
            self.u16(u);
        }
        self.u16(0)
    }

    pub fn u16_array(&mut self, v: &[u16]) -> &mut Self {
        self.u32(v.len() as u32);
        for &x in v {
            self.u16(x);
        }
        self
    }

    pub fn u32_array(&mut self, v: &[u32]) -> &mut Self {
        self.u32(v.len() as u32);
        for &x in v {
            self.u32(x);
        }
        self
    }

    /// `v` as data type `t` (integers are cut to the type's width).
    pub fn value(&mut self, t: u16, v: &Value) -> &mut Self {
        match (t, v) {
            (ty::STR, Value::Str(s)) => self.string(s),
            (ty::STR, _) => self.u8(0),
            (t, Value::Array(a)) if t & ty::ARRAY != 0 => {
                self.u32(a.len() as u32);
                for x in a {
                    self.value(t & !ty::ARRAY, x);
                }
                self
            }
            (ty::INT128 | ty::UINT128, Value::Wide(w)) => {
                self.b.extend_from_slice(w);
                self
            }
            (t, v) => {
                let x = v.as_i64().unwrap_or(0);
                match t {
                    ty::INT8 | ty::UINT8 => self.u8(x as u8),
                    ty::INT16 | ty::UINT16 => self.u16(x as u16),
                    ty::INT32 | ty::UINT32 => self.u32(x as u32),
                    _ => self.u64(x as u64),
                }
            }
        }
    }
}

/// A property value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Int(i64),
    UInt(u64),
    /// A 128-bit integer, as sent.
    Wide([u8; 16]),
    Array(Vec<Value>),
    Str(String),
}

impl Value {
    /// The value as an integer (`None` for strings, arrays and 128-bit values).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            Value::UInt(v) => Some(*v as i64),
            _ => None,
        }
    }
}

/// The DeviceInfo dataset (§5.5.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub standard_version: u16,
    pub vendor_extension_id: u32,
    pub vendor_extension_version: u16,
    pub vendor_extension_desc: String,
    pub functional_mode: u16,
    pub operations: Vec<u16>,
    pub events: Vec<u16>,
    pub properties: Vec<u16>,
    pub capture_formats: Vec<u16>,
    pub image_formats: Vec<u16>,
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
}

impl DeviceInfo {
    pub fn parse(b: &[u8]) -> Result<DeviceInfo, Malformed> {
        let mut r = Reader::new(b, "DeviceInfo");
        Ok(DeviceInfo {
            standard_version: r.u16()?,
            vendor_extension_id: r.u32()?,
            vendor_extension_version: r.u16()?,
            vendor_extension_desc: r.string()?,
            functional_mode: r.u16()?,
            operations: r.u16_array()?,
            events: r.u16_array()?,
            properties: r.u16_array()?,
            capture_formats: r.u16_array()?,
            image_formats: r.u16_array()?,
            manufacturer: r.string()?,
            model: r.string()?,
            device_version: r.string()?,
            serial_number: r.string()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u16(self.standard_version)
            .u32(self.vendor_extension_id)
            .u16(self.vendor_extension_version)
            .string(&self.vendor_extension_desc)
            .u16(self.functional_mode)
            .u16_array(&self.operations)
            .u16_array(&self.events)
            .u16_array(&self.properties)
            .u16_array(&self.capture_formats)
            .u16_array(&self.image_formats)
            .string(&self.manufacturer)
            .string(&self.model)
            .string(&self.device_version)
            .string(&self.serial_number);
    }

    /// The camera's name: the model, with the manufacturer first when the model doesn't
    /// already start with it.
    pub fn name(&self) -> String {
        let (m, model) = (self.manufacturer.trim(), self.model.trim());
        let first = m.split_whitespace().next().unwrap_or("");
        if m.is_empty()
            || model
                .to_ascii_lowercase()
                .starts_with(&first.to_ascii_lowercase())
        {
            model.to_owned()
        } else if model.is_empty() {
            m.to_owned()
        } else {
            format!("{first} {model}")
        }
    }
}

/// The StorageInfo dataset (§5.5.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageInfo {
    pub storage_type: u16,
    pub filesystem_type: u16,
    pub access: u16,
    pub max_capacity: u64,
    pub free_bytes: u64,
    /// 0xFFFFFFFF when the camera doesn't say.
    pub free_images: u32,
    pub description: String,
    pub volume_label: String,
}

impl StorageInfo {
    pub fn parse(b: &[u8]) -> Result<StorageInfo, Malformed> {
        let mut r = Reader::new(b, "StorageInfo");
        Ok(StorageInfo {
            storage_type: r.u16()?,
            filesystem_type: r.u16()?,
            access: r.u16()?,
            max_capacity: r.u64()?,
            free_bytes: r.u64()?,
            free_images: r.u32()?,
            description: r.string()?,
            volume_label: r.string()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u16(self.storage_type)
            .u16(self.filesystem_type)
            .u16(self.access)
            .u64(self.max_capacity)
            .u64(self.free_bytes)
            .u32(self.free_images)
            .string(&self.description)
            .string(&self.volume_label);
    }
}

/// The ObjectInfo dataset (§5.5.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObjectInfo {
    pub storage_id: u32,
    pub format: u16,
    pub protection: u16,
    /// 0xFFFFFFFF for objects of 4 GiB or more.
    pub compressed_size: u32,
    pub thumb_format: u16,
    pub thumb_size: u32,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub parent: u32,
    pub association_type: u16,
    pub association_desc: u32,
    pub sequence_number: u32,
    pub filename: String,
    /// `YYYYMMDDThhmmss[.s][Z|±hhmm]` (ISO 8601 basic), or empty.
    pub capture_date: String,
    pub modification_date: String,
    pub keywords: String,
}

impl ObjectInfo {
    pub fn parse(b: &[u8]) -> Result<ObjectInfo, Malformed> {
        let mut r = Reader::new(b, "ObjectInfo");
        Ok(ObjectInfo {
            storage_id: r.u32()?,
            format: r.u16()?,
            protection: r.u16()?,
            compressed_size: r.u32()?,
            thumb_format: r.u16()?,
            thumb_size: r.u32()?,
            thumb_width: r.u32()?,
            thumb_height: r.u32()?,
            width: r.u32()?,
            height: r.u32()?,
            bit_depth: r.u32()?,
            parent: r.u32()?,
            association_type: r.u16()?,
            association_desc: r.u32()?,
            sequence_number: r.u32()?,
            filename: r.string()?,
            // Some cameras stop after the name.
            capture_date: if r.remaining() > 0 {
                r.string()?
            } else {
                String::new()
            },
            modification_date: if r.remaining() > 0 {
                r.string()?
            } else {
                String::new()
            },
            keywords: if r.remaining() > 0 {
                r.string()?
            } else {
                String::new()
            },
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u32(self.storage_id)
            .u16(self.format)
            .u16(self.protection)
            .u32(self.compressed_size)
            .u16(self.thumb_format)
            .u32(self.thumb_size)
            .u32(self.thumb_width)
            .u32(self.thumb_height)
            .u32(self.width)
            .u32(self.height)
            .u32(self.bit_depth)
            .u32(self.parent)
            .u16(self.association_type)
            .u32(self.association_desc)
            .u32(self.sequence_number)
            .string(&self.filename)
            .string(&self.capture_date)
            .string(&self.modification_date)
            .string(&self.keywords);
    }

    /// A folder, not a file.
    pub fn is_folder(&self) -> bool {
        self.format == crate::codes::format::ASSOCIATION
    }

    /// The capture date as year, month, day, hour, minute, second (local to the camera).
    pub fn capture_time(&self) -> Option<[u32; 6]> {
        parse_date(&self.capture_date).or_else(|| parse_date(&self.modification_date))
    }
}

/// `YYYYMMDDThhmmss…` as [y, mo, d, h, mi, s].
pub fn parse_date(s: &str) -> Option<[u32; 6]> {
    let b = s.as_bytes();
    if b.len() < 15 || b[8] != b'T' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> { s.get(r)?.parse().ok() };
    let d = [
        num(0..4)?,
        num(4..6)?,
        num(6..8)?,
        num(9..11)?,
        num(11..13)?,
        num(13..15)?,
    ];
    let ok =
        (1..=12).contains(&d[1]) && (1..=31).contains(&d[2]) && d[3] < 24 && d[4] < 60 && d[5] < 61;
    ok.then_some(d)
}

/// What values a property may take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Form {
    /// Anything of the type.
    None,
    /// From `min` to `max` in steps of `step`.
    Range { min: Value, max: Value, step: Value },
    /// One of these.
    Enum(Vec<Value>),
}

/// The DevicePropDesc dataset (§5.5.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropDesc {
    pub code: u16,
    pub data_type: u16,
    /// The host may change it.
    pub writable: bool,
    pub default: Value,
    pub current: Value,
    pub form: Form,
}

impl PropDesc {
    pub fn parse(b: &[u8]) -> Result<PropDesc, Malformed> {
        let mut r = Reader::new(b, "DevicePropDesc");
        let code = r.u16()?;
        let data_type = r.u16()?;
        let writable = r.u8()? == 1;
        let default = r.value(data_type)?;
        let current = r.value(data_type)?;
        // Some cameras leave the form flag out when there is none.
        let flag = if r.remaining() > 0 { r.u8()? } else { 0 };
        let form = match flag {
            1 => Form::Range {
                min: r.value(data_type)?,
                max: r.value(data_type)?,
                step: r.value(data_type)?,
            },
            2 => {
                let n = r.u16()?;
                let mut v = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    v.push(r.value(data_type)?);
                }
                Form::Enum(v)
            }
            _ => Form::None,
        };
        Ok(PropDesc {
            code,
            data_type,
            writable,
            default,
            current,
            form,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u16(self.code)
            .u16(self.data_type)
            .u8(self.writable as u8)
            .value(self.data_type, &self.default)
            .value(self.data_type, &self.current);
        match &self.form {
            Form::None => {
                w.u8(0);
            }
            Form::Range { min, max, step } => {
                w.u8(1)
                    .value(self.data_type, min)
                    .value(self.data_type, max)
                    .value(self.data_type, step);
            }
            Form::Enum(v) => {
                w.u8(2).u16(v.len() as u16);
                for x in v {
                    w.value(self.data_type, x);
                }
            }
        }
    }

    /// The values the property may take, in the camera's order (a range is listed from
    /// `min` to `max`, at most `limit` values; `None` for a form without a list).
    pub fn choices(&self, limit: usize) -> Option<Vec<Value>> {
        match &self.form {
            Form::None => None,
            Form::Enum(v) => Some(v.clone()),
            Form::Range { min, max, step } => {
                let (lo, hi, st) = (min.as_i64()?, max.as_i64()?, step.as_i64()?.max(1));
                let signed = matches!(min, Value::Int(_));
                let mut out = Vec::new();
                let mut x = lo;
                while x <= hi && out.len() < limit {
                    out.push(if signed {
                        Value::Int(x)
                    } else {
                        Value::UInt(x as u64)
                    });
                    x = x.checked_add(st)?;
                }
                Some(out)
            }
        }
    }

    /// The value `by` steps from the current one in [`PropDesc::choices`] (clamped at the
    /// ends; `None` when there is no list or it is empty).
    pub fn step(&self, by: i32) -> Option<Value> {
        let c = self.choices(4096)?;
        if c.is_empty() {
            return None;
        }
        let at = c.iter().position(|v| *v == self.current).unwrap_or(0) as i64;
        let to = (at + by as i64).clamp(0, c.len() as i64 - 1) as usize;
        Some(c[to].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip_with_the_nul_counted() {
        let mut w = Writer::default();
        w.string("EOS R5").string("").string("Ünïcødé ✓");
        // 6 units + NUL.
        assert_eq!(w.b[0], 7);
        assert_eq!(w.b.len(), 1 + 14 + 1 + 1 + 20);
        let mut r = Reader::new(&w.b, "t");
        assert_eq!(r.string().unwrap(), "EOS R5");
        assert_eq!(r.string().unwrap(), "");
        assert_eq!(r.string().unwrap(), "Ünïcødé ✓");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn short_data_is_malformed() {
        assert_eq!(DeviceInfo::parse(&[1, 0, 0]), Err(Malformed("DeviceInfo")));
        // An array count larger than the data left.
        let mut w = Writer::default();
        w.u32(1_000_000);
        assert!(Reader::new(&w.b, "a").u16_array().is_err());
    }

    #[test]
    fn device_info_round_trips() {
        let d = DeviceInfo {
            standard_version: 100,
            vendor_extension_id: 6,
            vendor_extension_version: 100,
            vendor_extension_desc: "microsoft.com: 1.0".into(),
            operations: vec![0x1001, 0x1002, 0x100E],
            events: vec![0x4002, 0x400D],
            properties: vec![0x5001, 0x500F],
            capture_formats: vec![0x3801],
            image_formats: vec![0x3801, 0x3000],
            manufacturer: "Nikon Corporation".into(),
            model: "Z 6".into(),
            device_version: "V1.0".into(),
            serial_number: "123".into(),
            ..DeviceInfo::default()
        };
        let mut w = Writer::default();
        d.write(&mut w);
        assert_eq!(DeviceInfo::parse(&w.b).unwrap(), d);
        assert_eq!(d.name(), "Nikon Z 6");
        let canon = DeviceInfo {
            manufacturer: "Canon Inc.".into(),
            model: "Canon EOS R5".into(),
            ..DeviceInfo::default()
        };
        assert_eq!(canon.name(), "Canon EOS R5");
    }

    #[test]
    fn prop_desc_forms() {
        let enumd = PropDesc {
            code: 0x500F,
            data_type: ty::UINT16,
            writable: true,
            default: Value::UInt(100),
            current: Value::UInt(400),
            form: Form::Enum([100, 200, 400, 800].map(Value::UInt).to_vec()),
        };
        let mut w = Writer::default();
        enumd.write(&mut w);
        let back = PropDesc::parse(&w.b).unwrap();
        assert_eq!(back, enumd);
        assert_eq!(back.step(1), Some(Value::UInt(800)));
        assert_eq!(back.step(5), Some(Value::UInt(800)));
        assert_eq!(back.step(-2), Some(Value::UInt(100)));

        let range = PropDesc {
            code: 0x5010,
            data_type: ty::INT16,
            writable: true,
            default: Value::Int(0),
            current: Value::Int(-333),
            form: Form::Range {
                min: Value::Int(-1000),
                max: Value::Int(1000),
                step: Value::Int(333),
            },
        };
        let mut w = Writer::default();
        range.write(&mut w);
        let back = PropDesc::parse(&w.b).unwrap();
        assert_eq!(back, range);
        assert_eq!(
            back.choices(10).unwrap(),
            [-1000, -667, -334, -1, 332, 665, 998]
                .map(Value::Int)
                .to_vec()
        );
        // -333 is not on the list: stepping starts from the first.
        assert_eq!(back.step(1), Some(Value::Int(-667)));
    }

    #[test]
    fn object_info_dates_and_short_tails() {
        let o = ObjectInfo {
            filename: "DSC_0001.NEF".into(),
            capture_date: "20241103T142530.0".into(),
            ..ObjectInfo::default()
        };
        let mut w = Writer::default();
        o.write(&mut w);
        let back = ObjectInfo::parse(&w.b).unwrap();
        assert_eq!(back.capture_time(), Some([2024, 11, 3, 14, 25, 30]));
        // Cut after the file name.
        let mut w = Writer::default();
        let mut cut = o.clone();
        cut.capture_date.clear();
        cut.write(&mut w);
        let n = w.b.len() - 3;
        let back = ObjectInfo::parse(&w.b[..n]).unwrap();
        assert_eq!(back.filename, "DSC_0001.NEF");
        assert_eq!(back.capture_time(), None);
        assert_eq!(parse_date("20241303T000000"), None);
    }

    #[test]
    fn array_and_wide_values() {
        let mut w = Writer::default();
        let a = Value::Array(vec![Value::UInt(1), Value::UInt(2)]);
        w.value(ty::UINT16 | ty::ARRAY, &a);
        w.value(ty::UINT128, &Value::Wide([7; 16]));
        let mut r = Reader::new(&w.b, "v");
        assert_eq!(r.value(ty::UINT16 | ty::ARRAY).unwrap(), a);
        assert_eq!(r.value(ty::UINT128).unwrap(), Value::Wide([7; 16]));
        assert!(Reader::new(&[0; 4], "v").value(0x0042).is_err());
    }
}
