//! Radiometric thermal stills through the science path, read by `specular-fmt-thermal`:
//! FLIR radiometric JPEGs (R-JPEG, the FFF in APP1 segments) and standalone FFF files,
//! InfiRay `.irg`, InfraTec `.irb`, Testo `.bmt` and Fluke `.is2`.
//!
//! The value of a pixel is its temperature ([`ThermalImage::plane`], in the unit asked for);
//! for FLIR and Fluke it is computed from the stored counts with the object parameters, so
//! emissivity and reflected temperature can be changed. The image shows through the camera's palette
//! (an iron ramp when the file has none) from the coldest to the hottest pixel. A file with
//! an embedded visible photo has it as page 2 ([`PageLabel::Thermal`]).
//!
//! An R-JPEG is claimed by content (a FLIR APP1 segment before the scan) ahead of the JPEG
//! decoder; one whose FFF holds no raw thermal data opens as the plain JPEG it also is.
//!
//! FLIR sequences (`.seq`, raw counts; `.csq`, JPEG-LS counts) are a stack of frames on a
//! frame axis ([`AxisKind::Frame`], labelled with each frame's time from the first): science
//! mode steps and plays through it, each frame's temperatures computed from its own Planck
//! constants under the shown object parameters. Frames are indexed when the file opens and
//! read when shown.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_fmt_thermal::{self as th, Kind, Mapping, Pixels, Thermal};

pub use specular_fmt_thermal::{Camera as ThermalCamera, Conditions as ThermalConditions};

use crate::science::{
    AxisKind, ColourTable, NdAxis, NdPlane, PlaneCache, ScienceKind, ScienceSummary, VideoFacts,
};
use crate::whole::{self, Levels};
use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session, jpeg,
    png_dec,
};

/// One decoder per thermal format; each checks only its own signature.
pub struct ThermalDecoder(pub Format);

fn kind_of(f: Format) -> Kind {
    match f {
        Format::FlirJpeg => Kind::FlirJpeg,
        Format::FlirFff => Kind::FlirFff,
        Format::FlirSeq => Kind::FlirSeq,
        Format::InfirayIrg => Kind::InfirayIrg,
        Format::TestoBmt => Kind::TestoBmt,
        Format::FlukeIs2 => Kind::FlukeIs2,
        _ => Kind::InfratecIrb,
    }
}

fn format_of(k: Kind) -> Format {
    match k {
        Kind::FlirJpeg => Format::FlirJpeg,
        Kind::FlirFff => Format::FlirFff,
        Kind::FlirSeq | Kind::FlirCsq => Format::FlirSeq,
        Kind::InfirayIrg => Format::InfirayIrg,
        Kind::InfratecIrb => Format::InfratecIrb,
        Kind::TestoBmt => Format::TestoBmt,
        Kind::FlukeIs2 => Format::FlukeIs2,
    }
}

/// The thermal format a file is by its extension and content, for what content alone must
/// not claim: a Testo `.bmt` led by its JPEG preview (finding its header means a search),
/// and the older `.bmt` and `.is2` layouts (refused with the reason when opened).
pub(crate) fn by_name(path: &std::path::Path, data: &[u8]) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "bmt" if th::is_old_bmt(data) || th::is_kind(data, Kind::TestoBmt) => {
            Some(Format::TestoBmt)
        }
        "is2" if th::is_old_is2(data) => Some(Format::FlukeIs2),
        _ => None,
    }
}

impl Decoder for ThermalDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        // R-JPEG and IRG need more than the first bytes (`probe_file`).
        match self.0 {
            Format::FlirFff | Format::InfratecIrb => th::is_kind(head, kind_of(self.0)),
            _ => false,
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            // With its JPEG preview only by name (`by_name`).
            Format::TestoBmt => data.starts_with(b"<ToFo") && th::is_kind(data, Kind::TestoBmt),
            _ => th::is_kind(data, kind_of(self.0)),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::FlirFff => &["fff"],
            Format::FlirSeq => &["seq", "csq"],
            Format::InfirayIrg => &["irg"],
            Format::InfratecIrb => &["irb"],
            Format::TestoBmt => &["bmt"],
            Format::FlukeIs2 => &["is2"],
            // R-JPEGs are `.jpg` files: claimed by content only.
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        if self.0 == Format::FlirSeq {
            let seq = th::index_sequence(source.bytes()).map_err(thermal_err)?;
            let first = seq.frame(source.bytes(), 0).map_err(thermal_err)?;
            let image = ThermalImage::sequence(first, source.clone(), seq);
            return Ok(Arc::new(ThermalSession::new(source, Arc::new(image))?));
        }
        let refused = match self.0 {
            Format::TestoBmt if th::is_old_bmt(source.bytes()) => Some(th::OLD_BMT_REASON),
            Format::FlukeIs2 if th::is_old_is2(source.bytes()) => Some(th::OLD_IS2_REASON),
            _ => None,
        };
        if let Some(why) = refused {
            return Err(DecodeError::Unsupported(why.into()));
        }
        match th::read(source.bytes()) {
            Ok(t) => Ok(Arc::new(ThermalSession::new(
                source,
                Arc::new(ThermalImage::new(t)),
            )?)),
            // A FLIR JPEG without raw data is still a JPEG.
            Err(th::Error::NoRawData | th::Error::NotThermal) if self.0 == Format::FlirJpeg => {
                jpeg::JpegDecoder.open(source)
            }
            Err(e) => Err(thermal_err(e)),
        }
    }
}

fn thermal_err(e: th::Error) -> DecodeError {
    match e {
        th::Error::NotThermal => DecodeError::UnsupportedFormat,
        th::Error::NoRawData => DecodeError::Unsupported(e.to_string()),
        th::Error::Malformed { .. } => DecodeError::Malformed(e.to_string()),
        th::Error::Unsupported { .. } => DecodeError::Unsupported(e.to_string()),
    }
}

/// A temperature unit for values and readouts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
    Kelvin,
}

impl TemperatureUnit {
    pub const ALL: [TemperatureUnit; 3] = [
        TemperatureUnit::Celsius,
        TemperatureUnit::Fahrenheit,
        TemperatureUnit::Kelvin,
    ];

    pub fn symbol(self) -> &'static str {
        match self {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
            TemperatureUnit::Kelvin => "K",
        }
    }

    /// A temperature in degrees Celsius in this unit.
    pub fn from_celsius(self, c: f64) -> f64 {
        match self {
            TemperatureUnit::Celsius => c,
            TemperatureUnit::Fahrenheit => c * 1.8 + 32.0,
            TemperatureUnit::Kelvin => c + 273.15,
        }
    }

    /// A temperature in this unit in degrees Celsius.
    pub fn to_celsius(self, v: f64) -> f64 {
        match self {
            TemperatureUnit::Celsius => v,
            TemperatureUnit::Fahrenheit => (v - 32.0) / 1.8,
            TemperatureUnit::Kelvin => v - 273.15,
        }
    }

    /// A temperature difference (a span) in degrees Celsius in this unit.
    pub fn span_from_celsius(self, d: f64) -> f64 {
        match self {
            TemperatureUnit::Fahrenheit => d * 1.8,
            _ => d,
        }
    }

    /// The next unit in °C, °F, K order.
    pub fn next(self) -> TemperatureUnit {
        match self {
            TemperatureUnit::Celsius => TemperatureUnit::Fahrenheit,
            TemperatureUnit::Fahrenheit => TemperatureUnit::Kelvin,
            TemperatureUnit::Kelvin => TemperatureUnit::Celsius,
        }
    }

    fn code(self) -> u32 {
        self as u32
    }
}

/// A thermal still's (or a sequence's) temperatures under chosen conditions, and what the
/// file records.
pub struct ThermalImage {
    /// The still, or a sequence's first frame.
    thermal: Thermal,
    planes: PlaneCache,
    seq: Option<Frames>,
}

/// A sequence's frames: the file and where each frame lies.
struct Frames {
    source: Arc<Source>,
    index: th::Sequence,
}

impl std::fmt::Debug for ThermalImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThermalImage")
            .field("kind", &self.thermal.kind)
            .field("width", &self.thermal.width)
            .field("height", &self.thermal.height)
            .finish()
    }
}

/// The plane cache key of `c` in `unit`: the bits of every parameter.
fn plane_key(c: &ThermalConditions, unit: TemperatureUnit) -> Vec<u32> {
    let mut k = vec![unit.code()];
    for v in [
        c.emissivity,
        c.distance,
        c.reflected,
        c.atmospheric,
        c.window_temperature,
        c.window_transmission,
        c.humidity,
    ] {
        let b = v.map_or(u64::MAX, f64::to_bits);
        k.extend([(b >> 32) as u32, b as u32]);
    }
    k
}

impl ThermalImage {
    fn new(thermal: Thermal) -> ThermalImage {
        ThermalImage {
            thermal,
            planes: PlaneCache::new(),
            seq: None,
        }
    }

    fn sequence(first: Thermal, source: Arc<Source>, index: th::Sequence) -> ThermalImage {
        ThermalImage {
            thermal: first,
            planes: PlaneCache::new(),
            seq: Some(Frames { source, index }),
        }
    }

    /// Frames: 1 for a still.
    pub fn frame_count(&self) -> u32 {
        self.seq.as_ref().map_or(1, |s| s.index.len() as u32)
    }

    /// Seconds from the first frame to frame `i` (0 for a still).
    pub fn frame_seconds(&self, i: u32) -> f64 {
        self.seq
            .as_ref()
            .filter(|s| (i as usize) < s.index.len())
            .map_or(0.0, |s| s.index.seconds(i as usize))
    }

    /// How long frame `i` shows when played, seconds.
    pub fn frame_duration(&self, i: u32) -> f64 {
        self.seq.as_ref().map_or(0.0, |s| {
            s.index
                .duration((i as usize).min(s.index.len().saturating_sub(1)))
        })
    }

    /// Frames a second a sequence plays at (`None` for a still).
    pub fn frame_rate(&self) -> Option<f64> {
        self.seq.as_ref().map(|s| s.index.rate)
    }

    /// Frame `i` of a sequence, read in full (the still itself for 0 of a still).
    pub fn frame(&self, i: u32) -> Result<Thermal> {
        match &self.seq {
            Some(s) => s
                .index
                .frame(s.source.bytes(), i as usize)
                .map_err(thermal_err),
            None if i == 0 => Ok(self.thermal.clone()),
            None => Err(DecodeError::Malformed("no such frame".into())),
        }
    }

    /// The object parameters frame `i` records (a still's own; a sequence may change
    /// them while recording). Read when the file was indexed.
    pub fn frame_conditions(&self, i: u32) -> ThermalConditions {
        self.seq
            .as_ref()
            .and_then(|s| s.index.frames.get(i as usize))
            .map_or(self.thermal.conditions, |f| f.conditions)
    }

    /// The conditions frame `i` is computed under: `conditions` when they were edited, the
    /// frame's own when they are the file's (frame 0's, as [`ThermalImage::file_conditions`]
    /// gives them).
    pub fn effective_conditions(
        &self,
        i: u32,
        conditions: &ThermalConditions,
    ) -> ThermalConditions {
        if *conditions == self.thermal.conditions {
            self.frame_conditions(i)
        } else {
            *conditions
        }
    }

    /// Frame `i`'s temperatures in `unit` under `conditions`, not cached (for a pass over
    /// every frame). Conditions equal to the file's mean "as recorded": each frame of a
    /// sequence then uses its own ([`ThermalImage::effective_conditions`]).
    pub fn frame_values(
        &self,
        i: u32,
        conditions: &ThermalConditions,
        unit: TemperatureUnit,
    ) -> Result<Vec<f32>> {
        let mut values = if self.seq.is_none() && i == 0 {
            self.thermal.temperatures(conditions)
        } else {
            let frame = self.frame(i)?;
            let c = if *conditions == self.thermal.conditions {
                frame.conditions
            } else {
                *conditions
            };
            frame.temperatures(&c)
        };
        if unit != TemperatureUnit::Celsius {
            for v in &mut values {
                *v = unit.from_celsius(f64::from(*v)) as f32;
            }
        }
        Ok(values)
    }

    /// Everything the file records.
    pub fn thermal(&self) -> &Thermal {
        &self.thermal
    }

    pub fn camera(&self) -> &ThermalCamera {
        &self.thermal.camera
    }

    /// The object parameters as recorded.
    pub fn file_conditions(&self) -> ThermalConditions {
        self.thermal.conditions
    }

    /// Whether temperatures follow emissivity and reflected temperature (FLIR counts);
    /// other formats store temperatures.
    pub fn adjustable(&self) -> bool {
        self.thermal.adjustable()
    }

    /// The format's name ("FLIR radiometric JPEG").
    pub fn kind_name(&self) -> &'static str {
        self.thermal.kind.name()
    }

    /// The camera palette's isotherm colour, if it records one.
    pub fn isotherm_colour(&self) -> Option<[u8; 3]> {
        self.thermal.palette.as_ref().and_then(|p| p.isotherm)
    }

    /// The smallest and largest stored count (FLIR, IRG).
    pub fn raw_range(&self) -> Option<(u16, u16)> {
        self.thermal.raw_range()
    }

    /// Every pixel's temperature in `unit` under `conditions` (rows top-down, NaN where
    /// the calibration has no answer); kept until the session trims its memory.
    pub fn plane(
        &self,
        conditions: &ThermalConditions,
        unit: TemperatureUnit,
        cancel: &CancelToken,
    ) -> Result<Arc<NdPlane>> {
        self.plane_at(0, conditions, unit, cancel)
    }

    /// [`ThermalImage::plane`] of frame `frame` (a sequence's; 0 for a still).
    pub fn plane_at(
        &self,
        frame: u32,
        conditions: &ThermalConditions,
        unit: TemperatureUnit,
        cancel: &CancelToken,
    ) -> Result<Arc<NdPlane>> {
        let mut key = plane_key(conditions, unit);
        key.push(frame);
        self.planes.get_or(&key, cancel, || {
            let values = self.frame_values(frame, conditions, unit)?;
            Ok(NdPlane {
                width: self.thermal.width as u32,
                height: self.thermal.height as u32,
                values,
                truncated: false,
            })
        })
    }

    /// The coldest and hottest finite temperature of `p`.
    pub fn range_of(p: &NdPlane) -> Option<(f32, f32)> {
        p.values
            .iter()
            .filter(|v| v.is_finite())
            .fold(None, |r, &v| match r {
                None => Some((v, v)),
                Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
            })
    }

    fn memory_bytes(&self) -> usize {
        self.planes.memory_bytes()
    }
}

/// The camera palette as a 256-entry table, else an iron ramp.
fn palette_lut(t: &Thermal) -> Arc<ColourTable> {
    let mut out = [[0u8; 3]; 256];
    match t.palette.as_ref().filter(|p| p.colours.len() >= 2) {
        Some(p) => {
            for (o, c) in out.iter_mut().zip(p.resampled(256)) {
                *o = c;
            }
        }
        None => {
            // Black through indigo, magenta, red and orange to yellow-white.
            const STOPS: [(f32, [f32; 3]); 6] = [
                (0.0, [0.0, 0.0, 0.0]),
                (0.2, [0.17, 0.0, 0.45]),
                (0.45, [0.72, 0.0, 0.55]),
                (0.65, [0.93, 0.27, 0.05]),
                (0.85, [1.0, 0.72, 0.0]),
                (1.0, [1.0, 1.0, 0.85]),
            ];
            for (i, o) in out.iter_mut().enumerate() {
                let x = i as f32 / 255.0;
                let k = STOPS.windows(2).position(|w| x <= w[1].0).unwrap_or(4);
                let (a, b) = (STOPS[k], STOPS[k + 1]);
                let f = (x - a.0) / (b.0 - a.0);
                *o = std::array::from_fn(|c| {
                    ((a.1[c] + (b.1[c] - a.1[c]) * f) * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8
                });
            }
        }
    }
    Arc::new(out)
}

/// A plane through `lut` from its coldest to its hottest pixel; NaN transparent.
fn lut_raster(p: &NdPlane, lut: &ColourTable) -> Raster {
    let (lo, hi) = ThermalImage::range_of(p)
        .filter(|(l, h)| h > l)
        .unwrap_or((0.0, 1.0));
    let scale = 255.0 / (hi - lo);
    let mut rgba = Vec::with_capacity(p.values.len() * 4);
    for v in &p.values {
        if v.is_finite() {
            let c = lut[((v - lo) * scale).round().clamp(0.0, 255.0) as usize];
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        } else {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    whole::interleaved(p.width, p.height, PixelLayout::Rgba, rgba, p.truncated)
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

/// A number with at most `d` decimals, without trailing zeros.
fn num(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    }
}

/// The file's own fields, as header keywords (untranslated, like other formats' headers).
fn keywords(t: &Thermal, range: Option<(f32, f32)>) -> Vec<(String, String, String)> {
    let mut k = vec![kw("Format", t.kind.name().to_owned())];
    let c = &t.camera;
    for (name, v) in [
        ("Camera", &c.model),
        ("Part number", &c.part_number),
        ("Serial number", &c.serial),
        ("Software", &c.software),
        ("Lens", &c.lens),
        ("Lens part number", &c.lens_part_number),
        ("Lens serial number", &c.lens_serial),
        ("Filter", &c.filter),
    ] {
        if !v.is_empty() {
            k.push(kw(name, v.clone()));
        }
    }
    if let Some(f) = c.field_of_view {
        k.push(kw("Field of view", format!("{}°", num(f, 2))));
    }
    if let Some((lo, hi)) = c.range {
        k.push(kw(
            "Camera range",
            format!("{} … {} °C", num(lo, 2), num(hi, 2)),
        ));
    }
    if let Some(w) = c.wavelength {
        k.push(kw("Wavelength", format!("{} µm", num(w, 3))));
    }
    if let Some(d) = &c.date {
        k.push(kw("Date", d.clone()));
    }
    let o = &t.conditions;
    let deg = |v: f64| format!("{} °C", num(v, 2));
    for (name, v, f) in [
        (
            "Emissivity",
            o.emissivity,
            &(|v: f64| num(v, 3)) as &dyn Fn(f64) -> String,
        ),
        ("Object distance", o.distance, &|v| {
            format!("{} m", num(v, 2))
        }),
        ("Reflected temperature", o.reflected, &deg),
        ("Atmospheric temperature", o.atmospheric, &deg),
        ("Relative humidity", o.humidity, &|v| {
            format!("{} %", num(v * 100.0, 1))
        }),
        ("Window temperature", o.window_temperature, &deg),
        ("Window transmission", o.window_transmission, &|v| num(v, 3)),
    ] {
        if let Some(v) = v {
            k.push(kw(name, f(v)));
        }
    }
    match &t.pixels {
        Pixels::Counts {
            mapping: Mapping::Planck(p),
            ..
        } => {
            for (name, v) in [
                ("Planck R1", p.r1),
                ("Planck R2", p.r2),
                ("Planck B", p.b),
                ("Planck F", p.f),
                ("Planck O", p.o),
                ("Atmospheric alpha 1", p.alpha1),
                ("Atmospheric alpha 2", p.alpha2),
                ("Atmospheric beta 1", p.beta1),
                ("Atmospheric beta 2", p.beta2),
                ("Atmospheric X", p.x),
            ] {
                k.push(kw(name, format!("{v}")));
            }
        }
        Pixels::Counts {
            mapping: Mapping::Linear { divisor, offset },
            ..
        } if t.kind == Kind::TestoBmt => {
            k.push(kw(
                "Scale",
                format!(
                    "{} … {} °C over 16 bits",
                    num(-offset, 2),
                    num(65535.0 / divisor - offset, 2)
                ),
            ));
        }
        Pixels::Counts {
            mapping: Mapping::Linear { divisor, offset },
            ..
        } => {
            k.push(kw("Scale", format!("counts / {divisor} − {offset}")));
        }
        Pixels::Counts {
            mapping: Mapping::Fluke(f),
            ..
        } => {
            for (i, c) in f.curves().iter().enumerate() {
                k.push(kw(
                    &format!("Calibration curve {}", i + 1),
                    format!(
                        "{} … {} °C: {}·T² + {}·T + {}",
                        num(c.t0, 1),
                        num(c.t1, 1),
                        c.a,
                        c.b,
                        c.c
                    ),
                ));
            }
        }
        Pixels::Celsius(_) => {}
    }
    if let Some((lo, hi)) = t.raw_range() {
        k.push(kw("Raw range", format!("{lo} … {hi}")));
    }
    if let Some((lo, hi)) = range {
        k.push(kw(
            "Temperature range",
            format!("{} … {} °C", num(f64::from(lo), 2), num(f64::from(hi), 2)),
        ));
    }
    if let Some(p) = &t.palette
        && !p.name.is_empty()
    {
        k.push(kw("Palette", p.name.clone()));
    }
    if let Some(v) = &t.visible {
        k.push(kw("Visible photo", format!("{} × {}", v.width, v.height)));
    }
    k
}

/// What the session shares with its visible-photo page, to come back to it.
#[derive(Clone)]
struct Shared {
    source: Arc<Source>,
    image: Arc<ThermalImage>,
    info: ImageInfo,
    summary: Arc<ScienceSummary>,
}

struct ThermalSession {
    s: Shared,
    levels: Levels,
}

impl ThermalSession {
    fn new(source: Arc<Source>, image: Arc<ThermalImage>) -> Result<ThermalSession> {
        let t = &image.thermal;
        let (w, h) = (t.width as u32, t.height as u32);
        let format = format_of(t.kind);
        let p = image.plane(
            &t.conditions,
            TemperatureUnit::Celsius,
            &CancelToken::default(),
        )?;
        let range = ThermalImage::range_of(&p);
        let sample = match (&t.pixels, t.kind) {
            (Pixels::Counts { .. }, Kind::FlirCsq) => "16-bit counts, JPEG-LS".to_owned(),
            (Pixels::Counts { .. }, _) => "16-bit counts".to_owned(),
            (Pixels::Celsius(_), _) => "32-bit float temperatures".to_owned(),
        };
        let mut keywords = keywords(t, range);
        let (axes, video) = match &image.seq {
            Some(f) => {
                let (axis, video) = frame_axis(&f.index);
                let n = f.index.len();
                keywords.insert(1, kw("Frames", n.to_string()));
                keywords.insert(2, kw("Frame rate", format!("{} /s", num(f.index.rate, 3))));
                keywords.insert(
                    3,
                    kw(
                        "Duration",
                        format!(
                            "{} s",
                            num(f.index.seconds(n - 1) + f.index.duration(n - 1), 3)
                        ),
                    ),
                );
                (vec![axis], Some(video))
            }
            None => (Vec::new(), None),
        };
        let summary = ScienceSummary {
            axes,
            video,
            width: w,
            height: h,
            sample: sample.clone(),
            instrument: Some(t.camera.model.clone()).filter(|m| !m.is_empty()),
            date_obs: t.camera.date.clone(),
            unit: Some("°C".into()),
            keywords: Arc::new(keywords),
            kind: ScienceKind::Thermal,
            luts: vec![palette_lut(t)],
            thermal: Some(image.clone()),
            ..Default::default()
        };
        let info = ImageInfo {
            format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: match t.pixels {
                Pixels::Counts { .. } => 16,
                Pixels::Celsius(_) => 32,
            },
            progressive: false,
            coding: sample,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(ThermalSession {
            s: Shared {
                source,
                image,
                info,
                summary: Arc::new(summary),
            },
            levels: Levels::new(),
        })
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let t = &self.s.image.thermal;
        let p = self
            .s
            .image
            .plane(&t.conditions, TemperatureUnit::Celsius, cancel)?;
        Ok(lut_raster(&p, &self.s.summary.luts[0]))
    }
}

/// A sequence's frame axis, each position labelled with its time from the first frame, and
/// its capture facts.
fn frame_axis(seq: &th::Sequence) -> (NdAxis, VideoFacts) {
    let n = seq.len();
    let labels = (0..n)
        .map(|i| format!("{} s", num(seq.seconds(i), 3)))
        .collect();
    let axis = NdAxis {
        name: String::new(),
        len: n as u32,
        unit: "s".into(),
        labels,
        wcs_axis: None,
        linear: None,
        kind: AxisKind::Frame,
    };
    let time = |i: usize| seq.frames[i].time.map(th::utc_text);
    let video = VideoFacts {
        frames: n as u32,
        declared: n as u32,
        start: time(0),
        end: time(n - 1),
        frame_rate: Some(seq.rate),
    };
    (axis, video)
}

fn page_count(s: &Shared) -> u32 {
    if s.image.seq.is_none() && s.image.thermal.visible.is_some() {
        2
    } else {
        1
    }
}

/// Page 0 is the thermal image, page 1 the visible photo.
fn open_page(s: &Shared, page: u32) -> Result<Arc<dyn Session>> {
    match page {
        0 => Ok(Arc::new(ThermalSession {
            s: s.clone(),
            levels: Levels::new(),
        })),
        1 => {
            let v = s
                .image
                .thermal
                .visible
                .as_ref()
                .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
            let src = Arc::new(Source::from_entry(
                s.source.path(),
                &s.source,
                v.bytes.clone(),
            ));
            let inner = if v.bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                png_dec::PngDecoder.open(src)?
            } else {
                jpeg::JpegDecoder.open(src)?
            };
            Ok(Arc::new(VisiblePage {
                s: s.clone(),
                inner,
            }))
        }
        _ => Err(DecodeError::Malformed("no such page".into())),
    }
}

impl Session for ThermalSession {
    fn info(&self) -> &ImageInfo {
        &self.s.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.s.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.s.info.width, self.s.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.s.info.width) * u64::from(self.s.info.height);
        px * 8 + whole::level_bytes(self.s.info.width, self.s.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.s.image.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.s.image.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.s.summary)
    }
    /// The temperatures in degrees Celsius under the recorded conditions (of the frame
    /// `index` names, for a sequence).
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        let c = self.s.image.thermal.conditions;
        let frame = index.first().copied().unwrap_or(0);
        Some(
            self.s
                .image
                .plane_at(frame, &c, TemperatureUnit::Celsius, cancel),
        )
    }
    fn page_count(&self) -> u32 {
        page_count(&self.s)
    }
    fn page_label(&self) -> Option<PageLabel> {
        (page_count(&self.s) > 1).then_some(PageLabel::Thermal { visible: false })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.s, page)
    }
}

/// The embedded visible photo, decoded by its own format's decoder.
struct VisiblePage {
    s: Shared,
    inner: Arc<dyn Session>,
}

impl Session for VisiblePage {
    fn info(&self) -> &ImageInfo {
        self.inner.info()
    }
    fn source(&self) -> &Arc<Source> {
        self.inner.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.scaled_decode_cheap()
    }
    fn want_levels(&self, scales: &[u32]) {
        self.inner.want_levels(scales)
    }
    fn colour_space(&self) -> Option<&'static str> {
        self.inner.colour_space()
    }
    fn page_count(&self) -> u32 {
        2
    }
    fn page_index(&self) -> u32 {
        1
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(PageLabel::Thermal { visible: true })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.s, page)
    }
}

#[cfg(test)]
mod tests;
