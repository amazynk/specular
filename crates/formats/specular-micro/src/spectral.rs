//! Spectroscopy camera files: Princeton Instruments SPE (WinView/WinSpec 2.x and LightField
//! 3.0) and Andor SIF. Both hold frames from a CCD behind a spectrograph: each frame is
//! read as a page (one per frame and region of interest), its columns usually wavelengths.

use std::sync::Arc;

use crate::{Error, Result};

/// How the samples of a frame are stored (little-endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Num {
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Num {
    pub fn bytes(self) -> usize {
        match self {
            Num::U8 => 1,
            Num::I16 | Num::U16 => 2,
            Num::I32 | Num::U32 | Num::F32 => 4,
            Num::F64 => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Num::U8 => "8-bit unsigned",
            Num::I16 => "16-bit signed",
            Num::U16 => "16-bit unsigned",
            Num::I32 => "32-bit signed",
            Num::U32 => "32-bit unsigned",
            Num::F32 => "32-bit float",
            Num::F64 => "64-bit float",
        }
    }

    #[inline]
    fn read(self, b: &[u8]) -> f32 {
        match self {
            Num::U8 => f32::from(b[0]),
            Num::I16 => f32::from(i16::from_le_bytes([b[0], b[1]])),
            Num::U16 => f32::from(u16::from_le_bytes([b[0], b[1]])),
            Num::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
            Num::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
            Num::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Num::F64 => f64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8])) as f32,
        }
    }
}

/// One page: a frame's region.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub frame: usize,
    pub region: usize,
    pub width: usize,
    pub height: usize,
    /// Byte offset of the first sample.
    pub offset: usize,
    pub sample: Num,
    /// The wavelength (nm) of each column, when calibrated.
    pub wavelengths: Option<Arc<[f64]>>,
    /// Where the region lies on the sensor and its binning (x, y, x binning, y binning).
    pub sensor: Option<[u32; 4]>,
}

impl Page {
    /// The page's values, rows top-down, and whether the file ends before them.
    pub fn read(&self, data: &[u8]) -> Result<(Vec<f32>, bool)> {
        self.read_reserved(data).map(|(v, t, _)| (v, t))
    }

    /// [`Page::read`], with the memory budget's share of the values (reserved before they are
    /// allocated: a header of a few bytes can declare 4 GiB of them), for as long as the
    /// caller keeps them.
    pub fn read_reserved(
        &self,
        data: &[u8],
    ) -> Result<(Vec<f32>, bool, specular_core::Reservation)> {
        let n = self.width.checked_mul(self.height).ok_or(Error::TooLarge)?;
        if n as u64 > crate::MAX_PLANE {
            return Err(Error::TooLarge);
        }
        let held = specular_core::budget::reserve_n(n as u64, 4)?;
        let s = self.sample.bytes();
        let mut out = vec![0f32; n];
        let avail = data.len().saturating_sub(self.offset) / s;
        let body = data.get(self.offset..).unwrap_or(&[]);
        for (o, b) in out.iter_mut().zip(body.chunks_exact(s)) {
            *o = self.sample.read(b);
        }
        Ok((out, avail < n, held))
    }
}

/// A spectroscopy file's frames and facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spectral {
    /// "SPE 3.0", "SPE 2.x", "SIF 65567".
    pub version: String,
    pub pages: Vec<Page>,
    pub frames: usize,
    /// Seconds.
    pub exposure: Option<f64>,
    /// °C.
    pub temperature: Option<f64>,
    pub gain: Option<String>,
    pub detector: Option<String>,
    pub spectrograph: Option<String>,
    pub date: Option<String>,
    /// Further facts (key, value) for the info panel.
    pub facts: Vec<(String, String)>,
}

/// A polynomial calibration's wavelengths at `n` columns numbered from `first`.
pub(crate) fn polynomial(coeffs: &[f64], n: usize, first: f64) -> Option<Arc<[f64]>> {
    let c: Vec<f64> = coeffs
        .iter()
        .copied()
        .take_while(|v| v.is_finite())
        .collect();
    // Unset (all zero) or the identity (pixel numbers) is no calibration.
    if c.iter().all(|&v| v == 0.0)
        || (c.len() >= 2 && c[0] == 0.0 && c[1] == 1.0 && c[2..].iter().all(|&v| v == 0.0))
    {
        return None;
    }
    let w: Vec<f64> = (0..n)
        .map(|i| {
            let x = first + i as f64;
            c.iter().rev().fold(0.0, |acc, &k| acc * x + k)
        })
        .collect();
    w.iter().all(|v| v.is_finite()).then(|| w.into())
}
