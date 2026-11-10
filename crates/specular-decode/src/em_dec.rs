//! Microscopy images through the N-D science path: electron microscopy (Gatan
//! DigitalMicrograph DM3 / DM4 and MRC / MRCS through `specular-em`) and scanning probe
//! microscopy (Gwyddion `.gwy` and Igor binary waves through `specular-spm`).
//!
//! * **DigitalMicrograph:** one page per image of the `ImageList` (thumbnails skipped);
//!   dimensions past the second are cube axes with their calibrations, colour images a
//!   red / green / blue channel axis; the first two calibrations give physical pixel
//!   positions.
//! * **MRC:** sections (a stack or a volume) are a cube axis; the voxel size (or the FEI
//!   extended header's pixel size) gives pixel positions. Rows are stored bottom-up.
//!   Crystallographic maps stored in another axis order (MAPC, MAPR, MAPS) show as x, y
//!   and a z slider, from their start indices; volumes (not image stacks) offer MPR.
//! * **Gwyddion:** channels of one size share a page as a channel axis named by their titles,
//!   each with its own value unit; other sizes are further pages.
//! * **Igor:** the third dimension is a channel axis when it has labels (Asylum Research's
//!   height, amplitude, phase…), the fourth a cube axis; rows are stored bottom-up.
//! * **SPIDER:** a stack's images and a volume's slices are cube axes; the pixel size (Å)
//!   gives pixel positions. Recognised by a header that adds up (no signature).
//!
//! Scanning-probe pages offer plane levelling ([`ScienceSummary::levelling`]).

use std::sync::Arc;

use crate::medvol;
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_em::samples::Sample;
use specular_em::{dm, mrc};
use specular_spm::{gwy, ibw};

use crate::science::{
    self, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub(crate) fn em_err(e: specular_em::Error) -> DecodeError {
    use specular_em::Error as E;
    match e {
        E::NotDm | E::NotMrc => DecodeError::UnsupportedFormat,
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NoImage => DecodeError::Unsupported("no image in the file".into()),
    }
}

pub(crate) fn spm_err(e: specular_spm::Error) -> DecodeError {
    use specular_spm::Error as E;
    match e {
        E::NotGwy | E::NotIbw | E::NotNanoscope | E::NotSxm | E::NotJpk | E::NotMdt => {
            DecodeError::UnsupportedFormat
        }
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NoImage => DecodeError::Unsupported("no image in the file".into()),
    }
}

/// Which of the four formats a decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dm,
    Mrc,
    /// DeltaVision, by content only.
    Dv,
    Gwy,
    Ibw,
    /// SPIDER (`.spi`, `.spider`): a header that adds up, no signature.
    Spider,
}

pub struct MicroDecoder(pub Kind);

/// Compressed bytes inflated to probe a gzip CCP4/MRC map.
const GZ_PROBE: usize = 128 << 10;
/// Largest inflated `.map.gz`.
const MAX_GZ: usize = 16 << 30;

impl Decoder for MicroDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Dm => Format::Dm,
            Kind::Mrc => Format::Mrc,
            Kind::Dv => Format::DeltaVision,
            Kind::Gwy => Format::Gwyddion,
            Kind::Ibw => Format::Igor,
            Kind::Spider => Format::Spider,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Dm => dm::sniff(head).is_some(),
            Kind::Gwy => gwy::sniff(head),
            Kind::Dv => specular_em::dv::sniff(head),
            // MRC needs its 1024-byte header (see `probe_file`); Igor waves have no
            // signature worth trusting: both open by extension.
            Kind::Mrc | Kind::Ibw | Kind::Spider => false,
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            // `.map.gz` (and any gzip CCP4/MRC map): the header is inflated to look.
            Kind::Mrc => crate::sniff::is_mrc_head(&data[..data.len().min(GZ_PROBE)]),
            // The Priism ID is at byte 96, past the probe's head.
            Kind::Dv => specular_em::dv::sniff(data),
            Kind::Spider => false,
            _ => self.probe(&data[..data.len().min(crate::PROBE_HEAD)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Dm => &["dm3", "dm4"],
            // `.map` and `.map.gz` are claimed by content only (see `crate::sniff`).
            Kind::Mrc => &["mrc", "mrcs", "ccp4"],
            // `.dv` is DV video's: DeltaVision files are recognised by content.
            Kind::Dv => &[],
            Kind::Gwy => &["gwy"],
            Kind::Ibw => &["ibw"],
            Kind::Spider => &["spi", "spider"],
        }
    }
    /// SPIDER's header is checked for consistency only: likely, never certain.
    fn confidence(&self, data: &[u8]) -> Option<crate::Confidence> {
        match self.0 {
            Kind::Spider => specular_em::spider::sniff(data).then_some(crate::Confidence::Likely),
            _ => self.probe_file(data).then_some(crate::Confidence::Certain),
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let source = if self.0 == Kind::Mrc && source.bytes().starts_with(&[0x1f, 0x8b]) {
            let data = specular_png::inflate_gzip_members(source.bytes(), MAX_GZ, None)
                .map_err(|_| DecodeError::Malformed("gzip".into()))?;
            Arc::new(Source::from_vec(source.path(), data))
        } else {
            source
        };
        let data = source.bytes();
        let file = match self.0 {
            Kind::Dm => {
                let d = dm::parse(data).map_err(em_err)?;
                if d.images.is_empty() {
                    return Err(DecodeError::Unsupported("no image in the file".into()));
                }
                Parsed::Dm(d)
            }
            Kind::Mrc => Parsed::Mrc(Box::new(mrc::parse(data).map_err(em_err)?)),
            Kind::Dv => Parsed::Dv(Box::new(specular_em::dv::parse(data).map_err(em_err)?)),
            Kind::Gwy => {
                let g = gwy::parse(data).map_err(spm_err)?;
                let mut groups: Vec<Vec<usize>> = Vec::new();
                for (i, c) in g.channels.iter().enumerate() {
                    let c0 = |grp: &Vec<usize>| &g.channels[grp[0]];
                    match groups
                        .iter_mut()
                        .find(|grp| (c0(grp).xres, c0(grp).yres) == (c.xres, c.yres))
                    {
                        Some(grp) => grp.push(i),
                        None => groups.push(vec![i]),
                    }
                }
                if groups.is_empty() {
                    return Err(DecodeError::Unsupported("no image in the file".into()));
                }
                Parsed::Gwy(g, groups)
            }
            Kind::Ibw => Parsed::Ibw(Box::new(ibw::parse(data).map_err(spm_err)?)),
            Kind::Spider => {
                Parsed::Spider(Box::new(specular_em::spider::parse(data).map_err(em_err)?))
            }
        };
        Ok(Arc::new(MicroSession::new(source, Arc::new(file), 0)?))
    }
}

enum Parsed {
    Dm(dm::Dm),
    Mrc(Box<mrc::Mrc>),
    Dv(Box<specular_em::dv::Dv>),
    /// The file and its pages: channels of one size each.
    Gwy(gwy::Gwy, Vec<Vec<usize>>),
    Ibw(Box<ibw::Ibw>),
    Spider(Box<specular_em::spider::Spider>),
}

impl Parsed {
    fn pages(&self) -> u32 {
        match self {
            Parsed::Dm(d) => d.images.len() as u32,
            Parsed::Gwy(_, g) => g.len() as u32,
            Parsed::Mrc(_) | Parsed::Dv(_) | Parsed::Ibw(_) | Parsed::Spider(_) => 1,
        }
    }
}

struct MicroSession {
    source: Arc<Source>,
    file: Arc<Parsed>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    /// Whether stored rows run bottom-up.
    flip: bool,
    planes: PlaneCache,
    levels: Levels,
    /// A map's volume for MPR and the 3-D view (an MRC volume, not an image stack).
    volume: Option<crate::VolumeFacts>,
}

fn nonempty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn axis(len: usize, unit: &str, linear: Option<Linear>, kind: AxisKind) -> Result<NdAxis> {
    Ok(NdAxis {
        name: String::new(),
        len: u32::try_from(len).map_err(|_| DecodeError::TooLarge)?,
        unit: unit.trim().to_owned(),
        labels: Vec::new(),
        wcs_axis: None,
        linear,
        kind,
    })
}

fn sample_text(s: Sample, complex: bool) -> String {
    if complex {
        format!("{}, magnitude shown", s.describe())
    } else {
        s.describe().to_owned()
    }
}

fn kw(k: impl Into<String>, v: impl ToString) -> (String, String, String) {
    (k.into(), v.to_string(), String::new())
}

impl MicroSession {
    fn new(source: Arc<Source>, file: Arc<Parsed>, page: u32) -> Result<MicroSession> {
        let (format, mut summary, bits, flip) = match &*file {
            Parsed::Dm(d) => {
                let (s, b) = dm_summary(&d.images[page as usize])?;
                (Format::Dm, s, b, false)
            }
            Parsed::Mrc(m) => {
                let mdoc = specular_em::mdoc::read_beside(source.path());
                let (s, b) = mrc_summary(m, mdoc.as_ref())?;
                (Format::Mrc, s, b, true)
            }
            Parsed::Dv(d) => {
                let (s, b) = dv_summary(d)?;
                (Format::DeltaVision, s, b, true)
            }
            Parsed::Gwy(g, groups) => (
                Format::Gwyddion,
                gwy_summary(g, &groups[page as usize])?,
                64,
                false,
            ),
            Parsed::Ibw(w) => {
                let (s, b) = ibw_summary(w)?;
                (Format::Igor, s, b, true)
            }
            Parsed::Spider(sp) => (Format::Spider, spider_summary(sp)?, 32, false),
        };
        if summary.width == 0 || summary.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        if summary.planes() == 0 {
            return Err(DecodeError::Malformed("no planes".into()));
        }
        summary.title = summary.title.take().and_then(|t| nonempty(&t));
        let info = ImageInfo {
            format,
            width: summary.width,
            height: summary.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: bits.min(64) as u8,
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let volume = match &*file {
            Parsed::Mrc(m) if m.is_volume() => medvol::grids::map_header(m)
                .ok()
                .map(|h| crate::medvol_dec::facts_of(&h)),
            _ => None,
        };
        Ok(MicroSession {
            source,
            file,
            page,
            info,
            summary,
            flip,
            planes: PlaneCache::new(),
            levels: Levels::new(),
            volume,
        })
    }

    /// The stored plane number of an index (first axis fastest).
    fn plane_number(&self, index: &[u32]) -> Result<usize> {
        let (mut p, mut stride) = (0usize, 1usize);
        for (k, a) in self.summary.axes.iter().enumerate() {
            let i = index.get(k).copied().unwrap_or(0);
            if i >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
            p += i as usize * stride;
            stride *= a.len as usize;
        }
        Ok(p)
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let p = self.plane_number(index)?;
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let mut values = Vec::new();
        values
            .try_reserve_exact(w * h)
            .map_err(|_| DecodeError::TooLarge)?;
        values.resize(w * h, 0f32);
        let data = self.source.bytes();
        let truncated = match &*self.file {
            Parsed::Dm(d) => d.images[self.page as usize]
                .read_plane(data, p, &mut values)
                .map_err(em_err)?,
            Parsed::Mrc(m) => m.read_xyz(data, p, &mut values).map_err(em_err)?,
            Parsed::Dv(d) => {
                // The axes are the sequence's (fastest first) with more than one position.
                let mut at = [0usize; 3];
                let mut rest = p;
                for (k, l) in d.sequence.letters().into_iter().enumerate() {
                    let n = d.len(l);
                    if n > 1 {
                        at[k] = rest % n;
                        rest /= n;
                    }
                }
                let pos = |c: char| {
                    let k = d
                        .sequence
                        .letters()
                        .iter()
                        .position(|&l| l == c)
                        .unwrap_or(0);
                    at[k]
                };
                let section = d.section(pos('z'), pos('w'), pos('t'));
                d.mrc
                    .read_plane(data, section, &mut values)
                    .map_err(em_err)?
            }
            Parsed::Gwy(g, groups) => {
                let c = groups[self.page as usize]
                    .get(p)
                    .ok_or_else(|| DecodeError::Malformed("no such channel".into()))?;
                g.channels[*c]
                    .read_plane(data, &mut values)
                    .map_err(spm_err)?
            }
            Parsed::Ibw(wv) => wv.read_plane(data, p, &mut values).map_err(spm_err)?,
            Parsed::Spider(sp) => sp.read_plane(data, p, &mut values).map_err(em_err)?,
        };
        if self.flip && w > 0 {
            for y in 0..h / 2 {
                let (top, bottom) = values.split_at_mut((h - 1 - y) * w);
                top[y * w..(y + 1) * w].swap_with_slice(&mut bottom[..w]);
            }
        }
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

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let n = self.summary.axes.len();
        let planes = match self.summary.channel_axis {
            Some(c) => (0..self.summary.axes[c].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[c] = k;
                    self.plane(&idx, cancel)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![self.plane(&vec![0; n], cancel)?],
        };
        Ok(science::auto_raster(&planes, None))
    }
}

/// A calibration as a linear function of the index, unless it is the identity.
fn em_linear(c: &specular_em::Calibration) -> Option<Linear> {
    (!c.is_identity() && c.scale.is_finite() && c.scale != 0.0 && c.origin.is_finite()).then(|| {
        Linear {
            offset: c.at(0.0),
            step: c.scale,
        }
    })
}

fn dm_summary(img: &dm::DmImage) -> Result<(ScienceSummary, u32)> {
    let mut axes = Vec::new();
    let mut channel_axis = None;
    if img.is_rgb() {
        let mut a = axis(3, "", None, AxisKind::Channel)?;
        a.labels = ["Red", "Green", "Blue"].map(String::from).to_vec();
        axes.push(a);
        channel_axis = Some(0);
    }
    for (k, &len) in img.dims.iter().enumerate().skip(2) {
        let cal = img.calibrations.get(k).cloned().unwrap_or_default();
        axes.push(axis(len, &cal.units, em_linear(&cal), AxisKind::Cube)?);
    }
    let pixel = match (img.calibrations.first(), img.calibrations.get(1)) {
        (Some(cx), cy) if img.height() > 1 => {
            let cy = cy.cloned().unwrap_or_default();
            match (em_linear(cx), em_linear(&cy)) {
                (None, None) => None,
                (x, y) => Some(PixelCalibration {
                    x: x.unwrap_or(Linear {
                        offset: 0.0,
                        step: 1.0,
                    }),
                    y: y.unwrap_or(Linear {
                        offset: 0.0,
                        step: 1.0,
                    }),
                    unit_x: cx.units.clone(),
                    unit_y: cy.units.clone(),
                }),
            }
        }
        _ => None,
    };
    let m = &img.meta;
    let sample = if img.is_rgb() {
        "8-bit RGB".to_owned()
    } else {
        sample_text(img.sample(), img.is_complex())
    };
    let keywords = img.fields.iter().map(|(k, v)| kw(k.clone(), v)).collect();
    let summary = ScienceSummary {
        width: u32::try_from(img.width()).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(img.height()).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis,
        sample,
        instrument: m.microscope.clone(),
        date_obs: m.acquired.clone(),
        exposure: m.exposure,
        unit: img.value_unit().map(str::to_owned),
        keywords: Arc::new(keywords),
        kind: ScienceKind::ElectronMicroscopy,
        pixel,
        title: img.name.clone(),
        voltage: m.voltage,
        magnification: m.magnification,
        mode: m.mode.clone().or_else(|| m.signal.clone()),
        detector: m.device.clone(),
        ..Default::default()
    };
    Ok((summary, img.sample().bits()))
}

/// The most tilts listed one per row in the info panel.
const MAX_TILT_ROWS: usize = 200;

fn mrc_summary(
    m: &mrc::Mrc,
    mdoc: Option<&specular_em::mdoc::Mdoc>,
) -> Result<(ScienceSummary, u32)> {
    // Sections along z whatever the stored axis order (crystallographic maps keep y or z
    // fastest): x across, y up.
    let [sx, sy, sz] = m.size_xyz();
    if sx.saturating_mul(sy) as u64 > specular_em::MAX_PLANE {
        return Err(DecodeError::TooLarge);
    }
    let origin = m.origin_xyz();
    // Ångström voxel sizes, else the FEI header's pixel size (metres).
    let size = m
        .voxel
        .filter(|v| v[0] > 0.0 && v[1] > 0.0)
        .map(|v| (v[0] * 1e-10, v[1] * 1e-10, Some(v[2] * 1e-10)))
        .or_else(|| {
            let (x, y) = m.fei.as_ref()?.pixel_size?;
            (x > 0.0 && y > 0.0).then_some((x, y, None))
        });
    let mut axes = Vec::new();
    if sz > 1 {
        let z = size
            .and_then(|s| s.2)
            .filter(|&z| z > 0.0 && m.is_volume())
            .map(|z| Linear {
                offset: origin[2] * 1e-10,
                step: z,
            });
        let mut a = axis(sz, if z.is_some() { "m" } else { "" }, z, AxisKind::Cube)?;
        // A SerialEM tilt series: each section's tilt angle from the `.mdoc` beside it.
        if let Some(tilts) = mdoc.and_then(|d| d.tilts(sz)).filter(|_| !m.reordered()) {
            a.name = "Tilt".into();
            a.labels = tilts.iter().map(|t| format!("{t:.1}°")).collect();
        }
        axes.push(a);
    }
    let pixel = size.map(|(x, y, _)| {
        let (ox, oy) = (origin[0] * 1e-10, origin[1] * 1e-10);
        // Stored bottom-up: displayed row r is stored row ny − 1 − r.
        PixelCalibration {
            x: Linear {
                offset: ox,
                step: x,
            },
            y: Linear {
                offset: oy + (sy as f64 - 1.0) * y,
                step: -y,
            },
            unit_x: "m".into(),
            unit_y: "m".into(),
        }
    });
    let mut sample = match m.storage {
        mrc::Storage::Nibbles => "4-bit unsigned".to_owned(),
        mrc::Storage::Plain(s) => sample_text(s, s.is_complex()),
    };
    sample += &format!(", mode {}", m.mode);
    let three = |v: [f32; 3]| format!("{} {} {}", v[0], v[1], v[2]);
    let mut keywords = vec![
        kw("NX", m.nx),
        kw("NY", m.ny),
        kw("NZ", m.nz),
        kw("MODE", m.mode),
        kw(
            "NXSTART NYSTART NZSTART",
            format!("{} {} {}", m.start[0], m.start[1], m.start[2]),
        ),
        kw(
            "MXYZ",
            format!("{} {} {}", m.sampling[0], m.sampling[1], m.sampling[2]),
        ),
        kw("CELLA", three(m.cell)),
        kw("CELLB", three(m.angles)),
        kw(
            "MAPC MAPR MAPS",
            format!("{} {} {}", m.axes[0] + 1, m.axes[1] + 1, m.axes[2] + 1),
        ),
        kw("ORIGIN", three(m.origin)),
        kw("DMIN", m.dmin),
        kw("DMAX", m.dmax),
        kw("DMEAN", m.dmean),
        kw("RMS", m.rms),
        kw("ISPG", m.ispg),
        kw("NVERSION", m.nversion),
        kw("NSYMBT", m.nsymbt),
    ];
    if m.reordered() {
        keywords.insert(0, kw("Grid (x × y × z)", format!("{sx} × {sy} × {sz}")));
    }
    if m.exttyp.iter().any(|&b| b.is_ascii_graphic()) {
        keywords.push(kw("EXTTYP", String::from_utf8_lossy(&m.exttyp).trim()));
    }
    for (i, l) in m.labels.iter().enumerate() {
        keywords.push(kw(format!("LABEL{}", i + 1), l));
    }
    // A SerialEM `.mdoc` beside the file: the pixel size when the header has none, its global
    // values and titles, and each tilt's values.
    let mut pixel = pixel;
    if let Some(d) = mdoc {
        if pixel.is_none()
            && let Some(p) = d.pixel_spacing()
        {
            let s = p * 1e-10;
            pixel = Some(PixelCalibration {
                x: Linear {
                    offset: 0.0,
                    step: s,
                },
                y: Linear {
                    offset: (sy as f64 - 1.0) * s,
                    step: -s,
                },
                unit_x: "m".into(),
                unit_y: "m".into(),
            });
        }
        for (k, v) in &d.global {
            keywords.push(kw(format!("mdoc {k}"), v));
        }
        for t in &d.titles {
            keywords.push(kw("mdoc title", t));
        }
        let show = [
            "TiltAngle",
            "Defocus",
            "ExposureDose",
            "Magnification",
            "DateTime",
        ];
        for z in 0..sz.min(MAX_TILT_ROWS) {
            let Some(sec) = d.z(z) else { continue };
            let parts: Vec<String> = show
                .iter()
                .filter_map(|&k| sec.get(k).map(|v| format!("{k} {v}")))
                .collect();
            if !parts.is_empty() {
                keywords.push(kw(format!("Section {z}"), parts.join(", ")));
            }
        }
    }
    let fei = m.fei.clone().unwrap_or_default();
    if let Some(a) = &fei.application {
        keywords.push(kw("Application", a));
    }
    if let Some(c) = fei.camera_length {
        keywords.push(kw("Camera length (m)", c));
    }
    if let Some((x, y)) = fei.binning {
        keywords.push(kw("Binning", format!("{x} × {y}")));
    }
    if let Some(d) = fei.defocus {
        keywords.push(kw("Defocus (m)", d));
    }
    let summary = ScienceSummary {
        width: u32::try_from(sx).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(sy).map_err(|_| DecodeError::TooLarge)?,
        axes,
        sample,
        instrument: fei.microscope,
        exposure: fei.exposure,
        keywords: Arc::new(keywords),
        kind: ScienceKind::ElectronMicroscopy,
        pixel,
        voltage: fei.voltage,
        magnification: fei.magnification,
        mode: fei.mode.map(str::to_owned),
        detector: fei.camera,
        ..Default::default()
    };
    Ok((summary, m.bits()))
}

/// DeltaVision: wavelengths as channels, Z and time as series, in the file's section order.
fn dv_summary(d: &specular_em::dv::Dv) -> Result<(ScienceSummary, u32)> {
    let m = &d.mrc;
    let mut axes = Vec::new();
    let mut channel_axis = None;
    for l in d.sequence.letters() {
        let n = d.len(l);
        if n <= 1 {
            continue;
        }
        let mut a = match l {
            'w' => {
                channel_axis = Some(axes.len());
                let mut a = axis(n, "", None, AxisKind::Channel)?;
                a.labels = d
                    .waves
                    .iter()
                    .map(|&w| {
                        if w > 0 {
                            format!("{w} nm")
                        } else {
                            String::new()
                        }
                    })
                    .collect();
                if a.labels.iter().all(String::is_empty) {
                    a.labels.clear();
                }
                a
            }
            'z' => {
                let step = f64::from(d.pixel[2]);
                let lin = (step > 0.0).then(|| Linear {
                    offset: f64::from(d.origin[0]),
                    step,
                });
                let mut a = axis(
                    n,
                    if lin.is_some() { "µm" } else { "" },
                    lin,
                    AxisKind::Cube,
                )?;
                a.name = "Z".into();
                a
            }
            _ => {
                // The time stamps of the first two time points of the first Z and channel.
                let times: Vec<f64> = (0..n.min(2))
                    .filter_map(|t| d.sections.get(d.section(0, 0, t)))
                    .map(|s| f64::from(s.time))
                    .collect();
                let lin = (times.len() == 2 && times[1] > times[0]).then(|| Linear {
                    offset: times[0],
                    step: times[1] - times[0],
                });
                let mut a = axis(n, if lin.is_some() { "s" } else { "" }, lin, AxisKind::Cube)?;
                a.name = "Time".into();
                a
            }
        };
        a.name = a.name.trim().to_owned();
        axes.push(a);
    }
    let (px, py) = (f64::from(d.pixel[0]), f64::from(d.pixel[1]));
    let pixel = (px > 0.0 && py > 0.0).then(|| PixelCalibration {
        x: Linear {
            offset: f64::from(d.origin[1]),
            step: px,
        },
        // Stored bottom-up: displayed row r is stored row ny − 1 − r.
        y: Linear {
            offset: f64::from(d.origin[2]) + (m.ny as f64 - 1.0) * py,
            step: -py,
        },
        unit_x: "µm".into(),
        unit_y: "µm".into(),
    });
    let mut keywords = vec![
        kw("Sections", m.nz),
        kw("Z", d.nz),
        kw("Wavelengths", d.nw),
        kw("Time points", d.nt),
        kw(
            "Sequence",
            d.sequence
                .letters()
                .iter()
                .map(|c| c.to_ascii_uppercase())
                .collect::<String>(),
        ),
        kw("Pixel type", m.mode),
    ];
    if d.lens != 0 {
        keywords.push(kw("Lens ID", d.lens));
    }
    for (i, t) in d.titles.iter().enumerate() {
        keywords.push(kw(format!("Title {}", i + 1), t));
    }
    let first = d.sections.first();
    if let Some(s) = first.filter(|s| s.excitation > 0.0) {
        keywords.push(kw("Excitation (nm)", s.excitation));
    }
    if let Some(s) = first.filter(|s| s.emission > 0.0) {
        keywords.push(kw("Emission (nm)", s.emission));
    }
    let summary = ScienceSummary {
        width: u32::try_from(m.nx).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(m.ny).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis,
        sample: sample_text(m.sample(), false),
        exposure: first.map(|s| f64::from(s.exposure)).filter(|e| *e > 0.0),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        title: d.titles.first().cloned(),
        ..Default::default()
    };
    Ok((summary, m.bits()))
}

/// SPIDER: a stack's images, then a volume's slices, as cube axes (slices fastest).
fn spider_summary(sp: &specular_em::spider::Spider) -> Result<ScienceSummary> {
    let mut axes = Vec::new();
    if sp.nz > 1 {
        let mut a = axis(sp.nz, "", None, AxisKind::Cube)?;
        a.name = "Slice".into();
        axes.push(a);
    }
    if sp.images > 1 {
        let mut a = axis(sp.images, "", None, AxisKind::Cube)?;
        a.name = "Image".into();
        axes.push(a);
    }
    let pixel = sp.pixel_size.map(|px| {
        let px = f64::from(px);
        PixelCalibration {
            x: Linear {
                offset: 0.0,
                step: px,
            },
            y: Linear {
                offset: 0.0,
                step: px,
            },
            unit_x: "Å".into(),
            unit_y: "Å".into(),
        }
    });
    let mut keywords = vec![kw(
        "Kind",
        match (sp.stack, sp.volume) {
            (true, true) => "stack of volumes",
            (true, false) => "image stack",
            (false, true) => "volume",
            (false, false) => "image",
        },
    )];
    if sp.images > 1 {
        keywords.push(kw("Images", sp.images));
    }
    if sp.nz > 1 {
        keywords.push(kw("Slices", sp.nz));
    }
    if let Some([max, min, mean, sd]) = sp.stats {
        keywords.push(kw("Maximum", max));
        keywords.push(kw("Minimum", min));
        keywords.push(kw("Mean", mean));
        keywords.push(kw("Standard deviation", sd));
    }
    if sp.scale != 0.0 && sp.scale.is_finite() {
        keywords.push(kw("Scale", sp.scale));
    }
    if !sp.date.is_empty() {
        keywords.push(kw("Date", format!("{} {}", sp.date, sp.time).trim()));
    }
    keywords.push(kw(
        "Byte order",
        match sp.order {
            specular_em::samples::Order::Big => "big endian",
            specular_em::samples::Order::Little => "little endian",
        },
    ));
    Ok(ScienceSummary {
        width: u32::try_from(sp.nx).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(sp.ny).map_err(|_| DecodeError::TooLarge)?,
        axes,
        channel_axis: None,
        sample: Sample::F32.describe().to_owned(),
        keywords: Arc::new(keywords),
        kind: ScienceKind::Microscopy,
        pixel,
        title: (!sp.title.is_empty()).then(|| sp.title.clone()),
        ..Default::default()
    })
}

fn gwy_summary(g: &gwy::Gwy, group: &[usize]) -> Result<ScienceSummary> {
    let chans: Vec<&gwy::Channel> = group.iter().map(|&i| &g.channels[i]).collect();
    let first = chans[0];
    let mut axes = Vec::new();
    let mut units_by = None;
    if chans.len() > 1 {
        let mut a = axis(chans.len(), "", None, AxisKind::Channel)?;
        a.labels = chans
            .iter()
            .map(|c| c.title.clone().unwrap_or_default())
            .collect();
        axes.push(a);
        units_by = Some((0, chans.iter().map(|c| c.unit_z.clone()).collect()));
    }
    let (dx, dy) = (
        first.xreal / first.xres.max(1) as f64,
        first.yreal / first.yres.max(1) as f64,
    );
    let pixel =
        (dx.is_finite() && dy.is_finite() && dx > 0.0 && dy > 0.0).then(|| PixelCalibration {
            x: Linear {
                offset: first.xoff + 0.5 * dx,
                step: dx,
            },
            y: Linear {
                offset: first.yoff + 0.5 * dy,
                step: dy,
            },
            unit_x: first.unit_xy.clone(),
            unit_y: first.unit_xy.clone(),
        });
    let mut keywords = Vec::new();
    for c in &chans {
        let prefix = match (&c.title, chans.len()) {
            (Some(t), 2..) => format!("{t}: "),
            _ => String::new(),
        };
        for (k, v) in &c.meta {
            keywords.push(kw(format!("{prefix}{k}"), v));
        }
    }
    let meta = |key: &str| {
        first
            .meta
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| nonempty(v))
    };
    Ok(ScienceSummary {
        width: u32::try_from(first.xres).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(first.yres).map_err(|_| DecodeError::TooLarge)?,
        axes,
        sample: Sample::F64.describe().to_owned(),
        date_obs: meta("Date"),
        unit: nonempty(&first.unit_z),
        keywords: Arc::new(keywords),
        kind: ScienceKind::ScanningProbe,
        pixel,
        units_by,
        title: if chans.len() == 1 {
            first.title.clone()
        } else {
            None
        },
        mode: meta("Mode").or_else(|| meta("Imaging mode")),
        levelling: true,
        ..Default::default()
    })
}

fn ibw_summary(w: &ibw::Ibw) -> Result<(ScienceSummary, u32)> {
    let mut axes = Vec::new();
    for (k, d) in w.dims.iter().enumerate().skip(2) {
        let named = d.labels.iter().any(|l| !l.trim().is_empty());
        let mut a = if named && k == 2 {
            axis(d.len, "", None, AxisKind::Channel)?
        } else {
            let lin = (d.delta.is_finite()
                && d.offset.is_finite()
                && d.delta != 0.0
                && !(d.delta == 1.0 && d.offset == 0.0 && d.unit.is_empty()))
            .then_some(Linear {
                offset: d.offset,
                step: d.delta,
            });
            axis(d.len, &d.unit, lin, AxisKind::Cube)?
        };
        if named {
            a.labels = (0..d.len)
                .map(|i| d.labels.get(i).cloned().unwrap_or_default())
                .collect();
        }
        a.name = d.label.clone().unwrap_or_default();
        axes.push(a);
    }
    let h = w.height();
    let pixel = match (w.dims.first(), w.dims.get(1)) {
        (Some(x), Some(y))
            if h > 1
                && [x.delta, y.delta, x.offset, y.offset]
                    .iter()
                    .all(|v| v.is_finite())
                && x.delta != 0.0
                && y.delta != 0.0 =>
        {
            Some(PixelCalibration {
                x: Linear {
                    offset: x.offset,
                    step: x.delta,
                },
                // Stored bottom-up.
                y: Linear {
                    offset: y.offset + (h as f64 - 1.0) * y.delta,
                    step: -y.delta,
                },
                unit_x: x.unit.clone(),
                unit_y: y.unit.clone(),
            })
        }
        _ => None,
    };
    let pairs = w.note_pairs();
    let note = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| nonempty(v))
    };
    // Asylum writes numeric codes for some keys in older versions; keep only words.
    let mode = note("ImagingMode").filter(|m| m.parse::<f64>().is_err());
    let date = match (note("Date"), note("Time")) {
        (Some(d), Some(t)) => Some(format!("{d} {t}")),
        (d, _) => d,
    };
    let mut keywords: Vec<_> = pairs.iter().map(|(k, v)| kw(k.clone(), v)).collect();
    keywords.insert(0, kw("Igor version", w.version));
    let summary = ScienceSummary {
        width: u32::try_from(w.width()).map_err(|_| DecodeError::TooLarge)?,
        height: u32::try_from(h).map_err(|_| DecodeError::TooLarge)?,
        axes,
        sample: sample_text(w.sample, w.sample.is_complex()),
        instrument: note("MicroscopeModel"),
        date_obs: date,
        unit: nonempty(&w.data_unit),
        keywords: Arc::new(keywords),
        kind: ScienceKind::ScanningProbe,
        pixel,
        title: nonempty(&w.name),
        mode,
        levelling: true,
        ..Default::default()
    };
    Ok((summary, w.sample.bits()))
}

impl Session for MicroSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn research_volume(&self) -> Option<&crate::VolumeFacts> {
        self.volume.as_ref()
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
        let planes = if self.summary.channel_axis.is_some() {
            3
        } else {
            1
        };
        px * 4 * planes + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
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
    fn page_count(&self) -> u32 {
        self.file.pages()
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.pages() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.file.pages() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(MicroSession::new(
            self.source.clone(),
            self.file.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.summary.channel_axis.is_some() {
            "RGB"
        } else if self
            .summary
            .axes
            .iter()
            .any(|a| a.kind == AxisKind::Channel)
        {
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

    fn open(dir: &str, name: &str) -> Arc<dyn Session> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../formats")
            .join(dir)
            .join("tests/data")
            .join(name);
        let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        crate::Registry::new()
            .open(Arc::new(Source::from_vec(p.clone(), bytes)))
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    #[test]
    fn opens_each_fixture() {
        let cancel = CancelToken::new();
        for dir in ["specular-em", "specular-spm"] {
            let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../formats")
                .join(dir)
                .join("tests/data");
            let mut n = 0;
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    continue;
                }
                let name = p.file_name().unwrap().to_str().unwrap().to_owned();
                let s = open(dir, &name);
                let sci = s.science().expect("science summary");
                assert_ne!(sci.kind, ScienceKind::Astronomy, "{name}");
                let pl = s
                    .nd_plane(&vec![0; sci.axes.len()], &cancel)
                    .unwrap()
                    .unwrap();
                assert_eq!(pl.values.len(), (pl.width * pl.height) as usize);
                s.decode(&DecodeRequest::full(1), &cancel)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                n += 1;
            }
            assert!(n >= 7, "{}", d.display());
        }
    }

    #[test]
    fn formats_and_layout() {
        let s = open("specular-em", "stack.mrcs");
        assert_eq!(s.info().format, Format::Mrc);
        let sci = s.science().unwrap();
        assert_eq!(sci.axes.len(), 1);
        assert!(sci.axes[0].len > 1);
        let s = open("specular-em", "rgb.dm4");
        assert_eq!(s.info().format, Format::Dm);
        assert_eq!(s.science().unwrap().channel_axis, Some(0));
        let s = open("specular-spm", "channels.gwy");
        assert_eq!(s.info().format, Format::Gwyddion);
        let sci = s.science().unwrap();
        assert!(sci.levelling);
        assert!(sci.pixel.is_some());
        let s = open("specular-spm", "height.ibw");
        assert_eq!(s.info().format, Format::Igor);
        assert!(s.science().unwrap().levelling);
    }

    #[test]
    fn mrc_rows_are_flipped() {
        let cancel = CancelToken::new();
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-em/tests/data/mode2.mrc");
        let bytes = std::fs::read(&p).unwrap();
        let m = mrc::parse(&bytes).unwrap();
        let mut raw = vec![0f32; m.nx * m.ny];
        m.read_plane(&bytes, 0, &mut raw).unwrap();
        let s = open("specular-em", "mode2.mrc");
        let pl = s.nd_plane(&[], &cancel).unwrap().unwrap();
        assert!(m.ny > 1);
        assert_eq!(pl.values[..m.nx], raw[(m.ny - 1) * m.nx..]);
        assert_eq!(pl.values[(m.ny - 1) * m.nx..], raw[..m.nx]);
    }

    /// DeltaVision opens by content (`.dv` is DV video's too): wavelengths as channels with
    /// their nm, Z and time as series in the file's order, rows flipped as MRC's are, and
    /// each index reading the section the sequence puts there.
    #[test]
    fn deltavision_axes_and_sections() {
        let cancel = CancelToken::new();
        let s = open("specular-em", "dv/wzt-be.dv");
        assert_eq!(s.info().format, Format::DeltaVision);
        let sc = s.science().unwrap();
        let names: Vec<(&str, u32)> = sc.axes.iter().map(|a| (a.name.as_str(), a.len)).collect();
        assert_eq!(names, [("", 3), ("Z", 2), ("Time", 2)]);
        assert_eq!(sc.channel_axis, Some(0));
        assert_eq!(sc.axes[0].labels, ["488 nm", "561 nm", "640 nm"]);
        assert_eq!(sc.axes[2].linear.map(|l| l.step), Some(3.0));
        assert_eq!(sc.axes[2].unit, "s");
        assert!(
            sc.pixel
                .as_ref()
                .is_some_and(|p| (p.x.step - 0.0645).abs() < 1e-6)
        );
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-em/tests/data/dv/wzt-be.dv");
        let bytes = std::fs::read(&p).unwrap();
        let d = specular_em::dv::parse(&bytes).unwrap();
        let (nx, ny) = (d.mrc.nx, d.mrc.ny);
        let mut raw = vec![0f32; nx * ny];
        d.mrc
            .read_plane(&bytes, d.section(1, 2, 1), &mut raw)
            .unwrap();
        let pl = s.nd_plane(&[2, 1, 1], &cancel).unwrap().unwrap();
        assert_eq!(pl.values[..nx], raw[(ny - 1) * nx..]);
        let z = open("specular-em", "dv/zstack-u8.dv");
        assert_eq!(z.science().unwrap().axes.len(), 1);
    }

    /// An MRC tilt series with SerialEM's `.mdoc` beside it: one position per tilt, named by
    /// its angle, the pixel size from the sidecar, each tilt's values in the info panel.
    #[test]
    fn mrc_tilt_series_with_mdoc() {
        let s = open("specular-em", "mdoc/tilt.mrc");
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].name, "Tilt");
        assert_eq!(
            sc.axes[0].labels,
            ["0.0°", "3.0°", "-3.0°", "-6.0°", "6.0°"]
        );
        let p = sc.pixel.as_ref().unwrap();
        assert!((p.x.step - 2.17e-10).abs() < 1e-16, "{:?}", p.x);
        assert!(sc.keywords.iter().any(|(k, v, _)| k == "Section 3"
            && v.contains("TiltAngle -5.99988")
            && v.contains("DateTime 02-Nov-26  10:03:00")));
        // Without a sidecar, the stack is plain sections.
        let plain = open("specular-em", "stack.mrcs");
        assert!(
            plain
                .science()
                .unwrap()
                .axes
                .iter()
                .all(|a| a.name != "Tilt")
        );
    }

    /// A crystallographic map stored z, x, y (MAPC 3, MAPR 1, MAPS 2) shows x across, y up
    /// and a z slider from its start indices; it is a volume for MPR.
    #[test]
    fn ccp4_axis_order() {
        let s = open("specular-em", "zxy.map");
        let sc = s.science().unwrap();
        assert_eq!((sc.width, sc.height), (4, 5));
        assert_eq!(sc.axes[0].len, 6);
        let z = sc.axes[0].linear.unwrap();
        assert!((z.offset + 2.2e-10).abs() < 1e-15);
        let px = sc.pixel.as_ref().unwrap();
        assert!((px.x.offset - 2.4e-10).abs() < 1e-15);
        let v = s.research_volume().unwrap();
        assert_eq!(v.dims, [4, 5, 6]);
        assert_eq!(v.format, "CCP4/MRC map");
        // Section 2, displayed row 0 (y = 4), x = 1: (11·1 + 5·4 + 3·2) % 50 − 20, halved.
        let p = s.nd_plane(&[2], &CancelToken::new()).unwrap().unwrap();
        assert_eq!(p.values[1], 17.0 * 0.5);
        assert!(
            sc.keywords
                .iter()
                .any(|(k, v, _)| k == "MAPC MAPR MAPS" && v == "3 1 2")
        );
        // An EM image stack is no volume.
        assert!(
            open("specular-em", "stack.mrcs")
                .research_volume()
                .is_none()
        );
    }

    /// A 4×3×2 float CCP4/MRC map, little-endian, values 0, 0.5, 1, ...
    fn ccp4_map() -> Vec<u8> {
        let mut h = vec![0u8; mrc::HEADER];
        let mut put = |o: usize, v: u32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        for (o, v) in [(0, 4), (4, 3), (8, 2), (12, 2), (28, 4), (32, 3), (36, 2)] {
            put(o, v);
        }
        for (o, v) in [(64, 1), (68, 2), (72, 3)] {
            put(o, v);
        }
        for (o, v) in [
            (40, 4.0f32),
            (44, 3.0),
            (48, 2.0),
            (52, 90.0),
            (56, 90.0),
            (60, 90.0),
        ] {
            put(o, v.to_bits());
        }
        h[208..212].copy_from_slice(b"MAP ");
        h[212..214].copy_from_slice(&[0x44, 0x41]);
        for i in 0..24 {
            h.extend_from_slice(&(i as f32 * 0.5).to_le_bytes());
        }
        h
    }

    #[test]
    fn ccp4_map_and_gzip_by_content() {
        let dir = std::env::temp_dir().join(format!("specular-ccp4-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let map = ccp4_map();
        let gz = crate::exif_locate::tests::gzip(&map);
        std::fs::write(dir.join("emd.map"), &map).unwrap();
        std::fs::write(dir.join("emd.ccp4"), &map).unwrap();
        std::fs::write(dir.join("emd.map.gz"), &gz).unwrap();
        // Other `.map` files: a linker map, a gzip that is not MRC, and a short file.
        std::fs::write(dir.join("linker.map"), vec![b'x'; 4096]).unwrap();
        let other = crate::exif_locate::tests::gzip(&[7u8; 2048]);
        std::fs::write(dir.join("level.map.gz"), other).unwrap();
        std::fs::write(dir.join("tiny.map"), b"MAP ").unwrap();
        let reg = crate::Registry::new();
        let cancel = CancelToken::new();
        for name in ["emd.map", "emd.ccp4", "emd.map.gz"] {
            let p = dir.join(name);
            assert!(reg.supports_file(&p), "{name}");
            let s = reg.open_path(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, Format::Mrc, "{name}");
            assert_eq!((s.info().width, s.info().height), (4, 3), "{name}");
            let sci = s.science().unwrap();
            assert_eq!(sci.axes.len(), 1, "{name}");
            assert_eq!(sci.axes[0].len, 2, "{name}");
            let pl = s.nd_plane(&[1], &cancel).unwrap().unwrap();
            // Second section, rows flipped to top-down: the top row is the file's last row.
            assert_eq!(pl.values[..4], [10.0, 10.5, 11.0, 11.5], "{name}");
        }
        assert!(reg.supports_extension(&dir.join("emd.ccp4")));
        for name in ["linker.map", "level.map.gz", "tiny.map"] {
            let p = dir.join(name);
            assert!(!reg.supports_extension(&p), "{name}");
            assert!(!reg.supports_file(&p), "{name}");
            assert!(reg.open_path(&p).is_err(), "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Folder listings and opening by content for the formats without a type of their own:
    /// DeltaVision under `.dv` (DV video's), `.r3d` (RED's) and an upper-case name, ECAT 7
    /// under `.v` and `.V`; a Verilog `.v` and a `.dv` that is neither are left out or refused.
    #[test]
    fn deltavision_and_ecat_are_listed_by_content() {
        let data = |krate: &str, name: &str| {
            std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../formats")
                    .join(krate)
                    .join("tests/data")
                    .join(name),
            )
            .unwrap()
        };
        let dir = std::env::temp_dir().join(format!("specular-dvecat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dv = data("specular-em", "dv/wzt-be.dv");
        let ecat = data("specular-medvol", "ecat_u8.v");
        let reg = crate::Registry::new();
        for (name, bytes, format) in [
            ("cells.dv", &dv, Format::DeltaVision),
            ("cells.r3d", &dv, Format::DeltaVision),
            ("CELLS.DV", &dv, Format::DeltaVision),
            ("scan.v", &ecat, Format::Ecat),
            ("SCAN.V", &ecat, Format::Ecat),
        ] {
            let p = dir.join(name);
            std::fs::write(&p, bytes).unwrap();
            assert!(reg.supports_file(&p), "{name} listed");
            let s = reg.open_path(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, format, "{name}");
        }
        let verilog = dir.join("counter.v");
        std::fs::write(
            &verilog,
            "module counter(input clk);\nendmodule\n".repeat(20),
        )
        .unwrap();
        assert!(!reg.supports_file(&verilog), "Verilog is not listed");
        let other = dir.join("tape.dv");
        std::fs::write(&other, vec![0x55u8; 4096]).unwrap();
        assert!(reg.open_path(&other).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
