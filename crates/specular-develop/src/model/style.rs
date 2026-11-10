//! Styles (partial edits applied to other images) and presets (named field sets of one
//! module), the styles and presets Specular ships, and auto-apply rules.

use std::collections::HashMap;

use super::edit::{Edit, ModuleInstance};
use super::fields::{Fields, Value};
use super::history::{BUDGET, History, Step};
use super::kind::{ModuleKind, Registry};
use super::mask::{MaskExpr, MaskShape};

/// A partial edit: some module instances with their params, blends and the shapes those
/// blends use.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub name: String,
    pub items: Vec<ModuleInstance>,
    pub masks: Vec<MaskShape>,
}

/// How a style meets an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyMode {
    /// Merge: an item replaces the instance of the same kind and label, or is added.
    Append,
    /// Every instance of a kind the style has is removed first, then the items are added.
    Replace,
}

impl Style {
    /// A style from the chosen instances of `edit`, with the shapes their blends use.
    pub fn from_edit(
        name: &str,
        edit: &Edit,
        mut pick: impl FnMut(&ModuleInstance) -> bool,
    ) -> Self {
        let items: Vec<_> = edit.modules.iter().filter(|m| pick(m)).cloned().collect();
        let mut ids = Vec::new();
        for m in &items {
            if let Some(b) = &m.blend {
                b.mask.shape_ids(&mut ids);
            }
        }
        let masks = edit
            .masks
            .iter()
            .filter(|s| ids.contains(&s.id))
            .cloned()
            .collect();
        Self {
            name: name.to_owned(),
            items,
            masks,
        }
    }

    /// The default pick: everything but geometry (crop, lens), as copy and paste does.
    pub fn from_edit_default(name: &str, edit: &Edit, registry: &Registry) -> Self {
        Self::from_edit(name, edit, |m| {
            !registry.get(m.kind).is_some_and(|s| s.geometry)
        })
    }

    /// The style from the instances of `edit` whose kinds are in `kinds` (copy, selective).
    pub fn from_kinds(name: &str, edit: &Edit, kinds: &[ModuleKind]) -> Self {
        Self::from_edit(name, edit, |m| kinds.contains(&m.kind))
    }

    /// The module kinds the style sets, in its order, once each.
    pub fn kinds(&self) -> Vec<ModuleKind> {
        let mut v: Vec<ModuleKind> = Vec::new();
        for m in &self.items {
            if !v.contains(&m.kind) {
                v.push(m.kind);
            }
        }
        v
    }

    /// The style for a photo whose grain seed is `seed`: its grain takes the photo's seed,
    /// not the one of the photo it was copied from, so each photo keeps its own grain.
    pub fn with_grain_seed(&self, seed: i64) -> Self {
        let mut s = self.clone();
        for m in s.items.iter_mut().filter(|m| m.kind == ModuleKind::GRAIN) {
            let f = m.params.to_fields().with("seed", Value::Int(seed));
            m.params.set_fields(f);
        }
        s
    }

    /// `edit` with the style applied (the edit itself unchanged): what a hover previews and
    /// what a paste writes.
    pub fn applied(&self, edit: &Edit, registry: &Registry, mode: ApplyMode) -> Edit {
        let mut e = edit.clone();
        self.apply(&mut e, registry, mode);
        e
    }

    /// Pastes the style into `edit` as one history step named `what` (kept within the
    /// history's memory budget); whether the edit changed.
    pub fn paste(
        &self,
        edit: &mut Edit,
        history: &mut History,
        registry: &Registry,
        mode: ApplyMode,
        what: &str,
        time_ms: i64,
    ) -> bool {
        let after = self.applied(edit, registry, mode);
        if after.modules == edit.modules && after.masks == edit.masks {
            return false;
        }
        let step = Step::stack(what, edit, &after, time_ms);
        if history.push(edit, step).is_err() {
            return false;
        }
        history.trim(BUDGET);
        true
    }

    /// Applies the style to `edit`. Shapes get fresh ids in the edit, and "use mask from"
    /// references follow the instances they point to.
    pub fn apply(&self, edit: &mut Edit, registry: &Registry, mode: ApplyMode) {
        let mut shape_ids = HashMap::new();
        for s in &self.masks {
            let id = edit.next_mask_id();
            shape_ids.insert(s.id, id);
            edit.masks.push(MaskShape {
                id,
                shape: s.shape.clone(),
            });
        }
        if mode == ApplyMode::Replace {
            edit.modules
                .retain(|m| !self.items.iter().any(|i| i.kind == m.kind));
        }
        // Style instance → edit instance, for "use mask from".
        let mut instances: HashMap<(ModuleKind, u16), u16> = HashMap::new();
        let mut placed = Vec::new();
        for item in &self.items {
            let existing = edit.modules.iter().enumerate().position(|(i, m)| {
                m.kind == item.kind && m.label == item.label && !placed.contains(&i)
            });
            let mut m = item.clone();
            match existing {
                Some(i) => {
                    m.instance = edit.modules[i].instance;
                    instances.insert(item.id(), m.instance);
                    edit.modules[i] = m;
                    placed.push(i);
                }
                None => {
                    m.instance = edit.next_instance(item.kind);
                    instances.insert(item.id(), m.instance);
                    let at = edit.insert(registry, m);
                    for p in &mut placed {
                        if *p >= at {
                            *p += 1;
                        }
                    }
                    placed.push(at);
                }
            }
        }
        for i in placed {
            if let Some(b) = &mut edit.modules[i].blend {
                remap(&mut b.mask, &shape_ids, &instances);
            }
        }
    }
}

fn remap(
    e: &mut MaskExpr,
    shapes: &HashMap<u32, u32>,
    instances: &HashMap<(ModuleKind, u16), u16>,
) {
    match e {
        MaskExpr::Shape(id) => {
            if let Some(n) = shapes.get(id) {
                *id = *n;
            }
        }
        MaskExpr::FromModule { kind, instance } => {
            if let Some(n) = instances.get(&(*kind, *instance)) {
                *instance = *n;
            }
        }
        MaskExpr::Invert(a) => remap(a, shapes, instances),
        MaskExpr::Combine(_, a, b) => {
            remap(a, shapes, instances);
            remap(b, shapes, instances);
        }
        _ => {}
    }
}

/// A module preset: a name and some of the module's fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub kind: ModuleKind,
    pub name: String,
    /// The param version the fields are written in.
    pub version: u16,
    pub fields: Fields,
}

impl Preset {
    /// Copies the preset's fields onto an instance of its kind; false for another kind.
    pub fn apply_to(&self, m: &mut ModuleInstance) -> bool {
        if m.kind != self.kind {
            return false;
        }
        m.params.overlay(&self.fields);
        true
    }

    /// A preset of `m`'s current fields.
    pub fn of(name: &str, m: &ModuleInstance) -> Self {
        Self {
            kind: m.kind,
            name: name.to_owned(),
            version: m.version,
            fields: m.params.to_fields(),
        }
    }
}

/// An instance of `kind` with the registry's defaults and `fields` over them.
fn item(reg: &Registry, kind: ModuleKind, fields: &[(&str, f32)]) -> ModuleInstance {
    let mut m = ModuleInstance::new(reg, kind, 0);
    let mut f = Fields::new();
    for (n, v) in fields {
        f.set(n, Value::Float(*v));
    }
    m.params.overlay(&f);
    m
}

/// The styles Specular ships (a slightly punchier default first). Their names
/// are message ids (`develop-style-…`), translated where they are shown.
pub fn shipped_styles(reg: &Registry) -> Vec<Style> {
    use ModuleKind as K;
    let style = |name: &str, items: Vec<ModuleInstance>| Style {
        name: name.to_owned(),
        items,
        masks: Vec::new(),
    };
    vec![
        style(
            "develop-style-specular",
            vec![
                item(reg, K::VIEW_TRANSFORM, &[("contrast", 1.75)]),
                item(reg, K::LOCAL_CONTRAST, &[("clarity", 0.15)]),
                item(reg, K::COLOUR_BALANCE, &[("vibrance", 0.2)]),
            ],
        ),
        style(
            "develop-style-soft",
            vec![
                item(reg, K::VIEW_TRANSFORM, &[("contrast", 1.2)]),
                item(reg, K::TONE_EQUALIZER, &[("shadows_ev", 0.5)]),
            ],
        ),
        style(
            "develop-style-vivid",
            vec![
                item(reg, K::VIEW_TRANSFORM, &[("contrast", 2.0)]),
                item(
                    reg,
                    K::COLOUR_BALANCE,
                    &[("vibrance", 0.35), ("saturation", 0.1)],
                ),
            ],
        ),
        style(
            "develop-style-mono",
            vec![
                item(reg, K::BLACK_WHITE, &[]),
                item(reg, K::VIEW_TRANSFORM, &[("contrast", 1.8)]),
            ],
        ),
    ]
}

/// The module presets Specular ships, by kind. Names are message ids
/// (`develop-preset-…`).
pub fn shipped_presets() -> Vec<Preset> {
    use ModuleKind as K;
    let p = |kind: ModuleKind, name: &str, fields: &[(&str, f32)]| {
        let mut f = Fields::new();
        for (n, v) in fields {
            f.set(n, Value::Float(*v));
        }
        Preset {
            kind,
            name: name.to_owned(),
            version: 1,
            fields: f,
        }
    };
    vec![
        p(
            K::EXPOSURE,
            "develop-preset-plus-one",
            &[("exposure_ev", 1.0)],
        ),
        p(
            K::EXPOSURE,
            "develop-preset-minus-one",
            &[("exposure_ev", -1.0)],
        ),
        p(
            K::VIEW_TRANSFORM,
            "develop-preset-low-contrast",
            &[("contrast", 1.1)],
        ),
        p(
            K::VIEW_TRANSFORM,
            "develop-preset-high-contrast",
            &[("contrast", 2.2)],
        ),
        p(
            K::TONE_EQUALIZER,
            "develop-preset-open-shadows",
            &[("shadows_ev", 1.0), ("highlights_ev", -0.5)],
        ),
        p(
            K::LOCAL_CONTRAST,
            "develop-preset-crisp",
            &[("texture", 0.25), ("clarity", 0.2)],
        ),
        p(
            K::COLOUR_BALANCE,
            "develop-preset-muted",
            &[("vibrance", -0.3), ("saturation", -0.2)],
        ),
        p(
            K::DENOISE,
            "develop-preset-high-iso",
            &[("luminance", 0.6), ("colour", 0.5), ("detail", 0.4)],
        ),
        p(
            K::SHARPEN,
            "develop-preset-screen",
            &[("amount", 0.4), ("radius", 0.8)],
        ),
    ]
}

/// What an auto-apply rule looks at in a photo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhotoFacts {
    pub maker: String,
    pub model: String,
    pub lens: String,
    pub iso: Option<u32>,
    /// The file's extension, lower case (`cr3`, `jpg`).
    pub ext: String,
    pub raw: bool,
}

/// The kinds of file a rule can ask for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileType {
    #[default]
    Any,
    Raw,
    /// Everything that is not raw.
    Other,
}

/// "When the camera is … and the ISO is at least … apply style …".
/// Empty texts and `None` match anything; texts match case-insensitively as parts of the
/// photo's ("Canon" matches "Canon EOS R5").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutoRule {
    pub style: String,
    pub maker: String,
    pub model: String,
    pub lens: String,
    pub iso_min: Option<u32>,
    pub iso_max: Option<u32>,
    pub file_type: FileType,
}

impl AutoRule {
    pub fn matches(&self, f: &PhotoFacts) -> bool {
        let part = |want: &str, have: &str| {
            want.trim().is_empty() || have.to_lowercase().contains(&want.trim().to_lowercase())
        };
        let iso_ok = match (f.iso, self.iso_min, self.iso_max) {
            (_, None, None) => true,
            (None, _, _) => false,
            (Some(i), lo, hi) => lo.is_none_or(|l| i >= l) && hi.is_none_or(|h| i <= h),
        };
        let type_ok = match self.file_type {
            FileType::Any => true,
            FileType::Raw => f.raw,
            FileType::Other => !f.raw,
        };
        part(&self.maker, &f.maker)
            && part(&self.model, &f.model)
            && part(&self.lens, &f.lens)
            && iso_ok
            && type_ok
    }
}

/// The style the first matching rule names, if any.
pub fn auto_style<'a>(rules: &'a [AutoRule], f: &PhotoFacts) -> Option<&'a str> {
    rules
        .iter()
        .find(|r| r.matches(f))
        .map(|r| r.style.as_str())
}
