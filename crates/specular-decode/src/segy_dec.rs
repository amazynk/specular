//! Seismic data through the N-D science path: SEG-Y (rev 0, 1, 2.0 and 2.1) and Seismic Unix
//! `.su` files, read by `specular-segy`.
//!
//! * A **3-D post-stack volume** (traces on an inline × crossline grid, found from the
//!   header bytes 189/193, then 9/21, then 17/13) shows as inline sections: crosslines
//!   across, time (or depth) down, an "Inline" slider over the inline numbers, opening on
//!   the middle inline. Cells without a trace are NaN.
//! * Anything else is a **2-D line**: the traces across in file order (every n-th when the
//!   line is too large to show whole), time down.
//!
//! Sections show through the seismic red-white-blue colour table with limits symmetric
//! about 0; the textual header's cards, the binary header facts and the geometry are
//! header lines in the info panel.

use std::sync::Arc;

use specular_core::nd::Limits;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_segy::{Domain, Endian, Kind, Segy, Volume};

use crate::science::{
    AxisKind, ColourTable, Linear, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary,
    Seismic, seismic_lut,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

/// Values shown at most for a 2-D line (traces × samples); wider lines show every n-th
/// trace.
const MAX_LINE_VALUES: usize = 64 << 20;

/// SEG-Y (`Format::Segy`) or Seismic Unix (`Format::SeismicUnix`).
pub struct SegyDecoder(pub Format);

impl Decoder for SegyDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, _head: &[u8]) -> bool {
        // The textual and binary headers are 3600 bytes: see `probe_file`.
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        // Seismic Unix traces have no signature: by extension only.
        self.0 == Format::Segy && specular_segy::sniff(&data[..data.len().min(3600)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Segy => &["sgy", "segy"],
            _ => &["su"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(SegySession::open(source, self.0)?))
    }
}

/// The file's bytes (a memory map) shared with the reader.
struct Bytes(Arc<Source>);

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

pub(crate) fn segy_err(e: specular_segy::Error) -> DecodeError {
    use specular_segy::Error as E;
    match e {
        E::NotSegy | E::NotSu => DecodeError::UnsupportedFormat,
        E::Malformed(s) => DecodeError::Malformed(s),
        e @ (E::Unsupported(_) | E::VariableLength { .. }) => {
            DecodeError::Unsupported(e.to_string())
        }
        E::Io(e) => DecodeError::Io(e),
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// What the planes are.
enum Layout {
    Volume(Volume),
    /// A line: the traces shown, one per column.
    Line(Vec<u32>),
}

struct SegySession {
    source: Arc<Source>,
    segy: Segy,
    layout: Layout,
    info: ImageInfo,
    summary: ScienceSummary,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

/// "1250 … 1300 (step 2)", or the one number.
fn range(v: &[i32]) -> String {
    match v {
        [] => String::new(),
        [a] => a.to_string(),
        // Widened: the numbers come from trace headers and may be far apart.
        [a, b, ..] => format!(
            "{a} … {} (step {})",
            v[v.len() - 1],
            i64::from(*b) - i64::from(*a)
        ),
    }
}

/// A number without trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl SegySession {
    fn open(source: Arc<Source>, format: Format) -> Result<SegySession> {
        let su = format == Format::SeismicUnix;
        let segy = Segy::from_shared(Arc::new(Bytes(source.clone())), su).map_err(segy_err)?;
        if segy.traces == 0 || segy.samples == 0 {
            return Err(DecodeError::Unsupported("no traces in the file".into()));
        }
        let geometry = segy.geometry().map_err(segy_err)?;
        let ns = segy.samples;
        let depth = segy.domain == Domain::Depth;
        let unit = if depth {
            segy.length_unit().unwrap_or("").to_owned()
        } else {
            "ms".to_owned()
        };
        let any_xy = geometry.coords.iter().any(|c| c[0] != 0.0 || c[1] != 0.0);
        let mut seismic = Seismic {
            depth,
            unit,
            t0: segy.delay,
            dt: segy.interval / 1000.0,
            cdp: geometry.cdp.clone(),
            coords: if any_xy {
                geometry.coords.clone()
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let mut axes = Vec::new();
        let mut start = Vec::new();
        let (width, layout) = match geometry.volume {
            Some(v) => {
                seismic.inlines = v.inlines.clone();
                seismic.crosslines = v.crosslines.clone();
                seismic.key = v.key;
                seismic.traces = v.cells().to_vec();
                let n = v.inlines.len();
                axes.push(NdAxis {
                    name: "Inline".into(),
                    len: n as u32,
                    unit: String::new(),
                    labels: v.inlines.iter().map(|i| i.to_string()).collect(),
                    wcs_axis: None,
                    linear: Some(Linear {
                        offset: f64::from(v.inlines[0]),
                        step: f64::from(v.inlines[1]) - f64::from(v.inlines[0]),
                    }),
                    kind: AxisKind::Cube,
                });
                start.push((n / 2) as u32);
                (v.crosslines.len(), Layout::Volume(v))
            }
            None => {
                let stride = (segy.traces * ns).div_ceil(MAX_LINE_VALUES).max(1);
                let shown: Vec<u32> = (0..segy.traces).step_by(stride).map(|t| t as u32).collect();
                seismic.traces = shown.clone();
                (shown.len(), Layout::Line(shown))
            }
        };
        let keywords = keywords(&segy, &layout, geometry.source_coords, any_xy);
        let sample = format!(
            "{}, {}",
            segy.format.name(),
            match segy.endian {
                Endian::Big => "big-endian",
                Endian::Little => "little-endian",
            }
        );
        let title = segy
            .text
            .iter()
            .map(|c| card_text(c))
            .find(|c| !c.is_empty())
            .map(str::to_owned);
        let summary = ScienceSummary {
            width: width as u32,
            height: ns as u32,
            start,
            axes,
            sample: sample.clone(),
            title,
            kind: ScienceKind::Seismic,
            luts: vec![seismic_lut()],
            seismic: Some(Arc::new(seismic)),
            keywords: Arc::new(keywords),
            ..Default::default()
        };
        let info = ImageInfo {
            format,
            width: width as u32,
            height: ns as u32,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: segy.format.bits(),
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(SegySession {
            source,
            segy,
            layout,
            info,
            summary,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        let ns = self.segy.samples;
        self.planes.get_or(index, cancel, || {
            let (width, values) = match &self.layout {
                Layout::Volume(v) => {
                    let il = index.first().copied().unwrap_or(0) as usize;
                    if il >= v.inlines.len() {
                        return Err(DecodeError::Malformed("no such inline".into()));
                    }
                    let s = v.inline_section(&self.segy, il).map_err(segy_err)?;
                    (v.crosslines.len(), s)
                }
                Layout::Line(shown) => (shown.len(), self.line(shown, cancel)?),
            };
            Ok(NdPlane {
                width: width as u32,
                height: ns as u32,
                values,
                truncated: false,
            })
        })
    }

    /// The traces `shown` as a section: `values[k * shown.len() + column]`.
    fn line(&self, shown: &[u32], cancel: &CancelToken) -> Result<Vec<f32>> {
        let (nx, ns) = (shown.len(), self.segy.samples);
        let mut out = vec![0f32; nx * ns];
        let contiguous = shown.len() == self.segy.traces;
        // Batches of traces read together (contiguous) or one by one (strided).
        const BATCH: usize = 256;
        let mut buf = vec![0f32; BATCH * ns];
        let mut scratch = Vec::new();
        for (b, cols) in shown.chunks(BATCH).enumerate() {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            if contiguous {
                let t0 = cols[0] as usize;
                self.segy
                    .traces_into(t0..t0 + cols.len(), &mut buf[..cols.len() * ns])
                    .map_err(segy_err)?;
            } else {
                for (i, &t) in cols.iter().enumerate() {
                    self.segy
                        .trace_into(t as usize, &mut buf[i * ns..(i + 1) * ns], &mut scratch)
                        .map_err(segy_err)?;
                }
            }
            for (i, tr) in buf.chunks_exact(ns).take(cols.len()).enumerate() {
                let x = b * BATCH + i;
                for (k, v) in tr.iter().enumerate() {
                    out[k * nx + x] = *v;
                }
            }
        }
        Ok(out)
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let idx = if self.summary.start.len() == self.summary.axes.len() {
            self.summary.start.clone()
        } else {
            vec![0; self.summary.axes.len()]
        };
        let p = self.plane(&idx, cancel)?;
        Ok(seismic_raster(&p, &seismic_lut()))
    }
}

/// A card image without its "C 1" / "C01" prefix and trailing blanks.
fn card_text(card: &str) -> &str {
    let t = card.trim_end();
    let b = t.as_bytes();
    if b.first() == Some(&b'C')
        && b.len() >= 3
        && b[1..3].iter().all(|c| c.is_ascii_digit() || *c == b' ')
    {
        t[3..].trim()
    } else {
        t.trim()
    }
}

fn keywords(
    segy: &Segy,
    layout: &Layout,
    source_xy: bool,
    any_xy: bool,
) -> Vec<(String, String, String)> {
    let mut k = Vec::new();
    match segy.kind {
        Kind::SegY => {
            if let Some(r) = segy.revision() {
                k.push(kw("SEG-Y revision", r));
            }
        }
        Kind::Su => k.push(kw("File", "Seismic Unix traces".into())),
    }
    k.push(kw(
        "Sample format",
        format!("{} (code {})", segy.format.name(), segy.format.code()),
    ));
    k.push(kw(
        "Byte order",
        match segy.endian {
            Endian::Big => "big-endian",
            Endian::Little => "little-endian",
        }
        .into(),
    ));
    k.push(kw("Traces", segy.traces.to_string()));
    k.push(kw("Samples per trace", segy.samples.to_string()));
    let depth = segy.domain == Domain::Depth;
    let unit = if depth {
        segy.length_unit().unwrap_or("")
    } else {
        "ms"
    };
    k.push(kw(
        "Sample interval",
        format!("{} {unit}", num(segy.interval / 1000.0))
            .trim_end()
            .to_owned(),
    ));
    k.push(kw(
        if depth { "Depth range" } else { "Time range" },
        format!(
            "{} … {} {unit}",
            num(segy.sample_position(0.0)),
            num(segy.sample_position((segy.samples - 1) as f64))
        )
        .trim_end()
        .to_owned(),
    ));
    k.push(kw("Domain", if depth { "depth" } else { "time" }.into()));
    match layout {
        Layout::Volume(v) => {
            k.push(kw(
                "Geometry",
                format!(
                    "3-D volume, {} inlines × {} crosslines (bytes {} / {})",
                    v.inlines.len(),
                    v.crosslines.len(),
                    v.key.0,
                    v.key.1
                ),
            ));
            k.push(kw("Inlines", range(&v.inlines)));
            k.push(kw("Crosslines", range(&v.crosslines)));
            if v.gaps > 0 {
                k.push(kw("Empty cells", v.gaps.to_string()));
            }
            if v.duplicates > 0 {
                k.push(kw("Repeated cells", v.duplicates.to_string()));
            }
        }
        Layout::Line(shown) => {
            let mut g = "2-D line".to_owned();
            if shown.len() < segy.traces {
                g += &format!(", every {} traces shown", segy.traces.div_ceil(shown.len()));
            }
            k.push(kw("Geometry", g));
        }
    }
    if any_xy {
        k.push(kw(
            "Coordinates",
            if source_xy {
                "source X / Y"
            } else {
                "CDP X / Y"
            }
            .into(),
        ));
    }
    if let Some(u) = segy.length_unit() {
        k.push(kw("Length unit", u.into()));
    }
    if let Some(b) = &segy.binary {
        k.push(kw(
            "Textual header",
            if segy.ebcdic { "EBCDIC" } else { "ASCII" }.into(),
        ));
        let nonzero = |k: &mut Vec<_>, name: &str, v: i64| {
            if v != 0 {
                k.push(kw(name, v.to_string()));
            }
        };
        nonzero(&mut k, "Job", i64::from(b.job));
        nonzero(&mut k, "Line", i64::from(b.line));
        nonzero(&mut k, "Reel", i64::from(b.reel));
        nonzero(
            &mut k,
            "Traces per ensemble",
            i64::from(b.traces_per_ensemble),
        );
        nonzero(&mut k, "CDP fold", i64::from(b.fold));
        let sorting = match b.sorting {
            1 => Some("as recorded"),
            2 => Some("CDP ensemble"),
            3 => Some("single fold continuous profile"),
            4 => Some("horizontally stacked"),
            5 => Some("common source point"),
            6 => Some("common receiver point"),
            7 => Some("common offset point"),
            8 => Some("common mid-point"),
            9 => Some("common conversion point"),
            _ => None,
        };
        if let Some(s) = sorting {
            k.push(kw("Trace sorting", s.into()));
        }
        if segy.extra_headers > 0 {
            k.push(kw(
                "Additional trace headers",
                segy.extra_headers.to_string(),
            ));
        }
        if !segy.extended.is_empty() {
            k.push(kw(
                "Extended textual headers",
                segy.extended.len().to_string(),
            ));
        }
    }
    if segy.trailing > 0 {
        k.push(kw("Bytes after the last trace", segy.trailing.to_string()));
    }
    // The textual header, card by card (blank cards left out), then the extended stanzas.
    for (i, c) in segy.text.iter().enumerate() {
        let t = c.trim_end();
        if !card_text(t).is_empty() {
            k.push(kw(&format!("Card {:02}", i + 1), t.to_owned()));
        }
    }
    for (s, stanza) in segy.extended.iter().enumerate() {
        for (i, c) in stanza.iter().enumerate() {
            let t = c.trim_end();
            if !t.is_empty() {
                k.push(kw(
                    &format!("Extended {} card {:02}", s + 1, i + 1),
                    t.to_owned(),
                ));
            }
        }
    }
    k
}

/// A whole 3-D seismic volume read for MPR: `values[(il * crosslines + xl) * samples + k]`,
/// NaN where the grid has no trace.
#[derive(Clone, Debug)]
pub struct SeismicCube {
    pub values: Vec<f32>,
    pub inlines: usize,
    pub crosslines: usize,
    pub samples: usize,
}

/// Reads the post-stack volume of a SEG-Y or Seismic Unix file (by content). `Ok(None)`
/// when `cancel` stopped it; an error when the traces are not on an inline × crossline grid.
pub fn seismic_cube(path: &std::path::Path, cancel: &CancelToken) -> Result<Option<SeismicCube>> {
    let segy = Segy::open_any(path).map_err(segy_err)?;
    let geometry = segy.geometry().map_err(segy_err)?;
    let Some(v) = geometry.volume else {
        return Err(DecodeError::Unsupported(
            "the traces are a 2-D line, not a 3-D volume".into(),
        ));
    };
    let values = v
        .read(&segy, |_| !cancel.is_cancelled())
        .map_err(segy_err)?;
    Ok(values.map(|values| SeismicCube {
        values,
        inlines: v.inlines.len(),
        crosslines: v.crosslines.len(),
        samples: segy.samples,
    }))
}

/// A section as RGBA through `lut` at the default seismic gain (symmetric about 0); gaps
/// (NaN) mid grey.
pub(crate) fn seismic_raster(p: &NdPlane, lut: &ColourTable) -> Raster {
    let (lo, hi) = Limits::SEISMIC
        .compute(&p.values, p.width as usize)
        .filter(|(l, h)| h > l)
        .unwrap_or((-1.0, 1.0));
    let scale = 255.0 / (hi - lo);
    let mut rgba = Vec::with_capacity(p.values.len() * 4);
    for v in &p.values {
        if v.is_finite() {
            let i = ((v - lo) * scale).round().clamp(0.0, 255.0) as usize;
            let c = lut[i];
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        } else {
            rgba.extend_from_slice(&[128, 128, 128, 255]);
        }
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

impl Session for SegySession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 8 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Arc<Source> {
        let p = format!(
            "{}/../formats/specular-segy/tests/data/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        Arc::new(Source::from_vec(name, std::fs::read(p).unwrap()))
    }

    #[test]
    fn volume_sections() {
        let s = crate::Registry::new()
            .open(fixture("ibm_volume.sgy"))
            .unwrap();
        assert_eq!(s.info().format, Format::Segy);
        let sc = s.science().unwrap();
        assert_eq!(sc.kind, ScienceKind::Seismic);
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].name, "Inline");
        let seis = sc.seismic.as_ref().unwrap();
        assert!(seis.is_volume());
        assert_eq!(sc.width as usize, seis.crosslines.len());
        assert_eq!(sc.start, vec![(seis.inlines.len() / 2) as u32]);
        let cancel = CancelToken::new();
        let p = s.nd_plane(&[1], &cancel).unwrap().unwrap();
        assert_eq!((p.width, p.height), (sc.width, sc.height));
        // Column 0 of inline 1 is the trace in that cell.
        let pt = seis.at(&[1], 0, 3);
        let t = pt.trace.unwrap() - 1;
        let segy = Segy::from_bytes(s.source().bytes().to_vec()).unwrap();
        let tr = segy.trace(t).unwrap();
        assert_eq!(p.values[3 * p.width as usize], tr[3]);
        assert_eq!(pt.inline, Some(seis.inlines[1]));
        assert!((pt.time - (seis.t0 + 3.0 * seis.dt)).abs() < 1e-9);
        let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        assert_eq!((r.width, r.height), (sc.width, sc.height));
        assert!(
            s.science()
                .unwrap()
                .keywords
                .iter()
                .any(|k| k.0 == "Card 01")
        );
    }

    #[test]
    fn gaps_are_nan() {
        let s = crate::Registry::new()
            .open(fixture("gaps_volume.sgy"))
            .unwrap();
        let sc = s.science().unwrap();
        let seis = sc.seismic.as_ref().unwrap();
        let cancel = CancelToken::new();
        let mut gaps = 0;
        for il in 0..seis.inlines.len() as u32 {
            let p = s.nd_plane(&[il], &cancel).unwrap().unwrap();
            for x in 0..p.width {
                let gap = seis.at(&[il], x, 0).trace.is_none();
                assert_eq!(gap, p.values[x as usize].is_nan());
                gaps += usize::from(gap);
            }
        }
        assert!(gaps > 0);
    }

    #[test]
    fn line_and_su() {
        let cancel = CancelToken::new();
        let s = crate::Registry::new()
            .open(fixture("int16_line.sgy"))
            .unwrap();
        let sc = s.science().unwrap();
        let seis = sc.seismic.as_ref().unwrap();
        assert!(!seis.is_volume() && sc.axes.is_empty());
        let segy = Segy::from_bytes(s.source().bytes().to_vec()).unwrap();
        assert_eq!(sc.width as usize, segy.traces);
        let p = s.nd_plane(&[], &cancel).unwrap().unwrap();
        let last = segy.traces - 1;
        let tr = segy.trace(last).unwrap();
        assert_eq!(p.values[2 * p.width as usize + last], tr[2]);
        assert_eq!(seis.at(&[], last as u32, 0).trace, Some(last + 1));
        for name in ["line_be.su", "line_le.su"] {
            let s = crate::Registry::new().open(fixture(name)).unwrap();
            assert_eq!(s.info().format, Format::SeismicUnix);
            let p = s.nd_plane(&[], &cancel).unwrap().unwrap();
            assert!(p.values.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn variable_length_refused() {
        match crate::Registry::new().open(fixture("variable.sgy")) {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains("variable trace length"), "{m}"),
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("opened"),
        }
    }

    #[test]
    fn colour_table_and_cards() {
        let l = seismic_lut();
        assert_eq!(l[128][0], 255);
        assert!(l[0][2] > l[0][0] && l[255][0] > l[255][2]);
        assert_eq!(card_text("C 1 CLIENT: X"), "CLIENT: X");
        assert_eq!(card_text("C01 AREA"), "AREA");
        assert_eq!(card_text("C40 END TEXTUAL HEADER  "), "END TEXTUAL HEADER");
        assert_eq!(range(&[10, 12, 14]), "10 … 14 (step 2)");
        // Header numbers at the ends of the i32 range must not overflow.
        assert_eq!(
            range(&[i32::MIN, i32::MAX]),
            "-2147483648 … 2147483647 (step 4294967295)"
        );
        assert_eq!(num(4.0), "4");
        assert_eq!(num(0.5), "0.5");
    }

    /// Open timings on the corpus (`SPECULAR_SEGY_DIR`, from tools/corpus/fetch-segy.sh):
    /// open, first section through the colour table, and the whole volume read for MPR.
    #[test]
    fn corpus_open_timings() {
        let Some(dir) = std::env::var_os("SPECULAR_SEGY_DIR") else {
            return;
        };
        let dir = std::path::Path::new(&dir);
        let cancel = CancelToken::new();
        for name in [
            "segyio/f3.sgy",
            "xtgeo/ib_test_cube2.segy",
            "xtgeo/reek_depth_stack.segy",
        ] {
            let path = dir.join(name);
            if !path.exists() {
                continue;
            }
            let t = std::time::Instant::now();
            let s = crate::Registry::new()
                .open(Arc::new(Source::open(&path).unwrap()))
                .unwrap();
            let open = t.elapsed();
            let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
            let first = t.elapsed();
            let t = std::time::Instant::now();
            let cube = seismic_cube(&path, &cancel).unwrap().unwrap();
            let whole = t.elapsed();
            eprintln!(
                "{name}: {}x{}x{}, open {:.1} ms, first section ({}x{}) {:.1} ms, \
                 whole volume {:.1} ms",
                cube.inlines,
                cube.crosslines,
                cube.samples,
                open.as_secs_f64() * 1e3,
                r.width,
                r.height,
                first.as_secs_f64() * 1e3,
                whole.as_secs_f64() * 1e3
            );
        }
    }
}
