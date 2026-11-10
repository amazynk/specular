//! A colour-managed L*a*b* conversion, supplied by the caller for the whole process.
//!
//! The decoder converts L*a*b* (D50) to sRGB with a plain scalar formula. An application with
//! a colour-management engine registers a faster or more exact one with [`set_lab_hook`]; it
//! gets the samples where they lie (chunky rows or planes, no copy) and their encoding.

use std::sync::{Arc, RwLock};

/// How L*a*b* samples are stored: channel `c` is `sample · scale[c] + offset[c]` (L 0..100,
/// a*/b* about −128..127), the sample read as an unsigned or (`signed[c]`) two's-complement
/// integer of 8 or (`wide`) 16 bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabCoding {
    pub wide: bool,
    /// Byte order of 16-bit samples.
    pub big_endian: bool,
    pub signed: [bool; 3],
    pub scale: [f32; 3],
    pub offset: [f32; 3],
}

impl LabCoding {
    /// Channel `c` of a raw sample value (as read, unsigned).
    #[inline]
    pub fn value(&self, c: usize, raw: u32) -> f32 {
        let half = if self.wide { 32768 } else { 128 };
        let v = if self.signed[c] && raw >= half {
            raw as i32 - 2 * half as i32
        } else {
            raw as i32
        };
        v as f32 * self.scale[c] + self.offset[c]
    }
}

/// One run of pixels: `channels[c]` starts at the first pixel's sample `c`, consecutive
/// pixels lie `step` bytes apart.
#[derive(Clone, Copy, Debug)]
pub struct LabSamples<'a> {
    pub channels: [&'a [u8]; 3],
    pub step: usize,
    pub coding: LabCoding,
}

/// Converts one run: fills `rgba` (4 bytes per pixel, `rgba.len() / 4` pixels) with
/// sRGB-encoded colour, alpha 255.
pub type LabRowConverter = Arc<dyn Fn(&LabSamples, &mut [u8]) + Send + Sync>;

/// The converter to use; `None` keeps the built-in formula.
pub type LabHook = fn() -> Option<LabRowConverter>;

static HOOK: RwLock<Option<LabHook>> = RwLock::new(None);

/// Registers (or with `None` removes) the process-wide L*a*b* hook.
pub fn set_lab_hook(hook: Option<LabHook>) {
    if let Ok(mut h) = HOOK.write() {
        *h = hook;
    }
}

/// The converter, if a hook supplies one.
pub(crate) fn converter() -> Option<LabRowConverter> {
    let hook = HOOK.read().ok().and_then(|h| *h)?;
    hook()
}
