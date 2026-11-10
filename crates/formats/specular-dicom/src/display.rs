//! Display mapping of grey images (PS3.3 C.11): the modality transform (rescale slope /
//! intercept or a Modality LUT) to modality values such as Hounsfield units, then a VOI
//! transform (window centre / width with the LINEAR, LINEAR_EXACT or SIGMOID function, or a
//! VOI LUT) to display values, inverted for MONOCHROME1 or an INVERSE presentation LUT shape.

use crate::Result;
use crate::calib::Calibration;
use crate::graphics::{Graphics, SHUTTER};
use crate::orient::PatientAxes;
use crate::parse::{DataSet, Values};
use crate::pixel::{Frame, Grey, Lut, Pixels, alloc, read_lut};
use crate::rwvm::{RealWorld, RealWorldValue};

/// Stored value to modality value.
#[derive(Clone, Debug, PartialEq)]
pub enum Modality {
    Rescale { slope: f64, intercept: f64 },
    Lut(LutTable),
}

/// A LUT from a descriptor and data (Modality or VOI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LutTable {
    /// Input value of the first entry.
    pub first: i64,
    pub data: Vec<u16>,
    /// Output bits.
    pub bits: u16,
}

impl LutTable {
    pub(crate) fn from(l: Lut, data_bits: Option<u16>) -> LutTable {
        // The output range is 0..2^bits - 1 for the descriptor's bit count (PS3.3
        // C.11.2.1.1), which is often too small for the data; widen it to the data's range.
        let max = l.data.iter().copied().max().unwrap_or(0);
        let needed = (16 - max.leading_zeros()) as u16;
        let bits = data_bits.unwrap_or(l.nominal).max(needed).clamp(1, 16);
        LutTable {
            first: l.first,
            data: l.data,
            bits,
        }
    }

    #[inline]
    fn map(&self, v: f64) -> f64 {
        let i = (v.floor() as i64 - self.first).clamp(0, self.data.len() as i64 - 1);
        self.data.get(i as usize).copied().unwrap_or(0) as f64
    }
}

impl Modality {
    #[inline]
    pub fn apply(&self, stored: i64) -> f64 {
        match self {
            Modality::Rescale { slope, intercept } => stored as f64 * slope + intercept,
            Modality::Lut(l) => l.map(stored as f64),
        }
    }

    /// The modality value of a stored value that need not be an integer (float pixel data).
    #[inline]
    pub fn apply_f(&self, stored: f64) -> f64 {
        match self {
            Modality::Rescale { slope, intercept } => stored * slope + intercept,
            Modality::Lut(l) => l.map(stored),
        }
    }
}

/// VOI LUT function (0028,1056).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiFunction {
    Linear,
    LinearExact,
    Sigmoid,
}

/// Value of interest transform: modality value to a display fraction.
#[derive(Clone, Debug, PartialEq)]
pub enum Voi {
    Window {
        center: f64,
        width: f64,
        function: VoiFunction,
    },
    Lut(LutTable),
}

impl Voi {
    /// Display value in 0..=1.
    #[inline]
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Voi::Window {
                center,
                width,
                function,
            } => match function {
                // PS3.3 C.11.2.1.2.1.
                VoiFunction::Linear => {
                    let (c, w) = (center - 0.5, (width - 1.0).max(0.0));
                    if x <= c - w / 2.0 {
                        0.0
                    } else if x > c + w / 2.0 || w == 0.0 {
                        1.0
                    } else {
                        ((x - c) / w + 0.5).clamp(0.0, 1.0)
                    }
                }
                VoiFunction::LinearExact => {
                    if x <= center - width / 2.0 {
                        0.0
                    } else if x > center + width / 2.0 {
                        1.0
                    } else {
                        ((x - center) / width + 0.5).clamp(0.0, 1.0)
                    }
                }
                VoiFunction::Sigmoid => 1.0 / (1.0 + (-4.0 * (x - center) / width).exp()),
            },
            Voi::Lut(l) => l.map(x) / ((1u32 << l.bits) - 1) as f64,
        }
    }
}

/// Where the default VOI came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiSource {
    Window,
    Lut,
    /// The range of the values (padding excluded).
    MinMax,
}

/// Display-mapping attributes of one frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mapping {
    pub modality: Modality,
    pub rescale_type: Option<String>,
    pub windows: Vec<(f64, f64)>,
    /// Window Center & Width Explanation (0028,1055), one per window when given.
    pub explanations: Vec<String>,
    pub function: VoiFunction,
    pub voi_lut: Option<LutTable>,
    pub invert: bool,
    /// Presentation LUT (2050,0010): display fraction to P-values, after the VOI.
    pub plut: Option<LutTable>,
    /// Stored values to leave out of the default range.
    pub padding: Option<(i64, i64)>,
    /// Float / Double Float Pixel Padding Value.
    pub float_padding: Option<f64>,
    pub ct: bool,
}

/// The first item of `seq` for `frame` in the per-frame functional groups, else in the
/// shared ones.
pub(crate) fn functional(ds: &DataSet, frame: u32, seq: u32) -> Option<&DataSet> {
    let per = ds
        .get(0x5200_9230)
        .and_then(|e| e.items.get(frame as usize))
        .and_then(|i| i.item(seq));
    per.or_else(|| ds.item(0x5200_9229).and_then(|i| i.item(seq)))
}

impl Mapping {
    /// `float`: Float / Double Float Pixel Data, where LINEAR windows are read as
    /// LINEAR_EXACT (the ½ and 1 offsets of LINEAR are for integer stored values; a 0..1
    /// map windowed at 0.5 / 1 would otherwise threshold).
    pub(crate) fn read(
        ds: &DataSet,
        v: &Values,
        frame: u32,
        signed: bool,
        grey: bool,
        float: bool,
    ) -> Mapping {
        let text = |d: &DataSet, tag| {
            d.get(tag)
                .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        // Modality transform: per-frame Pixel Value Transformation, else the image's.
        let pvt = functional(ds, frame, 0x0028_9145);
        let rescale_src = pvt.filter(|d| d.get(0x0028_1053).is_some()).unwrap_or(ds);
        let num = |d: &DataSet, tag| d.get(tag).and_then(|e| v.number(e, 0));
        let mut slope = num(rescale_src, 0x0028_1053).filter(|s| *s != 0.0);
        let intercept = num(rescale_src, 0x0028_1052);
        let mut rescale_type = text(rescale_src, 0x0028_1054);
        // RT Dose: Dose Grid Scaling is the slope to the Dose Units (PS3.3 C.8.8.3).
        if slope.is_none()
            && intercept.is_none()
            && let Some(scale) = num(ds, 0x3004_000E).filter(|s| *s > 0.0)
        {
            slope = Some(scale);
            if rescale_type.is_none() {
                rescale_type =
                    (text(ds, 0x3004_0002).as_deref() == Some("GY")).then(|| "Gy".to_owned());
            }
        }
        let modality = match (slope, intercept) {
            (None, None) => ds
                .item(0x0028_3000)
                .and_then(|m| read_lut(m, v, 0x0028_3002, 0x0028_3006, None, signed))
                .map(|l| Modality::Lut(LutTable::from(l, None)))
                .unwrap_or(Modality::Rescale {
                    slope: 1.0,
                    intercept: 0.0,
                }),
            (s, i) => Modality::Rescale {
                slope: s.unwrap_or(1.0),
                intercept: i.unwrap_or(0.0),
            },
        };
        // Windows: per-frame Frame VOI LUT, else the image's.
        let voi_src = functional(ds, frame, 0x0028_9132)
            .filter(|d| d.get(0x0028_1050).is_some())
            .unwrap_or(ds);
        let mut windows = Vec::new();
        let mut explanations = Vec::new();
        if let (Some(c), Some(w)) = (voi_src.get(0x0028_1050), voi_src.get(0x0028_1051)) {
            let names = voi_src.get(0x0028_1055);
            for k in 0..v.count(c).min(v.count(w)).min(64) {
                if let (Some(c), Some(w)) = (v.number(c, k), v.number(w, k)) {
                    windows.push((c, w));
                    let name = names.and_then(|e| v.text_n(e, k)).unwrap_or_default();
                    explanations.push(name.to_owned());
                }
            }
        }
        let function = match text(voi_src, 0x0028_1056).as_deref() {
            Some("LINEAR_EXACT") => VoiFunction::LinearExact,
            Some("SIGMOID") => VoiFunction::Sigmoid,
            _ if float => VoiFunction::LinearExact,
            _ => VoiFunction::Linear,
        };
        let lut_signed =
            signed || matches!(&modality, Modality::Rescale { intercept, .. } if *intercept < 0.0);
        let voi_lut = ds
            .item(0x0028_3010)
            .and_then(|m| read_lut(m, v, 0x0028_3002, 0x0028_3006, None, lut_signed))
            .map(|l| LutTable::from(l, None));
        let monochrome1 = ds
            .get(0x0028_0004)
            .is_some_and(|e| v.text_bytes(e) == b"MONOCHROME1");
        let inverse = text(ds, 0x2050_0020).as_deref() == Some("INVERSE");
        let pad = |tag| {
            ds.get(tag).and_then(|e| v.int(e)).map(|x| {
                if signed && e_is_unsigned_word(x) {
                    x as u16 as i16 as i64
                } else {
                    x
                }
            })
        };
        let padding = match (pad(0x0028_0120), pad(0x0028_0121)) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
            (Some(a), None) => Some((a, a)),
            _ => None,
        };
        let ct = text(ds, 0x0008_0060).as_deref() == Some("CT");
        let plut = ds
            .item(0x2050_0010)
            .and_then(|m| read_lut(m, v, 0x0028_3002, 0x0028_3006, None, false))
            .map(|l| LutTable::from(l, None))
            .filter(|_| grey);
        let float_padding = [0x0028_0122, 0x0028_0123]
            .iter()
            .find_map(|&t| ds.get(t).and_then(|e| v.number(e, 0)));
        Mapping {
            modality,
            rescale_type,
            windows,
            explanations,
            function,
            voi_lut,
            invert: grey && (monochrome1 || inverse),
            plut,
            padding,
            float_padding,
            ct,
        }
    }
}

impl Mapping {
    /// A display fraction (VOI output, 0..=1) through the Presentation LUT, if any.
    #[inline]
    pub(crate) fn presentation(&self, d: f64) -> f64 {
        match &self.plut {
            Some(l) => {
                // Entry floor(d (n - 1)), as pydicom; the 1e-9 keeps an exact entry (d = k /
                // (n - 1), e.g. 273 / 4095 = 17 / 255) from rounding down to the one before.
                let n = l.data.len().max(1) as f64;
                l.map(l.first as f64 + d * (n - 1.0) + 1e-9) / ((1u32 << l.bits) - 1) as f64
            }
            None => d,
        }
    }

    /// A modality value to the displayed fraction (VOI, Presentation LUT, inversion).
    #[inline]
    fn display(&self, voi: &Voi, x: f64) -> f64 {
        let d = self.presentation(voi.apply(x));
        if self.invert { 1.0 - d } else { d }
    }
}

fn e_is_unsigned_word(x: i64) -> bool {
    (0x8000..=0xFFFF).contains(&x)
}

/// A decoded frame with its display mapping: the modality values a viewer reports under the
/// cursor and windows on, and the default rendering.
#[derive(Clone, Debug)]
pub struct ModalityFrame {
    pub frame: Frame,
    pub(crate) map: Mapping,
    /// Overlays and shutters of the frame.
    pub(crate) graphics: Option<Graphics>,
    pub(crate) calibration: Calibration,
    /// Where the frame's rows and columns point in the patient.
    pub(crate) axes: Option<PatientAxes>,
    /// SOP Instance UID of the image.
    pub(crate) sop_uid: Option<String>,
    /// SOP Instance UID of the presentation state applied ([`ModalityFrame::presented`]).
    pub(crate) presentation: Option<String>,
    /// Real World Value Mappings of the frame.
    pub(crate) rwvm: Vec<RealWorld>,
}

/// A rendered frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rendered {
    /// Grey display values, 0..=65535.
    Gray16(Vec<u16>),
    /// RGBA, alpha 255.
    Rgba8(Vec<u8>),
    /// RGBA, 0..=65535, alpha 65535.
    Rgba16(Vec<u16>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Rendered,
    pub truncated: bool,
}

impl Image {
    pub fn bytes(&self) -> usize {
        match &self.pixels {
            Rendered::Gray16(v) | Rendered::Rgba16(v) => v.len() * 2,
            Rendered::Rgba8(v) => v.len(),
        }
    }
}

impl ModalityFrame {
    /// A frame from outside DICOM (a research volume's slice): stored values mapped by
    /// `slope` and `intercept`, windowed LINEAR_EXACT by the first of `windows`, with
    /// `spacing` (between columns, between rows, in mm) in the patient and, when known,
    /// the patient directions of its rows and columns.
    pub fn from_values(
        frame: Frame,
        slope: f64,
        intercept: f64,
        windows: Vec<(f64, f64)>,
        spacing: Option<[f64; 2]>,
        axes: Option<([f64; 3], [f64; 3])>,
    ) -> ModalityFrame {
        ModalityFrame {
            frame,
            map: Mapping {
                modality: Modality::Rescale { slope, intercept },
                rescale_type: None,
                windows,
                explanations: Vec::new(),
                function: VoiFunction::LinearExact,
                voi_lut: None,
                invert: false,
                plut: None,
                padding: None,
                float_padding: None,
                ct: false,
            },
            graphics: None,
            calibration: Calibration {
                spacing: spacing.map(|[x, y]| crate::calib::Spacing {
                    x,
                    y,
                    kind: crate::calib::SpacingKind::Patient,
                }),
                regions: Vec::new(),
            },
            axes: axes.map(|(row, col)| PatientAxes {
                row,
                col,
                from_letters: false,
                anatomy: crate::orient::Anatomy::Biped,
            }),
            sop_uid: None,
            presentation: None,
            rwvm: Vec::new(),
        }
    }

    pub fn width(&self) -> u32 {
        self.frame.width
    }

    pub fn height(&self) -> u32 {
        self.frame.height
    }

    /// Whether the frame is grey (has modality values).
    pub fn is_grey(&self) -> bool {
        matches!(self.frame.pixels, Pixels::Grey(_))
    }

    /// The stored value at (x, y), for grey frames.
    pub fn stored_at(&self, x: u32, y: u32) -> Option<i64> {
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        if x >= self.frame.width || y >= self.frame.height {
            return None;
        }
        g.get(y as usize * self.frame.width as usize + x as usize)
    }

    /// The modality value at (x, y) (Hounsfield units for CT), for grey frames.
    pub fn value_at(&self, x: u32, y: u32) -> Option<f64> {
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        if x >= self.frame.width || y >= self.frame.height {
            return None;
        }
        let i = y as usize * self.frame.width as usize + x as usize;
        if g.is_float() {
            g.get_f64(i).map(|s| self.map.modality.apply_f(s))
        } else {
            g.get(i).map(|s| self.map.modality.apply(s))
        }
    }

    /// Smallest and largest finite float sample, padding excluded.
    fn float_range(&self) -> Option<(f64, f64)> {
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        let pad = self.map.float_padding;
        let mut r: Option<(f64, f64)> = None;
        let mut fold = |v: f64| {
            if v.is_finite() && pad != Some(v) {
                r = Some(match r {
                    None => (v, v),
                    Some((a, b)) => (a.min(v), b.max(v)),
                });
            }
        };
        match g {
            Grey::F32(d) => d.iter().for_each(|&x| fold(x as f64)),
            Grey::F64(d) => d.iter().for_each(|&x| fold(x)),
            _ => return None,
        }
        r
    }

    /// RGB samples at (x, y) for colour frames, with their significant bits.
    pub fn rgb_at(&self, x: u32, y: u32) -> Option<([u16; 3], u16)> {
        if x >= self.frame.width || y >= self.frame.height {
            return None;
        }
        let i = (y as usize * self.frame.width as usize + x as usize) * 3;
        match &self.frame.pixels {
            Pixels::Rgb8(d) => d
                .get(i..i + 3)
                .map(|p| ([p[0] as u16, p[1] as u16, p[2] as u16], self.frame.bits)),
            Pixels::Rgb16(d) => d
                .get(i..i + 3)
                .map(|p| ([p[0], p[1], p[2]], self.frame.bits)),
            Pixels::Grey(_) => None,
        }
    }

    pub fn modality(&self) -> &Modality {
        &self.map.modality
    }

    /// The Real World Value Mappings of the frame (empty without any).
    pub fn real_world(&self) -> &[RealWorld] {
        &self.rwvm
    }

    /// The real-world value at (x, y) of a grey frame, through the first mapping whose range
    /// covers the stored value.
    pub fn real_world_at(&self, x: u32, y: u32) -> Option<RealWorldValue> {
        if self.rwvm.is_empty() {
            return None;
        }
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        if x >= self.frame.width || y >= self.frame.height {
            return None;
        }
        let i = y as usize * self.frame.width as usize + x as usize;
        let s = if g.is_float() {
            g.get_f64(i)?
        } else {
            g.get(i)? as f64
        };
        crate::rwvm::value(&self.rwvm, s)
    }

    /// Unit of the modality values: Rescale Type when it names one, else "HU" for CT.
    pub fn unit(&self) -> Option<&str> {
        match self.map.rescale_type.as_deref() {
            Some("US") | None => (self.map.ct
                && matches!(self.map.modality, Modality::Rescale { .. }))
            .then_some("HU"),
            Some(t) => Some(t),
        }
    }

    /// The windows the file suggests (centre, width), in modality units.
    pub fn windows(&self) -> &[(f64, f64)] {
        &self.map.windows
    }

    /// The explanation of the file's window `k` (Window Center & Width Explanation, such
    /// as "BONE"), when it gives one.
    pub fn window_explanation(&self, k: usize) -> Option<&str> {
        self.map
            .explanations
            .get(k)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn voi_function(&self) -> VoiFunction {
        self.map.function
    }

    /// Whether display values are inverted (MONOCHROME1, INVERSE presentation LUT shape).
    pub fn inverted(&self) -> bool {
        self.map.invert
    }

    /// Smallest and largest stored value, padding excluded.
    pub fn stored_range(&self) -> Option<(i64, i64)> {
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        let pad = self.map.padding;
        let keep = |v: i64| !pad.is_some_and(|(a, b)| v >= a && v <= b);
        let mut r: Option<(i64, i64)> = None;
        let mut fold = |v: i64| {
            if keep(v) {
                r = Some(match r {
                    None => (v, v),
                    Some((a, b)) => (a.min(v), b.max(v)),
                });
            }
        };
        match g {
            Grey::U16(d) => {
                // A presence table keeps this one pass without branches per pixel.
                let mut seen = vec![false; 65536];
                for &x in d {
                    seen[x as usize] = true;
                }
                for (x, _) in seen.iter().enumerate().filter(|(_, s)| **s) {
                    fold(x as i64);
                }
            }
            Grey::I16(d) => {
                let mut seen = vec![false; 65536];
                for &x in d {
                    seen[(x as i32 + 32768) as usize] = true;
                }
                for (x, _) in seen.iter().enumerate().filter(|(_, s)| **s) {
                    fold(x as i64 - 32768);
                }
            }
            Grey::I32(d) => d.iter().for_each(|&x| fold(x as i64)),
            Grey::F32(_) | Grey::F64(_) => {
                let (a, b) = self.float_range()?;
                fold(a.floor() as i64);
                fold(b.ceil() as i64);
            }
        }
        r
    }

    /// Range of the modality values, padding excluded.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        if let Some((a, b)) = self.float_range() {
            let m = &self.map.modality;
            if let Modality::Rescale { .. } = m {
                let (x, y) = (m.apply_f(a), m.apply_f(b));
                return Some((x.min(y), x.max(y)));
            }
        }
        let (a, b) = self.stored_range()?;
        Some(match &self.map.modality {
            Modality::Rescale { .. } => {
                let (x, y) = (self.map.modality.apply(a), self.map.modality.apply(b));
                (x.min(y), x.max(y))
            }
            Modality::Lut(l) => {
                let lo = (a - l.first).clamp(0, l.data.len() as i64 - 1) as usize;
                let hi = (b - l.first).clamp(0, l.data.len() as i64 - 1) as usize;
                let s = &l.data[lo..=hi];
                (
                    s.iter().copied().min().unwrap_or(0) as f64,
                    s.iter().copied().max().unwrap_or(0) as f64,
                )
            }
        })
    }

    /// The default VOI: the first window, else the VOI LUT, else the value range.
    pub fn default_voi(&self) -> (Voi, VoiSource) {
        if let Some(&(center, width)) =
            self.map
                .windows
                .iter()
                .find(|(_, w)| match self.map.function {
                    VoiFunction::Linear => *w >= 1.0,
                    _ => *w > 0.0,
                })
        {
            return (
                Voi::Window {
                    center,
                    width,
                    function: self.map.function,
                },
                VoiSource::Window,
            );
        }
        if let Some(l) = &self.map.voi_lut {
            return (Voi::Lut(l.clone()), VoiSource::Lut);
        }
        let (lo, hi) = self.value_range().unwrap_or((0.0, 1.0));
        (
            Voi::Window {
                center: (lo + hi) / 2.0,
                width: (hi - lo).max(f64::MIN_POSITIVE),
                function: VoiFunction::LinearExact,
            },
            VoiSource::MinMax,
        )
    }

    /// Overlays and shutters, when the frame has any.
    pub fn graphics(&self) -> Option<&Graphics> {
        self.graphics.as_ref()
    }

    /// Where the frame's rows and columns point in the patient, for orientation labels.
    pub fn patient_axes(&self) -> Option<&PatientAxes> {
        self.axes.as_ref()
    }

    /// The frame's calibration for measurements.
    pub fn calibration(&self) -> &Calibration {
        &self.calibration
    }

    /// Statistics of the modality values (HU, SUV, ...) of the pixels `roi` covers, by the
    /// measurement layer's coverage rules (grey frames; `None` when it covers none).
    pub fn roi_stats(
        &self,
        roi: &specular_measure::Shape,
    ) -> Option<specular_measure::ChannelStats> {
        if !self.is_grey() {
            return None;
        }
        specular_measure::stats::roi_with(
            roi,
            self.width(),
            self.height(),
            specular_measure::stats::DEFAULT_BINS,
            |x, y| self.value_at(x, y),
        )
        .filter(|s| s.count > 0)
    }

    /// Renders with `voi` (grey) or as stored (colour), shutters and overlays burnt in.
    pub fn render(&self, voi: &Voi) -> Result<Image> {
        self.render_with(voi, true)
    }

    /// Renders with `voi`, with or without the overlays (shutters always apply).
    pub fn render_with(&self, voi: &Voi, overlays: bool) -> Result<Image> {
        let mut img = self.render_plain(voi)?;
        if let Some(g) = &self.graphics {
            burn(&mut img, g, overlays);
        }
        Ok(img)
    }

    fn render_plain(&self, voi: &Voi) -> Result<Image> {
        let f = &self.frame;
        let n = f.width as usize * f.height as usize;
        let pixels = match &f.pixels {
            Pixels::Grey(g) => {
                let map = &self.map;
                let m = &self.map.modality;
                let to16 = |stored: i64| -> u16 {
                    let d = map.display(voi, m.apply(stored));
                    (d * 65535.0 + 0.5) as u16
                };
                let mut out = alloc::<u16>(n)?;
                match g {
                    Grey::U16(d) => {
                        let lut: Vec<u16> = (0..65536i64).map(to16).collect();
                        for (o, &x) in out.iter_mut().zip(d) {
                            *o = lut[x as usize];
                        }
                    }
                    Grey::I16(d) => {
                        let lut: Vec<u16> = (-32768..32768i64).map(to16).collect();
                        for (o, &x) in out.iter_mut().zip(d) {
                            *o = lut[(x as i32 + 32768) as usize];
                        }
                    }
                    Grey::I32(d) => {
                        for (o, &x) in out.iter_mut().zip(d) {
                            *o = to16(x as i64);
                        }
                    }
                    Grey::F32(_) | Grey::F64(_) => {
                        let to16 = |s: f64| -> u16 {
                            if !s.is_finite() {
                                return 0;
                            }
                            let d = map.display(voi, m.apply_f(s));
                            (d * 65535.0 + 0.5) as u16
                        };
                        for (i, o) in out.iter_mut().enumerate() {
                            *o = to16(g.get_f64(i).unwrap_or(0.0));
                        }
                    }
                }
                Rendered::Gray16(out)
            }
            Pixels::Rgb8(d) => {
                let mut out = alloc::<u8>(n * 4)?;
                for (o, i) in out.chunks_exact_mut(4).zip(d.chunks_exact(3)) {
                    o[..3].copy_from_slice(i);
                    o[3] = 255;
                }
                Rendered::Rgba8(out)
            }
            Pixels::Rgb16(d) => {
                let bits = f.bits.clamp(1, 16) as u32;
                let max = ((1u32 << bits) - 1).max(1);
                let mut out = alloc::<u16>(n * 4)?;
                for (o, i) in out.chunks_exact_mut(4).zip(d.chunks_exact(3)) {
                    for (o, &v) in o.iter_mut().zip(i) {
                        *o = ((v as u32).min(max) * 65535 / max) as u16;
                    }
                    o[3] = 65535;
                }
                Rendered::Rgba16(out)
            }
        };
        Ok(Image {
            width: f.width,
            height: f.height,
            pixels,
            truncated: f.truncated,
        })
    }

    /// The modality values of a grey frame as f32, row-major (for window / level on the
    /// GPU). Exact for integers up to 2^24, so for every 16-bit stored value under an integer
    /// rescale; otherwise rounded to f32 (relative error ≤ 6e-8).
    pub fn values_f32(&self) -> Option<Vec<f32>> {
        let Pixels::Grey(g) = &self.frame.pixels else {
            return None;
        };
        let m = &self.map.modality;
        let n = self.frame.width as usize * self.frame.height as usize;
        let mut out = alloc::<f32>(n).ok()?;
        match g {
            Grey::U16(d) => {
                let lut: Vec<f32> = (0..65536i64).map(|s| m.apply(s) as f32).collect();
                for (o, &x) in out.iter_mut().zip(d) {
                    *o = lut[x as usize];
                }
            }
            Grey::I16(d) => {
                let lut: Vec<f32> = (-32768..32768i64).map(|s| m.apply(s) as f32).collect();
                for (o, &x) in out.iter_mut().zip(d) {
                    *o = lut[(x as i32 + 32768) as usize];
                }
            }
            Grey::I32(d) => {
                for (o, &x) in out.iter_mut().zip(d) {
                    *o = m.apply(x as i64) as f32;
                }
            }
            Grey::F32(d) => {
                for (o, &x) in out.iter_mut().zip(d) {
                    *o = m.apply_f(x as f64) as f32;
                }
            }
            Grey::F64(d) => {
                for (o, &x) in out.iter_mut().zip(d) {
                    *o = m.apply_f(x) as f32;
                }
            }
        }
        Some(out)
    }

    /// Display fractions (0..=1, before inversion) of a grey frame through `voi` and the
    /// Presentation LUT, row-major as f32: for the GPU when the mapping is not a window
    /// (a VOI LUT or a Presentation LUT), drawn with an identity window over 0..1.
    pub fn display_values_f32(&self, voi: &Voi) -> Option<Vec<f32>> {
        let mut v = self.values_f32()?;
        for x in &mut v {
            *x = if x.is_finite() {
                self.map.presentation(voi.apply(f64::from(*x))) as f32
            } else {
                0.0
            };
        }
        Some(v)
    }

    /// Whether the display mapping has a Presentation LUT table (not just a shape).
    pub fn has_presentation_lut(&self) -> bool {
        self.map.plut.is_some()
    }

    /// The image's SOP Instance UID.
    pub fn sop_instance_uid(&self) -> Option<&str> {
        self.sop_uid.as_deref()
    }

    /// The SOP Instance UID of the presentation state applied, if one is.
    pub fn presentation(&self) -> Option<&str> {
        self.presentation.as_deref()
    }

    /// Renders with the default VOI.
    pub fn render_default(&self) -> Result<Image> {
        self.render(&self.default_voi().0)
    }

    /// Memory held, in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.frame.bytes()
            + self.graphics.as_ref().map_or(0, Graphics::bytes)
            + match &self.map.modality {
                Modality::Lut(l) => l.data.len() * 2,
                _ => 0,
            }
            + self.map.voi_lut.as_ref().map_or(0, |l| l.data.len() * 2)
            + self.map.plut.as_ref().map_or(0, |l| l.data.len() * 2)
    }
}

/// Shutters (at their colour or grey) and overlays (in their inks) into a rendered image; a
/// grey image takes a coloured ink's luma (Rec. 709 weights on the sRGB values).
fn burn(img: &mut Image, g: &Graphics, overlays: bool) {
    if g.width != img.width || g.height != img.height {
        return;
    }
    let wide = |c: u8| u16::from(c) * 257;
    let shutter = g.shutter_rgb.map_or([g.shutter_grey; 3], |c| c.map(wide));
    let pick = |m: u8| -> Option<[u16; 3]> {
        if overlays && let Some(c) = g.ink(m) {
            Some(c.map(wide))
        } else if m & SHUTTER != 0 {
            Some(shutter)
        } else {
            None
        }
    };
    match &mut img.pixels {
        Rendered::Gray16(p) => {
            let luma = |c: [u16; 3]| {
                if c[0] == c[1] && c[1] == c[2] {
                    return c[0];
                }
                (0.2126 * f64::from(c[0]) + 0.7152 * f64::from(c[1]) + 0.0722 * f64::from(c[2]))
                    .round() as u16
            };
            for (o, &m) in p.iter_mut().zip(&g.mask) {
                if let Some(c) = pick(m) {
                    *o = luma(c);
                }
            }
        }
        Rendered::Rgba8(p) => {
            for (o, &m) in p.chunks_exact_mut(4).zip(&g.mask) {
                if let Some(c) = pick(m) {
                    for k in 0..3 {
                        o[k] = (c[k] >> 8) as u8;
                    }
                }
            }
        }
        Rendered::Rgba16(p) => {
            for (o, &m) in p.chunks_exact_mut(4).zip(&g.mask) {
                if let Some(c) = pick(m) {
                    o[..3].copy_from_slice(&c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_window_matches_the_standard() {
        let v = Voi::Window {
            center: 40.0,
            width: 400.0,
            function: VoiFunction::Linear,
        };
        // Bounds: c - 0.5 -+ (w - 1) / 2 = -160 and 239.
        assert_eq!(v.apply(-160.0), 0.0);
        assert_eq!(v.apply(240.0), 1.0);
        assert!((v.apply(39.5) - 0.5).abs() < 1e-9);
        let e = Voi::Window {
            center: 0.0,
            width: 10.0,
            function: VoiFunction::LinearExact,
        };
        assert_eq!(e.apply(-5.0), 0.0);
        assert!((e.apply(0.0) - 0.5).abs() < 1e-12);
        let s = Voi::Window {
            center: 0.0,
            width: 10.0,
            function: VoiFunction::Sigmoid,
        };
        assert!((s.apply(0.0) - 0.5).abs() < 1e-12);
    }
}
