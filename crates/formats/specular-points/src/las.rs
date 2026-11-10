//! LAS 1.0–1.4: the public header block, variable-length records (VLRs and EVLRs) and point
//! data records of formats 0–10, read in blocks in parallel; LAZ and COPC files share the
//! header and VLRs and differ in where the points are ([`crate::laz`], [`crate::copc`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use crate::build::{self, Colour};
use crate::laz::{self, Laz};
use crate::{
    Batch, Bounds, Channels, CloudInfo, Error, Limits, Loading, PointFormat, Result, copc, crs,
    malformed,
};

/// A point as decoded: raw integer coordinates and the attributes shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rec {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) z: i32,
    pub(crate) intensity: u16,
    pub(crate) class: u8,
    /// Return number (low 4 bits) and number of returns (high 4 bits).
    pub(crate) returns: u8,
    pub(crate) rgb: [u16; 3],
}

/// The smallest record of each point data format.
const RECORD_LEN: [usize; 11] = [20, 28, 26, 34, 57, 63, 30, 36, 38, 59, 67];

/// Points per block of an uncompressed file (a unit of parallel work and of progress).
const BLOCK: u64 = 1 << 16;

/// A variable-length record.
#[derive(Clone, Debug)]
pub(crate) struct Vlr<'a> {
    pub(crate) user: String,
    pub(crate) id: u16,
    pub(crate) data: &'a [u8],
}

/// The public header block.
#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub(crate) version: (u8, u8),
    pub(crate) header_size: usize,
    pub(crate) data_offset: usize,
    pub(crate) vlr_count: u32,
    pub(crate) format: u8,
    pub(crate) compressed: bool,
    pub(crate) record_len: usize,
    pub(crate) count: u64,
    pub(crate) scale: [f64; 3],
    pub(crate) offset: [f64; 3],
    /// min xyz, max xyz.
    pub(crate) extent: [f64; 6],
    pub(crate) evlr_start: u64,
    pub(crate) evlr_count: u32,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"))
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}

impl Header {
    pub(crate) fn parse(b: &[u8]) -> Result<Header> {
        if b.len() < 227 || !b.starts_with(b"LASF") {
            return malformed("not a LAS file (header too short)");
        }
        let version = (b[24], b[25]);
        if version.0 != 1 || version.1 > 4 {
            return Err(Error::Unsupported(format!(
                "LAS version {}.{}",
                version.0, version.1
            )));
        }
        let header_size = usize::from(u16_at(b, 94));
        if header_size < 227 || header_size > b.len() {
            return malformed(format!("LAS header size {header_size}"));
        }
        let raw_format = b[104];
        let format = raw_format & 0x3F;
        let compressed = raw_format & 0x80 != 0;
        if format > 10 {
            return Err(Error::Unsupported(format!("LAS point format {format}")));
        }
        let record_len = usize::from(u16_at(b, 105));
        if record_len < RECORD_LEN[usize::from(format)] {
            return malformed(format!(
                "{record_len}-byte records for point format {format}"
            ));
        }
        let legacy = u64::from(u32_at(b, 107));
        let (mut evlr_start, mut evlr_count, mut count) = (0, 0, legacy);
        if version.1 >= 4 && header_size >= 375 {
            evlr_start = u64_at(b, 235);
            evlr_count = u32_at(b, 243);
            let c = u64_at(b, 247);
            if c != 0 {
                count = c;
            }
        }
        let scale = [0, 1, 2].map(|i| f64_at(b, 131 + 8 * i));
        let offset = [0, 1, 2].map(|i| f64_at(b, 155 + 8 * i));
        if scale.iter().any(|s| !s.is_finite() || *s == 0.0)
            || offset.iter().any(|o| !o.is_finite())
        {
            return malformed("LAS scale or offset not finite and non-zero");
        }
        let mx = |i: usize| f64_at(b, 179 + 16 * i);
        let mn = |i: usize| f64_at(b, 187 + 16 * i);
        let extent = [mn(0), mn(1), mn(2), mx(0), mx(1), mx(2)];
        Ok(Header {
            version,
            header_size,
            data_offset: u32_at(b, 96) as usize,
            vlr_count: u32_at(b, 100),
            format,
            compressed,
            record_len,
            count,
            scale,
            offset,
            extent,
            evlr_start,
            evlr_count,
        })
    }

    /// The VLRs after the header and the EVLRs after the points; ones running past the file
    /// end the list.
    pub(crate) fn vlrs<'a>(&self, b: &'a [u8]) -> Vec<Vlr<'a>> {
        let mut out = Vec::new();
        let mut pos = self.header_size;
        for _ in 0..self.vlr_count.min(65_536) {
            let Some(h) = b.get(pos..pos + 54) else { break };
            let len = usize::from(u16_at(h, 20));
            let Some(data) = b.get(pos + 54..pos + 54 + len) else {
                break;
            };
            out.push(Vlr {
                user: id_string(&h[2..18]),
                id: u16_at(h, 18),
                data,
            });
            pos += 54 + len;
        }
        let mut pos = usize::try_from(self.evlr_start).unwrap_or(usize::MAX);
        for _ in 0..self.evlr_count.min(65_536) {
            let Some(h) = b.get(pos..pos.saturating_add(60)) else {
                break;
            };
            let len = usize::try_from(u64_at(h, 20)).unwrap_or(usize::MAX);
            let Some(data) = b.get(pos + 60..(pos + 60).saturating_add(len)) else {
                break;
            };
            out.push(Vlr {
                user: id_string(&h[2..18]),
                id: u16_at(h, 18),
                data,
            });
            pos += 60 + len;
        }
        out
    }

    /// Which optional fields the format has.
    pub(crate) fn channels(&self) -> Channels {
        Channels {
            rgb: matches!(self.format, 2 | 3 | 5 | 7 | 8 | 10),
            intensity: true,
            classification: true,
            returns: true,
        }
    }

    pub(crate) fn coordinate(&self, rec: &Rec) -> [f64; 3] {
        [
            f64::from(rec.x) * self.scale[0] + self.offset[0],
            f64::from(rec.y) * self.scale[1] + self.offset[1],
            f64::from(rec.z) * self.scale[2] + self.offset[2],
        ]
    }
}

fn id_string(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// An uncompressed record of `format` as a [`Rec`].
#[inline]
pub(crate) fn record(format: u8, r: &[u8]) -> Rec {
    let i = |o: usize| i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
    let rgb_at = |o: usize| [0, 2, 4].map(|k| u16_at(r, o + k));
    let mut rec = Rec {
        x: i(0),
        y: i(4),
        z: i(8),
        intensity: u16_at(r, 12),
        ..Rec::default()
    };
    if format <= 5 {
        rec.returns = (r[14] & 7) | (((r[14] >> 3) & 7) << 4);
        rec.class = r[15] & 0x1F;
        rec.rgb = match format {
            2 => rgb_at(20),
            3 | 5 => rgb_at(28),
            _ => [0; 3],
        };
    } else {
        rec.returns = r[14];
        rec.class = r[16];
        if matches!(format, 7 | 8 | 10) {
            rec.rgb = rgb_at(30);
        }
    }
    rec
}

/// A piece of the point data decoded as one: a block of raw records, a LAZ chunk or a COPC
/// node.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Unit {
    pub(crate) offset: usize,
    pub(crate) len: usize,
    pub(crate) count: u64,
    /// Index of its first point among all points read (for even subsampling).
    pub(crate) first: u64,
}

#[derive(Clone, Debug)]
enum Coding {
    Raw,
    Laz(Laz),
}

/// What [`crate::open`] learned and the work left.
pub(crate) struct Plan {
    header: Header,
    coding: Coding,
    units: Vec<Unit>,
    /// Keep every `stride`-th point.
    stride: u64,
    info: CloudInfo,
    origin: [f64; 3],
}

impl Plan {
    pub(crate) fn new(bytes: &[u8], limits: &Limits) -> Result<Plan> {
        let h = Header::parse(bytes)?;
        let vlrs = h.vlrs(bytes);
        let mut info = CloudInfo {
            format: Some(if h.compressed {
                PointFormat::Laz
            } else {
                PointFormat::Las
            }),
            version: Some(format!("{}.{}", h.version.0, h.version.1)),
            point_format: Some(h.format),
            crs: crs::name(&vlrs),
            total: h.count,
            channels: h.channels(),
            ..CloudInfo::default()
        };
        if h.count > limits.max_declared {
            return Err(Error::TooLarge(format!("{} points", h.count)));
        }
        if h.data_offset > bytes.len() || h.data_offset < h.header_size {
            return malformed(format!("LAS point data at {}", h.data_offset));
        }
        let (coding, mut units) = if h.compressed {
            let Some(v) = vlrs
                .iter()
                .find(|v| v.user == "laszip encoded" && v.id == 22204)
            else {
                return malformed("LAZ without its LASzip VLR");
            };
            let laz = Laz::parse(v.data, h.record_len)?;
            info.coding = Some(laz.describe());
            let units = match copc::nodes(bytes, &vlrs)? {
                Some(nodes) => {
                    info.copc = true;
                    // Whole levels while they fit the cap (at least the first).
                    let mut keep = 0;
                    let mut sum = 0u64;
                    let mut i = 0;
                    while i < nodes.len() {
                        let level = nodes[i].0;
                        let end = nodes[i..]
                            .iter()
                            .position(|(l, _)| *l != level)
                            .map_or(nodes.len(), |k| i + k);
                        let add: u64 = nodes[i..end].iter().map(|(_, u)| u.count).sum();
                        if keep > 0 && sum + add > limits.max_points {
                            break;
                        }
                        sum += add;
                        keep = end;
                        i = end;
                    }
                    nodes.into_iter().take(keep).map(|(_, u)| u).collect()
                }
                None => laz::chunks(bytes, h.data_offset, h.count, &laz)?
                    .into_iter()
                    .map(|c| Unit {
                        offset: c.offset,
                        len: c.len,
                        count: c.count,
                        first: 0,
                    })
                    .collect(),
            };
            (Coding::Laz(laz), units)
        } else {
            let room = (bytes.len() - h.data_offset) / h.record_len;
            let n = h.count.min(room as u64);
            let mut units = Vec::new();
            let mut i = 0;
            while i < n {
                let c = BLOCK.min(n - i);
                units.push(Unit {
                    offset: h.data_offset + i as usize * h.record_len,
                    len: c as usize * h.record_len,
                    count: c,
                    first: 0,
                });
                i += c;
            }
            (Coding::Raw, units)
        };
        let mut first = 0;
        for u in &mut units {
            u.first = first;
            first += u.count;
        }
        let total_read = first;
        let stride = total_read.div_ceil(limits.max_points.max(1)).max(1);
        if !info.copc {
            info.total = info.total.max(total_read);
        }
        let ext = h.extent;
        let finite = ext.iter().all(|v| v.is_finite()) && ext[0] <= ext[3];
        let origin = if finite {
            [0, 1, 2].map(|i| ((ext[i] + ext[i + 3]) / 2.0).round())
        } else {
            h.offset
        };
        if finite {
            info.extent = Some(ext);
        }
        Ok(Plan {
            header: h,
            coding,
            units,
            stride,
            info,
            origin,
        })
    }

    pub(crate) fn loading(&self) -> Loading {
        let mut bounds = Bounds::EMPTY;
        if let Some(e) = self.info.extent {
            for c in [[e[0], e[1], e[2]], [e[3], e[4], e[5]]] {
                bounds.add([0, 1, 2].map(|i| (c[i] - self.origin[i]) as f32));
            }
        }
        let total: u64 = self.units.iter().map(|u| u.count).sum();
        Loading::new(
            self.info.clone(),
            self.origin,
            bounds,
            total.div_ceil(self.stride),
        )
    }

    /// Decodes a unit's records.
    fn decode(&self, bytes: &[u8], u: &Unit, out: &mut Vec<Rec>) -> Result<()> {
        let Some(b) = bytes.get(u.offset..u.offset + u.len) else {
            return malformed("LAS points past the end of the file");
        };
        match &self.coding {
            Coding::Raw => {
                out.extend(
                    b.chunks_exact(self.header.record_len)
                        .map(|r| record(self.header.format, r)),
                );
                Ok(())
            }
            Coding::Laz(l) => {
                let count = usize::try_from(u.count).unwrap_or(usize::MAX);
                // A chunk can't hold more points than it has bytes (with some slack for
                // runs of identical points, which take next to nothing).
                if count > u.len.saturating_mul(64).max(1 << 16) {
                    return malformed(format!("LAZ chunk of {} bytes with {count} points", u.len));
                }
                laz::decode(&l.layout, b, count, out)
            }
        }
    }

    fn batch(&self, recs: &[Rec], first: u64, colour: &Colour) -> Batch {
        let s = self.stride;
        let skip = ((s - first % s) % s) as usize;
        let n = recs.len().saturating_sub(skip).div_ceil(s as usize);
        let mut b = Batch {
            positions: Vec::with_capacity(n),
            attrs: Vec::with_capacity(n),
        };
        for r in recs.iter().skip(skip).step_by(s as usize) {
            let p = self.header.coordinate(r);
            b.positions
                .push([0, 1, 2].map(|i| (p[i] - self.origin[i]) as f32));
            b.attrs.push([
                colour.pack(r.rgb) | (u32::from(r.class) << 24),
                u32::from(r.intensity) | (u32::from(r.returns) << 16),
            ]);
        }
        b
    }

    pub(crate) fn run(self, bytes: &[u8], loading: &Loading) -> Result<crate::PointCloud> {
        if self.units.is_empty() {
            return Err(Error::Empty);
        }
        let has_rgb = self.info.channels.rgb;
        // The first unit tells 8-bit colour (stored unscaled, as many writers do) from 16-bit,
        // and the intensity's range while the rest loads.
        let mut recs = Vec::new();
        self.decode(bytes, &self.units[0], &mut recs)?;
        let colour = Colour::of(has_rgb, recs.iter().map(|r| r.rgb));
        loading.set_intensity_range(build::intensity_range(recs.iter().map(|r| r.intensity)));
        let slots: Vec<Mutex<Option<Arc<Batch>>>> =
            self.units.iter().map(|_| Mutex::new(None)).collect();
        let b0 = Arc::new(self.batch(&recs, self.units[0].first, &colour));
        loading.push(b0.clone());
        *slots[0].lock() = Some(b0);
        drop(recs);

        // The rest in order of the units (COPC levels coarse to fine), each thread taking the
        // next one.
        let next = AtomicUsize::new(1);
        let failed: Mutex<Option<Error>> = Mutex::new(None);
        let threads = rayon::current_num_threads().max(1);
        rayon::scope(|s| {
            for _ in 0..threads {
                s.spawn(|_| {
                    let mut recs = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(u) = self.units.get(i) else { break };
                        if loading.cancelled() {
                            *failed.lock() = Some(Error::Cancelled);
                            break;
                        }
                        if failed.lock().is_some() {
                            break;
                        }
                        recs.clear();
                        if let Err(e) = self.decode(bytes, u, &mut recs) {
                            failed.lock().get_or_insert(e);
                            break;
                        }
                        let b = Arc::new(self.batch(&recs, u.first, &colour));
                        loading.push(b.clone());
                        *slots[i].lock() = Some(b);
                    }
                });
            }
        });
        if let Some(e) = failed.into_inner() {
            return Err(e);
        }
        let n: usize = slots
            .iter()
            .map(|s| s.lock().as_ref().map_or(0, |b| b.positions.len()))
            .sum();
        let mut positions = Vec::with_capacity(n);
        let mut attrs = Vec::with_capacity(n);
        for s in slots {
            if let Some(b) = s.into_inner() {
                positions.extend_from_slice(&b.positions);
                attrs.extend_from_slice(&b.attrs);
            }
        }
        build::finish(positions, attrs, self.info, self.origin)
    }
}
