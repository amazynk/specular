//! An edit: the module stack of one version of one file.

use super::fields::Fields;
use super::kind::{ModuleKind, Registry};
use super::mask::{Blend, MaskShape};
use super::params::Params;

/// The current edit schema: the binary layout's version ([`super::codec::FORMAT`]).
pub const SCHEMA: u16 = super::codec::FORMAT;
/// The current process version (the algorithm generation new edits get).
pub const PROCESS: u16 = 1;

/// Identifies the source file an edit was made on, to catch a replaced file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceKey {
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// BLAKE3 of the first and last 64 KiB.
    pub head_tail: [u8; 32],
}

/// One module in the stack.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleInstance {
    pub kind: ModuleKind,
    /// The param version, which also picks the kernel.
    pub version: u16,
    /// Distinguishes several instances of one kind (0 for the first).
    pub instance: u16,
    pub label: Option<String>,
    pub enabled: bool,
    pub params: Params,
    pub blend: Option<Blend>,
}

impl ModuleInstance {
    /// A new instance with the registry's defaults (empty fields for unknown kinds).
    pub fn new(registry: &Registry, kind: ModuleKind, instance: u16) -> Self {
        let (version, fields) = registry
            .get(kind)
            .map_or((1, Fields::new()), |s| (s.current, (s.defaults)()));
        Self {
            kind,
            version,
            instance,
            label: None,
            enabled: true,
            params: Params::from_fields(kind, fields),
            blend: None,
        }
    }

    /// A new instance from typed params at their current version.
    pub fn with_params<P: super::params::ModuleParams>(p: P, instance: u16) -> Self
    where
        Params: From<P>,
    {
        Self {
            kind: P::KIND,
            version: P::VERSION,
            instance,
            label: None,
            enabled: true,
            params: Params::from(p),
            blend: None,
        }
    }

    pub fn id(&self) -> (ModuleKind, u16) {
        (self.kind, self.instance)
    }
}

macro_rules! params_from {
    ($($t:ident),*) => {$(
        impl From<super::params::$t> for Params {
            fn from(p: super::params::$t) -> Self {
                Params::$t(p)
            }
        }
    )*};
}
params_from!(
    RawFrontEnd,
    Exposure,
    ColourCalibration,
    ViewTransform,
    CropOrientation
);

/// A named state to compare against.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub name: String,
    pub modules: Vec<ModuleInstance>,
    pub masks: Vec<MaskShape>,
}

impl Snapshot {
    /// The state of `edit` now, named.
    pub fn take(name: &str, edit: &Edit) -> Self {
        Self {
            name: name.to_owned(),
            modules: edit.modules.clone(),
            masks: edit.masks.clone(),
        }
    }

    /// `edit` as the snapshot has it (to compare against, or to go back to). The snapshots
    /// themselves stay.
    pub fn edit_from(&self, edit: &Edit) -> Edit {
        let mut e = edit.clone();
        e.modules.clone_from(&self.modules);
        e.masks.clone_from(&self.masks);
        e
    }
}

/// The edit of one version of a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub schema: u16,
    pub process: u16,
    pub source: SourceKey,
    /// In pipeline order.
    pub modules: Vec<ModuleInstance>,
    /// The user moved modules out of the recommended order.
    pub custom_order: bool,
    /// Shared shapes, referenced from blends.
    pub masks: Vec<MaskShape>,
    pub snapshots: Vec<Snapshot>,
}

/// The alpha's default stack, in pipeline order once sorted.
const DEFAULT_STACK: [ModuleKind; 5] = [
    ModuleKind::RAW_FRONT_END,
    ModuleKind::CROP_ORIENTATION,
    ModuleKind::EXPOSURE,
    ModuleKind::COLOUR_CALIBRATION,
    ModuleKind::VIEW_TRANSFORM,
];

impl Edit {
    /// An edit with no modules.
    pub fn empty(source: SourceKey) -> Self {
        Self {
            schema: SCHEMA,
            process: PROCESS,
            source,
            modules: Vec::new(),
            custom_order: false,
            masks: Vec::new(),
            snapshots: Vec::new(),
        }
    }

    /// The scene-referred default stack for a raw file (non-raw files drop the raw front end).
    pub fn default_for(registry: &Registry, source: SourceKey, raw: bool) -> Self {
        let mut e = Self::empty(source);
        for k in DEFAULT_STACK {
            if raw || k != ModuleKind::RAW_FRONT_END {
                e.modules.push(ModuleInstance::new(registry, k, 0));
            }
        }
        e.sort(registry);
        e
    }

    pub fn module(&self, kind: ModuleKind, instance: u16) -> Option<&ModuleInstance> {
        self.modules
            .iter()
            .find(|m| m.kind == kind && m.instance == instance)
    }

    pub fn module_mut(&mut self, kind: ModuleKind, instance: u16) -> Option<&mut ModuleInstance> {
        self.modules
            .iter_mut()
            .find(|m| m.kind == kind && m.instance == instance)
    }

    pub fn position(&self, kind: ModuleKind, instance: u16) -> Option<usize> {
        self.modules
            .iter()
            .position(|m| m.kind == kind && m.instance == instance)
    }

    /// The next free instance number for `kind`.
    pub fn next_instance(&self, kind: ModuleKind) -> u16 {
        self.modules
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| m.instance + 1)
            .max()
            .unwrap_or(0)
    }

    /// The next free mask shape id.
    pub fn next_mask_id(&self) -> u32 {
        self.masks.iter().map(|m| m.id + 1).max().unwrap_or(1)
    }

    pub fn mask(&self, id: u32) -> Option<&MaskShape> {
        self.masks.iter().find(|m| m.id == id)
    }

    /// The modules whose masks use `kind`/`instance`'s ("Use mask from"), in stack order.
    pub fn mask_users(&self, kind: ModuleKind, instance: u16) -> Vec<(ModuleKind, u16)> {
        self.modules
            .iter()
            .filter(|m| (m.kind, m.instance) != (kind, instance))
            .filter(|m| {
                m.blend
                    .as_ref()
                    .is_some_and(|b| b.mask.refers_to(kind, instance))
            })
            .map(|m| m.id())
            .collect()
    }

    /// Copies `kind`/`instance`'s combined mask into every module that uses it, so they
    /// keep it when it goes (its references are resolved first). Without a blend there the
    /// users mask nothing, as the reference did.
    pub fn inline_mask_users(&mut self, kind: ModuleKind, instance: u16) {
        let with = match self.module(kind, instance).and_then(|m| m.blend.as_ref()) {
            Some(b) => {
                let mut e = b.combined();
                // A reference of its own to a user would loop; leave that one as it is.
                e.replace_reference(kind, instance, &super::mask::MaskExpr::All);
                e
            }
            None => super::mask::MaskExpr::Invert(Box::new(super::mask::MaskExpr::All)),
        };
        for m in &mut self.modules {
            if (m.kind, m.instance) == (kind, instance) {
                continue;
            }
            if let Some(b) = &mut m.blend {
                b.mask.replace_reference(kind, instance, &with);
            }
        }
    }

    /// Inserts at the recommended position for its kind (after existing instances of it).
    pub fn insert(&mut self, registry: &Registry, m: ModuleInstance) -> usize {
        let rank = registry.rank(m.kind);
        let at = self
            .modules
            .iter()
            .position(|x| registry.rank(x.kind) > rank)
            .unwrap_or(self.modules.len());
        self.modules.insert(at, m);
        at
    }

    /// Restores the recommended order (stable for instances of one kind) and clears the flag.
    pub fn sort(&mut self, registry: &Registry) {
        self.modules.sort_by_key(|m| registry.rank(m.kind));
        self.custom_order = false;
    }

    /// Whether the stack is in the recommended order.
    pub fn in_recommended_order(&self, registry: &Registry) -> bool {
        self.modules
            .windows(2)
            .all(|w| registry.rank(w[0].kind) <= registry.rank(w[1].kind))
    }
}
