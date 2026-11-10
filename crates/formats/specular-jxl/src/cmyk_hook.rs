//! A colour-managed CMYK conversion, supplied by the caller for the whole process.
//!
//! The same contract as `specular-jpeg`'s and `specular-tiff`'s hooks, so one converter serves
//! all three: the decoder itself only knows the naive formula. An application with a
//! colour-management engine registers a hook with [`set_cmyk_hook`]; for each CMYK image the
//! hook gets the embedded ICC profile and may return a row converter, used instead of the
//! formula by [`crate::JxlFile::decode`] and [`crate::JxlFile::frames`].

use std::sync::{Arc, RwLock};

/// Converts one row: `ink` holds 4 bytes per pixel (C, M, Y, K ink amounts, 0 = no ink,
/// 255 = full), `rgba` 4 bytes per pixel to fill with sRGB-encoded colour (alpha 255).
pub type CmykRowConverter = Arc<dyn Fn(&[u8], &mut [u8]) + Send + Sync>;

/// Given the embedded profile (if any), the converter for an image; `None` keeps the naive
/// formula.
pub type CmykHook = fn(Option<&[u8]>) -> Option<CmykRowConverter>;

static HOOK: RwLock<Option<CmykHook>> = RwLock::new(None);

/// Registers (or with `None` removes) the process-wide CMYK hook.
pub fn set_cmyk_hook(hook: Option<CmykHook>) {
    if let Ok(mut h) = HOOK.write() {
        *h = hook;
    }
}

/// The converter for an image with profile `icc`, if a hook supplies one.
pub(crate) fn converter(icc: Option<&[u8]>) -> Option<CmykRowConverter> {
    let hook = HOOK.read().ok().and_then(|h| *h)?;
    hook(icc)
}
