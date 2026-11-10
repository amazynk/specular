//! Nikon NIS-Elements ND2.
//!
//! **Chunked layout (versions 2 and 3).** The file is a run of chunks, each a 16-byte header
//! (magic `0x0ABECEDA`, name length, data length, little-endian), its name and its data. The
//! last 40 bytes hold a signature and the offset of the chunk map, whose entries are
//! `name!` followed by the chunk's offset and size. Metadata chunks (`ImageAttributesLV!`,
//! `ImageMetadataLV!`, `ImageMetadataSeqLV|0!`, `ImageTextInfoLV!`; without `LV` in version
//! 2) are CLX Lite records or their XML form ([`crate::clx`]). Each frame is an
//! `ImageDataSeq|n!` chunk: an 8-byte timestamp, then the pixels, rows `uiWidthBytes` apart
//! with every component of every channel interleaved per pixel, or the same bytes
//! zlib-compressed (`eCompression` 0, lossless).
//!
//! Frames run through the experiment's loops in order, the last loop fastest (the
//! nesting of NIS-Elements' `ppNextLevelEx` levels). Spectral loops become channels, not
//! frames.
//!
//! **Legacy layout (version 1).** A JPEG 2000 family box file; the last 40 bytes give the
//! position of a box map listing every box with its LIM type. `jp2h` holds the size and
//! depth, `VIMD` (one per frame) the frame metadata with the picture planes (channels),
//! `AIM1` / `AIMD` the experiment, `TINF` the text info, and `LUNK` one JPEG 2000 codestream
//! per channel per frame.

use crate::clx::{self, Value};
use crate::{Error, Result, Sample};

const CHUNK_MAGIC: u32 = 0x0ABE_CEDA;
const FILE_SIGNATURE: &[u8] = b"ND2 FILE SIGNATURE CHUNK NAME01!";
const MAP_SIGNATURE: &[u8] = b"ND2 CHUNK MAP SIGNATURE 0000001!";
const FILEMAP_SIGNATURE: &[u8] = b"ND2 FILEMAP SIGNATURE NAME 0001!";
const LEGACY_MAP_SIGNATURE: &[u8] = b"LABORATORY IMAGING ND BOX MAP 00";
/// Largest inflated frame.
const MAX_FRAME: usize = 4 << 30;
/// Most components in one frame or one picture plane.
const MAX_COMPONENTS: usize = 1 << 12;

/// What a loop of the experiment steps through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopKind {
    /// Time points (a time loop or a multi-phase time loop).
    Time,
    /// Stage (XY) positions.
    Position,
    /// Focus (Z) planes.
    Z,
    /// Anything else NIS-Elements loops over.
    Custom,
}

/// One loop of the experiment.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub kind: LoopKind,
    pub count: usize,
    /// Z step (µm) or time period (ms), when known and non-zero.
    pub step: Option<f64>,
    /// Point names of a position loop (empty strings where unnamed).
    pub labels: Vec<String>,
}

/// One channel (a picture plane): `components` samples per pixel (3 for RGB cameras).
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub name: String,
    /// Display colour as the file gives it (`uiColor`, R in the low byte).
    pub colour: Option<[u8; 3]>,
    pub components: usize,
    /// Emission wavelength (nm), when the file gives one.
    pub emission: Option<f64>,
}

#[derive(Clone, Debug)]
enum Layout {
    Chunked {
        /// Per frame: the chunk's data range (from the timestamp on).
        frames: Vec<Option<(usize, usize)>>,
        width_bytes: usize,
        compressed: bool,
    },
    Legacy {
        /// Box offsets of the `LUNK` codestreams, channel-major per frame.
        lunk: Vec<usize>,
    },
}

/// A parsed ND2 file: everything except the pixels.
#[derive(Clone, Debug)]
pub struct Nd2 {
    /// `(major, minor)`: 1.0 for the legacy JPEG 2000 layout.
    pub version: (u32, u32),
    pub width: usize,
    pub height: usize,
    pub sample: Sample,
    /// Significant bits per component.
    pub bits: u32,
    /// Components per pixel in a frame (every channel's).
    pub components: usize,
    /// Channels in component order; their components add up to `components`.
    pub channels: Vec<Channel>,
    /// The experiment's loops, outermost first.
    pub loops: Vec<Loop>,
    /// Frames the file holds.
    pub frames: usize,
    /// Pixel size, µm (`dCalibration`), when calibrated.
    pub calibration: Option<f64>,
    pub objective: Option<String>,
    pub magnification: Option<f64>,
    pub numerical_aperture: Option<f64>,
    /// Text info items by name ("Description", "Capturing", "Date", "Optics"…).
    pub text: Vec<(String, String)>,
    /// Other metadata, flattened, for an info panel.
    pub meta: Vec<(String, String)>,
    layout: Layout,
}

fn bad(m: &str) -> Error {
    Error::Malformed(format!("ND2: {m}"))
}

/// Whether `head` starts like an ND2 file (either layout).
pub fn sniff(head: &[u8]) -> bool {
    let chunked = crate::le_u32(head, 0) == Some(CHUNK_MAGIC)
        && head.get(16..16 + FILE_SIGNATURE.len()) == Some(FILE_SIGNATURE);
    chunked || is_legacy(head)
}

/// Whether a whole file is a legacy (JPEG 2000) ND2: a JP2 signature box and Nikon's box map
/// at the end. Plain JPEG 2000 files have the first but not the second.
pub fn is_legacy_file(data: &[u8]) -> bool {
    is_legacy(data) && legacy_map(data).is_some()
}

/// Whether `head` starts like a chunk-map ND2 (versions 2 and 3).
pub fn sniff_chunked(head: &[u8]) -> bool {
    crate::le_u32(head, 0) == Some(CHUNK_MAGIC)
        && head.get(16..16 + FILE_SIGNATURE.len()) == Some(FILE_SIGNATURE)
}

fn is_legacy(head: &[u8]) -> bool {
    crate::be_u32(head, 0) == Some(12) && head.get(4..8) == Some(b"jP  ".as_slice())
}

/// Parse an ND2 file held in memory.
pub fn parse(data: &[u8]) -> Result<Nd2> {
    if crate::le_u32(data, 0) == Some(CHUNK_MAGIC) {
        parse_chunked(data)
    } else if is_legacy(data) && legacy_map(data).is_some() {
        parse_legacy(data)
    } else {
        Err(Error::NotNd2)
    }
}

/// A chunk's name and data range at `pos`.
fn chunk_at(data: &[u8], pos: usize) -> Option<(&[u8], usize, usize)> {
    if crate::le_u32(data, pos)? != CHUNK_MAGIC {
        return None;
    }
    let nlen = crate::le_u32(data, pos + 4)? as usize;
    let dlen = usize::try_from(crate::le_u64(data, pos + 8)?).ok()?;
    let name = data.get(pos + 16..pos.checked_add(16)?.checked_add(nlen)?)?;
    let start = pos + 16 + nlen;
    Some((name, start, dlen))
}

/// Chunk names (including the `!`) and positions.
fn chunk_map(data: &[u8]) -> Vec<(Vec<u8>, usize)> {
    let mut out = Vec::new();
    let n = data.len();
    if n >= 40 && &data[n - 40..n - 8] == MAP_SIGNATURE {
        let at = crate::le_u64(data, n - 8).and_then(|v| usize::try_from(v).ok());
        if let Some((name, start, dlen)) = at.and_then(|a| chunk_at(data, a))
            && name.starts_with(FILEMAP_SIGNATURE)
        {
            let map = &data[start..start.saturating_add(dlen).min(n)];
            let mut p = 0;
            while p < map.len() {
                let Some(bang) = map[p..].iter().position(|&c| c == b'!') else {
                    break;
                };
                let name = &map[p..p + bang + 1];
                if name == MAP_SIGNATURE {
                    break;
                }
                let Some(off) = crate::le_u64(map, p + bang + 1) else {
                    break;
                };
                if let Ok(off) = usize::try_from(off) {
                    out.push((name.to_vec(), off));
                }
                p += bang + 1 + 16;
            }
            if !out.is_empty() {
                return out;
            }
        }
    }
    // No usable map: walk the chunks from the start.
    let mut pos = 0usize;
    while let Some((name, start, dlen)) = chunk_at(data, pos) {
        let name = name.split(|&c| c == 0).next().unwrap_or(name);
        out.push((name.to_vec(), pos));
        match start.checked_add(dlen) {
            Some(next) if next > pos && next <= n => pos = next,
            _ => break,
        }
    }
    out
}

fn chunk_data<'a>(data: &'a [u8], map: &[(Vec<u8>, usize)], name: &[u8]) -> Option<&'a [u8]> {
    let &(_, pos) = map.iter().find(|(n, _)| n == name)?;
    let (_, start, dlen) = chunk_at(data, pos)?;
    data.get(start..start.checked_add(dlen)?)
}

fn decode_chunk(data: &[u8], map: &[(Vec<u8>, usize)], names: &[&[u8]]) -> Option<Value> {
    names
        .iter()
        .find_map(|n| chunk_data(data, map, n))
        .and_then(|c| clx::parse_any(c).ok())
}

fn parse_chunked(data: &[u8]) -> Result<Nd2> {
    let head = data.get(..112).ok_or(Error::Truncated("ND2 header"))?;
    if &head[16..48] != FILE_SIGNATURE {
        return Err(Error::NotNd2);
    }
    // "Ver3.0" in the signature chunk's data.
    let digit = |i: usize| char::from(head[48 + i]).to_digit(10);
    let version = (digit(3).unwrap_or(0), digit(5).unwrap_or(0));
    let map = chunk_map(data);
    let attrs = decode_chunk(data, &map, &[b"ImageAttributesLV!", b"ImageAttributes!"])
        .ok_or_else(|| bad("no image attributes"))?;
    let attrs = attrs.unwrap_single();
    let width = attrs.u64("uiWidth").unwrap_or(0) as usize;
    let height = attrs.u64("uiHeight").unwrap_or(0) as usize;
    let components = attrs.u64("uiComp").unwrap_or(1).max(1);
    let components = usize::try_from(components)
        .ok()
        .filter(|&c| c <= MAX_COMPONENTS)
        .ok_or_else(|| Error::Unsupported(format!("{components} components per pixel")))?;
    let bpc = attrs.u64("uiBpcInMemory").unwrap_or(16) as u32;
    let bits = attrs.u64("uiBpcSignificant").unwrap_or(u64::from(bpc)) as u32;
    let sample = match bpc {
        8 => Sample::U8,
        16 => Sample::U16,
        32 => Sample::F32,
        b => return Err(Error::Unsupported(format!("{b} bits per component"))),
    };
    let row = width
        .checked_mul(components * sample.bytes())
        .ok_or(Error::TooLarge)?;
    let width_bytes = attrs.u64("uiWidthBytes").map_or(row, |w| w as usize);
    let compressed = match attrs.u64("eCompression") {
        Some(0) => true,
        Some(1) => return Err(Error::Unsupported("lossy-compressed ND2 frames".into())),
        _ => false,
    };
    if width == 0 || height == 0 {
        return Err(bad("empty image"));
    }
    // The row, the plane and the whole frame (all within `width_bytes × height`) must not
    // overflow.
    if width
        .checked_mul(height)
        .is_none_or(|n| n as u64 > crate::MAX_PLANE)
        || width_bytes.checked_mul(height).is_none()
    {
        return Err(Error::TooLarge);
    }
    if width_bytes < row {
        return Err(bad("row stride"));
    }

    let picture = decode_chunk(
        data,
        &map,
        &[b"ImageMetadataSeqLV|0!", b"ImageMetadataSeq|0!"],
    );
    let picture = picture.as_ref().map(|p| p.unwrap_single().clone());
    let experiment = decode_chunk(data, &map, &[b"ImageMetadataLV!", b"ImageMetadata!"]);
    let experiment = experiment.as_ref().map(|e| e.unwrap_single().clone());
    let text_info = decode_chunk(data, &map, &[b"ImageTextInfoLV!", b"ImageTextInfo!"]);

    let mut loops = Vec::new();
    if let Some(e) = &experiment {
        load_experiment(e, 0, &mut loops, 0);
    }
    let loops: Vec<Loop> = loops.into_iter().map(|(_, l)| l).collect();

    let mut frames: Vec<Option<(usize, usize)>> = Vec::new();
    for (name, pos) in &map {
        let Some(rest) = name.strip_prefix(b"ImageDataSeq|") else {
            continue;
        };
        let Some(i) = std::str::from_utf8(rest.strip_suffix(b"!").unwrap_or(rest))
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        // Every frame has a chunk header of 16 bytes, so a file of n bytes holds fewer than
        // n / 16 of them.
        if i >= 1 << 26 || i >= data.len() / 16 {
            continue;
        }
        if let Some((_, start, dlen)) = chunk_at(data, *pos) {
            if frames.len() <= i {
                frames.resize(i + 1, None);
            }
            frames[i] = Some((start, dlen));
        }
    }
    let seq = attrs.u64("uiSequenceCount").unwrap_or(frames.len() as u64) as usize;
    let count = frames.len().max(seq.min(frames.len().max(1)));

    let channels = picture
        .as_ref()
        .map(|p| channels_of(p, components))
        .unwrap_or_default();
    let channels = reconcile_channels(channels, components);

    let calibration = picture.as_ref().and_then(|p| {
        let c = p.f64("dCalibration")?;
        (c > 0.0 && p.get("bCalibrated").and_then(Value::as_bool) != Some(false)).then_some(c)
    });
    let mut text = Vec::new();
    if let Some(t) = &text_info {
        text_items(t.unwrap_single(), &mut text);
    }
    let mut meta = Vec::new();
    if let Some(p) = &picture {
        for k in [
            "dObjectiveMag",
            "dObjectiveNA",
            "dZoom",
            "dRefractIndex1",
            "dProjectiveMag",
            "dPinholeRadius",
            "dXPos",
            "dYPos",
            "dZPos",
        ] {
            if let Some(v) = p.f64(k) {
                meta.push((clx::bare(k).to_owned(), v.to_string()));
            }
        }
    }
    meta.push(("Version".into(), format!("{}.{}", version.0, version.1)));
    meta.push((
        "Compression".into(),
        if compressed {
            "lossless (zlib)"
        } else {
            "none"
        }
        .into(),
    ));
    let pos = |k: &str| picture.as_ref().and_then(|p| p.f64(k)).filter(|v| *v > 0.0);
    Ok(Nd2 {
        version,
        width,
        height,
        sample,
        bits,
        components,
        channels,
        loops: fit_loops(loops, count),
        frames: count,
        calibration,
        objective: picture
            .as_ref()
            .and_then(|p| p.text("wsObjectiveName"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        magnification: pos("dObjectiveMag"),
        numerical_aperture: pos("dObjectiveNA"),
        text,
        meta,
        layout: Layout::Chunked {
            frames,
            width_bytes,
            compressed,
        },
    })
}

/// The loops' counts must cover the frames: when they do not (a file stopped early, or loops
/// this reader does not understand), the outermost loop is shortened, or one time-like axis
/// over every frame stands in.
fn fit_loops(mut loops: Vec<Loop>, frames: usize) -> Vec<Loop> {
    loops.retain(|l| l.count > 0);
    let product: usize = loops.iter().map(|l| l.count).fold(1, usize::saturating_mul);
    if product == frames || frames == 0 {
        return loops;
    }
    if product > frames && !loops.is_empty() {
        let inner: usize = loops[1..]
            .iter()
            .map(|l| l.count)
            .fold(1, usize::saturating_mul);
        if inner > 0 && frames.is_multiple_of(inner) && frames / inner > 0 {
            let outer = frames / inner;
            loops[0].count = outer;
            loops[0].labels.truncate(outer);
            return loops;
        }
    }
    if frames == 1 {
        return Vec::new();
    }
    vec![Loop {
        kind: LoopKind::Time,
        count: frames,
        step: None,
        labels: Vec::new(),
    }]
}

/// The children of a level that stands for a list (items in order).
fn list(v: &Value) -> Vec<&Value> {
    v.items().iter().map(|(_, v)| v).collect()
}

/// A loop parameters' level, unwrapping a one-item list.
fn loop_pars(e: &Value) -> Option<&Value> {
    let p = e.get("uLoopPars")?;
    match p.items() {
        [(n, inner)] if n.is_empty() || clx::bare(n).chars().all(|c| c.is_ascii_digit()) => {
            Some(inner)
        }
        _ => Some(p),
    }
}

fn valid_flags(v: Option<&Value>) -> Option<Vec<bool>> {
    match v? {
        Value::Bytes(b) => Some(b.iter().map(|&x| x != 0).collect()),
        l @ Value::Level(_) => Some(
            list(l)
                .into_iter()
                .map(|x| x.as_bool().unwrap_or(true))
                .collect(),
        ),
        _ => None,
    }
}

/// One loop of an experiment level: `None` for nothing to loop over, `Err(())` for a
/// spectral loop (channels, not frames).
fn single_loop(e: &Value) -> std::result::Result<Option<Loop>, ()> {
    let ty = e.u64("eType").unwrap_or(0);
    let Some(p) = loop_pars(e) else {
        return Ok(None);
    };
    let count = p.u64("uiCount").unwrap_or(0) as usize;
    let mk = |kind, count, step: Option<f64>| {
        Some(Loop {
            kind,
            count,
            step: step.filter(|s| s.is_finite() && *s != 0.0),
            labels: Vec::new(),
        })
    };
    Ok(match ty {
        1 if count > 0 => mk(LoopKind::Time, count, p.f64("dPeriod")),
        2 => {
            let points: Vec<&Value> = p.get("Points").map(list).unwrap_or_default();
            let valid = valid_flags(e.get("pItemValid"));
            if points.is_empty() {
                // Unnamed positions: counted, not listed (a hostile count costs nothing).
                let n = match valid {
                    Some(v) if !v.is_empty() => v.iter().take(count).filter(|&&ok| ok).count(),
                    _ => count,
                };
                return Ok(mk(LoopKind::Position, n, None));
            }
            let names: Vec<String> = {
                points
                    .iter()
                    .map(|pt| {
                        [pt.text("dPosName"), pt.text("pPosName")]
                            .into_iter()
                            .flatten()
                            .find(|s| !s.is_empty())
                            .unwrap_or("")
                            .to_owned()
                    })
                    .collect()
            };
            // Points without a validity flag are dropped, as NIS-Elements does.
            let names = match valid {
                Some(valid) if !valid.is_empty() => names
                    .into_iter()
                    .zip(valid)
                    .filter_map(|(n, ok)| ok.then_some(n))
                    .collect(),
                _ => names,
            };
            let mut l = mk(LoopKind::Position, names.len(), None);
            if let Some(l) = &mut l {
                l.labels = names;
            }
            l
        }
        4 => {
            let (lo, hi) = (
                p.f64("dZLow").unwrap_or(0.0),
                p.f64("dZHigh").unwrap_or(0.0),
            );
            let mut step = p.f64("dZStep").unwrap_or(0.0).abs();
            if step == 0.0 && count > 1 {
                step = (hi - lo).abs() / (count - 1) as f64;
            }
            mk(LoopKind::Z, count, Some(step))
        }
        8 => {
            let periods: Vec<&Value> = p.get("pPeriod").map(list).unwrap_or_default();
            let valid = valid_flags(p.get("pPeriodValid")).unwrap_or_default();
            let mut n = 0;
            let mut period = None;
            for (i, per) in periods.iter().enumerate() {
                if !valid.get(i).copied().unwrap_or(false) {
                    continue;
                }
                let c = per.u64("uiCount").unwrap_or(0) as usize;
                if c > 0 {
                    n = c.saturating_add(n);
                    period = period.or(per.f64("dPeriod"));
                }
            }
            mk(LoopKind::Time, n, period)
        }
        6 => {
            let n = p
                .get("pPlanes")
                .and_then(|pl| pl.u64("uiCount"))
                .or(p.u64("uiCount"))
                .unwrap_or(0);
            return if n > 0 { Err(()) } else { Ok(None) };
        }
        7 if count > 0 => mk(LoopKind::Custom, count, None),
        _ => None,
    })
}

/// Walk `ppNextLevelEx` the way NIS-Elements nests loops: each level's loop joins when it is
/// deeper than the last one kept; a same-level sibling of the same type replaces it when
/// longer. Spectral loops do not count as a level.
fn load_experiment(e: &Value, level: i32, dest: &mut Vec<(i32, Loop)>, depth: usize) {
    if depth > 32 {
        return;
    }
    let mut level = level;
    match single_loop(e) {
        Ok(Some(l)) if l.count > 0 => match dest.last() {
            None => dest.push((level, l)),
            Some((lv, _)) if *lv < level => dest.push((level, l)),
            Some((lv, prev)) => {
                if *lv == level && prev.kind == l.kind && prev.count < l.count {
                    let n = dest.len() - 1;
                    dest[n] = (level, l);
                }
            }
        },
        Ok(_) => return,
        Err(()) => level -= 1,
    }
    if let Some(next) = e.get("ppNextLevelEx") {
        for (_, item) in next.items() {
            load_experiment(item, level + 1, dest, depth + 1);
        }
    }
}

fn channels_of(p: &Value, components: usize) -> Vec<Channel> {
    let planes = p.get("sPicturePlanes");
    let Some(planes) = planes else {
        return Vec::new();
    };
    let list_v = planes.get("sPlaneNew").or_else(|| planes.get("sPlane"));
    let Some(list_v) = list_v else {
        return Vec::new();
    };
    list(list_v)
        .into_iter()
        .map(|pl| {
            let colour = pl.u64("uiColor").map(|c| {
                let c = c as u32;
                [(c & 255) as u8, (c >> 8 & 255) as u8, (c >> 16 & 255) as u8]
            });
            let emission = pl
                .path(&["pFluorescentProbe", "m_EmissionSpectrum"])
                .and_then(spectrum_peak)
                .or_else(|| {
                    pl.path(&["pFilterPath", "m_pFilter"])
                        .and_then(|f| f.items().first())
                        .and_then(|(_, f)| f.get("m_EmissionSpectrum"))
                        .and_then(spectrum_peak)
                });
            Channel {
                name: pl.text("sDescription").unwrap_or("").trim().to_owned(),
                colour,
                components: (pl.u64("uiCompCount").unwrap_or(1).max(1) as usize)
                    .min(MAX_COMPONENTS),
                emission,
            }
        })
        .take(components.max(1))
        .collect()
}

/// The wavelength at a spectrum's highest transmission.
fn spectrum_peak(s: &Value) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    for (_, p) in s.get("pPoint")?.items() {
        let (t, wl) = (
            p.f64("dTValue").unwrap_or(0.0),
            p.f64("dWavelength").unwrap_or(0.0),
        );
        if best.is_none_or(|(bt, _)| t > bt) {
            best = Some((t, wl));
        }
    }
    best.map(|b| b.1).filter(|w| *w > 0.0)
}

/// Channels whose components add up to the frame's; otherwise one grey channel per
/// component (or RGB components when there are three in one plane).
fn reconcile_channels(ch: Vec<Channel>, components: usize) -> Vec<Channel> {
    if !ch.is_empty() && ch.iter().map(|c| c.components).sum::<usize>() == components {
        return ch;
    }
    if ch.len() == components {
        return ch
            .into_iter()
            .map(|c| Channel { components: 1, ..c })
            .collect();
    }
    (0..components)
        .map(|i| {
            Channel {
                name: String::new(),
                colour: None,
                components: 1,
                emission: None,
            }
            .with_index(i)
        })
        .collect()
}

impl Channel {
    fn with_index(mut self, i: usize) -> Channel {
        self.name = format!("Channel {}", i + 1);
        self
    }
}

fn text_items(t: &Value, out: &mut Vec<(String, String)>) {
    const NAMES: [&str; 15] = [
        "Image ID",
        "Type",
        "Group",
        "Sample ID",
        "Author",
        "Description",
        "Capturing",
        "Sampling",
        "Location",
        "Date",
        "Conclusion",
        "Info 1",
        "Info 2",
        "Optics",
        "Application version",
    ];
    for (k, v) in t.items() {
        let Some(s) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let k = clx::bare(k);
        let name = k
            .strip_prefix("TextInfoItem_")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| NAMES.get(n))
            .map_or_else(|| k.to_owned(), |s| (*s).to_owned());
        out.push((name, s.replace("\r\n", "\n")));
    }
}

/// A legacy box map entry: box type, LIM type, offset.
type BoxEntry = ([u8; 4], [u8; 4], usize);

/// The legacy box map: `(box type, LIM type, offset)` per box.
fn legacy_map(data: &[u8]) -> Option<Vec<BoxEntry>> {
    let n = data.len();
    if n < 40 || &data[n - 40..n - 8] != LEGACY_MAP_SIGNATURE {
        return None;
    }
    let back = usize::try_from(crate::le_u64(data, n - 8)?).ok()?;
    let start = n.checked_sub(back)?;
    let count = crate::be_u32(data, start)? as usize;
    let mut out = Vec::with_capacity(count.min(1 << 16));
    for i in 0..count {
        let e = start + 4 + i * 16;
        let Some(entry) = data.get(e..e + 12) else {
            break;
        };
        let bt: [u8; 4] = entry[0..4].try_into().ok()?;
        let lt: [u8; 4] = entry[4..8].try_into().ok()?;
        let off = crate::le_u32(entry, 8)? as usize;
        out.push((bt, lt, off));
    }
    Some(out)
}

/// A legacy box's payload at `off`.
fn legacy_box(data: &[u8], off: usize) -> Option<&[u8]> {
    let len = crate::be_u32(data, off)? as usize;
    let (start, end) = if len == 1 {
        let l = u64::from_be_bytes(data.get(off + 8..off + 16)?.try_into().ok()?);
        (off + 16, off.checked_add(usize::try_from(l).ok()?)?)
    } else if len == 0 {
        (off + 8, data.len())
    } else {
        (off + 8, off.checked_add(len)?)
    };
    data.get(start..end.min(data.len()))
}

fn parse_legacy(data: &[u8]) -> Result<Nd2> {
    let map = legacy_map(data).ok_or(Error::NotNd2)?;
    let of = |t: &[u8; 4]| -> Vec<usize> {
        map.iter()
            .filter(|(bt, lt, _)| {
                if matches!(bt, b"jP  " | b"ftyp" | b"jp2h") {
                    bt == t
                } else {
                    lt == t
                }
            })
            .map(|e| e.2)
            .collect()
    };
    let jp2h = *of(b"jp2h").first().ok_or_else(|| bad("no jp2h box"))?;
    let ihdr = data
        .get(jp2h + 8..jp2h + 8 + 22)
        .ok_or(Error::Truncated("ihdr"))?;
    if &ihdr[4..8] != b"ihdr" {
        return Err(bad("no ihdr box"));
    }
    let height = crate::be_u32(ihdr, 8).unwrap_or(0) as usize;
    let width = crate::be_u32(ihdr, 12).unwrap_or(0) as usize;
    let bpc = u32::from(ihdr[18] & 0x7f) + 1;
    if width == 0 || height == 0 {
        return Err(bad("empty image"));
    }
    if (width * height) as u64 > crate::MAX_PLANE {
        return Err(Error::TooLarge);
    }
    let xml = |t: &[u8; 4], i: usize| -> Option<Value> {
        let off = *of(t).get(i)?;
        clx::parse_xml(legacy_box(data, off)?).ok()
    };
    let frame0 = xml(b"VIMD", 0);
    let frame0 = frame0.as_ref().map(|v| v.unwrap_single().clone());
    let lunk = of(b"LUNK");
    let vcal = of(b"VCAL").len();

    // Picture planes: `PicturePlanes.Plane` items with `OpticalConfigName`, `Color`, `CompCount`.
    let mut channels = Vec::new();
    let mut ncomp = 1;
    if let Some(pp) = frame0.as_ref().and_then(|f| f.get("PicturePlanes")) {
        ncomp = (pp.u64("CompCount").unwrap_or(1).max(1) as usize).min(MAX_COMPONENTS);
        if let Some(planes) = pp.get("Plane") {
            for pl in list(planes) {
                let colour = pl.u64("Color").map(|c| {
                    let c = c as u32;
                    [(c & 255) as u8, (c >> 8 & 255) as u8, (c >> 16 & 255) as u8]
                });
                channels.push(Channel {
                    name: pl
                        .text("OpticalConfigName")
                        .or_else(|| pl.text("Description"))
                        .unwrap_or("")
                        .trim()
                        .to_owned(),
                    colour,
                    components: (pl.u64("CompCount").unwrap_or(1).max(1) as usize)
                        .min(MAX_COMPONENTS),
                    emission: None,
                });
            }
        }
    }
    if channels.is_empty() {
        channels.push(Channel {
            name: String::new(),
            colour: None,
            components: ncomp,
            emission: None,
        });
    }
    let codestreams = channels.len();
    let components: usize = channels.iter().map(|c| c.components).sum();
    let frames = if vcal > 0 {
        vcal.min(lunk.len() / codestreams.max(1))
    } else {
        lunk.len() / codestreams.max(1)
    };
    if frames == 0 {
        return Err(Error::NoImage);
    }

    // Text info: the "Metadata:" description carries the dimensions, e.g.
    // "Dimensions: T(3) x XY(2) x λ(2) x Z(5)".
    let mut text = Vec::new();
    let mut dims: Vec<(String, usize)> = Vec::new();
    if let Some(t) = xml(b"TINF", 0) {
        let mut items = Vec::new();
        t.unwrap_single().flatten("", 64, &mut items);
        for (_, v) in items {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            if !v.starts_with("Metadata:") {
                text.push(("Info".to_owned(), v.replace("\r\n", "\n")));
                continue;
            }
            if let Some(d) = v.lines().find_map(|l| l.trim().strip_prefix("Dimensions:")) {
                dims = d
                    .split('x')
                    .filter_map(|part| {
                        let part = part.trim();
                        let (name, rest) = part.split_once('(')?;
                        let n = rest.trim_end_matches(')').trim().parse().ok()?;
                        Some((name.trim().trim_end_matches('\'').to_owned(), n))
                    })
                    .collect();
            }
            text.push(("Description".to_owned(), v.replace("\r\n", "\n")));
        }
    }
    let loops: Vec<Loop> = dims
        .iter()
        .filter_map(|(name, n)| {
            let kind = match name.as_str() {
                "T" => LoopKind::Time,
                "XY" => LoopKind::Position,
                "Z" => LoopKind::Z,
                // Channels are planes of a frame, not frames.
                "λ" | "C" => return None,
                _ => LoopKind::Custom,
            };
            Some(Loop {
                kind,
                count: *n,
                step: None,
                labels: Vec::new(),
            })
        })
        .collect();
    let calibration = frame0
        .as_ref()
        .and_then(|f| f.f64("Calibration"))
        .filter(|c| *c > 0.0);
    let pos = |k: &str| frame0.as_ref().and_then(|f| f.f64(k)).filter(|v| *v > 0.0);
    Ok(Nd2 {
        version: (1, 0),
        width,
        height,
        sample: if bpc <= 8 { Sample::U8 } else { Sample::U16 },
        bits: bpc,
        components,
        channels,
        loops: fit_loops(loops, frames),
        frames,
        calibration,
        objective: frame0
            .as_ref()
            .and_then(|f| f.text("ObjectiveName"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        magnification: pos("ObjectiveMag"),
        numerical_aperture: pos("ObjectiveNA"),
        text,
        meta: vec![("Version".into(), "1.0 (JPEG 2000)".into())],
        layout: Layout::Legacy { lunk },
    })
}

impl Nd2 {
    pub fn is_legacy(&self) -> bool {
        matches!(self.layout, Layout::Legacy { .. })
    }

    /// The frame number of a position on each loop (outermost first).
    pub fn frame_index(&self, pos: &[usize]) -> Option<usize> {
        let mut f = 0usize;
        for (k, l) in self.loops.iter().enumerate() {
            let i = pos.get(k).copied().unwrap_or(0);
            if i >= l.count {
                return None;
            }
            f = f.checked_mul(l.count)?.checked_add(i)?;
        }
        Some(f)
    }

    /// Text info by name.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.text
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Read component `component` (across every channel's components) of frame `frame` into
    /// `out` (`width × height` values, rows top-down). Returns whether data was missing (a
    /// frame the file does not hold reads as zeros).
    pub fn read_plane(
        &self,
        data: &[u8],
        frame: usize,
        component: usize,
        out: &mut [f32],
    ) -> Result<bool> {
        let (w, h) = (self.width, self.height);
        if out.len() < w * h || component >= self.components {
            return Err(bad("plane request"));
        }
        out[..w * h].fill(0.0);
        match &self.layout {
            Layout::Chunked {
                frames,
                width_bytes,
                compressed,
            } => {
                let Some(&Some((start, len))) = frames.get(frame) else {
                    return Ok(true);
                };
                let chunk = &data[start.min(data.len())..start.saturating_add(len).min(data.len())];
                let short = chunk.len() < len;
                let body = chunk.get(8..).unwrap_or(&[]);
                let need = width_bytes * (h - 1) + w * self.components * self.sample.bytes();
                let inflated;
                let px: &[u8] = if *compressed {
                    inflated =
                        specular_png::inflate_zlib(body, need.max(*width_bytes * h).min(MAX_FRAME))
                            .map_err(|_| bad("frame inflate"))?;
                    &inflated
                } else {
                    body
                };
                let bs = self.sample.bytes();
                let step = self.components * bs;
                let mut truncated = short;
                for y in 0..h {
                    let row = y * width_bytes + component * bs;
                    let o = &mut out[y * w..(y + 1) * w];
                    if row + (w - 1) * step + bs > px.len() {
                        truncated = true;
                        break;
                    }
                    for (x, v) in o.iter_mut().enumerate() {
                        let at = row + x * step;
                        *v = self.sample.read(&px[at..at + bs]);
                    }
                }
                Ok(truncated)
            }
            Layout::Legacy { lunk } => {
                // Which channel's codestream holds this component.
                let (mut ch, mut c) = (0, component);
                while ch < self.channels.len() && c >= self.channels[ch].components {
                    c -= self.channels[ch].components;
                    ch += 1;
                }
                let Some(&off) = lunk.get(frame * self.channels.len() + ch) else {
                    return Ok(true);
                };
                let Some(cs) = legacy_box(data, off) else {
                    return Ok(true);
                };
                let dec = specular_jp2::Decoder::new(cs).map_err(|e| bad(&e.to_string()))?;
                let comps = dec
                    .decode_components(0, None)
                    .map_err(|e| bad(&e.to_string()))?;
                let Some(comp) = comps.get(c) else {
                    return Ok(true);
                };
                let (cw, ch_) = (comp.width as usize, comp.height as usize);
                for y in 0..h.min(ch_) {
                    for x in 0..w.min(cw) {
                        out[y * w + x] = f32::from(comp.samples[y * cw + x]);
                    }
                }
                Ok(cw < w || ch_ < h)
            }
        }
    }
}

#[cfg(test)]
mod hostile {
    use super::*;
    use crate::clx::Value;

    /// A CLX Lite record of type `ty`.
    fn rec(ty: u8, name: &str, val: &[u8]) -> Vec<u8> {
        let mut v = vec![ty, (name.encode_utf16().count() + 1) as u8];
        for u in name.encode_utf16() {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(val);
        v
    }

    fn chunk(name: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = CHUNK_MAGIC.to_le_bytes().to_vec();
        v.extend_from_slice(&(name.len() as u32).to_le_bytes());
        v.extend_from_slice(&(body.len() as u64).to_le_bytes());
        v.extend_from_slice(name);
        v.extend_from_slice(body);
        v
    }

    /// A version 3 file with these attributes (`uiWidth` and so on, as u64) and more chunks.
    fn file(attrs: &[(&str, u64)], more: &[Vec<u8>]) -> Vec<u8> {
        let mut head = b"Ver3.0".to_vec();
        head.resize(112 - 16 - FILE_SIGNATURE.len(), 0);
        let mut f = chunk(FILE_SIGNATURE, &head);
        let a: Vec<u8> = attrs
            .iter()
            .flat_map(|(k, v)| rec(5, k, &v.to_le_bytes()))
            .collect();
        f.extend(chunk(b"ImageAttributesLV!", &a));
        for m in more {
            f.extend_from_slice(m);
        }
        f
    }

    /// `uiWidth` of 2^62 with 4096 float components: the row size overflows.
    #[test]
    fn row_size_overflowing() {
        let f = file(
            &[
                ("uiWidth", 1 << 62),
                ("uiHeight", 1),
                ("uiComp", 4096),
                ("uiBpcInMemory", 32),
            ],
            &[],
        );
        assert!(matches!(parse(&f), Err(Error::TooLarge)));
        // More components than any frame has.
        let f = file(&[("uiWidth", 1), ("uiHeight", 1), ("uiComp", 1 << 40)], &[]);
        assert!(matches!(parse(&f), Err(Error::Unsupported(_))));
        // A row stride whose frame overflows.
        let f = file(
            &[("uiWidth", 1), ("uiHeight", 2), ("uiWidthBytes", u64::MAX)],
            &[],
        );
        assert!(matches!(parse(&f), Err(Error::TooLarge)));
    }

    /// One frame chunk numbered 67 108 000: no frame list of that length.
    #[test]
    fn frame_number_far_past_the_file() {
        let f = file(
            &[("uiWidth", 2), ("uiHeight", 2), ("uiBpcInMemory", 8)],
            &[chunk(b"ImageDataSeq|67108000!", &[0; 12])],
        );
        let nd2 = parse(&f).unwrap();
        assert!(nd2.frames <= 1);
    }

    /// An XY loop of 2^40 positions without points.
    #[test]
    fn unnamed_positions_of_a_huge_count() {
        let pars = Value::Level(vec![("uiCount".into(), Value::UInt(1 << 40))]);
        let e = Value::Level(vec![
            ("eType".into(), Value::UInt(2)),
            ("uLoopPars".into(), pars),
        ]);
        let l = single_loop(&e).unwrap().unwrap();
        assert_eq!(l.count, 1 << 40);
        assert!(l.labels.is_empty());
    }

    /// A chunk map entry at the end of the address space.
    #[test]
    fn chunk_at_the_end_of_the_address_space() {
        assert!(chunk_at(&[0; 64], usize::MAX - 2).is_none());
    }
}
