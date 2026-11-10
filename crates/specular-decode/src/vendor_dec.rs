//! Vendor light-microscopy files through the N-D science path (`specular-micro`).
//!
//! * **Leica LIF / LOF:** one page per image (series) of the file, named by its path in the
//!   project; channels are a channel axis coloured by their LUT names (or a colour camera's
//!   red, green and blue), the other dimensions (Z, T, λ, mosaic tiles…) cube axes with their
//!   calibrations; X and Y give physical pixel positions.
//! * **Nikon ND2:** the chunk-map layout (versions 2 and 3) and the legacy JPEG 2000 one
//!   (version 1). Every component of a frame is a position of the channel axis (named and
//!   coloured by its channel, red / green / blue for a colour camera); the experiment's loops
//!   (Z, multipoint, time, innermost first) are cube axes; `dCalibration` gives the pixel
//!   size.
//!
//! * **Olympus OIR:** one image; channels (named, coloured by their linear colour table) are
//!   the channel axis, then λ (nm), Z (µm) and T (s) with their steps; the channel's pixel
//!   size gives physical positions.
//!
//! * **ICS (Image Cytometry Standard 1.0 / 2.0):** the `.ics` header with its `.ids` file
//!   (or inline data, or a named source file in the same folder); opening the `.ids` opens
//!   its `.ics`. A channel dimension is the channel axis (labelled by emission wavelength
//!   when the sensor gives them); the other dimensions past x and y are cube axes in
//!   storage order with their origins, scales and units; complex samples get a part axis.
//!   gzip and compress data are decompressed once, on the first plane.
//!
//! Zeiss CZI is read by libCZI in the decode host ([`crate::czi_dec`]).
//!
//! The default view composites every channel (up to [`MAX_COMPOSITE`]) in its colour, each
//! through its own zscale limits, as the science view's automatic look does.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use std::sync::OnceLock;

use specular_micro::{Sample, ics, lif, nd2, oir};

use crate::science::{
    AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PixelSource, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

/// Channels the default view composites (the science view's own limit).
pub(crate) const MAX_COMPOSITE: usize = 16;

fn micro_err(e: specular_micro::Error) -> DecodeError {
    use specular_micro::Error as E;
    match e {
        E::NotLif | E::NotNd2 => DecodeError::UnsupportedFormat,
        E::NoImage => DecodeError::Unsupported("no image in the file".into()),
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// Which format a decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Leica LIF and its single-image sibling LOF.
    Lif,
    Nd2,
    Oir,
    /// Image Cytometry Standard (`.ics` with `.ids`).
    Ics,
}

pub struct VendorDecoder(pub Kind);

impl Decoder for VendorDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Lif => Format::Lif,
            Kind::Nd2 => Format::Nd2,
            Kind::Oir => Format::Oir,
            Kind::Ics => Format::Ics,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Lif => lif::sniff(head),
            Kind::Oir => oir::sniff(head),
            Kind::Ics => ics::sniff(head),
            // The legacy layout starts like any JP2 file: see `probe_file`.
            Kind::Nd2 => nd2::sniff_chunked(head),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Lif => lif::sniff(&data[..data.len().min(crate::PROBE_HEAD)]),
            Kind::Nd2 => nd2::sniff_chunked(data) || nd2::is_legacy_file(data),
            Kind::Oir => oir::sniff(data),
            Kind::Ics => ics::sniff(data),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Lif => &["lif", "lof"],
            Kind::Nd2 => &["nd2"],
            Kind::Oir => &["oir"],
            Kind::Ics => &["ics", "ids"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let file = match self.0 {
            Kind::Lif => {
                let stem = source
                    .path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_owned();
                let l = lif::parse(data, &stem).map_err(micro_err)?;
                // Images with pixels and channels; folders and empty acquisitions skipped.
                let pages: Vec<usize> = l
                    .images
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| !i.channels.is_empty() && i.block.is_some_and(|(_, n)| n > 0))
                    .map(|(k, _)| k)
                    .collect();
                if pages.is_empty() {
                    return Err(DecodeError::Unsupported("no image in the file".into()));
                }
                Parsed::Lif(l, pages)
            }
            Kind::Nd2 => {
                let n = nd2::parse(data).map_err(micro_err)?;
                if n.width == 0 || n.height == 0 || n.components == 0 {
                    return Err(DecodeError::Unsupported("no image in the file".into()));
                }
                Parsed::Nd2(Box::new(n))
            }
            Kind::Oir => Parsed::Oir(Box::new(oir::parse(data).map_err(micro_err)?)),
            Kind::Ics => Parsed::Ics(Box::new(IcsFile::open(&source)?)),
        };
        Ok(Arc::new(VendorSession::new(source, Arc::new(file), 0)?))
    }
}

enum Parsed {
    /// The file and the images shown as pages.
    Lif(lif::Lif, Vec<usize>),
    Nd2(Box<nd2::Nd2>),
    Oir(Box<oir::Oir>),
    Ics(Box<IcsFile>),
}

/// An ICS header and the file its samples are in.
struct IcsFile {
    ics: ics::Ics,
    /// The `.ics` itself (inline data), its `.ids` or the named source file.
    data: Arc<Source>,
    /// The decompressed samples (gzip, compress), made on the first plane.
    decoded: OnceLock<std::result::Result<Arc<Vec<u8>>, String>>,
}

/// A file of `folder` by its plain name, matched without case when the exact name is
/// missing; `None` when there is none.
fn same_folder(folder: &std::path::Path, name: &str) -> Option<Source> {
    let plain = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        == Some(name)
        && name != "."
        && name != "..";
    if !plain {
        return None;
    }
    let open = |p: std::path::PathBuf| Source::open(&p).ok().filter(|s| !s.is_folder());
    if let Some(s) = open(folder.join(name)) {
        return Some(s);
    }
    let found = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .take(1 << 16)
        .find(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })?;
    open(found.path())
}

fn missing(name: &str) -> DecodeError {
    DecodeError::Unsupported(format!(
        "the ICS data file {name} is missing beside the header"
    ))
}

impl IcsFile {
    /// From the `.ics`, or from its `.ids` (whose `.ics` is read for the header).
    fn open(source: &Arc<Source>) -> Result<IcsFile> {
        let path = source.path().to_path_buf();
        let folder = path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        if !ics::sniff(source.bytes()) {
            // An `.ids`: its header is the `.ics` of the same name, its samples this file.
            let hs = same_folder(&folder, &format!("{stem}.ics")).ok_or_else(|| {
                DecodeError::Unsupported(
                    "an ICS data file (.ids) without its header (.ics) beside it".into(),
                )
            })?;
            let ics = ics::parse(hs.bytes()).map_err(micro_err)?;
            if !matches!(ics.data, ics::DataAt::Ids(_)) {
                return Err(DecodeError::Unsupported(
                    "the ICS header beside this file keeps its samples elsewhere".into(),
                ));
            }
            return Ok(IcsFile {
                ics,
                data: source.clone(),
                decoded: OnceLock::new(),
            });
        }
        let mut ics = ics::parse(source.bytes()).map_err(micro_err)?;
        let at = ics.data.clone();
        let data = match &at {
            ics::DataAt::Inline(_) => source.clone(),
            // The header's file name, else the `.ids` beside this `.ics`; either compressed
            // on its own (`.ids.Z`, `.ids.gz`), as libics finds them.
            ics::DataAt::Ids(name) => {
                let own = format!("{stem}.ids");
                let s = [name.clone(), own.clone()]
                    .iter()
                    .flat_map(|n| [n.clone(), format!("{n}.Z"), format!("{n}.gz")])
                    .find_map(|n| same_folder(&folder, &n))
                    .ok_or_else(|| missing(name))?;
                match s.bytes() {
                    [0x1f, 0x9d, ..] => ics.compression = ics::Compression::Compress,
                    [0x1f, 0x8b, ..] => ics.compression = ics::Compression::Gzip,
                    _ => {}
                }
                Arc::new(s)
            }
            ics::DataAt::Source { file, .. } => {
                Arc::new(same_folder(&folder, file).ok_or_else(|| missing(file))?)
            }
        };
        Ok(IcsFile {
            ics,
            data,
            decoded: OnceLock::new(),
        })
    }

    /// The sample stream (decompressed once).
    fn stream(&self) -> Result<std::borrow::Cow<'_, [u8]>> {
        use std::borrow::Cow;
        if self.ics.compression == ics::Compression::None {
            return self.ics.stream(self.data.bytes()).map_err(micro_err);
        }
        let r = self
            .decoded
            .get_or_init(|| match self.ics.stream(self.data.bytes()) {
                Ok(Cow::Owned(v)) => Ok(Arc::new(v)),
                Ok(Cow::Borrowed(b)) => Ok(Arc::new(b.to_vec())),
                Err(e) => Err(e.to_string()),
            });
        match r {
            Ok(v) => Ok(Cow::Borrowed(v.as_slice())),
            Err(e) => Err(DecodeError::Malformed(e.clone())),
        }
    }

    fn memory_bytes(&self) -> usize {
        match self.decoded.get() {
            Some(Ok(v)) => v.len(),
            _ => 0,
        }
    }
}

impl Parsed {
    fn pages(&self) -> u32 {
        match self {
            Parsed::Lif(_, p) => p.len() as u32,
            Parsed::Nd2(_) | Parsed::Oir(_) | Parsed::Ics(_) => 1,
        }
    }
}

/// How an index of the summary's axes maps onto the file.
enum Map {
    /// LIF: the channel axis (when there is more than one channel) and the image's extra
    /// dimensions in file order.
    Lif { image: usize, channel_axis: bool },
    /// ND2: the channel (component) axis when there is more than one component, then the
    /// loops innermost first.
    Nd2 { channel_axis: bool },
    /// OIR: the channel axis when there is more than one channel, then λ, Z and T where
    /// they have more than one position.
    Oir { channel_axis: bool, lzt: [bool; 3] },
    /// ICS: the header's dimension for each axis ([`ICS_PART`] for the complex part axis).
    Ics { dims: Vec<usize> },
}

/// The axis of a complex ICS image's parts.
const ICS_PART: usize = usize::MAX;

struct VendorSession {
    source: Arc<Source>,
    file: Arc<Parsed>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    map: Map,
    planes: PlaneCache,
    levels: Levels,
}

fn kw(k: impl Into<String>, v: impl ToString) -> (String, String, String) {
    (k.into(), v.to_string(), String::new())
}

fn axis(
    len: usize,
    name: &str,
    unit: &str,
    linear: Option<Linear>,
    kind: AxisKind,
) -> Result<NdAxis> {
    Ok(NdAxis {
        name: name.to_owned(),
        len: u32::try_from(len).map_err(|_| DecodeError::TooLarge)?,
        unit: unit.trim().to_owned(),
        labels: Vec::new(),
        wcs_axis: None,
        linear,
        kind,
    })
}

fn sample_text(s: Sample, bits: u32) -> String {
    let stored = 8 * s.bytes() as u32;
    let base = match s {
        Sample::F32 | Sample::F64 => format!("{stored}-bit float"),
        _ => format!("{stored}-bit unsigned"),
    };
    if !s.is_float() && bits > 0 && bits < stored {
        format!("{bits}-bit unsigned in {stored}")
    } else {
        base
    }
}

/// LIF lengths are in metres (sometimes µm for old files), seconds or nm.
fn lif_unit(u: &str) -> &str {
    match u.trim() {
        "um" | "micron" => "µm",
        u => u,
    }
}

fn lif_summary(l: &lif::Lif, img: &lif::Image) -> Result<(ScienceSummary, bool)> {
    let mut axes = Vec::new();
    let channel_axis = img.channels.len() > 1;
    if channel_axis {
        let mut a = axis(img.channels.len(), "Channel", "", None, AxisKind::Channel)?;
        a.labels = img.channels.iter().map(|c| c.name.clone()).collect();
        axes.push(a);
    }
    for d in img.extra_dims() {
        let linear = d.step().map(|step| Linear {
            offset: d.origin,
            step,
        });
        let unit = if linear.is_some() {
            lif_unit(&d.unit)
        } else {
            ""
        };
        axes.push(axis(d.count, d.kind.code(), unit, linear, AxisKind::Cube)?);
    }
    let pixel = img.pixel_size().map(|(sx, sy, unit)| {
        let origin = |k: lif::DimKind| {
            img.dims
                .iter()
                .find(|d| d.kind == k)
                .map_or(0.0, |d| if d.origin.is_finite() { d.origin } else { 0.0 })
        };
        PixelCalibration {
            x: Linear {
                offset: origin(lif::DimKind::X),
                step: sx,
            },
            y: Linear {
                offset: origin(lif::DimKind::Y),
                step: sy,
            },
            unit_x: lif_unit(unit).to_owned(),
            unit_y: lif_unit(unit).to_owned(),
        }
    });
    let mut keywords = vec![
        kw("Image", &img.path),
        kw(if l.lof { "LOF version" } else { "LIF version" }, l.version),
    ];
    for d in &img.dims {
        let mut v = d.count.to_string();
        if let Some(s) = d.step() {
            v += &format!(", step {s} {}", d.unit);
        }
        keywords.push(kw(format!("Dimension {}", d.kind.code()), v));
    }
    for (i, c) in img.channels.iter().enumerate() {
        let mut v = c.name.clone();
        if !c.lut.is_empty() {
            v += &format!(", LUT {}", c.lut);
        }
        v += &format!(", {} bits", c.bits);
        keywords.push(kw(format!("Channel {}", i + 1), v));
    }
    let get = |k: &str| {
        img.meta
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    keywords.extend(img.meta.iter().map(|(k, v)| kw(k.clone(), v)));
    let colours: Vec<[f32; 3]> = img
        .channels
        .iter()
        .map(|c| c.colour())
        .collect::<Option<_>>()
        .unwrap_or_default();
    let ch0 = &img.channels[0];
    let summary = ScienceSummary {
        width: u32::try_from(img.width()).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(img.height()).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis: channel_axis.then_some(0),
        sample: sample_text(ch0.sample, img.bits()),
        instrument: get("Microscope"),
        unit: Some(ch0.unit.trim().to_owned()).filter(|u| !u.is_empty()),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        pixel_source: Some(PixelSource::Vendor),
        title: Some(img.path.clone()).filter(|t| !t.trim().is_empty()),
        magnification: get("Magnification").and_then(|m| m.trim().parse().ok()),
        detector: get("Objective"),
        channel_colours: if channel_axis { colours } else { Vec::new() },
        vendor: Some("Leica".into()),
        ..Default::default()
    };
    Ok((summary, channel_axis))
}

/// Components per pixel past which an ND2 is refused: the count comes from the file
/// (`uiComp`, or the channels' `CompCount` summed) and sizes the channel labels.
const MAX_COMPONENTS: usize = 1 << 16;

/// A display colour, components in 0..=1.
type Rgb = [f32; 3];

/// A label and colour per component: a channel's name and colour, or a colour camera's R/G/B.
fn component_labels(
    channels: &[nd2::Channel],
    components: usize,
) -> Result<(Vec<String>, Vec<Option<Rgb>>)> {
    if components > MAX_COMPONENTS {
        return Err(DecodeError::TooLarge);
    }
    let mut labels = Vec::new();
    let mut colours = Vec::new();
    for c in channels {
        if labels.len() >= components {
            break;
        }
        if c.components == 3 {
            for (k, rgb) in [
                ("R", [1.0, 0.0, 0.0]),
                ("G", [0.0, 1.0, 0.0]),
                ("B", [0.0, 0.0, 1.0]),
            ] {
                labels.push(if c.name.trim().is_empty() {
                    k.to_owned()
                } else {
                    format!("{} {k}", c.name.trim())
                });
                colours.push(Some(rgb));
            }
        } else {
            for _ in 0..c.components.min(components - labels.len()) {
                labels.push(c.name.trim().to_owned());
                colours.push(c.colour.map(|c| c.map(|v| f32::from(v) / 255.0)));
            }
        }
    }
    labels.resize(components, String::new());
    colours.resize(components, None);
    Ok((labels, colours))
}

fn nd2_summary(n: &nd2::Nd2) -> Result<(ScienceSummary, bool)> {
    let mut axes = Vec::new();
    let channel_axis = n.components > 1;
    let (labels, colours) = component_labels(&n.channels, n.components)?;
    if channel_axis {
        let mut a = axis(n.components, "Channel", "", None, AxisKind::Channel)?;
        a.labels = labels;
        axes.push(a);
    }
    for l in n.loops.iter().rev() {
        let (name, unit, linear) = match l.kind {
            nd2::LoopKind::Z => (
                "Z",
                "µm",
                l.step
                    .filter(|s| *s != 0.0 && s.is_finite())
                    .map(|step| Linear { offset: 0.0, step }),
            ),
            nd2::LoopKind::Time => ("T", "", None),
            nd2::LoopKind::Position => ("XY", "", None),
            nd2::LoopKind::Custom => ("", "", None),
        };
        let mut a = axis(
            l.count,
            name,
            if linear.is_some() { unit } else { "" },
            linear,
            AxisKind::Cube,
        )?;
        if l.labels.len() == l.count && l.labels.iter().any(|s| !s.trim().is_empty()) {
            a.labels = l.labels.clone();
        }
        axes.push(a);
    }
    let pixel = n
        .calibration
        .filter(|c| *c > 0.0 && c.is_finite())
        .map(|c| PixelCalibration {
            x: Linear {
                offset: 0.0,
                step: c,
            },
            y: Linear {
                offset: 0.0,
                step: c,
            },
            unit_x: "µm".into(),
            unit_y: "µm".into(),
        });
    let mut keywords = vec![
        kw("ND2 version", format!("{}.{}", n.version.0, n.version.1)),
        kw("Frames", n.frames),
        kw("Components", n.components),
    ];
    if let Some(c) = n.calibration {
        keywords.push(kw("Calibration (µm/px)", c));
    }
    if let Some(o) = &n.objective {
        keywords.push(kw("Objective", o));
    }
    if let Some(na) = n.numerical_aperture {
        keywords.push(kw("Numerical aperture", na));
    }
    for (i, c) in n.channels.iter().enumerate() {
        let mut v = c.name.clone();
        if let Some(e) = c.emission {
            v += &format!(", emission {e} nm");
        }
        keywords.push(kw(format!("Channel {}", i + 1), v));
    }
    keywords.extend(n.text.iter().map(|(k, v)| kw(k.clone(), v)));
    keywords.extend(n.meta.iter().map(|(k, v)| kw(k.clone(), v)));
    let all_colours: Option<Vec<[f32; 3]>> = colours.into_iter().collect();
    let summary = ScienceSummary {
        width: u32::try_from(n.width).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(n.height).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis: channel_axis.then_some(0),
        sample: sample_text(n.sample, n.bits),
        date_obs: n.text("Date").map(str::to_owned),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        pixel_source: Some(PixelSource::Vendor),
        title: n
            .text("Description")
            .and_then(|d| d.lines().next())
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty()),
        magnification: n.magnification,
        detector: n.objective.clone(),
        channel_colours: if channel_axis {
            all_colours.unwrap_or_default()
        } else {
            Vec::new()
        },
        vendor: Some("Nikon".into()),
        ..Default::default()
    };
    Ok((summary, channel_axis))
}

fn oir_summary(o: &oir::Oir) -> Result<(ScienceSummary, Map)> {
    let mut axes = Vec::new();
    let channel_axis = o.channels.len() > 1;
    if channel_axis {
        let mut a = axis(o.channels.len(), "Channel", "", None, AxisKind::Channel)?;
        a.labels = o.channels.iter().map(|c| c.name.clone()).collect();
        axes.push(a);
    }
    let mut lzt = [false; 3];
    for (k, (name, len, cal, unit)) in [
        ("λ", o.size_l, o.lambda, "nm"),
        ("Z", o.size_z, o.z, "µm"),
        ("T", o.size_t, o.t, "s"),
    ]
    .into_iter()
    .enumerate()
    {
        if len > 1 {
            let linear = cal
                .filter(|c| c.step != 0.0 && c.step.is_finite())
                .map(|c| Linear {
                    offset: c.start,
                    step: c.step,
                });
            axes.push(axis(
                len,
                name,
                if linear.is_some() { unit } else { "" },
                linear,
                AxisKind::Cube,
            )?);
            lzt[k] = true;
        }
    }
    let pixel = o
        .pixel
        .filter(|p| p.0 > 0.0 && p.1 > 0.0)
        .map(|(x, y)| PixelCalibration {
            x: Linear {
                offset: 0.0,
                step: x,
            },
            y: Linear {
                offset: 0.0,
                step: y,
            },
            unit_x: "µm".into(),
            unit_y: "µm".into(),
        });
    let mut keywords = vec![kw("Channels", o.channels.len())];
    for (i, c) in o.channels.iter().enumerate() {
        let mut v = c.name.clone();
        if let (Some(a), Some(b)) = (c.start_nm, c.end_nm) {
            v += &format!(", {a}–{b} nm");
        }
        keywords.push(kw(format!("Channel {}", i + 1), v));
    }
    if let Some(z) = o.z.filter(|_| o.size_z > 1) {
        keywords.push(kw("Z step (µm)", z.step));
    }
    if let Some(t) = o.t.filter(|_| o.size_t > 1) {
        keywords.push(kw("Time step (s)", t.step));
    }
    if let Some(na) = o.numerical_aperture {
        keywords.push(kw("Numerical aperture", na));
    }
    keywords.extend(o.meta.iter().map(|(k, v)| kw(k.clone(), v)));
    let colours: Vec<[f32; 3]> = o
        .channels
        .iter()
        .map(|c| c.colour.map(|c| c.map(|v| f32::from(v) / 255.0)))
        .collect::<Option<_>>()
        .unwrap_or_default();
    let sample = o.sample.unwrap_or(Sample::U16);
    let summary = ScienceSummary {
        width: u32::try_from(o.width).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(o.height).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis: channel_axis.then_some(0),
        sample: sample_text(sample, o.bits),
        date_obs: o.date.clone(),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        pixel_source: Some(PixelSource::Vendor),
        magnification: o.magnification,
        detector: o.objective.clone(),
        channel_colours: if channel_axis { colours } else { Vec::new() },
        vendor: Some("Olympus".into()),
        ..Default::default()
    };
    Ok((summary, Map::Oir { channel_axis, lzt }))
}

/// An ICS unit as shown ("micrometers" → "µm"); empty for none or "undefined".
fn ics_unit(u: &str) -> String {
    let l = u.trim().to_ascii_lowercase();
    match l.as_str() {
        "" | "undefined" | "pixel" | "pixels" | "relative" | "bits" | "none" => String::new(),
        "micrometer" | "micrometers" | "micrometre" | "micrometres" | "micron" | "microns"
        | "um" | "µm" => "µm".into(),
        "nanometer" | "nanometers" | "nanometre" | "nanometres" | "nm" => "nm".into(),
        "millimeter" | "millimeters" | "millimetre" | "millimetres" | "mm" => "mm".into(),
        "meter" | "meters" | "metre" | "metres" | "m" => "m".into(),
        "second" | "seconds" | "sec" | "s" => "s".into(),
        "millisecond" | "milliseconds" | "ms" => "ms".into(),
        "microsecond" | "microseconds" | "us" | "µs" => "µs".into(),
        "nanosecond" | "nanoseconds" | "ns" => "ns".into(),
        _ => u.trim().to_owned(),
    }
}

fn ics_sample_text(i: &ics::Ics) -> String {
    let sign = if i.signed { "signed" } else { "unsigned" };
    let base = match (i.format, i.significant_bits) {
        (ics::Format::Integer, Some(b)) if b > 0 && b < i.bits => {
            format!("{b}-bit {sign} in {}", i.bits)
        }
        (ics::Format::Integer, _) => format!("{}-bit {sign}", i.bits),
        (ics::Format::Real, _) => format!("{}-bit float", i.bits),
        (ics::Format::Complex, _) => format!("{}-bit complex", i.bits),
    };
    match i.compression {
        ics::Compression::None => base,
        c => format!("{base}, {}", c.name()),
    }
}

fn ics_summary(f: &IcsFile) -> Result<(ScienceSummary, Map)> {
    let i = &f.ics;
    let mut axes = Vec::new();
    let mut dims = Vec::new();
    let channel_axis = i.channel.filter(|&c| i.dims[c].len > 1);
    if let Some(c) = channel_axis {
        let d = &i.dims[c];
        let mut a = axis(d.len, "Channel", "", None, AxisKind::Channel)?;
        if i.emission.len() == d.len {
            a.labels = i.emission.iter().map(|w| format!("{w} nm")).collect();
        }
        axes.push(a);
        dims.push(c);
    }
    for k in i.plane_dims() {
        let d = &i.dims[k];
        if Some(k) == channel_axis || d.len < 2 {
            continue;
        }
        let unit = ics_unit(&d.unit);
        let linear = (!unit.is_empty() && d.scale != 0.0).then_some(Linear {
            offset: d.origin,
            step: d.scale,
        });
        let name = match d.name.to_ascii_lowercase().as_str() {
            "z" => "Z".to_owned(),
            "t" => "T".to_owned(),
            _ if !d.label.is_empty() => d.label.clone(),
            _ => d.name.clone(),
        };
        axes.push(axis(d.len, &name, &unit, linear, AxisKind::Cube)?);
        dims.push(k);
    }
    if i.format == ics::Format::Complex {
        axes.push(NdAxis {
            name: String::new(),
            len: 4,
            unit: String::new(),
            labels: Vec::new(),
            wcs_axis: None,
            linear: None,
            kind: AxisKind::Part,
        });
        dims.push(ICS_PART);
    }
    let (dx, dy) = (&i.dims[i.x], &i.dims[i.y]);
    let (ux, uy) = (ics_unit(&dx.unit), ics_unit(&dy.unit));
    let pixel = (!ux.is_empty() && dx.scale > 0.0 && dy.scale > 0.0).then(|| PixelCalibration {
        x: Linear {
            offset: dx.origin,
            step: dx.scale,
        },
        y: Linear {
            offset: dy.origin,
            step: dy.scale,
        },
        unit_y: if uy.is_empty() { ux.clone() } else { uy },
        unit_x: ux,
    });
    let list = |v: &[f64]| v.iter().map(f64::to_string).collect::<Vec<_>>().join(", ");
    let mut keywords = vec![
        kw("ICS version", &i.version),
        kw("Compression", i.compression.name()),
        kw(
            "Byte order",
            i.byte_order
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        kw(
            "Data",
            match &i.data {
                ics::DataAt::Inline(o) => format!("inline at byte {o}"),
                ics::DataAt::Ids(n) => n.clone(),
                ics::DataAt::Source { file, offset } => format!("{file} at byte {offset}"),
            },
        ),
    ];
    for d in &i.dims {
        let unit = ics_unit(&d.unit);
        let mut v = d.len.to_string();
        if !unit.is_empty() {
            v += &format!(" × {} {unit}", d.scale);
        }
        if !d.label.is_empty() && !d.label.eq_ignore_ascii_case(&d.name) {
            v += &format!(" ({})", d.label);
        }
        keywords.push(kw(format!("Dimension {}", d.name), v));
    }
    if !i.emission.is_empty() {
        keywords.push(kw("Emission (nm)", list(&i.emission)));
    }
    if !i.excitation.is_empty() {
        keywords.push(kw("Excitation (nm)", list(&i.excitation)));
    }
    keywords.extend(i.facts.iter().map(|(k, v)| kw(k.clone(), v)));
    let get = |k: &str| {
        i.facts
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.clone())
            .filter(|v| !v.trim().is_empty())
    };
    let summary = ScienceSummary {
        width: u32::try_from(i.width()).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(i.height()).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis: channel_axis.map(|_| 0),
        sample: ics_sample_text(i),
        unit: Some(ics_unit(&i.value_unit)).filter(|u| !u.is_empty()),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        pixel_source: Some(PixelSource::Vendor),
        detector: get("sensor model").or_else(|| get("sensor type")),
        date_obs: get("history date").or_else(|| get("history creation date")),
        ..Default::default()
    };
    Ok((summary, Map::Ics { dims }))
}

impl VendorSession {
    fn new(source: Arc<Source>, file: Arc<Parsed>, page: u32) -> Result<VendorSession> {
        let (format, summary, bits, map) = match &*file {
            Parsed::Lif(l, pages) => {
                let image = pages[page as usize];
                let img = &l.images[image];
                let (s, channel_axis) = lif_summary(l, img)?;
                (
                    Format::Lif,
                    s,
                    img.bits(),
                    Map::Lif {
                        image,
                        channel_axis,
                    },
                )
            }
            Parsed::Nd2(n) => {
                let (s, channel_axis) = nd2_summary(n)?;
                (Format::Nd2, s, n.bits, Map::Nd2 { channel_axis })
            }
            Parsed::Oir(o) => {
                let (s, map) = oir_summary(o)?;
                (Format::Oir, s, o.bits, map)
            }
            Parsed::Ics(f) => {
                let (s, map) = ics_summary(f)?;
                let bits = f.ics.significant_bits.unwrap_or(f.ics.bits);
                (Format::Ics, s, bits.min(64), map)
            }
        };
        if summary.width == 0 || summary.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        if summary.planes() == 0 {
            return Err(DecodeError::Malformed("no planes".into()));
        }
        let px = u64::from(summary.width) * u64::from(summary.height);
        if px > specular_micro::MAX_PLANE {
            return Err(DecodeError::TooLarge);
        }
        let info = ImageInfo {
            format,
            width: summary.width,
            height: summary.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: bits.clamp(1, 64) as u8,
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(VendorSession {
            source,
            file,
            page,
            info,
            summary,
            map,
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        for (k, a) in self.summary.axes.iter().enumerate() {
            if index.get(k).copied().unwrap_or(0) >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
        }
        let at = |k: usize| index.get(k).copied().unwrap_or(0) as usize;
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let mut values = Vec::new();
        values
            .try_reserve_exact(w * h)
            .map_err(|_| DecodeError::TooLarge)?;
        values.resize(w * h, 0f32);
        let data = self.source.bytes();
        let truncated = match (&*self.file, &self.map) {
            (
                Parsed::Lif(l, _),
                &Map::Lif {
                    image,
                    channel_axis,
                },
            ) => {
                let skip = usize::from(channel_axis);
                let channel = if channel_axis { at(0) } else { 0 };
                let pos: Vec<usize> = (skip..self.summary.axes.len()).map(at).collect();
                l.images[image]
                    .read_plane(data, channel, &pos, &mut values)
                    .map_err(micro_err)?
            }
            (Parsed::Nd2(n), &Map::Nd2 { channel_axis }) => {
                let skip = usize::from(channel_axis);
                let component = if channel_axis { at(0) } else { 0 };
                // Axes hold the loops innermost first; the file counts outermost first.
                let pos: Vec<usize> = (skip..self.summary.axes.len()).rev().map(at).collect();
                let frame = n
                    .frame_index(&pos)
                    .ok_or_else(|| DecodeError::Malformed("no such frame".into()))?;
                if frame >= n.frames {
                    values.fill(0.0);
                    true
                } else {
                    n.read_plane(data, frame, component, &mut values)
                        .map_err(micro_err)?
                }
            }
            (Parsed::Oir(o), &Map::Oir { channel_axis, lzt }) => {
                let mut k = usize::from(channel_axis);
                let c = if channel_axis { at(0) } else { 0 };
                let mut pos = [0usize; 3];
                for (i, on) in lzt.iter().enumerate() {
                    if *on {
                        pos[i] = at(k);
                        k += 1;
                    }
                }
                o.read_plane(data, c, pos[0], pos[1], pos[2], &mut values)
                    .map_err(micro_err)?
            }
            (Parsed::Ics(f), Map::Ics { dims }) => {
                let mut pos = vec![0usize; f.ics.dims.len()];
                let mut part = 0u8;
                for (k, &d) in dims.iter().enumerate() {
                    if d == ICS_PART {
                        part = at(k) as u8;
                    } else if let Some(p) = pos.get_mut(d) {
                        *p = at(k);
                    }
                }
                let stream = f.stream()?;
                f.ics
                    .read_plane(&stream, &pos, part, &mut values)
                    .map_err(micro_err)?
            }
            _ => return Err(DecodeError::Malformed("plane map".into())),
        };
        Ok(NdPlane {
            width: self.info.width,
            height: self.info.height,
            values,
            truncated,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    /// Every channel (up to [`MAX_COMPOSITE`]) through its zscale limits, added in its colour.
    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let s = &self.summary;
        let (w, h) = (self.info.width, self.info.height);
        let n = w as usize * h as usize;
        let base = vec![0u32; s.axes.len()];
        let limits = |p: &NdPlane| {
            specular_core::nd::Limits::ZScale
                .compute(&p.values, p.width as usize)
                .filter(|(lo, hi)| hi > lo)
                .unwrap_or((0.0, 1.0))
        };
        let level = |x: f32, (lo, hi): (f32, f32)| -> f32 {
            if !x.is_finite() || hi <= lo {
                return 0.0;
            }
            ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
        let Some(ca) = s.channel_axis else {
            let p = self.plane(&base, cancel)?;
            let r = limits(&p);
            let grey: Vec<u8> = p
                .values
                .iter()
                .map(|&x| (level(x, r) * 255.0 + 0.5) as u8)
                .collect();
            return Ok(whole::interleaved(
                w,
                h,
                PixelLayout::Gray,
                grey,
                p.truncated,
            ));
        };
        let len = (s.axes[ca].len as usize).min(MAX_COMPOSITE);
        let colours = composite_colours(&s.channel_colours, len);
        let mut acc = vec![[0f32; 3]; n];
        let mut truncated = false;
        for (c, colour) in colours.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let mut idx = base.clone();
            idx[ca] = c as u32;
            let p = self.plane(&idx, cancel)?;
            truncated |= p.truncated;
            let r = limits(&p);
            for (a, &x) in acc.iter_mut().zip(&p.values) {
                let v = level(x, r);
                for k in 0..3 {
                    a[k] += v * colour[k];
                }
            }
        }
        let rgba: Vec<u8> = acc
            .iter()
            .flat_map(|a| {
                let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                [q(a[0]), q(a[1]), q(a[2]), 255]
            })
            .collect();
        Ok(whole::interleaved(w, h, PixelLayout::Rgba, rgba, truncated))
    }
}

/// The file's colours when it gives one per channel, else red, green, blue, then the
/// science view's further defaults.
fn composite_colours(own: &[[f32; 3]], n: usize) -> Vec<[f32; 3]> {
    if own.len() >= n {
        return own[..n].to_vec();
    }
    const DEFAULTS: [[f32; 3]; 8] = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.5, 0.0],
        [0.5, 0.5, 1.0],
    ];
    if n == 1 {
        return vec![[1.0; 3]];
    }
    (0..n).map(|k| DEFAULTS[k % DEFAULTS.len()]).collect()
}

impl Session for VendorSession {
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
        let planes = self.summary.channel_axis.map_or(1, |a| {
            u64::from(self.summary.axes[a].len).min(MAX_COMPOSITE as u64)
        });
        px * 4 * (planes + 3) + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        let own = match &*self.file {
            Parsed::Ics(f) => f.memory_bytes(),
            _ => 0,
        };
        self.levels.memory_bytes() + self.planes.memory_bytes() + own
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.file.pages()
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        match (&*self.file, &self.map) {
            (Parsed::Lif(l, pages), Map::Lif { image, .. }) if pages.len() > 1 => {
                let img = &l.images[*image];
                let name = if img.path.trim().is_empty() {
                    &img.name
                } else {
                    &img.path
                };
                Some(PageLabel::Sheet {
                    name: Arc::from(name.as_str()),
                    model: false,
                })
            }
            _ => None,
        }
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.file.pages() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(VendorSession::new(
            self.source.clone(),
            self.file.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        let rgb = match &*self.file {
            Parsed::Nd2(n) => n.channels.len() == 1 && n.components == 3,
            Parsed::Oir(_) | Parsed::Ics(_) => false,
            Parsed::Lif(l, _) => match self.map {
                Map::Lif { image, .. } => {
                    let c = &l.images[image].channels;
                    c.len() == 3 && c.iter().all(|c| c.tag != 0)
                }
                Map::Nd2 { .. } | Map::Oir { .. } | Map::Ics { .. } => false,
            },
        };
        Some(if rgb {
            "RGB"
        } else if self.summary.channel_axis.is_some() {
            "Multichannel"
        } else {
            "Greyscale"
        })
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
    use std::path::PathBuf;

    fn open(name: &str) -> Arc<dyn Session> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-micro/tests/data")
            .join(name);
        let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        crate::Registry::new()
            .open(Arc::new(Source::from_vec(p.clone(), bytes)))
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    #[test]
    fn opens_each_fixture() {
        let cancel = CancelToken::new();
        let d =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-micro/tests/data");
        let mut n = 0;
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                continue;
            }
            let name = p.file_name().unwrap().to_str().unwrap().to_owned();
            let first = open(&name);
            for page in 0..first.page_count() {
                let s = first.open_page(page).unwrap();
                let sci = s.science().expect("science summary");
                assert_eq!(sci.kind, ScienceKind::Microscopy, "{name}");
                let last: Vec<u32> = sci.axes.iter().map(|a| a.len - 1).collect();
                let pl = s.nd_plane(&last, &cancel).unwrap().unwrap();
                assert_eq!(pl.values.len(), (pl.width * pl.height) as usize);
                s.decode(&DecodeRequest::full(1), &cancel)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
            }
            n += 1;
        }
        assert!(n >= 7, "{}", d.display());
    }

    #[test]
    fn ics_axes_units_and_companions() {
        let cancel = CancelToken::new();
        let s = open("ics/be_s16_chxyt.ics");
        assert_eq!(s.info().format, Format::Ics);
        let sci = s.science().unwrap();
        let names: Vec<&str> = sci.axes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Channel", "T"]);
        assert_eq!(sci.axes[0].labels, ["450 nm", "520 nm", "610 nm"]);
        assert_eq!(sci.axes[1].unit, "s");
        assert_eq!(sci.pixel.as_ref().unwrap().unit_x, "µm");
        assert_eq!(sci.sample, "16-bit signed");
        // The .ids opens its .ics; gzip, complex and 64-bit samples read.
        let ids = open("ics/complex64.ids");
        let sci = ids.science().unwrap();
        assert_eq!(sci.axes[0].kind, AxisKind::Part);
        for name in [
            "ics/v1_u16_zyx.ics",
            "ics/v2_gzip_f32.ics",
            "ics/gz_u64.ics",
        ] {
            let s = open(name);
            let sci = s.science().unwrap();
            let last: Vec<u32> = sci.axes.iter().map(|a| a.len - 1).collect();
            let p = s.nd_plane(&last, &cancel).unwrap().unwrap();
            assert!(!p.truncated, "{name}");
            s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        }
        let z = open("ics/v1_u16_zyx.ics");
        let sci = z.science().unwrap();
        assert_eq!(sci.axes[0].name, "Z");
        assert_eq!(sci.axes[0].linear.unwrap().step, 0.5);
    }

    #[test]
    fn lif_series_are_pages() {
        let s = open("multi.lif");
        assert_eq!(s.info().format, Format::Lif);
        assert_eq!(s.page_count(), 4);
        let sci = s.science().unwrap();
        assert_eq!(sci.channel_axis, Some(0));
        assert_eq!(sci.axes[0].labels, ["FITC", "TRITC", "DAPI"]);
        assert_eq!(sci.channel_colours[0], [0.0, 1.0, 0.0]);
        assert_eq!(sci.axes[1].name, "Z");
        assert!(sci.pixel.is_some());
        let p = s.open_page(1).unwrap();
        let sci = p.science().unwrap();
        assert_eq!(sci.axes[0].len, 6);
        assert_eq!(sci.channel_colours.len(), 6);
        assert!(matches!(p.page_label(), Some(PageLabel::Sheet { .. })));
        assert_eq!(p.colour_model(), Some("Multichannel"));
        assert_eq!(s.open_page(2).unwrap().colour_model(), Some("RGB"));
    }

    #[test]
    fn nd2_loops_are_axes() {
        let s = open("loops_v3.nd2");
        assert_eq!(s.info().format, Format::Nd2);
        let sci = s.science().unwrap();
        let names: Vec<&str> = sci.axes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Channel", "Z", "XY", "T"]);
        assert_eq!(sci.axes[0].len, 5);
        assert_eq!(sci.channel_colours.len(), 5);
        assert_eq!(sci.axes[2].labels.len(), sci.axes[2].len as usize);
        assert!(sci.pixel.is_some());
        // A composite of five channels is not grey.
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.width, s.info().width);
        let legacy = open("legacy_v1.nd2");
        assert_eq!(legacy.info().format, Format::Nd2);
        assert_eq!(legacy.science().unwrap().axes[0].len, 2);
    }
}

/// Synthetic hostile inputs: none may panic or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    fn channel(components: usize) -> nd2::Channel {
        nd2::Channel {
            name: "GFP".into(),
            colour: Some([0, 255, 0]),
            components,
            emission: None,
        }
    }

    /// A component count of four billion (`uiComp`) is refused rather than labelled.
    #[test]
    fn huge_component_count_is_refused() {
        let r = component_labels(&[channel(1)], 4_000_000_000);
        assert!(matches!(r, Err(DecodeError::TooLarge)));
    }

    /// A channel claiming more components than the frame has adds no more than the frame's.
    #[test]
    fn channel_components_past_the_frame_are_clamped() {
        let (labels, colours) = component_labels(&[channel(usize::MAX), channel(2)], 4).unwrap();
        assert_eq!((labels.len(), colours.len()), (4, 4));
        // Valid: an RGB camera and a grey channel.
        let (labels, _) = component_labels(&[channel(3), channel(1)], 4).unwrap();
        assert_eq!(labels, ["GFP R", "GFP G", "GFP B", "GFP"]);
    }
}
