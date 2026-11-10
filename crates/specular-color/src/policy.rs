//! Process-wide colour-management policy (set from the settings) and caches of baked
//! transforms, so decoders on worker threads convert consistently.

use crate::cmyk::CmykConverter;
use crate::gpu::{self, SourceLut};
use crate::icc::{ColorSpace, Profile};
use crate::transform::Intent;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// What the user asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Policy {
    /// Colour management on. When off, embedded profiles are ignored, CMYK is converted
    /// naively and the display is treated as sRGB.
    pub enabled: bool,
    /// Rendering intent.
    pub intent: Intent,
    /// Black point compensation.
    pub bpc: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            enabled: true,
            intent: Intent::Relative,
            bpc: true,
        }
    }
}

static POLICY: RwLock<Policy> = RwLock::new(Policy {
    enabled: true,
    intent: Intent::Relative,
    bpc: true,
});

/// Sets the policy. Images decoded afterwards use it; callers reload what's on screen.
pub fn set_policy(p: Policy) {
    if let Ok(mut g) = POLICY.write() {
        *g = p;
    }
}

/// The current policy.
pub fn policy() -> Policy {
    POLICY.read().map(|g| *g).unwrap_or_default()
}

type Key = (u64, Policy);
type Slot = Arc<OnceLock<Arc<CmykConverter>>>;

/// The CMYK converter for an embedded profile under the current policy: the profile's own
/// table, the generic CMYK model when it's missing or unusable, or the naive formula when
/// colour management is off. Cached (baking takes tens of milliseconds).
pub fn cmyk_converter(icc: Option<&[u8]>) -> Arc<CmykConverter> {
    let p = policy();
    if !p.enabled {
        static NAIVE: OnceLock<Arc<CmykConverter>> = OnceLock::new();
        return NAIVE
            .get_or_init(|| Arc::new(CmykConverter::naive()))
            .clone();
    }
    let parsed = icc
        .and_then(|d| Profile::parse(d).ok())
        .filter(|p| p.color_space == ColorSpace::Cmyk);
    let key = (parsed.as_ref().map_or(0, |p| p.hash), p);
    static CACHE: Mutex<Vec<(Key, Slot)>> = Mutex::new(Vec::new());
    let slot = {
        let Ok(mut c) = CACHE.lock() else {
            return Arc::new(CmykConverter::generic(p.intent, p.bpc));
        };
        if let Some(i) = c.iter().position(|(k, _)| *k == key) {
            let e = c.remove(i);
            let s = e.1.clone();
            c.push(e);
            s
        } else {
            if c.len() >= 4 {
                c.remove(0);
            }
            let s: Slot = Arc::default();
            c.push((key, s.clone()));
            s
        }
    };
    slot.get_or_init(|| {
        let conv = parsed.and_then(|prof| CmykConverter::new(&prof, p.intent, p.bpc).ok());
        Arc::new(conv.unwrap_or_else(|| CmykConverter::generic(p.intent, p.bpc)))
    })
    .clone()
}

/// The GPU source transform for an embedded RGB/gray profile under the current policy
/// (`None`: treat as sRGB).
pub fn source_lut(icc: &[u8]) -> Option<SourceLut> {
    let p = policy();
    if !p.enabled {
        return None;
    }
    gpu::source_lut(icc, p.intent, p.bpc)
}
