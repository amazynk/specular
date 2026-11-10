//! A format-neutral description of a materials-science image, the one shape the TIA, Bruker
//! BCF, EBSD, AFM and areal-topography readers hand to the viewer: pages of equally sized
//! planes (channels, or positions along one series axis) with their lateral calibration,
//! value units and acquisition facts, read one plane at a time as `f32`.
//!
//! A page may also carry a spectrum per pixel ([`Spectra`]: an EDS hypermap, a TIA spectrum
//! image) and crystal orientations per pixel ([`Orientations`]: an EBSD map) for the readout.
//!
//! Rows are top-down as displayed: a reader whose file stores rows bottom-up says so per plane
//! ([`PlaneData::Stored::bottom_up`]) or flips its own [`PlaneData::Values`].

use std::sync::Arc;

use crate::samples::{self, Order, Sample};

/// The field an image comes from, for wording and the info panel's group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Field {
    /// TEM / STEM / SEM images and spectra (TIA, BCF).
    #[default]
    ElectronMicroscopy,
    /// Scanning probe height maps and other channels (Nanoscope, Nanonis, JPK, NT-MDT).
    ScanningProbe,
    /// Areal topography from profilometers and optical surface metrology (X3P, Zygo,
    /// Keyence, Sensofar, Alicona, Digital Surf).
    Surface,
    /// Electron backscatter diffraction maps (.ang, .ctf).
    Ebsd,
}

/// A linear calibration: the value at index `i` is `offset + step × i`, in `unit`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lateral {
    pub offset: f64,
    pub step: f64,
    /// "m", "nm", "µm", "1/nm", "keV"…; may be empty.
    pub unit: String,
}

impl Lateral {
    pub fn new(offset: f64, step: f64, unit: impl Into<String>) -> Lateral {
        Lateral {
            offset,
            step,
            unit: unit.into(),
        }
    }

    pub fn at(&self, i: f64) -> f64 {
        self.offset + self.step * i
    }

    /// Finite with a nonzero step.
    pub fn is_usable(&self) -> bool {
        self.offset.is_finite() && self.step.is_finite() && self.step != 0.0
    }
}

/// What a page's planes are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisKind {
    /// Different quantities of one scan (height, phase, amplitude; IQ, CI), each with its own
    /// label and unit.
    #[default]
    Channel,
    /// Positions along one physical axis (a time or defocus series, a stack).
    Series,
}

/// The axis the planes of a page lie along (ignored for a single plane).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub kind: AxisKind,
    /// The axis' name ("Channel", "Frame", "Defocus"); may be empty.
    pub name: String,
    /// World value of each position, for a series.
    pub linear: Option<Lateral>,
}

/// One plane: its label, value unit and where its values come from.
#[derive(Clone, Debug)]
pub struct Plane {
    /// The channel's name ("Height", "Phase", "Image quality"); may be empty.
    pub label: String,
    /// Unit of the values ("m", "deg", "V"); may be empty.
    pub unit: String,
    pub data: PlaneData,
}

/// Where a plane's values come from.
#[derive(Clone, Debug)]
pub enum PlaneData {
    /// Values already computed (decompressed or derived), `width × height`, rows top-down,
    /// NaN where undefined.
    Values(Arc<[f32]>),
    /// Samples stored contiguously in the file, row-major: the value of a raw sample `r` is
    /// `r × scale + add`.
    Stored {
        offset: usize,
        sample: Sample,
        order: Order,
        /// Stored rows run bottom-up.
        bottom_up: bool,
        scale: f64,
        add: f64,
    },
}

impl Plane {
    /// A plane of computed values.
    pub fn values(label: impl Into<String>, unit: impl Into<String>, v: Vec<f32>) -> Plane {
        Plane {
            label: label.into(),
            unit: unit.into(),
            data: PlaneData::Values(v.into()),
        }
    }

    /// Reads the plane's `width × height` values into `out` (rows top-down). Samples past the
    /// end of `file` become NaN; returns whether any were missing.
    pub fn read(&self, file: &[u8], width: usize, height: usize, out: &mut [f32]) -> bool {
        let n = width * height;
        let out = &mut out[..n];
        match &self.data {
            PlaneData::Values(v) => {
                let k = v.len().min(n);
                out[..k].copy_from_slice(&v[..k]);
                out[k..].fill(f32::NAN);
                k < n
            }
            &PlaneData::Stored {
                offset,
                sample,
                order,
                bottom_up,
                scale,
                add,
            } => {
                let bytes = file.get(offset..).unwrap_or(&[]);
                let missing = samples::convert(bytes, sample, order, sample.bytes(), out);
                if scale != 1.0 || add != 0.0 {
                    for v in out.iter_mut() {
                        *v = (f64::from(*v) * scale + add) as f32;
                    }
                }
                if bottom_up && width > 0 {
                    for y in 0..height / 2 {
                        let (top, bottom) = out.split_at_mut((height - 1 - y) * width);
                        top[y * width..(y + 1) * width].swap_with_slice(&mut bottom[..width]);
                    }
                }
                missing
            }
        }
    }
}

/// A spectrum per pixel (an EDS hypermap, a TIA spectrum image), read at the cursor.
pub trait Spectra: Send + Sync + std::fmt::Debug {
    /// The grid the spectra are on: columns, rows. The page's pixels map onto it by
    /// proportion when the sizes differ.
    fn grid(&self) -> (usize, usize);
    /// Channels per spectrum.
    fn channels(&self) -> usize;
    /// The channel axis: energy of channel `k` (keV for EDS, eV for EELS).
    fn axis(&self) -> Lateral;
    /// The spectrum at grid point (`x`, `y`) (counts per channel), from the file's bytes.
    fn at(&self, file: &[u8], x: usize, y: usize) -> Option<Vec<f32>>;
    /// The file's sum spectrum, when it stores one.
    fn sum(&self) -> Option<Vec<f32>> {
        None
    }
}

/// A crystal phase of an orientation map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phase {
    pub name: String,
    /// The Laue or point group as the file writes it ("m-3m", "6/mmm", "43", "62").
    pub symmetry: String,
}

/// Crystal orientations per pixel (an EBSD map), for the readout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Orientations {
    pub width: usize,
    pub height: usize,
    /// Bunge Euler angles (φ1, Φ, φ2) in radians per displayed pixel (rows top-down); NaN
    /// where nothing was indexed.
    pub euler: Vec<[f32; 3]>,
    /// Index into `phases` per pixel; `u8::MAX` where nothing was indexed.
    pub phase: Vec<u8>,
    pub phases: Vec<Phase>,
}

impl Orientations {
    /// Euler angles and phase at a displayed pixel, when indexed.
    pub fn at(&self, x: usize, y: usize) -> Option<([f32; 3], Option<&Phase>)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = y * self.width + x;
        let e = *self.euler.get(i)?;
        if e.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let ph = self
            .phase
            .get(i)
            .and_then(|&p| self.phases.get(usize::from(p)));
        Some((e, ph))
    }
}

/// One image: equally sized planes with their calibration and acquisition facts.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub field: Field,
    /// The image's title or the dataset's name.
    pub title: Option<String>,
    pub width: usize,
    pub height: usize,
    pub planes: Vec<Plane>,
    /// What the planes are (when there are several).
    pub axis: Axis,
    /// Physical position of displayed column `i` (pixel centres).
    pub x: Option<Lateral>,
    /// Physical position of displayed row `j` (pixel centres, rows top-down).
    pub y: Option<Lateral>,
    /// The stored sample type, for the info panel; `None` for computed values.
    pub sample: Option<Sample>,
    pub instrument: Option<String>,
    /// Acquisition date and time as the file writes it.
    pub date: Option<String>,
    /// Accelerating voltage, volts.
    pub voltage: Option<f64>,
    pub magnification: Option<f64>,
    /// Imaging mode ("STEM", "Tapping", "Contact").
    pub mode: Option<String>,
    pub detector: Option<String>,
    /// Seconds.
    pub exposure: Option<f64>,
    /// Header fields (name, value), in file order.
    pub meta: Vec<(String, String)>,
    /// The planes are height maps that plane levelling suits.
    pub levelling: bool,
    /// The first three planes are red, green and blue in 0..=1 (an IPF map, a colour image),
    /// shown composited at that fixed range.
    pub rgb: bool,
    pub spectra: Option<Arc<dyn Spectra>>,
    pub orientations: Option<Arc<Orientations>>,
}

impl Page {
    /// Reads plane `k` into a new `width × height` buffer: values and whether samples were
    /// missing.
    pub fn read_plane(&self, file: &[u8], k: usize) -> Option<(Vec<f32>, bool)> {
        let p = self.planes.get(k)?;
        let n = self.width.checked_mul(self.height)?;
        // Stored samples must mostly be in the file (a cut file still shows): a header
        // claiming a 2^30-sample plane over a few bytes was given its 4 GB buffer.
        if let PlaneData::Stored { offset, sample, .. } = &p.data {
            let have = file.len().saturating_sub(*offset) as u64;
            let need = (n as u64).saturating_mul(sample.bytes() as u64);
            if n > 1 << 24 && have < need / 16 {
                return None;
            }
        }
        let mut v = Vec::new();
        v.try_reserve_exact(n).ok()?;
        v.resize(n, 0f32);
        let missing = p.read(file, self.width, self.height, &mut v);
        Some((v, missing))
    }
}

/// A file's pages.
#[derive(Clone, Debug, Default)]
pub struct Dataset {
    pub pages: Vec<Page>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_scaled_and_flipped() {
        let bytes: Vec<u8> = [1i16, 2, 3, 4, 5, 6]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let p = Plane {
            label: "Height".into(),
            unit: "m".into(),
            data: PlaneData::Stored {
                offset: 0,
                sample: Sample::I16,
                order: Order::Little,
                bottom_up: true,
                scale: 2.0,
                add: 1.0,
            },
        };
        let mut out = [0f32; 6];
        assert!(!p.read(&bytes, 3, 2, &mut out));
        assert_eq!(out, [9.0, 11.0, 13.0, 3.0, 5.0, 7.0]);
        let mut out = [0f32; 8];
        assert!(p.read(&bytes, 4, 2, &mut out));
        let v = Plane::values("", "", vec![1.0, 2.0]);
        let mut out = [0f32; 3];
        assert!(v.read(&[], 3, 1, &mut out));
        assert!(out[2].is_nan());
    }

    /// A page claiming 32768×32768 stored f32 samples over 64 bytes: its 4 GB buffer was
    /// reserved (and zeroed) before reading. Refused now; a plane cut to 1/16 still reads.
    #[test]
    fn stored_plane_beyond_the_file() {
        let plane = |offset| Plane {
            label: String::new(),
            unit: String::new(),
            data: PlaneData::Stored {
                offset,
                sample: Sample::F32,
                order: Order::Little,
                bottom_up: false,
                scale: 1.0,
                add: 0.0,
            },
        };
        let page = Page {
            width: 32768,
            height: 32768,
            planes: vec![plane(0)],
            ..Default::default()
        };
        assert!(page.read_plane(&[0; 64], 0).is_none());
        let page = Page {
            width: 4096,
            height: 4096,
            planes: vec![plane(0)],
            ..Default::default()
        };
        let file = vec![0u8; 4096 * 4096 * 4 / 16];
        let (v, missing) = page.read_plane(&file, 0).unwrap();
        assert!(missing && v.len() == 4096 * 4096);
    }

    #[test]
    fn orientation_lookup() {
        let o = Orientations {
            width: 2,
            height: 1,
            euler: vec![[0.1, 0.2, 0.3], [f32::NAN; 3]],
            phase: vec![0, u8::MAX],
            phases: vec![Phase {
                name: "Ni".into(),
                symmetry: "m-3m".into(),
            }],
        };
        assert_eq!(o.at(0, 0).unwrap().1.unwrap().name, "Ni");
        assert!(o.at(1, 0).is_none());
        assert!(o.at(2, 0).is_none());
    }
}
