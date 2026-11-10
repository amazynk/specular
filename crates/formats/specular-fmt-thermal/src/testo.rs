//! Testo `.bmt` from the testo 865, 868, 871, 872 and 883.
//!
//! Layout as Testo publishes it in its own sample reader (github.com/testo/read_bmt), from
//! which only the layout is taken: an optional JPEG preview; a `<ToFo>` header naming the
//! size of an XML schema, the size of the data block and its byte order; the schema (nested
//! `<group name>` and `<item name type size>`); one byte; then each item's bytes in schema
//! order. The thermal image is the `Images/Ir` item, an OpenCV matrix (six 32-bit fields:
//! dimensions, rows, columns, depth, channels, element size; then the pixels) of 16-bit
//! values spanning −101 °C to 1001 °C. Temperatures are stored already corrected, so they
//! cannot be recomputed for another emissivity (the `Recompute` coefficients are not
//! described).
//!
//! The older cameras (testo 875, 880, 882, 885, 890) write a different `.bmt` that starts
//! with a bitmap and has no published layout; it is refused with that reason.

use crate::{
    Camera, Conditions, Error, Kind, Mapping, Palette, Pixels, Result, Thermal, Visible, jpeg_size,
    malformed,
};

const F: &str = "Testo BMT";

/// Kelvin of a stored count: `count / DIVISOR − OFFSET` gives degrees Celsius.
pub(crate) const DIVISOR: f64 = 65535.0 / 1102.0;
pub(crate) const OFFSET: f64 = 101.0;

/// Where the `<ToFo` header may start (after the JPEG preview).
const SEARCH: usize = 4 << 20;

/// The reason an older `.bmt` is refused.
pub const OLD_BMT_REASON: &str =
    "older Testo BMT (testo 875–890): no published layout for its temperatures";

/// Whether `d` starts like an older Testo `.bmt` (a bitmap). Only meaningful together
/// with the `.bmt` extension.
pub fn is_old_bmt(d: &[u8]) -> bool {
    d.starts_with(b"BM") && d.len() > 54
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The value of `name="…"` in a tag's text.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = tag.find(&key)? + key.len();
    let len = tag[at..].find('"')?;
    Some(&tag[at..at + len])
}

struct Header {
    xml: std::ops::Range<usize>,
    data: std::ops::Range<usize>,
    big: bool,
}

fn header(d: &[u8]) -> Option<Header> {
    let start = find(&d[..d.len().min(SEARCH)], b"<ToFo")?;
    let end = start + find(&d[start..d.len().min(start + 4096)], b"</ToFo>")? + 7;
    let tofo = std::str::from_utf8(&d[start..end]).ok()?;
    let tag_of = |name: &str| {
        let at = tofo.find(&format!("<{name} "))?;
        let len = tofo[at..].find('>')?;
        Some(&tofo[at..at + len])
    };
    let xml_tag = tag_of("xml")?;
    let data_tag = tag_of("data")?;
    let xml_len: usize = attr(xml_tag, "size")?.parse().ok()?;
    let data_len: usize = attr(data_tag, "size")?.parse().ok()?;
    let big = match attr(data_tag, "endianness")? {
        "little" => false,
        "big" => true,
        _ => return None,
    };
    let xml = end..end.checked_add(xml_len)?;
    // One byte (a line feed) separates the schema from the data.
    let data_at = xml.end.checked_add(1)?;
    let data = data_at..data_at.checked_add(data_len)?;
    (data.end <= d.len()).then_some(Header { xml, data, big })
}

/// A JPEG preview (or nothing) and then the `<ToFo>` header with sizes that fit the file.
pub(crate) fn is_bmt(d: &[u8]) -> bool {
    (d.starts_with(&[0xff, 0xd8]) || d.starts_with(b"<ToFo")) && header(d).is_some()
}

/// One schema item: its path ("Images/Ir"), type, and bytes.
struct Item<'a> {
    path: String,
    ty: String,
    bytes: &'a [u8],
}

/// The schema's items in order, each given its slice of the data block.
fn items<'a>(schema: &str, data: &'a [u8]) -> Result<Vec<Item<'a>>> {
    let mut out = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    let mut at = 0usize;
    let mut rest = schema;
    while let Some(open) = rest.find('<') {
        let close = rest[open..]
            .find('>')
            .ok_or_else(|| malformed(F, "unterminated schema tag"))?;
        let tag = &rest[open..open + close + 1];
        rest = &rest[open + close + 1..];
        if tag.starts_with("</group") {
            groups.pop();
        } else if tag.starts_with("<group") {
            let name = attr(tag, "name").ok_or_else(|| malformed(F, "group without a name"))?;
            if !tag.ends_with("/>") {
                groups.push(name.to_owned());
            }
        } else if tag.starts_with("<item") {
            let (Some(name), Some(ty), Some(size)) =
                (attr(tag, "name"), attr(tag, "type"), attr(tag, "size"))
            else {
                return Err(malformed(F, "schema item without name, type or size"));
            };
            let size: usize = size
                .parse()
                .map_err(|_| malformed(F, "schema item size is not a number"))?;
            let bytes = at
                .checked_add(size)
                .and_then(|end| data.get(at..end))
                .ok_or_else(|| malformed(F, "schema items run past the data"))?;
            at += size;
            let mut path = groups.get(1..).unwrap_or_default().join("/");
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(name);
            out.push(Item {
                path,
                ty: ty.replace("&lt;", "<").replace("&gt;", ">"),
                bytes,
            });
        }
    }
    Ok(out)
}

struct Fields<'a> {
    items: Vec<Item<'a>>,
    big: bool,
}

impl<'a> Fields<'a> {
    fn get(&self, path: &str) -> Option<&Item<'a>> {
        self.items.iter().find(|i| i.path == path)
    }
    fn u32_at(&self, b: &[u8], at: usize) -> Option<u32> {
        let a: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
        Some(if self.big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        })
    }
    fn f32(&self, path: &str) -> Option<f64> {
        let i = self.get(path)?;
        let v = f64::from(f32::from_bits(self.u32_at(i.bytes, 0)?));
        v.is_finite().then_some(v)
    }
    /// A `Temperature<…>` item in degrees Celsius (stored in kelvin).
    fn celsius(&self, path: &str) -> Option<f64> {
        self.get(path)
            .filter(|i| i.ty.starts_with("Temperature"))
            .and_then(|_| self.f32(path))
            .map(|k| k - 273.15)
    }
    fn uint(&self, path: &str) -> Option<u64> {
        let b = self.get(path)?.bytes;
        if b.is_empty() || b.len() > 8 {
            return None;
        }
        let mut a = [0u8; 8];
        Some(if self.big {
            a[8 - b.len()..].copy_from_slice(b);
            u64::from_be_bytes(a)
        } else {
            a[..b.len()].copy_from_slice(b);
            u64::from_le_bytes(a)
        })
    }
    /// A `string`: a 32-bit length, then that many bytes.
    fn string(&self, path: &str) -> String {
        self.get(path)
            .and_then(|i| {
                let n = self.u32_at(i.bytes, 0)? as usize;
                let s = i.bytes.get(4..4 + n.min(i.bytes.len().saturating_sub(4)))?;
                Some(String::from_utf8_lossy(s).trim_end_matches('\0').to_owned())
            })
            .unwrap_or_default()
    }
    /// A `vecuint8`: a 32-bit length, then the bytes.
    fn vec(&self, path: &str) -> Option<&'a [u8]> {
        let i = self.get(path)?;
        let n = self.u32_at(i.bytes, 0)? as usize;
        i.bytes.get(4..4usize.checked_add(n)?)
    }
    fn version(&self, path: &str) -> Option<String> {
        let b = self.get(path)?.bytes;
        let v: Vec<String> = (0..b.len() / 4)
            .filter_map(|k| self.u32_at(b, 4 * k).map(|x| x.to_string()))
            .collect();
        (!v.is_empty()).then(|| v.join("."))
    }
}

/// "YYYY-MM-DD HH:MM:SS" from seconds since 1970 as recorded (no zone is stored).
fn date(secs: u64) -> Option<String> {
    let secs = i64::try_from(secs).ok().filter(|&s| s > 0)?;
    let (y, m, d) = crate::flir::civil(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        t / 3600,
        t / 60 % 60,
        t % 60
    ))
}

pub(crate) fn read(d: &[u8]) -> Result<Thermal> {
    let h = header(d).ok_or(Error::NotThermal)?;
    let schema = std::str::from_utf8(&d[h.xml.clone()])
        .map_err(|_| malformed(F, "the schema is not text"))?;
    let f = Fields {
        items: items(schema, &d[h.data.clone()])?,
        big: h.big,
    };
    let ir = f
        .get("Images/Ir")
        .ok_or_else(|| malformed(F, "no thermal image (Images/Ir)"))?;
    let m = |k: usize| f.u32_at(ir.bytes, 4 * k).map(|v| v as usize);
    let (rows, cols, depth, channels, elem) = match (m(1), m(2), m(3), m(4), m(5)) {
        (Some(r), Some(c), Some(dp), Some(ch), Some(e)) => (r, c, dp, ch, e),
        _ => return Err(malformed(F, "thermal matrix header cut short")),
    };
    // OpenCV depth 2 is CV_16U.
    if depth != 2 || channels != 1 || elem != 2 {
        return Err(Error::Unsupported {
            format: F,
            what: format!("thermal matrix of depth {depth}, {channels} channels"),
        });
    }
    let n = rows
        .checked_mul(cols)
        .filter(|&n| n > 0)
        .ok_or_else(|| malformed(F, "empty thermal matrix"))?;
    let px = n
        .checked_mul(2)
        .and_then(|b| ir.bytes.get(24..b.checked_add(24)?))
        .ok_or_else(|| malformed(F, "thermal matrix shorter than its size"))?;
    let values = px
        .chunks_exact(2)
        .map(|b| {
            if h.big {
                u16::from_be_bytes([b[0], b[1]])
            } else {
                u16::from_le_bytes([b[0], b[1]])
            }
        })
        .collect();
    let visible = f
        .vec("Images/Vis")
        .filter(|j| j.starts_with(&[0xff, 0xd8]))
        .map(|j| {
            let (width, height) = jpeg_size(j).unwrap_or((0, 0));
            Visible {
                bytes: j.to_vec(),
                width,
                height,
            }
        });
    let palette = f.vec("Palette/Colors").and_then(|c| {
        let colours: Vec<[u8; 3]> = c.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        (colours.len() >= 2).then(|| Palette {
            name: f.string("Palette/ActivePalette"),
            colours,
            above: None,
            below: None,
            isotherm: None,
        })
    });
    let range = f
        .celsius("ActiveMeasRange/Bottom")
        .zip(f.celsius("ActiveMeasRange/Top"));
    let humidity = f
        .f32("Environment/Humidity")
        .filter(|h| (0.0..=100.0).contains(h) && *h > 0.0)
        .map(|h| h / 100.0);
    let camera = Camera {
        model: f.string("DeviceInfo/DeviceName"),
        serial: f.uint("Serial").map(|s| s.to_string()).unwrap_or_default(),
        software: f
            .version("DeviceInfo/FirmwareVersion")
            .map(|v| format!("firmware {v}"))
            .unwrap_or_default(),
        range,
        date: f.uint("FileInfo/DateTime").and_then(date),
        ..Camera::default()
    };
    Ok(Thermal {
        kind: Kind::TestoBmt,
        width: cols,
        height: rows,
        pixels: Pixels::Counts {
            values,
            mapping: Mapping::Linear {
                divisor: DIVISOR,
                offset: OFFSET,
            },
        },
        conditions: Conditions {
            emissivity: f.f32("Environment/EmissivityValue"),
            reflected: f.celsius("Environment/ReflectedTemperature"),
            atmospheric: f.celsius("Environment/Temperature"),
            humidity,
            ..Conditions::default()
        },
        camera,
        palette,
        visible,
        pip: None,
    })
}
