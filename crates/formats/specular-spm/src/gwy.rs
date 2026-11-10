//! Gwyddion native files (`.gwy`): [`parse`] reads the object tree and lists the image
//! channels; [`Channel::read_plane`] reads one channel's values.
//!
//! A `GWYP` file (Gwyddion 2) is the magic and one serialized object: its type name
//! (NUL-terminated), a `u32` little-endian byte size and components, each a name, a type
//! character and a value: `b` / `c` one byte, `i` `i32`, `q` `i64`, `d` `f64`, `s` a
//! NUL-terminated string, `o` a nested object; upper-case types are arrays (`u32` count
//! first). The root `GwyContainer` holds `/N/data` (`GwyDataField`: `xres`, `yres`, `xreal`,
//! `yreal`, `xoff`, `yoff`, `si_unit_xy` and `si_unit_z` as `GwySIUnit` objects with a
//! `unitstr`, and `data`, an `f64` array, top row first), `/N/data/title` and `/N/meta` (a
//! container of strings).
//!
//! A `GWYO` file (Gwyddion 1) has the same objects, but its root container lists items as a
//! `u32` value type (GLib's fundamental type numbers), the key and the value.

use specular_em::samples::{self, Order, Sample};

use crate::{Error, Result};

/// One image channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The channel number (`/N/data`).
    pub id: u32,
    pub title: Option<String>,
    pub xres: usize,
    pub yres: usize,
    /// Physical size in `unit_xy`.
    pub xreal: f64,
    pub yreal: f64,
    pub xoff: f64,
    pub yoff: f64,
    pub unit_xy: String,
    pub unit_z: String,
    /// `/N/meta` strings.
    pub meta: Vec<(String, String)>,
    data_offset: usize,
    data_count: usize,
}

/// A parsed file: its channels, by channel number.
#[derive(Clone, Debug, PartialEq)]
pub struct Gwy {
    /// Whether this is the older `GWYO` container.
    pub old: bool,
    pub channels: Vec<Channel>,
}

/// Whether `head` starts with a Gwyddion magic.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"GWYP") || head.starts_with(b"GWYO")
}

/// Deepest object nesting.
const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Num(f64),
    Str(String),
    Obj(Obj),
    /// An `f64` array: element count and offset.
    Doubles(usize, usize),
    Other,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Obj {
    name: String,
    items: Vec<(String, Val)>,
}

impl Obj {
    fn get(&self, key: &str) -> Option<&Val> {
        self.items.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn num(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Val::Num(v) => Some(*v),
            _ => None,
        }
    }

    fn unit(&self, key: &str) -> String {
        match self.get(key) {
            Some(Val::Obj(o)) => match o.get("unitstr") {
                Some(Val::Str(s)) => base_unit(s),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }
}

/// A unit string without its SI prefix: values are always stored in base units and Gwyddion
/// ignores a prefix in `unitstr` (Gwyddion 1 wrote e.g. `µm` next to values in metres).
/// Only a prefix before a known base unit is dropped (`nm` → `m`, `mV` → `V`; `min`, `mol`
/// and `m` stay).
pub fn base_unit(s: &str) -> String {
    const PREFIXES: &[&str] = &[
        "Y", "Z", "E", "P", "T", "G", "M", "k", "h", "da", "d", "c", "m", "µ", "μ", "u", "n", "p",
        "f", "a", "z", "y",
    ];
    const BASES: &[&str] = &[
        "Hz", "Pa", "Ohm", "Ω", "deg", "rad", "m", "V", "A", "N", "s", "W", "K", "C", "F", "T",
        "S", "J", "H",
    ];
    let t = s.trim();
    for p in PREFIXES {
        let Some(rest) = t.strip_prefix(p) else {
            continue;
        };
        for b in BASES {
            if let Some(after) = rest.strip_prefix(b)
                && after.chars().next().is_none_or(|c| !c.is_alphabetic())
            {
                return rest.to_string();
            }
        }
    }
    t.to_string()
}

/// Text as UTF-8, else Latin-1 (Gwyddion 1 wrote the locale's encoding).
fn text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

struct Reader<'a> {
    file: &'a [u8],
    pos: usize,
    end: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let e = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.end)
            .ok_or(Error::Truncated("Gwyddion object"))?;
        let s = &self.file[self.pos..e];
        self.pos = e;
        Ok(s)
    }

    fn cstr(&mut self) -> Result<String> {
        let rest = &self.file[self.pos..self.end];
        let n = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or(Error::Truncated("Gwyddion string"))?;
        let s = text(&rest[..n]);
        self.pos += n + 1;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f64(&mut self) -> Result<f64> {
        Ok(samples::f64_at(self.take(8)?, true))
    }

    /// An object: type name, size, components.
    fn object(&mut self, depth: usize) -> Result<Obj> {
        if depth > MAX_DEPTH {
            return Err(Error::Malformed("objects nested too deeply".into()));
        }
        let name = self.cstr()?;
        let size = self.u32()? as usize;
        let end = self
            .pos
            .checked_add(size)
            .filter(|&e| e <= self.end)
            .ok_or(Error::Truncated("Gwyddion object"))?;
        let outer = std::mem::replace(&mut self.end, end);
        let mut items = Vec::new();
        while self.pos < end {
            let key = self.cstr()?;
            let t = self.take(1)?[0];
            let v = self.component(t, depth)?;
            items.push((key, v));
        }
        self.end = outer;
        Ok(Obj { name, items })
    }

    fn component(&mut self, t: u8, depth: usize) -> Result<Val> {
        Ok(match t {
            b'b' => Val::Num(f64::from(u8::from(self.take(1)?[0] != 0))),
            b'c' => Val::Num(f64::from(self.take(1)?[0])),
            b'i' => {
                let b = self.take(4)?;
                Val::Num(f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            }
            b'q' => {
                let b = self.take(8)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                Val::Num(i64::from_le_bytes(a) as f64)
            }
            b'd' => Val::Num(self.f64()?),
            b's' => Val::Str(self.cstr()?),
            b'o' => Val::Obj(self.object(depth + 1)?),
            b'C' | b'I' | b'Q' | b'D' | b'B' => {
                let n = self.u32()? as usize;
                let size = match t {
                    b'C' | b'B' => 1,
                    b'I' => 4,
                    _ => 8,
                };
                let offset = self.pos;
                self.take(n.checked_mul(size).ok_or(Error::TooLarge)?)?;
                if t == b'D' {
                    Val::Doubles(n, offset)
                } else {
                    Val::Other
                }
            }
            b'S' => {
                let n = self.u32()?;
                for _ in 0..n {
                    self.cstr()?;
                }
                Val::Other
            }
            b'O' => {
                let n = self.u32()?;
                for _ in 0..n {
                    self.object(depth + 1)?;
                }
                Val::Other
            }
            t => {
                return Err(Error::Malformed(format!(
                    "component type {:?}",
                    char::from(t)
                )));
            }
        })
    }

    /// Gwyddion 1's container items: GLib type, key, value.
    fn old_container(&mut self) -> Result<Obj> {
        let name = self.cstr()?;
        if name != "GwyContainer" {
            return Err(Error::Malformed(format!("root object {name:?}")));
        }
        let size = self.u32()? as usize;
        let end = self.pos.saturating_add(size).min(self.end);
        self.end = end;
        let mut items = Vec::new();
        while self.pos < end {
            let Ok(t) = self.u32() else { break };
            let Ok(key) = self.cstr() else { break };
            let v = match t {
                // char, uchar, boolean
                12 | 16 | 20 => Val::Num(f64::from(self.take(1)?[0])),
                // int, uint, enum, flags
                24 | 28 | 48 | 52 => {
                    let b = self.take(4)?;
                    Val::Num(f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])))
                }
                // long, ulong, int64, uint64
                32 | 36 | 40 | 44 => {
                    self.take(8)?;
                    Val::Other
                }
                56 => {
                    let b = self.take(4)?;
                    Val::Num(f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])))
                }
                60 => Val::Num(self.f64()?),
                64 => Val::Str(self.cstr()?),
                80 => Val::Obj(self.object(1)?),
                // Unknown: the rest can't be found.
                _ => break,
            };
            items.push((key, v));
        }
        Ok(Obj {
            name: "GwyContainer".into(),
            items,
        })
    }
}

/// Parses a whole file.
pub fn parse(file: &[u8]) -> Result<Gwy> {
    let old = if file.starts_with(b"GWYP") {
        false
    } else if file.starts_with(b"GWYO") {
        true
    } else {
        return Err(Error::NotGwy);
    };
    let mut r = Reader {
        file,
        pos: 4,
        end: file.len(),
    };
    let root = if old {
        r.old_container()?
    } else {
        r.object(0)?
    };
    if root.name != "GwyContainer" {
        return Err(Error::Malformed(format!("root object {:?}", root.name)));
    }
    let mut channels = Vec::new();
    for (key, v) in &root.items {
        let Some(id) = key
            .strip_prefix('/')
            .and_then(|k| k.strip_suffix("/data"))
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Val::Obj(df) = v else { continue };
        if df.name != "GwyDataField" {
            continue;
        }
        let (Some(xres), Some(yres)) = (df.num("xres"), df.num("yres")) else {
            continue;
        };
        if xres < 1.0 || yres < 1.0 || xres * yres > specular_em::MAX_PLANE as f64 {
            continue;
        }
        let Some(&Val::Doubles(count, offset)) = df.get("data") else {
            continue;
        };
        let title = match root.get(&format!("/{id}/data/title")) {
            Some(Val::Str(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => None,
        };
        let meta = match root.get(&format!("/{id}/meta")) {
            Some(Val::Obj(m)) => m
                .items
                .iter()
                .filter_map(|(k, v)| match v {
                    Val::Str(s) => Some((k.clone(), s.clone())),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let real = |k: &str| df.num(k).filter(|v| v.is_finite() && *v > 0.0);
        let off = |k: &str| df.num(k).filter(|v| v.is_finite()).unwrap_or(0.0);
        channels.push(Channel {
            id,
            title,
            xres: xres as usize,
            yres: yres as usize,
            xreal: real("xreal").unwrap_or(xres),
            yreal: real("yreal").unwrap_or(yres),
            xoff: off("xoff"),
            yoff: off("yoff"),
            unit_xy: df.unit("si_unit_xy"),
            unit_z: df.unit("si_unit_z"),
            meta,
            data_offset: offset,
            data_count: count,
        });
    }
    if channels.is_empty() {
        return Err(Error::NoImage);
    }
    channels.sort_by_key(|c| c.id);
    Ok(Gwy { old, channels })
}

impl Channel {
    /// Reads the channel (`xres × yres` values, top row first) into `out`. Returns whether
    /// samples were missing (they read as NaN).
    pub fn read_plane(&self, file: &[u8], out: &mut [f32]) -> Result<bool> {
        if out.len() != self.xres * self.yres {
            return Err(Error::Malformed("plane size".into()));
        }
        let n = self.data_count.min(out.len());
        let end = self.data_offset.saturating_add(n * 8).min(file.len());
        let data = file.get(self.data_offset..end).unwrap_or(&[]);
        let missing = samples::convert(data, Sample::F64, Order::Little, 8, out);
        Ok(missing || self.data_count < out.len())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn obj(name: &str, comps: &[u8]) -> Vec<u8> {
        let mut b = name.as_bytes().to_vec();
        b.push(0);
        b.extend((comps.len() as u32).to_le_bytes());
        b.extend(comps);
        b
    }

    fn comp(key: &str, t: u8, v: &[u8]) -> Vec<u8> {
        let mut b = key.as_bytes().to_vec();
        b.push(0);
        b.push(t);
        b.extend(v);
        b
    }

    fn unit(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        obj("GwySIUnit", &comp("unitstr", b's', &v))
    }

    pub(crate) fn field(w: i32, h: i32, values: &[f64]) -> Vec<u8> {
        let mut c = Vec::new();
        c.extend(comp("xres", b'i', &w.to_le_bytes()));
        c.extend(comp("yres", b'i', &h.to_le_bytes()));
        c.extend(comp("xreal", b'd', &2e-6f64.to_le_bytes()));
        c.extend(comp("yreal", b'd', &1e-6f64.to_le_bytes()));
        c.extend(comp("si_unit_xy", b'o', &unit("m")));
        c.extend(comp("si_unit_z", b'o', &unit("m")));
        let mut d = (values.len() as u32).to_le_bytes().to_vec();
        for v in values {
            d.extend(v.to_le_bytes());
        }
        c.extend(comp("data", b'D', &d));
        obj("GwyDataField", &c)
    }

    pub(crate) fn file() -> Vec<u8> {
        let mut c = Vec::new();
        c.extend(comp("/1/data", b'o', &field(2, 1, &[5.0, 6.0])));
        c.extend(comp(
            "/0/data",
            b'o',
            &field(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
        ));
        c.extend(comp("/0/data/title", b's', b"Height\0"));
        c.extend(comp(
            "/0/meta",
            b'o',
            &obj("GwyContainer", &comp("Date", b's', b"today\0")),
        ));
        c.extend(comp("/0/show", b'D', &0u32.to_le_bytes()));
        let mut f = b"GWYP".to_vec();
        f.extend(obj("GwyContainer", &c));
        f
    }

    #[test]
    fn tiny() {
        let f = file();
        let g = parse(&f).unwrap();
        assert_eq!(g.channels.len(), 2);
        let c = &g.channels[0];
        assert_eq!((c.id, c.xres, c.yres), (0, 3, 2));
        assert_eq!(c.title.as_deref(), Some("Height"));
        assert_eq!((c.unit_xy.as_str(), c.unit_z.as_str()), ("m", "m"));
        assert_eq!(c.meta, [("Date".to_string(), "today".to_string())]);
        let mut out = vec![0f32; 6];
        assert!(!c.read_plane(&f, &mut out).unwrap());
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        for cut in 0..f.len() {
            let _ = parse(&f[..cut]);
        }
        assert_eq!(parse(b"GWYQ").unwrap_err(), Error::NotGwy);
    }

    #[test]
    fn base_units() {
        for (s, want) in [
            ("nm", "m"),
            ("µm", "m"),
            ("um", "m"),
            ("mm", "m"),
            ("m", "m"),
            ("mV", "V"),
            ("nA", "A"),
            ("kHz", "Hz"),
            ("nm/s", "m/s"),
            ("min", "min"),
            ("mol", "mol"),
            ("deg", "deg"),
            ("Pa", "Pa"),
            ("", ""),
        ] {
            assert_eq!(base_unit(s), want, "{s}");
        }
    }

    #[test]
    fn old_container() {
        let mut f = b"GWYOGwyContainer\0".to_vec();
        let mut items = Vec::new();
        items.extend(64u32.to_le_bytes());
        items.extend(b"/0/base/palette\0Blue\0");
        items.extend(80u32.to_le_bytes());
        items.extend(b"/0/data\0");
        items.extend(field(1, 2, &[7.0, 8.0]));
        f.extend((items.len() as u32).to_le_bytes());
        f.extend(items);
        let g = parse(&f).unwrap();
        assert!(g.old);
        let mut out = vec![0f32; 2];
        g.channels[0].read_plane(&f, &mut out).unwrap();
        assert_eq!(out, [7.0, 8.0]);
    }
}
